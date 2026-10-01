use httpageboy::Server;
pub use qata::{TestError, TestResult};
use std::cell::RefCell;
use std::collections::HashMap;
#[cfg(feature = "sync")]
use std::io::{Read, Write};
#[cfg(feature = "sync")]
use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

pub const POOL_SIZE: u8 = 10;
pub const DEFAULT_TEST_SERVER_URL: &str = "127.0.0.1:0";
pub const INTERVAL: Duration = Duration::from_millis(250);

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
const WAIT_ATTEMPTS: usize = 20;

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
const WAIT_DELAY: Duration = Duration::from_millis(100);

static LAST_ACTIVE_URL: OnceLock<Mutex<Option<&'static str>>> = OnceLock::new();
static SERVER_REGISTRY: OnceLock<Mutex<HashMap<String, TestServerRecord>>> = OnceLock::new();

#[derive(Debug)]
struct TestServerRecord {
  url: &'static str,
  shutdown_tx: Option<mpsc::Sender<()>>,
}

thread_local! {
  static ACTIVE_SERVER_URL: RefCell<Option<&'static str>> = RefCell::new(None);
}

fn server_registry() -> &'static Mutex<HashMap<String, TestServerRecord>> {
  SERVER_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn registry_guard() -> std::sync::MutexGuard<'static, HashMap<String, TestServerRecord>> {
  server_registry().lock().unwrap_or_else(|err| err.into_inner())
}

fn set_active_url(url: &'static str) {
  ACTIVE_SERVER_URL.with(|slot| {
    *slot.borrow_mut() = Some(url);
  });
  LAST_ACTIVE_URL
    .get_or_init(|| Mutex::new(None))
    .lock()
    .unwrap_or_else(|err| err.into_inner())
    .replace(url);
}

fn clear_active_url(url: &'static str) {
  ACTIVE_SERVER_URL.with(|slot| {
    if slot.borrow().as_ref().is_some_and(|active| *active == url) {
      *slot.borrow_mut() = None;
    }
  });
  let mut last_active = LAST_ACTIVE_URL
    .get_or_init(|| Mutex::new(None))
    .lock()
    .unwrap_or_else(|err| err.into_inner());
  if last_active.as_ref().is_some_and(|active| *active == url) {
    *last_active = None;
  }
}

pub fn active_test_server_url() -> &'static str {
  if let Some(url) = ACTIVE_SERVER_URL.with(|slot| *slot.borrow()) {
    return url;
  }

  if let Some(url) = *LAST_ACTIVE_URL
    .get_or_init(|| Mutex::new(None))
    .lock()
    .unwrap_or_else(|err| err.into_inner())
  {
    set_active_url(url);
    return url;
  }

  let fallback = registry_guard().values().next().map(|record| record.url);
  if let Some(url) = fallback {
    set_active_url(url);
    return url;
  }

  set_active_url(DEFAULT_TEST_SERVER_URL);
  DEFAULT_TEST_SERVER_URL
}

pub fn is_test_server_registered(server_url: &str) -> bool {
  let registry = registry_guard();
  registry.contains_key(server_url) || registry.values().any(|record| record.url == server_url)
}

pub fn shutdown_test_server(server_url: &str) -> TestResult {
  let removed = registry_guard().remove(server_url);
  let Some(record) = removed else {
    return Ok(());
  };
  if let Some(tx) = record.shutdown_tx {
    let _ = tx.send(());
  }
  clear_active_url(record.url);
  thread::sleep(INTERVAL);
  Ok(())
}

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
fn compare_response(buffer: Vec<u8>, expected_response: &[u8]) -> TestResult<String> {
  let buffer_string = String::from_utf8_lossy(&buffer).to_string();
  let expected_response_string = String::from_utf8_lossy(expected_response).to_string();

  if buffer_string.contains(&expected_response_string) {
    Ok(buffer_string)
  } else {
    Err(TestError::new(format!(
      "received response did not contain expected response\nreceived: {}\nexpected: {}",
      buffer_string, expected_response_string
    )))
  }
}

#[cfg(feature = "sync")]
fn wait_for_server(url: &str) -> TestResult {
  for _ in 0..WAIT_ATTEMPTS {
    if TcpStream::connect(url).is_ok() {
      return Ok(());
    }
    thread::sleep(WAIT_DELAY);
  }
  Err(TestError::new(format!("test server not reachable at {}", url)))
}

#[cfg(feature = "sync")]
fn perform_test(url: &str, request: &[u8], expected_response: &[u8]) -> TestResult<String> {
  wait_for_server(url)?;
  let mut stream = TcpStream::connect(url)
    .map_err(|err| TestError::new(format!("failed to connect to test server {}: {}", url, err)))?;
  stream
    .write_all(request)
    .map_err(|err| TestError::new(format!("failed to write request to test server: {}", err)))?;
  let _ = stream.shutdown(std::net::Shutdown::Write);

  let mut buffer = Vec::new();
  stream
    .read_to_end(&mut buffer)
    .map_err(|err| TestError::new(format!("failed to read response from test server: {}", err)))?;

  compare_response(buffer, expected_response)
}

#[cfg(feature = "sync")]
pub fn setup_test_server<F>(server_url: Option<&str>, server_factory: F) -> TestResult<&'static str>
where
  F: FnOnce() -> Server + Send + 'static,
{
  let server_url = server_url.unwrap_or_else(|| active_test_server_url());
  let mut registry = registry_guard();
  if let Some(record) = registry.get(server_url) {
    set_active_url(record.url);
    return Ok(record.url);
  }

  let server = server_factory();
  let leaked_url: &'static str = Box::leak(server.url().to_owned().into_boxed_str());
  let (shutdown_tx, shutdown_rx) = mpsc::channel();
  registry.insert(
    server_url.to_string(),
    TestServerRecord {
      url: leaked_url,
      shutdown_tx: Some(shutdown_tx),
    },
  );
  drop(registry);

  thread::spawn(move || {
    server.run_until_shutdown(shutdown_rx);
  });
  thread::sleep(INTERVAL);
  set_active_url(leaked_url);
  Ok(leaked_url)
}

#[cfg(feature = "sync")]
pub fn run_test(request: &[u8], expected_response: &[u8], target_url: Option<&str>) -> TestResult<String> {
  let url = target_url
    .map(|s| s.to_string())
    .unwrap_or_else(|| active_test_server_url().to_string());
  perform_test(&url, request, expected_response)
}

#[cfg(all(feature = "async_tokio", not(feature = "sync")))]
pub async fn setup_test_server<F, Fut>(server_url: Option<&str>, server_factory: F) -> TestResult<&'static str>
where
  F: FnOnce() -> Fut + Send + 'static,
  Fut: std::future::Future<Output = Server> + Send + 'static,
{
  let server_url = server_url.unwrap_or_else(|| active_test_server_url());
  let mut registry = registry_guard();
  if let Some(record) = registry.get(server_url) {
    set_active_url(record.url);
    return Ok(record.url);
  }

  let (url_tx, url_rx) = mpsc::channel();
  let (shutdown_tx, shutdown_rx) = mpsc::channel();
  let rt = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .map_err(|err| TestError::new(format!("failed to build Tokio runtime: {}", err)))?;
  thread::spawn(move || {
    rt.block_on(async move {
      let server = server_factory().await;
      let leaked_url: &'static str = Box::leak(server.url().to_owned().into_boxed_str());
      let _ = url_tx.send(leaked_url);
      server.run_until_shutdown(shutdown_rx).await;
    });
  });

  let leaked_url = url_rx
    .recv()
    .map_err(|err| TestError::new(format!("server url not sent: {}", err)))?;
  registry.insert(
    server_url.to_string(),
    TestServerRecord {
      url: leaked_url,
      shutdown_tx: Some(shutdown_tx),
    },
  );
  drop(registry);
  thread::sleep(INTERVAL);
  set_active_url(leaked_url);
  Ok(leaked_url)
}

#[cfg(all(feature = "async_tokio", not(feature = "sync")))]
pub async fn run_test(request: &[u8], expected_response: &[u8], target_url: Option<&str>) -> TestResult<String> {
  use tokio::io::{AsyncReadExt, AsyncWriteExt};
  let url = target_url
    .map(|s| s.to_string())
    .unwrap_or_else(|| active_test_server_url().to_string());
  let mut stream = {
    let mut attempt = 0;
    loop {
      match tokio::net::TcpStream::connect(&url).await {
        Ok(stream) => break stream,
        Err(_err) if attempt + 1 < WAIT_ATTEMPTS => {
          attempt += 1;
          tokio::time::sleep(WAIT_DELAY).await;
        }
        Err(err) => {
          return Err(TestError::new(format!(
            "failed to connect to test server {}: {}",
            url, err
          )));
        }
      }
    }
  };
  stream
    .write_all(request)
    .await
    .map_err(|err| TestError::new(format!("failed to write request to test server: {}", err)))?;
  let _ = stream.shutdown().await;

  let mut buffer = Vec::new();
  stream
    .read_to_end(&mut buffer)
    .await
    .map_err(|err| TestError::new(format!("failed to read response from test server: {}", err)))?;

  compare_response(buffer, expected_response)
}

#[cfg(all(feature = "async_std", not(any(feature = "sync", feature = "async_tokio"))))]
pub async fn setup_test_server<F, Fut>(server_url: Option<&str>, server_factory: F) -> TestResult<&'static str>
where
  F: FnOnce() -> Fut + Send + 'static,
  Fut: std::future::Future<Output = Server> + Send + 'static,
{
  let server_url = server_url.unwrap_or_else(|| active_test_server_url());
  let mut registry = registry_guard();
  if let Some(record) = registry.get(server_url) {
    set_active_url(record.url);
    return Ok(record.url);
  }

  let (url_tx, url_rx) = mpsc::channel();
  let (shutdown_tx, shutdown_rx) = mpsc::channel();
  thread::spawn(move || {
    async_std::task::block_on(async move {
      let server = server_factory().await;
      let leaked_url: &'static str = Box::leak(server.url().to_owned().into_boxed_str());
      let _ = url_tx.send(leaked_url);
      server.run_until_shutdown(shutdown_rx).await;
    });
  });

  let leaked_url = url_rx
    .recv()
    .map_err(|err| TestError::new(format!("server url not sent: {}", err)))?;
  registry.insert(
    server_url.to_string(),
    TestServerRecord {
      url: leaked_url,
      shutdown_tx: Some(shutdown_tx),
    },
  );
  drop(registry);
  thread::sleep(INTERVAL);
  set_active_url(leaked_url);
  Ok(leaked_url)
}

#[cfg(all(feature = "async_std", not(any(feature = "sync", feature = "async_tokio"))))]
pub async fn run_test(request: &[u8], expected_response: &[u8], target_url: Option<&str>) -> TestResult<String> {
  use async_std::io::prelude::*;
  use async_std::net::{Shutdown, TcpStream};
  let url = target_url
    .map(|s| s.to_string())
    .unwrap_or_else(|| active_test_server_url().to_string());
  let mut stream = {
    let mut attempt = 0;
    loop {
      match TcpStream::connect(&url).await {
        Ok(stream) => break stream,
        Err(_err) if attempt + 1 < WAIT_ATTEMPTS => {
          attempt += 1;
          async_std::task::sleep(WAIT_DELAY).await;
        }
        Err(err) => {
          return Err(TestError::new(format!(
            "failed to connect to test server {}: {}",
            url, err
          )));
        }
      }
    }
  };
  stream
    .write_all(request)
    .await
    .map_err(|err| TestError::new(format!("failed to write request to test server: {}", err)))?;
  let _ = stream.shutdown(Shutdown::Write);

  let mut buffer = Vec::new();
  stream
    .read_to_end(&mut buffer)
    .await
    .map_err(|err| TestError::new(format!("failed to read response from test server: {}", err)))?;

  compare_response(buffer, expected_response)
}

#[cfg(all(
  feature = "async_smol",
  not(any(feature = "sync", feature = "async_tokio", feature = "async_std"))
))]
pub async fn setup_test_server<F, Fut>(server_url: Option<&str>, server_factory: F) -> TestResult<&'static str>
where
  F: FnOnce() -> Fut + Send + 'static,
  Fut: std::future::Future<Output = Server> + Send + 'static,
{
  let server_url = server_url.unwrap_or_else(|| active_test_server_url());
  let mut registry = registry_guard();
  if let Some(record) = registry.get(server_url) {
    set_active_url(record.url);
    return Ok(record.url);
  }

  let (url_tx, url_rx) = mpsc::channel();
  let (shutdown_tx, shutdown_rx) = mpsc::channel();
  thread::spawn(move || {
    smol::block_on(async move {
      let server = server_factory().await;
      let leaked_url: &'static str = Box::leak(server.url().to_owned().into_boxed_str());
      let _ = url_tx.send(leaked_url);
      server.run_until_shutdown(shutdown_rx).await;
    });
  });

  let leaked_url = url_rx
    .recv()
    .map_err(|err| TestError::new(format!("server url not sent: {}", err)))?;
  registry.insert(
    server_url.to_string(),
    TestServerRecord {
      url: leaked_url,
      shutdown_tx: Some(shutdown_tx),
    },
  );
  drop(registry);
  thread::sleep(INTERVAL);
  set_active_url(leaked_url);
  Ok(leaked_url)
}

#[cfg(all(
  feature = "async_smol",
  not(any(feature = "sync", feature = "async_tokio", feature = "async_std"))
))]
pub async fn run_test(request: &[u8], expected_response: &[u8], target_url: Option<&str>) -> TestResult<String> {
  use smol::io::AsyncReadExt;
  use smol::io::AsyncWriteExt;
  let url = target_url
    .map(|s| s.to_string())
    .unwrap_or_else(|| active_test_server_url().to_string());
  let mut stream = {
    let mut attempt = 0;
    loop {
      match smol::net::TcpStream::connect(&url).await {
        Ok(stream) => break stream,
        Err(_err) if attempt + 1 < WAIT_ATTEMPTS => {
          attempt += 1;
          smol::Timer::after(WAIT_DELAY).await;
        }
        Err(err) => {
          return Err(TestError::new(format!(
            "failed to connect to test server {}: {}",
            url, err
          )));
        }
      }
    }
  };
  stream
    .write_all(request)
    .await
    .map_err(|err| TestError::new(format!("failed to write request to test server: {}", err)))?;
  let _ = stream.shutdown(std::net::Shutdown::Write);

  let mut buffer = Vec::new();
  stream
    .read_to_end(&mut buffer)
    .await
    .map_err(|err| TestError::new(format!("failed to read response from test server: {}", err)))?;

  compare_response(buffer, expected_response)
}
