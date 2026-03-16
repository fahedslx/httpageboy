pub mod shared;

#[cfg(feature = "sync")]
pub mod sync;

#[cfg(any(feature = "async_tokio", feature = "async_smol", feature = "async_std"))]
pub mod r#async;
