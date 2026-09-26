#![cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]

use crate::core::handler::Handler;
use crate::core::request_handler::Rh;
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

  pub(crate) fn into_parts(self) -> ((Rt, String), Rh) {
    ((self.method, self.path), Rh { handler: self.handler })
  }
}

/// Builds a route while keeping runtime-specific handler wrapping out of user code.
#[macro_export]
macro_rules! route {
  ($path:expr, $method:expr, $handler_fn:expr) => {
    $crate::Route::new($path, $method, $crate::handler!($handler_fn))
  };
}
