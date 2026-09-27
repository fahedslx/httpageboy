/// Default maximum request body size: 8 MiB.
pub const DEFAULT_BODY_LIMIT_BYTES: usize = 8 * 1024 * 1024;
/// Default maximum request head size: 64 KiB.
pub const DEFAULT_HEADER_LIMIT_BYTES: usize = 64 * 1024;
/// Default maximum idle time while bytes of one request are being received.
pub const DEFAULT_IDLE_TIMEOUT_MS: u64 = 500;
/// Default maximum total time to receive one complete request.
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
/// Default idle time between requests on one persistent HTTP connection.
pub const DEFAULT_KEEP_ALIVE_TIMEOUT_SECS: u64 = 3;
/// Default maximum number of HTTP requests handled on one connection.
pub const DEFAULT_MAX_REQUESTS_PER_CONNECTION: usize = 20;

#[derive(Clone, Copy, Debug)]
pub struct RequestLimits {
  pub body_bytes: usize,
  pub header_bytes: usize,
  pub idle_timeout: std::time::Duration,
  pub request_timeout: std::time::Duration,
  pub keep_alive_timeout: std::time::Duration,
  pub max_requests: usize,
}

impl Default for RequestLimits {
  fn default() -> Self {
    Self {
      body_bytes: DEFAULT_BODY_LIMIT_BYTES,
      header_bytes: DEFAULT_HEADER_LIMIT_BYTES,
      idle_timeout: std::time::Duration::from_millis(DEFAULT_IDLE_TIMEOUT_MS),
      request_timeout: std::time::Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECS),
      keep_alive_timeout: std::time::Duration::from_secs(DEFAULT_KEEP_ALIVE_TIMEOUT_SECS),
      max_requests: DEFAULT_MAX_REQUESTS_PER_CONNECTION,
    }
  }
}

pub(crate) enum StreamRead {
  Ready(
    crate::core::request::Request,
    Option<crate::core::response::Response>,
  ),
  Idle,
  Closed,
  Error(crate::core::response::Response),
}

fn error_response(
  status: crate::core::status_code::StatusCode,
) -> crate::core::response::Response {
  crate::core::response::Response {
    status,
    headers: vec![],
    body: Vec::new().into(),
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BodyFraming {
  None,
  Length(usize),
  Chunked,
}

fn extract_body_headers(
  raw: &str,
) -> Result<BodyFraming, crate::core::status_code::StatusCode> {
  use crate::core::status_code::StatusCode;

  let mut content_length: Option<usize> = None;
  let mut transfer_codings: Vec<String> = Vec::new();

  for line in raw.split("\r\n").skip(1) {
    if line.is_empty() {
      break;
    }
    let Some((name, value)) = line.split_once(':') else {
      return Err(StatusCode::BadRequest);
    };

    if name.eq_ignore_ascii_case("content-length") {
      if content_length.is_some() {
        return Err(StatusCode::BadRequest);
      }
      let length = value.trim().parse::<usize>().map_err(|_| StatusCode::BadRequest)?;
      content_length = Some(length);
    } else if name.eq_ignore_ascii_case("transfer-encoding") {
      for coding in value.split(',') {
        let coding = coding.trim();
        if coding.is_empty() {
          return Err(StatusCode::BadRequest);
        }
        transfer_codings.push(coding.to_ascii_lowercase());
      }
    }
  }

  if content_length.is_some() && !transfer_codings.is_empty() {
    return Err(StatusCode::BadRequest);
  }

  if !transfer_codings.is_empty() {
    if transfer_codings.len() != 1 || transfer_codings[0] != "chunked" {
      return Err(StatusCode::NotImplemented);
    }
    return Ok(BodyFraming::Chunked);
  }

  Ok(match content_length {
    Some(length) => BodyFraming::Length(length),
    None => BodyFraming::None,
  })
}

fn find_crlf(bytes: &[u8]) -> Option<usize> {
  bytes.windows(2).position(|window| window == b"\r\n")
}

fn take_framed_request(
  buffer: &mut Vec<u8>,
  limits: &RequestLimits,
) -> Result<Option<Vec<u8>>, crate::core::status_code::StatusCode> {
  use crate::core::status_code::StatusCode;

  let Some(separator) = buffer.windows(4).position(|window| window == b"\r\n\r\n") else {
    if buffer.len() > limits.header_bytes {
      return Err(StatusCode::RequestHeaderFieldsTooLarge);
    }
    return Ok(None);
  };

  let head_end = separator + 4;
  if head_end > limits.header_bytes {
    return Err(StatusCode::RequestHeaderFieldsTooLarge);
  }

  let head = std::str::from_utf8(&buffer[..separator]).map_err(|_| StatusCode::BadRequest)?;
  let framing = extract_body_headers(head)?;

  match framing {
    BodyFraming::None => Ok(Some(buffer.drain(..head_end).collect())),
    BodyFraming::Length(length) => {
      if length > limits.body_bytes {
        return Err(StatusCode::PayloadTooLarge);
      }
      let end = head_end.checked_add(length).ok_or(StatusCode::PayloadTooLarge)?;
      if buffer.len() < end {
        return Ok(None);
      }
      Ok(Some(buffer.drain(..end).collect()))
    }
    BodyFraming::Chunked => {
      let mut cursor = head_end;
      let mut decoded = Vec::new();
      let mut metadata_bytes = 0usize;

      loop {
        let Some(line_len) = find_crlf(&buffer[cursor..]) else {
          if buffer.len().saturating_sub(cursor) > limits.header_bytes {
            return Err(StatusCode::RequestHeaderFieldsTooLarge);
          }
          return Ok(None);
        };

        metadata_bytes = metadata_bytes.saturating_add(line_len + 2);
        if metadata_bytes > limits.header_bytes {
          return Err(StatusCode::RequestHeaderFieldsTooLarge);
        }

        let line_end = cursor + line_len;
        let line = std::str::from_utf8(&buffer[cursor..line_end])
          .map_err(|_| StatusCode::BadRequest)?;
        let size_text = line.split(';').next().unwrap_or("").trim();
        if size_text.is_empty() {
          return Err(StatusCode::BadRequest);
        }
        let size_u64 = u64::from_str_radix(size_text, 16).map_err(|_| StatusCode::BadRequest)?;
        let size = usize::try_from(size_u64).map_err(|_| StatusCode::PayloadTooLarge)?;
        cursor = line_end + 2;

        if size == 0 {
          if buffer.len() < cursor + 2 {
            return Ok(None);
          }

          let consumed = if &buffer[cursor..cursor + 2] == b"\r\n" {
            cursor + 2
          } else {
            let Some(trailer_len) = buffer[cursor..]
              .windows(4)
              .position(|window| window == b"\r\n\r\n")
            else {
              if buffer.len().saturating_sub(cursor) > limits.header_bytes {
                return Err(StatusCode::RequestHeaderFieldsTooLarge);
              }
              return Ok(None);
            };

            if trailer_len + 4 > limits.header_bytes {
              return Err(StatusCode::RequestHeaderFieldsTooLarge);
            }

            let trailer = std::str::from_utf8(&buffer[cursor..cursor + trailer_len])
              .map_err(|_| StatusCode::BadRequest)?;
            if trailer
              .split("\r\n")
              .any(|line| !line.is_empty() && !line.contains(':'))
            {
              return Err(StatusCode::BadRequest);
            }
            cursor + trailer_len + 4
          };

          let mut raw = buffer[..head_end].to_vec();
          raw.extend_from_slice(&decoded);
          buffer.drain(..consumed);
          return Ok(Some(raw));
        }

        if decoded.len().saturating_add(size) > limits.body_bytes {
          return Err(StatusCode::PayloadTooLarge);
        }

        let data_end = cursor.checked_add(size).ok_or(StatusCode::PayloadTooLarge)?;
        let chunk_end = data_end.checked_add(2).ok_or(StatusCode::PayloadTooLarge)?;
        if buffer.len() < chunk_end {
          return Ok(None);
        }
        if &buffer[data_end..chunk_end] != b"\r\n" {
          return Err(StatusCode::BadRequest);
        }

        decoded.extend_from_slice(&buffer[cursor..data_end]);
        cursor = chunk_end;
      }
    }
  }
}

/// Generates a persistent request reader for a specific async runtime.
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
    pub(crate) async fn $func_name(
      stream: &mut $stream_ty,
      buffer: &mut Vec<u8>,
      routes: &std::collections::HashMap<(crate::core::request_type::Rt, String), crate::core::route::RouteEntry>,
      file_bases: &[String],
      limits: &crate::core::request::RequestLimits,
      keep_alive: bool,
    ) -> crate::core::request::StreamRead {
      use $async_read_ext;

      let mut started = if buffer.is_empty() {
        None
      } else {
        Some(std::time::Instant::now())
      };

      loop {
        match crate::core::request::take_framed_request(buffer, limits) {
          Ok(Some(raw)) => {
            let (request, early) =
              crate::core::request::Request::parse_raw_async(raw, routes, file_bases).await;
            return crate::core::request::StreamRead::Ready(request, early);
          }
          Ok(None) => {}
          Err(status) => {
            return crate::core::request::StreamRead::Error(
              crate::core::request::error_response(status),
            );
          }
        }

        let wait = if let Some(started_at) = started {
          let Some(total_left) = limits.request_timeout.checked_sub(started_at.elapsed()) else {
            return crate::core::request::StreamRead::Error(
              crate::core::request::error_response(
                crate::core::status_code::StatusCode::RequestTimeout,
              ),
            );
          };
          std::cmp::min(limits.idle_timeout, total_left)
        } else if keep_alive {
          limits.keep_alive_timeout
        } else {
          limits.idle_timeout
        };

        if wait.is_zero() {
          return if started.is_some() {
            crate::core::request::StreamRead::Error(
              crate::core::request::error_response(
                crate::core::status_code::StatusCode::RequestTimeout,
              ),
            )
          } else {
            crate::core::request::StreamRead::Idle
          };
        }

        let mut chunk = [0u8; 4096];

        #[cfg(feature = "async_tokio")]
        let read = match tokio::time::timeout(wait, stream.read(&mut chunk)).await {
          Ok(result) => result,
          Err(_) => {
            return if started.is_some() {
              crate::core::request::StreamRead::Error(
                crate::core::request::error_response(
                  crate::core::status_code::StatusCode::RequestTimeout,
                ),
              )
            } else {
              crate::core::request::StreamRead::Idle
            };
          }
        };

        #[cfg(all(feature = "async_std", not(feature = "async_tokio")))]
        let read = match async_std::future::timeout(wait, stream.read(&mut chunk)).await {
          Ok(result) => result,
          Err(_) => {
            return if started.is_some() {
              crate::core::request::StreamRead::Error(
                crate::core::request::error_response(
                  crate::core::status_code::StatusCode::RequestTimeout,
                ),
              )
            } else {
              crate::core::request::StreamRead::Idle
            };
          }
        };

        #[cfg(all(feature = "async_smol", not(any(feature = "async_tokio", feature = "async_std"))))]
        let read = {
          use futures_lite::future;
          future::race(
            async { stream.read(&mut chunk).await.map(Some) },
            async {
              smol::Timer::after(wait).await;
              Ok(None)
            },
          )
          .await
          .and_then(|result| match result {
            Some(read) => Ok(read),
            None => Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "request read timed out")),
          })
        };

        match read {
          Ok(0) => {
            return if buffer.is_empty() {
              crate::core::request::StreamRead::Closed
            } else {
              crate::core::request::StreamRead::Error(
                crate::core::request::error_response(
                  crate::core::status_code::StatusCode::BadRequest,
                ),
              )
            };
          }
          Ok(n) => {
            if started.is_none() {
              started = Some(std::time::Instant::now());
            }
            buffer.extend_from_slice(&chunk[..n]);
          }
          Err(error)
            if error.kind() == std::io::ErrorKind::TimedOut
              || error.kind() == std::io::ErrorKind::WouldBlock =>
          {
            return if started.is_some() {
              crate::core::request::StreamRead::Error(
                crate::core::request::error_response(
                  crate::core::status_code::StatusCode::RequestTimeout,
                ),
              )
            } else {
              crate::core::request::StreamRead::Idle
            };
          }
          Err(_) => {
            return crate::core::request::StreamRead::Error(
              crate::core::request::error_response(
                crate::core::status_code::StatusCode::BadRequest,
              ),
            );
          }
        }
      }
    }
  };
}

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
use crate::core::route::RouteEntry;
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
  pub body: Vec<u8>,
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

  pub fn body_text(&self) -> Result<&str, std::str::Utf8Error> {
    std::str::from_utf8(&self.body)
  }

  pub fn origin(&self) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|(k, _)| k.eq_ignore_ascii_case("origin"))
      .map(|(_, v)| v.as_str())
  }

  pub(crate) fn wants_close(&self) -> bool {
    self
      .headers
      .iter()
      .filter(|(key, _)| key.eq_ignore_ascii_case("connection"))
      .flat_map(|(_, value)| value.split(','))
      .any(|token| token.trim().eq_ignore_ascii_case("close"))
  }

  pub(crate) fn upgrade_handler(
    &self,
    routes: &HashMap<(Rt, String), RouteEntry>,
  ) -> Option<std::sync::Arc<dyn crate::core::upgrade::UpgradeHandler>> {
    if let Some(entry) = routes.get(&(self.method.clone(), self.path.clone())) {
      return entry.upgrade.clone();
    }

    for ((method, route_path), entry) in routes {
      if *method == self.method && !Self::extract_params(route_path, &self.path).is_empty() {
        return entry.upgrade.clone();
      }
    }

    None
  }

  #[cfg(feature = "sync")]
  pub(crate) fn parse_stream_sync(
    stream: &mut TcpStream,
    buffer: &mut Vec<u8>,
    routes: &HashMap<(Rt, String), RouteEntry>,
    file_bases: &[String],
    limits: &RequestLimits,
    keep_alive: bool,
  ) -> StreamRead {
    use std::io::Read;
    use std::time::Instant;

    let mut started = if buffer.is_empty() {
      None
    } else {
      Some(Instant::now())
    };

    loop {
      match take_framed_request(buffer, limits) {
        Ok(Some(raw)) => {
          let (request, early) = Self::parse_raw_sync(raw, routes, file_bases);
          let _ = stream.set_read_timeout(None);
          return StreamRead::Ready(request, early);
        }
        Ok(None) => {}
        Err(status) => {
          let _ = stream.set_read_timeout(None);
          return StreamRead::Error(error_response(status));
        }
      }

      let wait = if let Some(started_at) = started {
        let Some(total_left) = limits.request_timeout.checked_sub(started_at.elapsed()) else {
          let _ = stream.set_read_timeout(None);
          return StreamRead::Error(error_response(StatusCode::RequestTimeout));
        };
        std::cmp::min(limits.idle_timeout, total_left)
      } else if keep_alive {
        limits.keep_alive_timeout
      } else {
        limits.idle_timeout
      };

      if wait.is_zero() {
        let _ = stream.set_read_timeout(None);
        return if started.is_some() {
          StreamRead::Error(error_response(StatusCode::RequestTimeout))
        } else {
          StreamRead::Idle
        };
      }

      let _ = stream.set_read_timeout(Some(wait));
      let mut chunk = [0u8; 4096];

      match stream.read(&mut chunk) {
        Ok(0) => {
          let _ = stream.set_read_timeout(None);
          return if buffer.is_empty() {
            StreamRead::Closed
          } else {
            StreamRead::Error(error_response(StatusCode::BadRequest))
          };
        }
        Ok(n) => {
          if started.is_none() {
            started = Some(Instant::now());
          }
          buffer.extend_from_slice(&chunk[..n]);
        }
        Err(error)
          if error.kind() == std::io::ErrorKind::TimedOut
            || error.kind() == std::io::ErrorKind::WouldBlock =>
        {
          let _ = stream.set_read_timeout(None);
          return if started.is_some() {
            StreamRead::Error(error_response(StatusCode::RequestTimeout))
          } else {
            StreamRead::Idle
          };
        }
        Err(_) => {
          let _ = stream.set_read_timeout(None);
          return StreamRead::Error(error_response(StatusCode::BadRequest));
        }
      }
    }
  }

  #[cfg(feature = "sync")]
  fn parse_raw_sync(
    raw: Vec<u8>,
    routes: &HashMap<(Rt, String), RouteEntry>,
    file_bases: &[String],
  ) -> (Self, Option<Response>) {
    match Self::parse_raw_only(raw, routes) {
      Ok(mut request) => {
        let early = request.route_sync(routes, file_bases);
        (request, early)
      }
      Err(response) => (Self::default(), Some(response)),
    }
  }

  #[cfg(any(feature = "async_tokio", feature = "async_std", feature = "async_smol"))]
  async fn parse_raw_async(
    raw: Vec<u8>,
    routes: &HashMap<(Rt, String), RouteEntry>,
    file_bases: &[String],
  ) -> (Self, Option<Response>) {
    match Self::parse_raw_only(raw, routes) {
      Ok(mut request) => {
        let early = request.route_async(routes, file_bases).await;
        (request, early)
      }
      Err(response) => (Self::default(), Some(response)),
    }
  }

  fn parse_raw_only(
    raw: Vec<u8>,
    routes: &HashMap<(Rt, String), RouteEntry>,
  ) -> Result<Self, Response> {
    let separator = raw
      .windows(4)
      .position(|window| window == b"\r\n\r\n")
      .ok_or_else(|| Response {
        status: StatusCode::BadRequest,
        headers: vec![],
        body: Vec::new().into(),
      })?;

    let head = std::str::from_utf8(&raw[..separator]).map_err(|_| Response {
      status: StatusCode::BadRequest,
      headers: vec![],
      body: Vec::new().into(),
    })?;
    let body = raw[separator + 4..].to_vec();

    let mut lines = head.split("\r\n");
    let request_line = lines.next().ok_or_else(|| Response {
      status: StatusCode::BadRequest,
      headers: vec![],
      body: Vec::new().into(),
    })?;
    let parts: Vec<&str> = request_line.split_whitespace().collect();

    if parts.len() != 3 {
      return Err(Response {
        status: StatusCode::BadRequest,
        headers: vec![],
        body: Vec::new().into(),
      });
    }

    let method_str = parts[0];
    let path_str = parts[1];
    let version = parts[2];
    let allowed = [
      "GET", "POST", "PUT", "DELETE", "OPTIONS", "HEAD", "PATCH", "CONNECT", "TRACE", "QUERY",
    ];

    if !allowed.contains(&method_str) {
      return Err(Response {
        status: StatusCode::MethodNotAllowed,
        headers: vec![],
        body: Vec::new().into(),
      });
    }
    if version != "HTTP/1.1" {
      return Err(Response {
        status: StatusCode::HttpVersionNotSupported,
        headers: vec![],
        body: Vec::new().into(),
      });
    }

    const MAX_URI: usize = 2000;
    if path_str.len() > MAX_URI {
      return Err(Response {
        status: StatusCode::UriTooLong,
        headers: vec![],
        body: Vec::new().into(),
      });
    }

    let headers = lines
      .filter_map(|line| {
        let (name, value) = line.split_once(':')?;
        Some((name.to_string(), value.trim_start().to_string()))
      })
      .collect();

    let mut path = path_str.to_string();
    let mut params = HashMap::new();
    let query = if let Some(position) = path.find('?') {
      let query = path[position + 1..].to_string();
      path.truncate(position);
      Some(query)
    } else {
      None
    };

    for (method, route_path) in routes.keys() {
      if *method == RequestType::from_str(method_str) {
        let path_params = Self::extract_params(route_path, &path);
        if !path_params.is_empty() {
          params.extend(path_params);
          break;
        }
      }
    }

    if let Some(query) = query {
      for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
          params.insert(key.to_string(), value.to_string());
        }
      }
    }

    Ok(Request {
      method: RequestType::from_str(method_str),
      path,
      version: version.to_string(),
      headers,
      body,
      params,
    })
  }

  #[cfg(feature = "sync")]
  pub fn route_sync(&mut self, routes: &HashMap<(Rt, String), RouteEntry>, file_bases: &[String]) -> Option<Response> {
    if let Some(entry) = routes.get(&(self.method.clone(), self.path.clone())) {
      return Some(futures::executor::block_on(entry.handler.handle(self)));
    }
    for ((m, rp), entry) in routes {
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
          return Some(futures::executor::block_on(entry.handler.handle(self)));
        }
      }
    }
    if self.method == Rt::GET {
      return Some(self.serve_file(file_bases));
    }
    None
  }

  #[cfg(any(feature = "async_tokio", feature = "async_std", feature = "async_smol"))]
  pub async fn route_async(&mut self, routes: &HashMap<(Rt, String), RouteEntry>, file_bases: &[String]) -> Option<Response> {
    if let Some(entry) = routes.get(&(self.method.clone(), self.path.clone())) {
      return Some(entry.handler.handle(self).await);
    }
    for ((m, rp), entry) in routes {
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
          return Some(entry.handler.handle(self).await);
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
            status: StatusCode::Ok,
            headers: vec![(
              "Content-Type".to_string(),
              crate::core::utils::get_content_type_quick(&real_path),
            )],
            body: data.into(),
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
      body: Vec::new(),
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
    let body = String::from_utf8_lossy(&self.body);
    write!(
      f,
      "Method: {}\n\
       Path: {}\n\
       Version: {}\n\
       Headers: {:#?},\n\
       Body: {}\n\
       Params: {}",
      self.method, self.path, self.version, self.headers, body, params_str
    )
  }
}

#[cfg(feature = "sync")]
pub fn handle_request_sync(
  req: &mut Request,
  routes: &HashMap<(Rt, String), RouteEntry>,
  file_bases: &[String],
) -> Option<Response> {
  req.route_sync(routes, file_bases)
}

#[cfg(any(feature = "async_tokio", feature = "async_std", feature = "async_smol"))]
pub async fn handle_request_async(
  req: &mut Request,
  routes: &HashMap<(Rt, String), RouteEntry>,
  file_bases: &[String],
) -> Option<Response> {
  req.route_async(routes, file_bases).await
}

#[cfg(test)]
mod request_tests {
  use super::*;

  #[test]
  fn preserves_binary_request_body() {
    let mut raw = b"POST /binary HTTP/1.1\r\nContent-Length: 4\r\n\r\n".to_vec();
    raw.extend_from_slice(&[0x00, 0xff, 0x01, 0x02]);

    let request = Request::parse_raw_only(raw, &HashMap::new()).expect("valid request");

    assert_eq!(request.body, vec![0x00, 0xff, 0x01, 0x02]);
    assert!(request.body_text().is_err());
  }

  #[test]
  fn rejects_ambiguous_body_framing() {
    let duplicate = "POST / HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\n";
    assert_eq!(extract_body_headers(duplicate), Err(StatusCode::BadRequest));

    let mixed = "POST / HTTP/1.1\r\nContent-Length: 1\r\nTransfer-Encoding: chunked\r\n\r\n";
    assert_eq!(extract_body_headers(mixed), Err(StatusCode::BadRequest));
  }

  #[test]
  fn decodes_chunked_body_and_preserves_pipelined_request() {
    let mut buffer = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\nGET /next HTTP/1.1\r\n\r\n".to_vec();
    let limits = RequestLimits::default();

    let raw = take_framed_request(&mut buffer, &limits)
      .expect("valid framing")
      .expect("complete request");
    let request = Request::parse_raw_only(raw, &HashMap::new()).expect("valid request");

    assert_eq!(request.body, b"Wikipedia");
    assert_eq!(buffer, b"GET /next HTTP/1.1\r\n\r\n");
  }

  #[test]
  fn supports_chunk_extensions_and_trailers() {
    let mut buffer = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n3;foo=bar\r\nabc\r\n0\r\nX-Test: yes\r\n\r\n".to_vec();
    let limits = RequestLimits::default();

    let raw = take_framed_request(&mut buffer, &limits)
      .expect("valid framing")
      .expect("complete request");
    let request = Request::parse_raw_only(raw, &HashMap::new()).expect("valid request");

    assert_eq!(request.body, b"abc");
    assert!(buffer.is_empty());
  }

  #[test]
  fn rejects_unsupported_transfer_coding() {
    let raw = "POST / HTTP/1.1\r\nTransfer-Encoding: gzip, chunked\r\n\r\n";
    assert_eq!(extract_body_headers(raw), Err(StatusCode::NotImplemented));
  }

  #[test]
  fn enforces_chunked_body_limit() {
    let mut limits = RequestLimits::default();
    limits.body_bytes = 3;
    let mut buffer = b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n4\r\ntest\r\n0\r\n\r\n".to_vec();

    assert_eq!(
      take_framed_request(&mut buffer, &limits),
      Err(StatusCode::PayloadTooLarge)
    );
  }
}
