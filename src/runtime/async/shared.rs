use crate::core::cors::CorsPolicy;
use crate::core::request::RequestLimits;
use crate::core::request_type::Rt;
use crate::core::route::{Route, RouteEntry};
use crate::core::response::Response;
use crate::runtime::shared as runtime_shared;
use crate::runtime::shared::INTERNAL_SERVER_ERROR_HEAD;
use async_trait::async_trait;
use std::collections::HashMap;
use std::io::Result;
use std::sync::Arc;
use std::time::Duration;

/// A trait that abstracts over the different async TCP streams.
/// This allows us to write generic code that can work with any of the supported runtimes.
#[async_trait]
pub trait AsyncStream: Send + Sync {
  async fn write_all(&mut self, buf: &[u8]) -> Result<()>;
  async fn flush(&mut self) -> Result<()>;
  async fn shutdown(&mut self) -> Result<()>;
}

/// Sends a response to the client over the given stream.
pub async fn send_response<S: AsyncStream>(
  stream: &mut S,
  resp: &Response,
  method: Option<&Rt>,
  close: bool,
  cors: Option<&CorsPolicy>,
  origin: Option<&str>,
) -> bool {
  let Some(head) = runtime_shared::response_head(resp, method, close, cors, origin) else {
    let _ = stream.write_all(INTERNAL_SERVER_ERROR_HEAD.as_bytes()).await;
    let _ = stream.flush().await;
    let _ = stream.shutdown().await;
    return false;
  };

  let _ = stream.write_all(head.as_bytes()).await;
  if runtime_shared::response_has_content(method, resp) {
    let _ = stream.write_all(resp.body.as_ref()).await;
  }
  let _ = stream.flush().await;
  if close {
    let _ = stream.shutdown().await;
  }
  true
}

/// A generic server implementation that is parameterized over a listener type.
/// This allows us to share the server logic between the different async runtimes.
pub struct GenericServer<L> {
  pub listener: L,
  pub url: String,
  pub routes: Arc<HashMap<(Rt, String), RouteEntry>>,
  pub files_sources: Arc<Vec<String>>,
  pub cors: Option<Arc<CorsPolicy>>,
  pub limits: RequestLimits,
}

impl<L> GenericServer<L> {
  pub fn set_body_limit(&mut self, bytes: usize) {
    self.limits.body_bytes = bytes;
  }

  pub fn set_header_limit(&mut self, bytes: usize) {
    self.limits.header_bytes = bytes;
  }

  pub fn set_idle_timeout(&mut self, timeout: Duration) {
    self.limits.idle_timeout = timeout;
  }

  pub fn set_req_timeout(&mut self, timeout: Duration) {
    self.limits.request_timeout = timeout;
  }

  pub fn set_keep_alive(&mut self, timeout: Duration) {
    self.limits.keep_alive_timeout = timeout;
  }

  pub fn set_max_requests(&mut self, requests: usize) {
    self.limits.max_requests = requests.max(1);
  }

  pub fn routes<I>(&mut self, routes: I)
  where
    I: IntoIterator<Item = Route>,
  {
    let route_map = Arc::get_mut(&mut self.routes).unwrap();
    for route in routes {
      let (key, handler) = route.into_parts();
      route_map.insert(key, handler);
    }
  }

  pub fn url(&self) -> &str {
    self.url.as_str()
  }

  /// Adds a new directory to serve static files from.
  pub fn add_files_source<S>(&mut self, base: S)
  where
    S: Into<String>,
  {
    Arc::get_mut(&mut self.files_sources)
      .unwrap()
      .push(runtime_shared::file_source_path(base));
  }
}
