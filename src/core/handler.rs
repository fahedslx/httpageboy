// src/core/handler.rs

#![cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]

use crate::{Request, Response};
use async_trait::async_trait;
use futures::future::BoxFuture;
use std::sync::Arc;

/// The core, unified `Handler` trait, powered by `async-trait`.
#[async_trait]
pub trait Handler: Send + Sync {
  async fn handle(&self, request: &Request) -> Response;
}

// Blanket implementation for Arc<dyn Handler> for convenience.
#[async_trait]
impl Handler for Arc<dyn Handler> {
  async fn handle(&self, request: &Request) -> Response {
    (**self).handle(request).await
  }
}

// Internal adapters used by route!.

// Wraps a synchronous route function.
struct SyncFnHandler<F>(F);

#[async_trait]
impl<F> Handler for SyncFnHandler<F>
where
  F: for<'a> Fn(&'a Request) -> Response + Send + Sync,
{
  async fn handle(&self, request: &Request) -> Response {
    (self.0)(request)
  }
}

/// Wraps a synchronous function, turning it into a type that implements `Handler`.
pub fn sync_h<F>(f: F) -> Arc<dyn Handler>
where
  F: for<'a> Fn(&'a Request) -> Response + Send + Sync + 'static,
{
  Arc::new(SyncFnHandler(f))
}

// Wraps an asynchronous route function that returns a BoxFuture.
struct AsyncFnHandler<F>(F);

#[async_trait]
impl<F> Handler for AsyncFnHandler<F>
where
  F: for<'a> Fn(&'a Request) -> BoxFuture<'a, Response> + Send + Sync,
{
  async fn handle(&self, request: &Request) -> Response {
    (self.0)(request).await
  }
}

/// Wraps an asynchronous closure that returns a BoxFuture.
pub fn async_h<F>(f: F) -> Arc<dyn Handler>
where
  F: for<'a> Fn(&'a Request) -> BoxFuture<'a, Response> + Send + Sync + 'static,
{
  Arc::new(AsyncFnHandler(f))
}

