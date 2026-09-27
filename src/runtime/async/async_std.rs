use crate::core::cors::CorsPolicy;
use crate::core::request::{StreamRead, handle_request_async};
use crate::runtime::r#async::shared;
use crate::runtime::shared::{print_server_info, response_or_default};
use async_std::io::prelude::*;
use async_std::net::{Shutdown, TcpListener, TcpStream};
use async_std::task::spawn;
use async_trait::async_trait;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

#[async_trait]
impl shared::AsyncStream for TcpStream {
  async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
    WriteExt::write_all(self, buf).await
  }

  async fn flush(&mut self) -> std::io::Result<()> {
    WriteExt::flush(self).await
  }

  async fn shutdown(&mut self) -> std::io::Result<()> {
    std::future::ready(TcpStream::shutdown(self, Shutdown::Both)).await
  }
}

/// A non-blocking HTTP server powered by async-std.
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
  pub async fn new(serving_url: &str) -> std::io::Result<Self> {
    let listener = TcpListener::bind(serving_url).await?;
    let url = listener.local_addr()?.to_string();
    Ok(Server(shared::GenericServer {
      listener,
      url,
      routes: Arc::new(Default::default()),
      files_sources: Arc::new(Vec::new()),
      cors: Some(Arc::new(CorsPolicy::default())),
      limits: Default::default(),
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
    print_server_info(self.listener.local_addr().unwrap());
    while let Ok((stream, _)) = self.listener.accept().await {
      self.handle_stream(stream);
    }
  }

  pub async fn run_until_shutdown(&self, shutdown_rx: mpsc::Receiver<()>) {
    print_server_info(self.listener.local_addr().unwrap());
    loop {
      if shutdown_rx.try_recv().is_ok() {
        break;
      }
      if let Ok(Ok((stream, _))) = async_std::future::timeout(Duration::from_millis(10), self.listener.accept()).await {
        self.handle_stream(stream);
      }
    }
  }

  fn handle_stream(&self, mut stream: TcpStream) {
    let routes = self.routes.clone();
    let files = self.files_sources.clone();
    let cors_policy = self.cors.clone();
    let limits = self.limits;

    spawn(async move {
      let mut buffer = Vec::new();
      let mut handled = 0usize;

      loop {
        let (mut req, early) = match crate::core::request::parse_stream_async_std(
          &mut stream,
          &mut buffer,
          &routes,
          &files,
          &limits,
          handled > 0,
        )
        .await
        {
          StreamRead::Ready(request, early) => (request, early),
          StreamRead::Idle | StreamRead::Closed => break,
          StreamRead::Error(response) => {
            let _ = shared::send_response(
              &mut stream,
              &response,
              true,
              cors_policy.as_deref(),
              None,
            )
            .await;
            break;
          }
        };

        handled += 1;
        let origin = req.origin().map(str::to_string);
        let method = req.method.clone();
        let upgrade = req.upgrade_handler(&routes);
        let request_close = req.wants_close();

        let resp = match early {
          Some(r) => r,
          None => {
            let routed = handle_request_async(&mut req, &routes, &files).await;
            response_or_default(routed, &method, cors_policy.as_deref())
          }
        };

        if resp.status == crate::StatusCode::SwitchingProtocols {
          if let Some(upgrade) = upgrade {
            if !shared::send_response(
              &mut stream,
              &resp,
              false,
              cors_policy.as_deref(),
              origin.as_deref(),
            )
            .await
            {
              return;
            }
            upgrade.handle(req, stream).await;
            return;
          }
        }

        let response_close = resp
          .headers
          .iter()
          .filter(|(key, _)| key.eq_ignore_ascii_case("connection"))
          .flat_map(|(_, value)| value.split(','))
          .any(|token| token.trim().eq_ignore_ascii_case("close"));
        let close = request_close || response_close || handled >= limits.max_requests;

        if !shared::send_response(
          &mut stream,
          &resp,
          close,
          cors_policy.as_deref(),
          origin.as_deref(),
        )
        .await
        {
          break;
        }

        if close {
          break;
        }
      }
    });
  }}
