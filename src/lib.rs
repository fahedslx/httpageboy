pub mod core;

// Common re-exports (always available)
pub use crate::core::{
  cors::CorsPolicy,
  request_type::Rt,
  response::Response,
  status_code::StatusCode,
  test_utils,
};

// Feature-gated re-exports (exist only when any handler feature is enabled)
#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
pub use crate::core::{handler::Handler, request::Request, request_handler::Rh};

pub mod runtime {
  #[cfg(feature = "sync")]
  pub mod sync {
    pub mod server;
    pub mod threadpool;
  }

  #[cfg(any(feature = "async_tokio", feature = "async_smol", feature = "async_std"))]
  pub mod r#async {
    #[cfg(feature = "async_std")]
    pub mod async_std;
    pub mod shared;
    #[cfg(feature = "async_smol")]
    pub mod smol;
    #[cfg(feature = "async_tokio")]
    pub mod tokio;
  }

  pub mod shared;
}

// Server export selection
#[cfg(feature = "sync")]
pub use runtime::sync::server::Server;

#[cfg(all(not(feature = "sync"), feature = "async_tokio"))]
pub use runtime::r#async::tokio::Server;

#[cfg(all(not(feature = "sync"), not(feature = "async_tokio"), feature = "async_smol"))]
pub use runtime::r#async::smol::Server;

#[cfg(all(
  not(feature = "sync"),
  not(feature = "async_tokio"),
  not(feature = "async_smol"),
  feature = "async_std"
))]
pub use runtime::r#async::async_std::Server;

// Fallback dummy server if no feature is active
#[cfg(all(
  not(feature = "sync"),
  not(feature = "async_tokio"),
  not(feature = "async_smol"),
  not(feature = "async_std")
))]
pub struct Server;

#[cfg(all(
  not(feature = "sync"),
  not(feature = "async_tokio"),
  not(feature = "async_smol"),
  not(feature = "async_std")
))]
impl Server {
  pub fn new() -> Self {
    eprintln!(
      "\n❌ No feature is active.\n\nActivate a feature when compiling:\n\n    cargo run --features sync\n    cargo run --features async_tokio\n    cargo run --features async_std\n    cargo run --features async_smol\n"
    );
    panic!("No feature selected.");
  }
}
