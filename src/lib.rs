#[cfg(not(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
)))]
compile_error!(
  "Select one runtime feature, e.g. --features sync or --features async_tokio."
);

#[cfg(any(
  all(feature = "sync", feature = "async_tokio"),
  all(feature = "sync", feature = "async_std"),
  all(feature = "sync", feature = "async_smol"),
  all(feature = "async_tokio", feature = "async_std"),
  all(feature = "async_tokio", feature = "async_smol"),
  all(feature = "async_std", feature = "async_smol")
))]
compile_error!("Select exactly one runtime feature.");

pub mod core;

// Common re-exports (always available)
pub use crate::core::{cors::CorsPolicy, request_type::Rt, response::Response, status_code::StatusCode, test_utils};

// Feature-gated re-exports (exist only when any handler feature is enabled)
#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
pub use crate::core::{request::Request, route::Route, upgrade::{Connection, UpgradeHandler}};

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
