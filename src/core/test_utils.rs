use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
#[cfg(feature = "sync")]
use std::io::{Read, Write};
#[cfg(feature = "sync")]
use std::net::TcpStream;
#[cfg(any(feature = "async_tokio", feature = "async_std", feature = "async_smol"))]
use std::pin::Pin;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestError {
  pub message: String,
}

pub type TestResult<T = ()> = Result<T, TestError>;

impl TestError {
  pub fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for TestError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.message)
  }
}

impl std::error::Error for TestError {}

impl From<std::io::Error> for TestError {
  fn from(err: std::io::Error) -> Self {
    Self::new(err.to_string())
  }
}

impl From<&str> for TestError {
  fn from(err: &str) -> Self {
    Self::new(err)
  }
}

impl From<String> for TestError {
  fn from(err: String) -> Self {
    Self::new(err)
  }
}

#[cfg(feature = "sync")]
pub struct TestContext<BeforeEach, AfterEach> {
  before_each: BeforeEach,
  after_each: AfterEach,
}

#[cfg(feature = "sync")]
impl<BeforeEach, AfterEach> TestContext<BeforeEach, AfterEach>
where
  BeforeEach: FnMut() -> TestResult,
  AfterEach: FnMut() -> TestResult,
{
  pub fn run(&mut self, request: &[u8], expected_response: &[u8]) -> TestResult<String> {
    let result = (self.before_each)().and_then(|_| run_test(request, expected_response, None));
    let cleanup = (self.after_each)();
    match (result, cleanup) {
      (Ok(response), Ok(())) => Ok(response),
      (Err(err), _) => Err(err),
      (Ok(_), Err(err)) => Err(err),
    }
  }
}

#[cfg(feature = "sync")]
pub fn run_test_case<Before, BeforeEach, Test, AfterEach, After>(
  before: Before,
  before_each: BeforeEach,
  test: Test,
  after_each: AfterEach,
  after: After,
) -> TestResult
where
  Before: FnOnce() -> TestResult,
  BeforeEach: FnMut() -> TestResult,
  Test: FnOnce(&mut TestContext<BeforeEach, AfterEach>) -> TestResult,
  AfterEach: FnMut() -> TestResult,
  After: FnOnce() -> TestResult,
{
  let mut result = before();
  if result.is_ok() {
    let mut context = TestContext {
      before_each,
      after_each,
    };
    result = test(&mut context);
  }
  let cleanup = after();
  match (result, cleanup) {
    (Ok(()), Ok(())) => Ok(()),
    (Err(err), _) => Err(err),
    (Ok(()), Err(err)) => Err(err),
  }
}

#[cfg(all(
  not(feature = "sync"),
  any(feature = "async_tokio", feature = "async_std", feature = "async_smol")
))]
pub type TestFuture<'a, T = ()> = Pin<Box<dyn std::future::Future<Output = TestResult<T>> + 'a>>;

#[cfg(all(
  not(feature = "sync"),
  any(feature = "async_tokio", feature = "async_std", feature = "async_smol")
))]
#[async_trait::async_trait(?Send)]
pub trait AsyncTestHook {
  async fn call(&mut self) -> TestResult;
}

#[cfg(all(
  not(feature = "sync"),
  any(feature = "async_tokio", feature = "async_std", feature = "async_smol")
))]
#[async_trait::async_trait(?Send)]
impl<F, Fut> AsyncTestHook for F
where
  F: FnMut() -> Fut,
  Fut: std::future::Future<Output = TestResult>,
{
  async fn call(&mut self) -> TestResult {
    self().await
  }
}

#[cfg(all(
  not(feature = "sync"),
  any(feature = "async_tokio", feature = "async_std", feature = "async_smol")
))]
pub struct TestContext<BeforeEach, AfterEach> {
  before_each: BeforeEach,
  after_each: AfterEach,
}

#[cfg(all(
  not(feature = "sync"),
  any(feature = "async_tokio", feature = "async_std", feature = "async_smol")
))]
impl<BeforeEach, AfterEach> TestContext<BeforeEach, AfterEach>
where
  BeforeEach: AsyncTestHook,
  AfterEach: AsyncTestHook,
{
  pub async fn run(&mut self, request: &[u8], expected_response: &[u8]) -> TestResult<String> {
    let result = match self.before_each.call().await {
      Ok(()) => run_test(request, expected_response, None).await,
      Err(err) => Err(err),
    };
    let cleanup = self.after_each.call().await;
    match (result, cleanup) {
      (Ok(response), Ok(())) => Ok(response),
      (Err(err), _) => Err(err),
      (Ok(_), Err(err)) => Err(err),
    }
  }
}

#[cfg(all(
  not(feature = "sync"),
  any(feature = "async_tokio", feature = "async_std", feature = "async_smol")
))]
pub async fn run_test_case<Before, BeforeFut, BeforeEach, Test, AfterEach, After, AfterFut>(
  before: Before,
  before_each: BeforeEach,
  test: Test,
  after_each: AfterEach,
  after: After,
) -> TestResult
where
  Before: FnOnce() -> BeforeFut,
  BeforeFut: std::future::Future<Output = TestResult>,
  BeforeEach: AsyncTestHook,
  Test: for<'a> FnOnce(&'a mut TestContext<BeforeEach, AfterEach>) -> TestFuture<'a>,
  AfterEach: AsyncTestHook,
  After: FnOnce() -> AfterFut,
  AfterFut: std::future::Future<Output = TestResult>,
{
  let mut result = before().await;
  if result.is_ok() {
    let mut context = TestContext {
      before_each,
      after_each,
    };
    result = test(&mut context).await;
  }
  let cleanup = after().await;
  match (result, cleanup) {
    (Ok(()), Ok(())) => Ok(()),
    (Err(err), _) => Err(err),
    (Ok(()), Err(err)) => Err(err),
  }
}

#[cfg(feature = "sync")]
#[macro_export]
macro_rules! test_case {
  (before $before:block before_each $before_each:block test |$ctx:ident| $test:block after_each $after_each:block after $after:block) => {
    $crate::test_utils::run_test_case(
      || -> $crate::test_utils::TestResult { $before Ok(()) },
      || -> $crate::test_utils::TestResult { $before_each Ok(()) },
      |__httpageboy_ctx| -> $crate::test_utils::TestResult {
        let $ctx = __httpageboy_ctx;
        $test
        Ok(())
      },
      || -> $crate::test_utils::TestResult { $after_each Ok(()) },
      || -> $crate::test_utils::TestResult { $after Ok(()) },
    )
  };
  (before $before:block before_each $before_each:block test |$ctx:ident| $test:block after_each $after_each:block) => {
    $crate::test_case! { before $before before_each $before_each test |$ctx| $test after_each $after_each after {} }
  };
  (before $before:block before_each $before_each:block test |$ctx:ident| $test:block after $after:block) => {
    $crate::test_case! { before $before before_each $before_each test |$ctx| $test after_each {} after $after }
  };
  (before $before:block before_each $before_each:block test |$ctx:ident| $test:block) => {
    $crate::test_case! { before $before before_each $before_each test |$ctx| $test after_each {} after {} }
  };
  (before $before:block test |$ctx:ident| $test:block after_each $after_each:block after $after:block) => {
    $crate::test_case! { before $before before_each {} test |$ctx| $test after_each $after_each after $after }
  };
  (before $before:block test |$ctx:ident| $test:block after_each $after_each:block) => {
    $crate::test_case! { before $before before_each {} test |$ctx| $test after_each $after_each after {} }
  };
  (before $before:block test |$ctx:ident| $test:block after $after:block) => {
    $crate::test_case! { before $before before_each {} test |$ctx| $test after_each {} after $after }
  };
  (before $before:block test |$ctx:ident| $test:block) => {
    $crate::test_case! { before $before before_each {} test |$ctx| $test after_each {} after {} }
  };
  (before_each $before_each:block test |$ctx:ident| $test:block after_each $after_each:block after $after:block) => {
    $crate::test_case! { before {} before_each $before_each test |$ctx| $test after_each $after_each after $after }
  };
  (before_each $before_each:block test |$ctx:ident| $test:block after_each $after_each:block) => {
    $crate::test_case! { before {} before_each $before_each test |$ctx| $test after_each $after_each after {} }
  };
  (before_each $before_each:block test |$ctx:ident| $test:block after $after:block) => {
    $crate::test_case! { before {} before_each $before_each test |$ctx| $test after_each {} after $after }
  };
  (before_each $before_each:block test |$ctx:ident| $test:block) => {
    $crate::test_case! { before {} before_each $before_each test |$ctx| $test after_each {} after {} }
  };
  (test |$ctx:ident| $test:block after_each $after_each:block after $after:block) => {
    $crate::test_case! { before {} before_each {} test |$ctx| $test after_each $after_each after $after }
  };
  (test |$ctx:ident| $test:block after_each $after_each:block) => {
    $crate::test_case! { before {} before_each {} test |$ctx| $test after_each $after_each after {} }
  };
  (test |$ctx:ident| $test:block after $after:block) => {
    $crate::test_case! { before {} before_each {} test |$ctx| $test after_each {} after $after }
  };
  (test |$ctx:ident| $test:block) => {
    $crate::test_case! { before {} before_each {} test |$ctx| $test after_each {} after {} }
  };
}

#[cfg(all(
  not(feature = "sync"),
  any(feature = "async_tokio", feature = "async_std", feature = "async_smol")
))]
#[macro_export]
macro_rules! test_case {
  (before $before:block before_each $before_each:block test |$ctx:ident| $test:block after_each $after_each:block after $after:block) => {
    $crate::test_utils::run_test_case(
      || async { $before Ok(()) },
      || async { $before_each Ok(()) },
      |__httpageboy_ctx| -> $crate::test_utils::TestFuture<'_> {
        Box::pin(async {
          let $ctx = __httpageboy_ctx;
          $test
          Ok(())
        })
      },
      || async { $after_each Ok(()) },
      || async { $after Ok(()) },
    )
    .await
  };
  (before $before:block before_each $before_each:block test |$ctx:ident| $test:block after_each $after_each:block) => {
    $crate::test_case! { before $before before_each $before_each test |$ctx| $test after_each $after_each after {} }
  };
  (before $before:block before_each $before_each:block test |$ctx:ident| $test:block after $after:block) => {
    $crate::test_case! { before $before before_each $before_each test |$ctx| $test after_each {} after $after }
  };
  (before $before:block before_each $before_each:block test |$ctx:ident| $test:block) => {
    $crate::test_case! { before $before before_each $before_each test |$ctx| $test after_each {} after {} }
  };
  (before $before:block test |$ctx:ident| $test:block after_each $after_each:block after $after:block) => {
    $crate::test_case! { before $before before_each {} test |$ctx| $test after_each $after_each after $after }
  };
  (before $before:block test |$ctx:ident| $test:block after_each $after_each:block) => {
    $crate::test_case! { before $before before_each {} test |$ctx| $test after_each $after_each after {} }
  };
  (before $before:block test |$ctx:ident| $test:block after $after:block) => {
    $crate::test_case! { before $before before_each {} test |$ctx| $test after_each {} after $after }
  };
  (before $before:block test |$ctx:ident| $test:block) => {
    $crate::test_case! { before $before before_each {} test |$ctx| $test after_each {} after {} }
  };
  (before_each $before_each:block test |$ctx:ident| $test:block after_each $after_each:block after $after:block) => {
    $crate::test_case! { before {} before_each $before_each test |$ctx| $test after_each $after_each after $after }
  };
  (before_each $before_each:block test |$ctx:ident| $test:block after_each $after_each:block) => {
    $crate::test_case! { before {} before_each $before_each test |$ctx| $test after_each $after_each after {} }
  };
  (before_each $before_each:block test |$ctx:ident| $test:block after $after:block) => {
    $crate::test_case! { before {} before_each $before_each test |$ctx| $test after_each {} after $after }
  };
  (before_each $before_each:block test |$ctx:ident| $test:block) => {
    $crate::test_case! { before {} before_each $before_each test |$ctx| $test after_each {} after {} }
  };
  (test |$ctx:ident| $test:block after_each $after_each:block after $after:block) => {
    $crate::test_case! { before {} before_each {} test |$ctx| $test after_each $after_each after $after }
  };
  (test |$ctx:ident| $test:block after_each $after_each:block) => {
    $crate::test_case! { before {} before_each {} test |$ctx| $test after_each $after_each after {} }
  };
  (test |$ctx:ident| $test:block after $after:block) => {
    $crate::test_case! { before {} before_each {} test |$ctx| $test after_each {} after $after }
  };
  (test |$ctx:ident| $test:block) => {
    $crate::test_case! { before {} before_each {} test |$ctx| $test after_each {} after {} }
  };
}

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
  registry_guard().contains_key(server_url)
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
