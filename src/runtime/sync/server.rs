#![cfg(feature = "sync")]

use crate::core::cors::CorsPolicy;
use crate::core::handler::Handler;
use crate::core::request::{Request, handle_request_sync};
use crate::core::request_handler::Rh;
use crate::core::request_type::Rt;
use crate::core::response::Response;
use crate::runtime::shared::print_server_info;
use crate::runtime::sync::threadpool::ThreadPool;
use std::collections::HashMap;
use std::io::prelude::Write;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

pub struct Server {
  url: String,
  listener: TcpListener,
  pool: Arc<Mutex<ThreadPool>>,
  routes: HashMap<(Rt, String), Rh>,
  files_sources: Vec<String>,
  auto_close: bool,
  cors: Option<Arc<CorsPolicy>>,
}

impl Server {
  pub fn new(
    serving_url: &str,
    pool_size: u8,
    routes_list: Option<HashMap<(Rt, String), Rh>>,
  ) -> Result<Server, std::io::Error> {
    let listener = TcpListener::bind(serving_url)?;
    let url = listener.local_addr()?.to_string();
    let pool = Arc::new(Mutex::new(ThreadPool::new(pool_size as usize)));
    let routes = routes_list.unwrap_or_default();

    Ok(Server {
      url,
      listener,
      pool,
      routes,
      files_sources: Vec::new(),
      auto_close: true,
      cors: Some(Arc::new(CorsPolicy::default())),
    })
  }

  pub fn set_auto_close(&mut self, state: bool) {
    self.auto_close = state;
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

  pub fn add_route(&mut self, path: &str, rt: Rt, handler: Arc<dyn Handler>) {
    let key = (rt, path.to_string());
    self.routes.insert(key, Rh { handler });
  }

  pub fn add_files_source<S>(&mut self, base: S)
  where
    S: Into<String>,
  {
    let s = base.into();
    let canonical = PathBuf::from(&s)
      .canonicalize()
      .map(|p| p.to_string_lossy().to_string())
      .unwrap_or(s.clone());
    self.files_sources.push(canonical);
  }

  pub fn run(&self) {
    print_server_info(self.listener.local_addr().unwrap(), self.auto_close);
    for stream in self.listener.incoming() {
      match stream {
        Ok(stream) => {
          let routes_local = self.routes.clone();
          let sources_local = self.files_sources.clone();
          let close_flag = self.auto_close;
          let cors_policy = self.cors.clone();
          let pool = Arc::clone(&self.pool);
          pool.lock().unwrap().run(move || {
            let (mut request, early_resp) = Request::parse_stream_sync(&stream, &routes_local, &sources_local);
            let origin = request.origin().map(str::to_string);
          let method = request.method.clone();
          let answer = if let Some(resp) = early_resp {
            Some(resp)
          } else {
            let routed = handle_request_sync(&mut request, &routes_local, &sources_local);
            if routed.is_none()
              && method == crate::core::request_type::RequestType::OPTIONS
              && cors_policy.is_some()
            {
              Some(Self::preflight_response(cors_policy.as_deref()))
            } else {
              routed
            }
          };
            match answer {
              Some(response) => Self::send_response(
                stream,
                &response,
                close_flag,
                cors_policy.as_deref(),
                origin.as_deref(),
              ),
              None => Self::send_response(
                stream,
                &Response::new(),
                close_flag,
                cors_policy.as_deref(),
                origin.as_deref(),
              ),
            }
          });
        }
        Err(_err) => {
          // could log error here
        }
      }
    }
  }

  pub fn stop(&self) {
    let mut pool = self.pool.lock().unwrap();
    pool.stop();
  }

  fn preflight_response(cors: Option<&CorsPolicy>) -> Response {
    if let Some(policy) = cors {
      return policy.preflight_response();
    }
    Response::new()
  }

  fn send_response(
    mut stream: TcpStream,
    response: &Response,
    close: bool,
    cors: Option<&CorsPolicy>,
    origin: Option<&str>,
  ) {
    let connection_header = if close { "Connection: close\r\n" } else { "" };
    let mut header = format!(
      "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{}",
      response.status,
      response.content_type,
      response.content.len(),
      connection_header
    );
    if let Some(policy) = cors {
      for (k, v) in policy.header_lines(origin) {
        header.push_str(&format!("{}: {}\r\n", k, v));
      }
    }
    header.push_str("\r\n");
    let _ = stream.write_all(header.as_bytes());

    if response.content_type.starts_with("image/") {
      let _ = stream.write_all(&response.content);
    } else {
      let text = String::from_utf8_lossy(&response.content);
      let _ = stream.write_all(text.as_bytes());
    }

    let _ = stream.flush();
    if close {
      let _ = stream.shutdown(Shutdown::Both);
    }
  }
}
