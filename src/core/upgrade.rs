#![cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]

use crate::Request;
use async_trait::async_trait;

#[cfg(feature = "sync")]
pub type Connection = std::net::TcpStream;

#[cfg(all(not(feature = "sync"), feature = "async_tokio"))]
pub type Connection = tokio::net::TcpStream;

#[cfg(all(
  not(feature = "sync"),
  not(feature = "async_tokio"),
  feature = "async_smol"
))]
pub type Connection = smol::net::TcpStream;

#[cfg(all(
  not(feature = "sync"),
  not(feature = "async_tokio"),
  not(feature = "async_smol"),
  feature = "async_std"
))]
pub type Connection = async_std::net::TcpStream;

#[async_trait]
pub trait UpgradeHandler: Send + Sync {
  async fn handle(&self, request: Request, connection: Connection);
}
