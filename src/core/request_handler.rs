#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
mod request_handler_enabled {
  use crate::core::handler::Handler;
  use std::sync::Arc;

  pub type Rh = RequestHandler;

  /// A wrapper for a route handler.
  /// It stores the handler as a type-erased, shareable trait object.
  pub struct RequestHandler {
    pub handler: Arc<dyn Handler>,
  }

  impl Clone for RequestHandler {
    fn clone(&self) -> Self {
      RequestHandler {
        handler: self.handler.clone(),
      }
    }
  }

  impl std::fmt::Debug for RequestHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.debug_struct("RequestHandler")
        .field("handler", &"Arc<dyn Handler>")
        .finish()
    }
  }
}

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
pub use request_handler_enabled::*;
