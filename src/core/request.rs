/// Generates a `parse_stream` function for a specific async runtime.
///
/// This macro abstracts the common logic of reading and parsing an HTTP request
/// from a TCP stream, while allowing the caller to specify the runtime-specific
/// types and traits (stream type, BufReader, and I/O extension traits).
pub const READ_TIMEOUT_MS: u64 = 50;
pub const BODY_READ_LIMIT_BYTES: u64 = 512;

macro_rules! create_async_parse_stream {
    (
        $(#[$outer:meta])*
        $func_name:ident,
        $stream_ty:ty,
        $buf_reader:ty,
        $async_read_ext:path,
        $async_buf_read_ext:path
    ) => {
        $(#[$outer])*
        pub async fn $func_name(
            stream: &mut $stream_ty,
            routes: &std::collections::HashMap<(crate::core::request_type::Rt, String), crate::core::request_handler::Rh>,
            file_bases: &[String],
        ) -> (crate::core::request::Request, Option<crate::core::response::Response>) {
            use $async_read_ext;
            use $async_buf_read_ext;

            let mut reader = <$buf_reader>::new(stream);
            let mut raw = String::new();
            let header_timeout = std::time::Duration::from_millis(crate::core::request::READ_TIMEOUT_MS);

            // Read headers only
            loop {
                let mut line = String::new();
                #[cfg(feature = "async_tokio")]
                {
                    let read_fut = reader.read_line(&mut line);
                    let sleep = tokio::time::sleep(header_timeout);
                    futures::pin_mut!(read_fut, sleep);
                    match futures::future::select(read_fut, sleep).await {
                        futures::future::Either::Left((Ok(n), _)) if n > 0 => {
                            raw.push_str(&line);
                            if raw.contains("\r\n\r\n") {
                                break;
                            }
                        }
                        _ => break,
                    }
                }

                #[cfg(all(feature = "async_std", not(feature = "async_tokio")))]
                {
                    let read_fut = reader.read_line(&mut line);
                    let sleep = async_std::task::sleep(header_timeout);
                    futures::pin_mut!(read_fut, sleep);
                    match futures::future::select(read_fut, sleep).await {
                        futures::future::Either::Left((Ok(n), _)) if n > 0 => {
                            raw.push_str(&line);
                            if raw.contains("\r\n\r\n") {
                                break;
                            }
                        }
                        _ => break,
                    }
                }

                #[cfg(all(feature = "async_smol", not(any(feature = "async_tokio", feature = "async_std"))))]
                {
                    let read_fut = reader.read_line(&mut line);
                    let sleep = smol::Timer::after(header_timeout);
                    futures::pin_mut!(read_fut, sleep);
                    match futures::future::select(read_fut, sleep).await {
                        futures::future::Either::Left((Ok(n), _)) if n > 0 => {
                            raw.push_str(&line);
                            if raw.contains("\r\n\r\n") {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
            }

            // Extract method from the first line
            let method = raw
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().next())
                .unwrap_or("");

            let (content_length, has_transfer_encoding) = crate::core::request::extract_body_headers(&raw);

            // Read declared body size. For POST/PUT/DELETE/PATCH without Content-Length or Transfer-Encoding, fall back to a timed read.
            if content_length > 0 {
                // Read exactly content_length bytes
                let mut buf = vec![0; content_length];
                let _ = reader.read_exact(&mut buf).await;
                raw.push_str(&String::from_utf8_lossy(&buf));
            } else if method == "POST" || method == "PUT" || method == "DELETE" || method == "PATCH" {
                if has_transfer_encoding {
                    // Read all until EOF for POST/PUT/DELETE/PATCH with Transfer-Encoding and without Content-Length
                    let mut rest = String::new();

                    #[cfg(feature = "async_tokio")]
                    {
                        use std::time::Duration;
                        let mut limited = reader.take(crate::core::request::BODY_READ_LIMIT_BYTES);
                        let read_fut = limited.read_to_string(&mut rest);
                        let sleep = tokio::time::sleep(Duration::from_millis(crate::core::request::READ_TIMEOUT_MS));
                        futures::pin_mut!(read_fut, sleep);
                        if matches!(
                            futures::future::select(read_fut, sleep).await,
                            futures::future::Either::Left((Ok(_), _))
                        ) {
                            raw.push_str(&rest);
                        }
                    }

                    #[cfg(all(feature = "async_std", not(feature = "async_tokio")))]
                    {
                        use std::time::Duration;
                        let mut limited = reader.take(crate::core::request::BODY_READ_LIMIT_BYTES);
                        let read_fut = limited.read_to_string(&mut rest);
                        let sleep = async_std::task::sleep(Duration::from_millis(crate::core::request::READ_TIMEOUT_MS));
                        futures::pin_mut!(read_fut, sleep);
                        if matches!(
                            futures::future::select(read_fut, sleep).await,
                            futures::future::Either::Left((Ok(_), _))
                        ) {
                            raw.push_str(&rest);
                        }
                    }

                    #[cfg(all(feature = "async_smol", not(any(feature = "async_tokio", feature = "async_std"))))]
                    {
                        use std::time::Duration;
                        let mut limited = reader.take(crate::core::request::BODY_READ_LIMIT_BYTES);
                        let read_fut = limited.read_to_string(&mut rest);
                        let sleep = smol::Timer::after(Duration::from_millis(crate::core::request::READ_TIMEOUT_MS));
                        futures::pin_mut!(read_fut, sleep);
                        if matches!(
                            futures::future::select(read_fut, sleep).await,
                            futures::future::Either::Left((Ok(_), _))
                        ) {
                            raw.push_str(&rest);
                        }
                    }
                } else {
                    // No length hints; read opportunistically with a short timeout to avoid hanging.
                    let mut buf: Vec<u8> = Vec::new();

                    #[cfg(feature = "async_tokio")]
                    {
                        use std::time::Duration;
                        let mut chunk = [0u8; 128];
                        loop {
                            let read_fut = reader.read(&mut chunk);
                            let sleep = tokio::time::sleep(Duration::from_millis(crate::core::request::READ_TIMEOUT_MS));
                            futures::pin_mut!(read_fut, sleep);
                            match futures::future::select(read_fut, sleep).await {
                                futures::future::Either::Left((Ok(0), _)) => break,
                                futures::future::Either::Left((Ok(n), _)) => {
                                    buf.extend_from_slice(&chunk[..n]);
                                    if buf.len() as u64 >= crate::core::request::BODY_READ_LIMIT_BYTES {
                                        break;
                                    }
                                }
                                futures::future::Either::Left((Err(_), _)) => break,
                                futures::future::Either::Right(_) => break,
                            }
                        }
                    }

                    #[cfg(all(feature = "async_std", not(feature = "async_tokio")))]
                    {
                        use std::time::Duration;
                        let mut chunk = [0u8; 128];
                        loop {
                            let read_fut = reader.read(&mut chunk);
                            let sleep = async_std::task::sleep(Duration::from_millis(crate::core::request::READ_TIMEOUT_MS));
                            futures::pin_mut!(read_fut, sleep);
                            match futures::future::select(read_fut, sleep).await {
                                futures::future::Either::Left((Ok(0), _)) => break,
                                futures::future::Either::Left((Ok(n), _)) => {
                                    buf.extend_from_slice(&chunk[..n]);
                                    if buf.len() as u64 >= crate::core::request::BODY_READ_LIMIT_BYTES {
                                        break;
                                    }
                                }
                                futures::future::Either::Left((Err(_), _)) => break,
                                futures::future::Either::Right(_) => break,
                            }
                        }
                    }

                    #[cfg(all(feature = "async_smol", not(any(feature = "async_tokio", feature = "async_std"))))]
                    {
                        use std::time::Duration;
                        let mut chunk = [0u8; 128];
                        loop {
                            let read_fut = reader.read(&mut chunk);
                            let sleep = smol::Timer::after(Duration::from_millis(crate::core::request::READ_TIMEOUT_MS));
                            futures::pin_mut!(read_fut, sleep);
                            match futures::future::select(read_fut, sleep).await {
                                futures::future::Either::Left((Ok(0), _)) => break,
                                futures::future::Either::Left((Ok(n), _)) => {
                                    buf.extend_from_slice(&chunk[..n]);
                                    if buf.len() as u64 >= crate::core::request::BODY_READ_LIMIT_BYTES {
                                        break;
                                    }
                                }
                                futures::future::Either::Left((Err(_), _)) => break,
                                futures::future::Either::Right(_) => break,
                            }
                        }
                    }

                    if !buf.is_empty() {
                        raw.push_str(&String::from_utf8_lossy(&buf));
                    }
                }
            }

            crate::core::request::Request::parse_raw_async(raw, routes, file_bases).await
        }
    };
}

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
fn extract_body_headers(raw: &str) -> (usize, bool) {
  let mut content_length = 0usize;
  let mut has_transfer_encoding = false;
  for line in raw.lines() {
    let lower = line.to_ascii_lowercase();
    if lower.starts_with("content-length:") {
      if let Some(len) = line.split(':').nth(1).and_then(|v| v.trim().parse::<usize>().ok()) {
        content_length = len;
      }
    } else if lower.starts_with("transfer-encoding:") {
      has_transfer_encoding = true;
    }
  }
  (content_length, has_transfer_encoding)
}

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
use crate::core::request_handler::Rh;
#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
use crate::core::request_type::{RequestType, Rt};
#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
use crate::core::response::Response;
#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
use crate::core::status_code::StatusCode;
#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
use std::collections::{BTreeMap, HashMap};
#[cfg(feature = "sync")]
use std::net::TcpStream;
#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
use std::path::Path;

#[cfg(feature = "async_std")]
use async_std;
#[cfg(feature = "async_smol")]
use futures_lite;
#[cfg(feature = "async_smol")]
use smol;
#[cfg(feature = "async_tokio")]
use tokio;

create_async_parse_stream!(
  #[cfg(feature = "async_tokio")]
  parse_stream_tokio,
  tokio::net::TcpStream,
  tokio::io::BufReader<_>,
  tokio::io::AsyncReadExt,
  tokio::io::AsyncBufReadExt
);

create_async_parse_stream!(
  #[cfg(feature = "async_std")]
  parse_stream_async_std,
  async_std::net::TcpStream,
  async_std::io::BufReader<_>,
  async_std::io::ReadExt,
  async_std::io::BufReadExt
);

create_async_parse_stream!(
  #[cfg(feature = "async_smol")]
  parse_stream_smol,
  smol::net::TcpStream,
  futures_lite::io::BufReader<_>,
  futures_lite::io::AsyncReadExt,
  futures_lite::io::AsyncBufReadExt
);

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
pub struct Request {
  pub method: RequestType,
  pub path: String,
  pub version: String,
  pub headers: Vec<(String, String)>,
  pub body: String,
  pub params: HashMap<String, String>,
}

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
impl Request {
  fn extract_params(route: &str, path: &str) -> HashMap<String, String> {
    let mut sorted: BTreeMap<String, String> = BTreeMap::new();
    let route_parts = route.split('/').collect::<Vec<_>>();
    let path_parts = path.split('/').collect::<Vec<_>>();
    if route_parts.len() != path_parts.len() {
      return HashMap::new();
    }
    for (i, part) in route_parts.iter().enumerate() {
      if part.starts_with('{') && part.ends_with('}') {
        let key = part.trim_matches(&['{', '}'][..]).to_string();
        sorted.insert(key, path_parts[i].to_string());
      } else if *part != path_parts[i] {
        return HashMap::new();
      }
    }
    sorted.into_iter().collect()
  }

  pub fn origin(&self) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|(k, _)| k.eq_ignore_ascii_case("origin"))
      .map(|(_, v)| v.as_str())
  }

  #[cfg(feature = "sync")]
  pub fn parse_stream_sync(
    stream: &TcpStream,
    routes: &HashMap<(Rt, String), Rh>,
    file_bases: &[String],
  ) -> (Self, Option<Response>) {
    use std::io::{BufRead, BufReader, Read};
    use std::time::Duration;

    let mut reader = BufReader::new(stream);
    let mut raw = String::new();
    let header_timeout = Duration::from_millis(READ_TIMEOUT_MS);
    let _ = stream.set_read_timeout(Some(header_timeout));

    // Read only headers
    loop {
      let mut line = String::new();
      if reader.read_line(&mut line).ok().filter(|&n| n > 0).is_none() {
        break;
      }
      raw.push_str(&line);
      if raw.contains("\r\n\r\n") {
        break;
      }
    }

    // Extract method from the first line
    let method = raw
      .lines()
      .next()
      .and_then(|l| l.split_whitespace().next())
      .unwrap_or("");

    let (content_length, has_transfer_encoding) = extract_body_headers(&raw);
    let _ = stream.set_read_timeout(None);

    // Require Content-Length when provided; otherwise read with a short timeout to avoid blocking on keep-alive.
    if content_length > 0 {
      // Read exactly content_length
      let mut buf = vec![0; content_length];
      let _ = reader.read_exact(&mut buf);
      raw.push_str(&String::from_utf8_lossy(&buf));
    } else if method == "POST" || method == "PUT" || method == "DELETE" || method == "PATCH" {
      if has_transfer_encoding {
        // Read all until EOF for POST/PUT/DELETE/PATCH without Content-Length
        let mut rest = String::new();
        let _ = stream.set_read_timeout(Some(Duration::from_millis(READ_TIMEOUT_MS)));
        let _ = reader.take(BODY_READ_LIMIT_BYTES).read_to_string(&mut rest);
        let _ = stream.set_read_timeout(None);
        raw.push_str(&rest);
      } else {
        // No Content-Length or Transfer-Encoding; read whatever is readily available.
        let _ = stream.set_read_timeout(Some(Duration::from_millis(READ_TIMEOUT_MS)));
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 128];
        loop {
          match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
              buf.extend_from_slice(&chunk[..n]);
              if buf.len() as u64 >= BODY_READ_LIMIT_BYTES {
                break;
              }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
              break;
            }
            Err(_) => break,
          }
        }
        let _ = stream.set_read_timeout(None);
        if !buf.is_empty() {
          raw.push_str(&String::from_utf8_lossy(&buf));
        }
      }
    }

    Self::parse_raw_sync(raw, routes, file_bases)
  }

  #[cfg(feature = "sync")]
  pub fn parse_raw_sync(
    raw: String,
    routes: &HashMap<(Rt, String), Rh>,
    file_bases: &[String],
  ) -> (Self, Option<Response>) {
    if raw.trim().is_empty() {
      return (
        Self::default(),
        Some(Response {
          status: StatusCode::BadRequest.to_string(),
          content_type: String::new(),
          content: Vec::new(),
        }),
      );
    }
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() < 3 {
      return (
        Self::default(),
        Some(Response {
          status: StatusCode::BadRequest.to_string(),
          content_type: String::new(),
          content: Vec::new(),
        }),
      );
    }
    let method_str = parts[0];
    let path_str = parts[1];
    let version = parts[2];
    let allowed = ["GET", "POST", "PUT", "DELETE", "OPTIONS", "HEAD", "PATCH", "CONNECT", "TRACE"];
    if !allowed.contains(&method_str) {
      return (
        Self::default(),
        Some(Response {
          status: StatusCode::MethodNotAllowed.to_string(),
          content_type: String::new(),
          content: Vec::new(),
        }),
      );
    }
    if version != "HTTP/1.1" {
      return (
        Self::default(),
        Some(Response {
          status: StatusCode::HttpVersionNotSupported.to_string(),
          content_type: String::new(),
          content: Vec::new(),
        }),
      );
    }
    const MAX_URI: usize = 2000;
    if path_str.len() > MAX_URI {
      return (
        Self::default(),
        Some(Response {
          status: StatusCode::UriTooLong.to_string(),
          content_type: String::new(),
          content: Vec::new(),
        }),
      );
    }
    let mut req = Self::parse_raw_only(raw, routes);
    let early = req.route_sync(routes, file_bases);
    (req, early)
  }

  #[cfg(any(feature = "async_tokio", feature = "async_std", feature = "async_smol"))]
  pub async fn parse_raw_async(
    raw: String,
    routes: &HashMap<(Rt, String), Rh>,
    file_bases: &[String],
  ) -> (Self, Option<Response>) {
    if raw.trim().is_empty() {
      return (
        Self::default(),
        Some(Response {
          status: StatusCode::BadRequest.to_string(),
          content_type: String::new(),
          content: Vec::new(),
        }),
      );
    }
    let parts: Vec<&str> = raw.split_whitespace().collect();
    if parts.len() < 3 {
      return (
        Self::default(),
        Some(Response {
          status: StatusCode::BadRequest.to_string(),
          content_type: String::new(),
          content: Vec::new(),
        }),
      );
    }
    let method_str = parts[0];
    let path_str = parts[1];
    let version = parts[2];
    let allowed = ["GET", "POST", "PUT", "DELETE", "OPTIONS", "HEAD", "PATCH", "CONNECT", "TRACE"];
    if !allowed.contains(&method_str) {
      return (
        Self::default(),
        Some(Response {
          status: StatusCode::MethodNotAllowed.to_string(),
          content_type: String::new(),
          content: Vec::new(),
        }),
      );
    }
    if version != "HTTP/1.1" {
      return (
        Self::default(),
        Some(Response {
          status: StatusCode::HttpVersionNotSupported.to_string(),
          content_type: String::new(),
          content: Vec::new(),
        }),
      );
    }
    const MAX_URI: usize = 2000;
    if path_str.len() > MAX_URI {
      return (
        Self::default(),
        Some(Response {
          status: StatusCode::UriTooLong.to_string(),
          content_type: String::new(),
          content: Vec::new(),
        }),
      );
    }
    let mut req = Self::parse_raw_only(raw, routes);
    // route is async under these features, await it here
    let early = req.route_async(routes, file_bases).await;
    (req, early)
  }

  fn parse_raw_only(raw: String, routes: &HashMap<(Rt, String), Rh>) -> Self {
    let lines: Vec<&str> = raw.split("\r\n").collect();
    let mut cut = 0;
    for (i, &l) in lines.iter().enumerate() {
      if l.trim().is_empty() {
        cut = i;
        break;
      }
    }
    let headers = lines[..cut]
      .iter()
      .filter_map(|&h| {
        let p: Vec<&str> = h.split(": ").collect();
        (p.len() == 2).then(|| (p[0].to_string(), p[1].to_string()))
      })
      .collect();
    let body = lines[cut + 1..].join("\r\n");
    let parts: Vec<&str> = raw.split_whitespace().collect();
    let mut path = parts[1].to_string();
    let mut params = HashMap::new();
    let query_opt = if let Some(qpos) = path.find('?') {
      let qs = path[qpos + 1..].to_string();
      path.truncate(qpos);
      Some(qs)
    } else {
      None
    };
    for (m, rp) in routes.keys() {
      if *m == RequestType::from_str(parts[0]) {
        for (k, v) in Self::extract_params(rp, &path) {
          params.insert(k, v);
        }
        break;
      }
    }
    if let Some(qs) = query_opt {
      for p in qs.split('&') {
        if let Some(eq) = p.find('=') {
          params.insert(p[..eq].to_string(), p[eq + 1..].to_string());
        }
      }
    }
    Request {
      method: RequestType::from_str(parts[0]),
      path,
      version: parts[2].to_string(),
      headers,
      body,
      params,
    }
  }

  #[cfg(feature = "sync")]
  pub fn route_sync(&mut self, routes: &HashMap<(Rt, String), Rh>, file_bases: &[String]) -> Option<Response> {
    if let Some(rh) = routes.get(&(self.method.clone(), self.path.clone())) {
      return Some(futures::executor::block_on(rh.handler.handle(self)));
    }
    for ((m, rp), rh) in routes {
      if *m == self.method {
        let path_p = Self::extract_params(rp, &self.path);
        if !path_p.is_empty() {
          let mut merged = HashMap::new();
          for (k, v) in path_p {
            merged.insert(k, v);
          }
          for (k, v) in self.params.drain() {
            merged.insert(k, v);
          }
          self.params = merged;
          return Some(futures::executor::block_on(rh.handler.handle(self)));
        }
      }
    }
    if self.method == Rt::GET {
      return Some(self.serve_file(file_bases));
    }
    None
  }

  #[cfg(any(feature = "async_tokio", feature = "async_std", feature = "async_smol"))]
  pub async fn route_async(&mut self, routes: &HashMap<(Rt, String), Rh>, file_bases: &[String]) -> Option<Response> {
    if let Some(rh) = routes.get(&(self.method.clone(), self.path.clone())) {
      return Some(rh.handler.handle(self).await);
    }
    for ((m, rp), rh) in routes {
      if *m == self.method {
        let path_p = Self::extract_params(rp, &self.path);
        if !path_p.is_empty() {
          let mut merged = HashMap::new();
          for (k, v) in path_p {
            merged.insert(k, v);
          }
          for (k, v) in self.params.drain() {
            merged.insert(k, v);
          }
          self.params = merged;
          return Some(rh.handler.handle(self).await);
        }
      }
    }
    if self.method == Rt::GET {
      return Some(self.serve_file(file_bases));
    }
    None
  }

  fn serve_file(&self, bases: &[String]) -> Response {
    for base in bases {
      let base_path = Path::new(base);
      if let Some(real_path) = crate::core::utils::secure_path(base_path, &self.path) {
        if let Ok(data) = std::fs::read(&real_path) {
          return Response {
            status: StatusCode::Ok.to_string(),
            content_type: crate::core::utils::get_content_type_quick(&real_path),
            content: data,
          };
        }
      }
    }
    Response::new()
  }
}

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
impl Default for Request {
  fn default() -> Self {
    Request {
      method: RequestType::GET,
      path: String::new(),
      version: String::new(),
      headers: vec![],
      body: String::new(),
      params: HashMap::new(),
    }
  }
}

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
use std::fmt::{Display, Formatter};

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
impl Display for Request {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let mut keys: Vec<&String> = self.params.keys().collect();
    keys.sort();
    let params_str = {
      let parts: Vec<String> = keys
        .into_iter()
        .map(|k| format!("\"{}\": \"{}\"", k, self.params[k]))
        .collect();
      format!("{{{}}}", parts.join(", "))
    };
    write!(
      f,
      "Method: {}\n\
       Path: {}\n\
       Version: {}\n\
       Headers: {:#?},\n\
       Body: {}\n\
       Params: {}",
      self.method, self.path, self.version, self.headers, self.body, params_str
    )
  }
}

#[cfg(feature = "sync")]
pub fn handle_request_sync(
  req: &mut Request,
  routes: &HashMap<(Rt, String), Rh>,
  file_bases: &[String],
) -> Option<Response> {
  req.route_sync(routes, file_bases)
}

#[cfg(any(feature = "async_tokio", feature = "async_std", feature = "async_smol"))]
pub async fn handle_request_async(
  req: &mut Request,
  routes: &HashMap<(Rt, String), Rh>,
  file_bases: &[String],
) -> Option<Response> {
  req.route_async(routes, file_bases).await
}
