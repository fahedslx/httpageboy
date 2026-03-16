use std::cell::RefCell;
use std::collections::HashMap;
#[cfg(feature = "sync")]
use std::io::{Read, Write};
#[cfg(feature = "sync")]
use std::net::TcpStream;
#[cfg(any(feature = "async_tokio", feature = "async_std", feature = "async_smol"))]
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
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
static SERVER_REGISTRY: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();

#[cfg(feature = "sync")]
use crate::runtime::sync::server::Server;

#[cfg(all(feature = "async_tokio", not(feature = "sync")))]
use crate::runtime::r#async::tokio::Server;

#[cfg(all(feature = "async_smol", not(any(feature = "sync", feature = "async_tokio"))))]
use crate::runtime::r#async::smol::Server;

#[cfg(all(
  feature = "async_std",
  not(any(feature = "sync", feature = "async_tokio", feature = "async_smol"))
))]
use crate::runtime::r#async::async_std::Server;

thread_local! {
  static ACTIVE_SERVER_URL: RefCell<Option<&'static str>> = RefCell::new(None);
}

fn server_registry() -> &'static Mutex<HashMap<String, &'static str>> {
  SERVER_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn registry_guard() -> std::sync::MutexGuard<'static, HashMap<String, &'static str>> {
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

pub fn active_test_server_url() -> &'static str {
  if let Some(url) = ACTIVE_SERVER_URL.with(|slot| slot.borrow().clone()) {
    return url;
  }

  if let Some(url) = LAST_ACTIVE_URL
    .get_or_init(|| Mutex::new(None))
    .lock()
    .unwrap_or_else(|err| err.into_inner())
    .clone()
  {
    set_active_url(url);
    return url;
  }

  let fallback = registry_guard().values().next().copied();
  if let Some(url) = fallback {
    set_active_url(url);
    return url;
  }

  set_active_url(DEFAULT_TEST_SERVER_URL);
  DEFAULT_TEST_SERVER_URL
}

#[cfg(feature = "sync")]
fn wait_for_server(url: &str) {
  for _ in 0..WAIT_ATTEMPTS {
    if TcpStream::connect(url).is_ok() {
      return;
    }
    thread::sleep(WAIT_DELAY);
  }
  panic!("test server not reachable at {}", url);
}

#[cfg(feature = "sync")]
fn perform_test(url: &str, request: &[u8], expected_response: &[u8]) -> String {
  wait_for_server(url);
  let mut stream = TcpStream::connect(url).expect("failed to connect to test server");
  stream
    .write_all(request)
    .expect("failed to write request to test server");
  let _ = stream.shutdown(std::net::Shutdown::Write);

  let mut buffer = Vec::new();
  stream
    .read_to_end(&mut buffer)
    .expect("failed to read response from test server");

  let buffer_string = String::from_utf8_lossy(&buffer).to_string();
  let expected_response_string = String::from_utf8_lossy(expected_response).to_string();

  assert!(
    buffer_string.contains(&expected_response_string),
    "ASSERT FAILED:\n\nRECEIVED: {} \nEXPECTED: {} \n\n",
    buffer_string,
    expected_response_string
  );

  buffer_string
}

#[cfg(feature = "sync")]
pub fn setup_test_server<F>(server_url: Option<&str>, server_factory: F)
where
  F: FnOnce() -> Server + Send + 'static,
{
  let server_url = server_url.unwrap_or_else(|| active_test_server_url());
  let mut registry = registry_guard();
  if let Some(url) = registry.get(server_url) {
    set_active_url(*url);
    return;
  }

  let server = server_factory();
  let leaked_url: &'static str = Box::leak(server.url().to_owned().into_boxed_str());
  registry.insert(server_url.to_string(), leaked_url);
  let active_url = leaked_url;
  drop(registry);

  thread::spawn(move || {
    server.run();
  });
  thread::sleep(INTERVAL);
  set_active_url(active_url);
}

#[cfg(feature = "sync")]
pub fn run_test(request: &[u8], expected_response: &[u8], target_url: Option<&str>) -> String {
  let url = target_url
    .map(|s| s.to_string())
    .unwrap_or_else(|| active_test_server_url().to_string());
  perform_test(&url, request, expected_response)
}

// async_tokio
#[cfg(all(feature = "async_tokio", not(feature = "sync")))]
pub async fn setup_test_server<F, Fut>(server_url: Option<&str>, server_factory: F)
where
  F: FnOnce() -> Fut + Send + 'static,
  Fut: std::future::Future<Output = Server> + Send + 'static,
{
  let server_url = server_url.unwrap_or_else(|| active_test_server_url());
  let mut registry = registry_guard();
  if let Some(url) = registry.get(server_url) {
    set_active_url(*url);
    return;
  }

  let (tx, rx) = mpsc::channel();
  thread::spawn(move || {
    let rt = tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .build()
      .unwrap();
    rt.block_on(async move {
      let server = server_factory().await;
      let leaked_url: &'static str = Box::leak(server.url().to_owned().into_boxed_str());
      let _ = tx.send(leaked_url);
      server.run().await;
    });
  });

  let leaked_url = rx.recv().expect("server url not sent");
  registry.insert(server_url.to_string(), leaked_url);
  drop(registry);
  thread::sleep(INTERVAL);
  set_active_url(leaked_url);
}

#[cfg(all(feature = "async_tokio", not(feature = "sync")))]
pub async fn run_test(request: &[u8], expected_response: &[u8], target_url: Option<&str>) -> String {
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
          continue;
        }
        Err(err) => panic!("failed to connect to test server {}: {:?}", url, err),
      }
    }
  };
  stream
    .write_all(request)
    .await
    .expect("failed to write request to test server");
  let _ = stream.shutdown().await;

  let mut buffer = Vec::new();
  stream
    .read_to_end(&mut buffer)
    .await
    .expect("failed to read response from test server");

  let buffer_string = String::from_utf8_lossy(&buffer).to_string();
  let expected_response_string = String::from_utf8_lossy(expected_response).to_string();

  assert!(
    buffer_string.contains(&expected_response_string),
    "ASSERT FAILED:\n\nRECEIVED: {} \nEXPECTED: {} \n\n",
    buffer_string,
    expected_response_string
  );

  buffer_string
}

// async_std
#[cfg(all(feature = "async_std", not(any(feature = "sync", feature = "async_tokio"))))]
pub async fn setup_test_server<F, Fut>(server_url: Option<&str>, server_factory: F)
where
  F: FnOnce() -> Fut + Send + 'static,
  Fut: std::future::Future<Output = Server> + Send + 'static,
{
  let server_url = server_url.unwrap_or_else(|| active_test_server_url());
  let mut registry = registry_guard();
  if let Some(url) = registry.get(server_url) {
    set_active_url(*url);
    return;
  }

  let (tx, rx) = mpsc::channel();
  thread::spawn(move || {
    async_std::task::block_on(async move {
      let server = server_factory().await;
      let leaked_url: &'static str = Box::leak(server.url().to_owned().into_boxed_str());
      let _ = tx.send(leaked_url);
      server.run().await;
    });
  });

  let leaked_url = rx.recv().expect("server url not sent");
  registry.insert(server_url.to_string(), leaked_url);
  drop(registry);
  thread::sleep(INTERVAL);
  set_active_url(leaked_url);
}

#[cfg(all(feature = "async_std", not(any(feature = "sync", feature = "async_tokio"))))]
pub async fn run_test(request: &[u8], expected_response: &[u8], target_url: Option<&str>) -> String {
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
          continue;
        }
        Err(err) => panic!("failed to connect to test server {}: {:?}", url, err),
      }
    }
  };
  stream
    .write_all(request)
    .await
    .expect("failed to write request to test server");
  let _ = stream.shutdown(Shutdown::Write);

  let mut buffer = Vec::new();
  stream
    .read_to_end(&mut buffer)
    .await
    .expect("failed to read response from test server");

  let buffer_string = String::from_utf8_lossy(&buffer).to_string();
  let expected_response_string = String::from_utf8_lossy(expected_response).to_string();

  assert!(
    buffer_string.contains(&expected_response_string),
    "ASSERT FAILED:\n\nRECEIVED: {} \nEXPECTED: {} \n\n",
    buffer_string,
    expected_response_string
  );

  buffer_string
}

#[cfg(all(
  feature = "async_smol",
  not(any(feature = "sync", feature = "async_tokio", feature = "async_std"))
))]
pub async fn setup_test_server<F, Fut>(server_url: Option<&str>, server_factory: F)
where
  F: FnOnce() -> Fut + Send + 'static,
  Fut: std::future::Future<Output = Server> + Send + 'static,
{
  let server_url = server_url.unwrap_or_else(|| active_test_server_url());
  let mut registry = registry_guard();
  if let Some(url) = registry.get(server_url) {
    set_active_url(*url);
    return;
  }

  let (tx, rx) = mpsc::channel();
  thread::spawn(move || {
    smol::block_on(async move {
      let server = server_factory().await;
      let leaked_url: &'static str = Box::leak(server.url().to_owned().into_boxed_str());
      let _ = tx.send(leaked_url);
      server.run().await;
    });
  });

  let leaked_url = rx.recv().expect("server url not sent");
  registry.insert(server_url.to_string(), leaked_url);
  drop(registry);
  thread::sleep(INTERVAL);
  set_active_url(leaked_url);
}

#[cfg(all(
  feature = "async_smol",
  not(any(feature = "sync", feature = "async_tokio", feature = "async_std"))
))]
pub async fn run_test(request: &[u8], expected_response: &[u8], target_url: Option<&str>) -> String {
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
          continue;
        }
        Err(err) => panic!("failed to connect to test server {}: {:?}", url, err),
      }
    }
  };
  stream
    .write_all(request)
    .await
    .expect("failed to write request to test server");
  let _ = stream.shutdown(std::net::Shutdown::Write);

  let mut buffer = Vec::new();
  stream
    .read_to_end(&mut buffer)
    .await
    .expect("failed to read response from test server");

  let buffer_string = String::from_utf8_lossy(&buffer).to_string();
  let expected_response_string = String::from_utf8_lossy(expected_response).to_string();

  assert!(
    buffer_string.contains(&expected_response_string),
    "ASSERT FAILED:\n\nRECEIVED: {} \nEXPECTED: {} \n\n",
    buffer_string,
    expected_response_string
  );

  buffer_string
}
