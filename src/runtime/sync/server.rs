#![cfg(feature = "sync")]

use crate::core::cors::CorsPolicy;
use crate::core::request::{Request, RequestLimits, handle_request_sync};
use crate::core::request_type::Rt;
use crate::core::route::{Route, RouteEntry};
use crate::core::response::Response;
use crate::runtime::shared::{file_source_path, print_server_info, response_head, response_or_default};
use crate::runtime::sync::threadpool::ThreadPool;
use std::collections::HashMap;
use std::io::prelude::Write;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::time::Duration;

pub struct Server {
  url: String,
  listener: TcpListener,
  pool: Arc<Mutex<ThreadPool>>,
  routes: HashMap<(Rt, String), RouteEntry>,
  files_sources: Vec<String>,
  auto_close: bool,
  cors: Option<Arc<CorsPolicy>>,
  limits: RequestLimits,
}

impl Server {
  pub fn new(serving_url: &str, pool_size: u8) -> Result<Server, std::io::Error> {
    let listener = TcpListener::bind(serving_url)?;
    let url = listener.local_addr()?.to_string();
    let pool = Arc::new(Mutex::new(ThreadPool::new(pool_size as usize)));

    Ok(Server {
      url,
      listener,
      pool,
      routes: HashMap::new(),
      files_sources: Vec::new(),
      auto_close: true,
      cors: Some(Arc::new(CorsPolicy::default())),
      limits: RequestLimits::default(),
    })
  }

  pub fn set_auto_close(&mut self, state: bool) {
    self.auto_close = state;
  }

  pub fn set_body_limit(&mut self, bytes: usize) {
    self.limits.body_bytes = bytes;
  }

  pub fn set_header_limit(&mut self, bytes: usize) {
    self.limits.header_bytes = bytes;
  }

  pub fn set_read_timeout(&mut self, timeout: Duration) {
    self.limits.read_timeout = timeout;
  }

  pub fn set_cors(&mut self, policy: CorsPolicy) {
    self.cors = Some(Arc::new(policy));
  }

  pub fn set_cors_str(&mut self, config: &str) {
    self.set_cors(CorsPolicy::from_config_str(config));
  }

  pub fn url(&self) -> &str {
    self.url.as_str()
  }

  pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
    self.listener.local_addr()
  }

  pub fn routes<I>(&mut self, routes: I)
  where
    I: IntoIterator<Item = Route>,
  {
    for route in routes {
      let (key, handler) = route.into_parts();
      self.routes.insert(key, handler);
    }
  }

  pub fn add_files_source<S>(&mut self, base: S)
  where
    S: Into<String>,
  {
    self.files_sources.push(file_source_path(base));
  }

  pub fn run(&self) {
    print_server_info(self.listener.local_addr().unwrap(), self.auto_close);
    for stream in self.listener.incoming() {
      match stream {
        Ok(stream) => {
          self.handle_stream(stream);
        }
        Err(_err) => {
          // could log error here
        }
      }
    }
  }

  pub fn run_until_shutdown(&self, shutdown_rx: mpsc::Receiver<()>) {
    print_server_info(self.listener.local_addr().unwrap(), self.auto_close);
    let _ = self.listener.set_nonblocking(true);
    loop {
      if shutdown_rx.try_recv().is_ok() {
        break;
      }
      match self.listener.accept() {
        Ok((stream, _)) => self.handle_stream(stream),
        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
          std::thread::sleep(Duration::from_millis(10));
        }
        Err(_err) => {}
      }
    }
    self.stop();
  }

  pub fn stop(&self) {
    let mut pool = self.pool.lock().unwrap();
    pool.stop();
  }

  fn handle_stream(&self, stream: TcpStream) {
    let routes_local = self.routes.clone();
    let sources_local = self.files_sources.clone();
    let close_flag = self.auto_close;
    let cors_policy = self.cors.clone();
    let limits = self.limits;
    let pool = Arc::clone(&self.pool);
    pool.lock().unwrap().run(move || {
      let mut stream = stream;
      let (mut request, early_resp) = Request::parse_stream_sync(&stream, &routes_local, &sources_local, &limits);
      let origin = request.origin().map(str::to_string);
      let method = request.method.clone();
      let upgrade = request.upgrade_handler(&routes_local);
      let response = if let Some(resp) = early_resp {
        resp
      } else {
        let routed = handle_request_sync(&mut request, &routes_local, &sources_local);
        response_or_default(routed, &method, cors_policy.as_deref())
      };

      if response.status.starts_with("101 ") {
        if let Some(upgrade) = upgrade {
          Self::send_response(&mut stream, &response, false, cors_policy.as_deref(), origin.as_deref());
          futures::executor::block_on(upgrade.handle(request, stream));
          return;
        }
      }

      Self::send_response(
        &mut stream,
        &response,
        close_flag,
        cors_policy.as_deref(),
        origin.as_deref(),
      );
    });
  }

  fn send_response(
    stream: &mut TcpStream,
    response: &Response,
    close: bool,
    cors: Option<&CorsPolicy>,
    origin: Option<&str>,
  ) {
    let header = response_head(response, close, cors, origin);
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(&response.body);

    let _ = stream.flush();
    if close {
      let _ = stream.shutdown(Shutdown::Both);
    }
  }
}
