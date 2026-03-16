use super::shared;
use crate::core::cors::CorsPolicy;
use crate::core::request::handle_request_async;
use crate::core::request_handler::Rh;
use crate::core::response::Response;
use crate::runtime::shared::print_server_info;
use async_trait::async_trait;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

#[async_trait]
impl shared::AsyncStream for TcpStream {
  async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
    AsyncWriteExt::write_all(self, buf).await
  }

  async fn flush(&mut self) -> std::io::Result<()> {
    AsyncWriteExt::flush(self).await
  }

  async fn shutdown(&mut self) -> std::io::Result<()> {
    AsyncWriteExt::shutdown(self).await
  }
}

/// A non-blocking HTTP server powered by Tokio.
pub struct Server(pub shared::GenericServer<TcpListener>);

impl Deref for Server {
  type Target = shared::GenericServer<TcpListener>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for Server {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl Server {
  /// Creates a new server and binds to the specified URL.
  pub async fn new(
    serving_url: &str,
    routes_list: Option<HashMap<(crate::core::request_type::Rt, String), Rh>>,
  ) -> std::io::Result<Self> {
    let listener = TcpListener::bind(serving_url).await?;
    let url = listener.local_addr()?.to_string();
    Ok(Server(shared::GenericServer {
      listener,
      url,
      routes: Arc::new(routes_list.unwrap_or_default()),
      files_sources: Arc::new(Vec::new()),
      auto_close: true,
      cors: Some(Arc::new(CorsPolicy::default())),
    }))
  }

  /// Returns the socket address the server is currently bound to.
  pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
    self.listener.local_addr()
  }

  pub fn url(&self) -> &str {
    self.0.url.as_str()
  }

  pub fn set_cors(&mut self, policy: CorsPolicy) {
    self.0.cors = Some(Arc::new(policy));
  }

  pub fn set_cors_str(&mut self, config: &str) {
    self.set_cors(CorsPolicy::from_config_str(config));
  }

  /// Starts the server and begins accepting connections.
  pub async fn run(&self) {
    print_server_info(self.listener.local_addr().unwrap(), self.auto_close);
    loop {
      if let Ok((mut stream, _)) = self.listener.accept().await {
        let routes = self.routes.clone();
        let sources = self.files_sources.clone();
        let close_flag = self.auto_close;
        let cors_policy = self.cors.clone();

        tokio::spawn(async move {
          let (mut req, early) = crate::core::request::parse_stream_tokio(
            &mut stream,
            &routes,
            &sources,
          )
          .await;
          let origin = req.origin().map(str::to_string);
          let method = req.method.clone();
          let resp = match early {
            Some(r) => r,
            None => {
              let routed = handle_request_async(&mut req, &routes, &sources).await;
              if routed.is_none()
                && method == crate::core::request_type::RequestType::OPTIONS
                && cors_policy.is_some()
              {
                cors_policy
                  .as_deref()
                  .map(|policy| policy.preflight_response())
                  .unwrap_or_else(Response::new)
              } else {
                routed.unwrap_or_else(Response::new)
              }
            }
          };
          shared::send_response(
            &mut stream,
            &resp,
            close_flag,
            cors_policy.as_deref(),
            origin.as_deref(),
          )
          .await;
        });
      }
    }
  }
}
