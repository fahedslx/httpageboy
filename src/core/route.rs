#![cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]

use crate::core::handler::Handler;
use crate::core::request_type::Rt;
use std::sync::Arc;

/// A declarative HTTP route that can be registered individually or in a batch.
pub struct Route {
  path: String,
  method: Rt,
  handler: Arc<dyn Handler>,
}

impl Route {
  pub fn new<P>(path: P, method: Rt, handler: Arc<dyn Handler>) -> Self
  where
    P: Into<String>,
  {
    Self {
      path: path.into(),
      method,
      handler,
    }
  }

  pub(crate) fn into_parts(self) -> ((Rt, String), Arc<dyn Handler>) {
    ((self.method, self.path), self.handler)
  }
}

/// Builds a route for synchronous servers.
#[macro_export]
#[cfg(feature = "sync")]
macro_rules! route {
  ($path:expr, $method:expr, $handler_fn:expr) => {
    $crate::Route::new($path, $method, $crate::core::handler::sync_h($handler_fn))
  };
}

/// Builds a route for asynchronous servers.
#[macro_export]
#[cfg(all(
  any(feature = "async_tokio", feature = "async_std", feature = "async_smol"),
  not(feature = "sync")
))]
macro_rules! route {
  ($path:expr, $method:expr, $handler_fn:expr) => {
    $crate::Route::new(
      $path,
      $method,
      $crate::core::handler::async_h(move |req| Box::pin($handler_fn(req))),
    )
  };
}
