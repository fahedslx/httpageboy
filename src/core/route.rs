#![cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]

use crate::core::handler::Handler;
use crate::core::request_type::Rt;
use crate::core::upgrade::UpgradeHandler;
use std::sync::Arc;

#[derive(Clone)]
#[doc(hidden)]
pub struct RouteEntry {
  pub handler: Arc<dyn Handler>,
  pub upgrade: Option<Arc<dyn UpgradeHandler>>,
}

/// A declarative route that can finish as HTTP or hand the connection to an extension.
pub struct Route {
  path: String,
  method: Rt,
  entry: RouteEntry,
}

impl Route {
  pub fn new<P>(path: P, method: Rt, handler: Arc<dyn Handler>) -> Self
  where
    P: Into<String>,
  {
    Self {
      path: path.into(),
      method,
      entry: RouteEntry {
        handler,
        upgrade: None,
      },
    }
  }

  pub fn with_upgrade<P>(
    path: P,
    method: Rt,
    handler: Arc<dyn Handler>,
    upgrade: Arc<dyn UpgradeHandler>,
  ) -> Self
  where
    P: Into<String>,
  {
    Self {
      path: path.into(),
      method,
      entry: RouteEntry {
        handler,
        upgrade: Some(upgrade),
      },
    }
  }

  pub(crate) fn into_parts(self) -> ((Rt, String), RouteEntry) {
    ((self.method, self.path), self.entry)
  }
}

/// Builds a route for synchronous servers.
#[macro_export]
#[cfg(feature = "sync")]
macro_rules! route {
  ($path:expr, $method:expr, $handler_fn:expr) => {
    $crate::Route::new($path, $method, $crate::core::handler::sync_h($handler_fn))
  };
  ($path:expr, $method:expr, $handler_fn:expr, $protocol:expr) => {
    $protocol.route($path, $method, $handler_fn)
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
  ($path:expr, $method:expr, $handler_fn:expr, $protocol:expr) => {
    $protocol.route($path, $method, $handler_fn)
  };
}
