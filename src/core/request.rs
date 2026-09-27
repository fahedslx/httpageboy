/// Default maximum request body size: 8 MiB.
pub const DEFAULT_BODY_LIMIT_BYTES: usize = 8 * 1024 * 1024;
/// Default maximum request head size: 64 KiB.
pub const DEFAULT_HEADER_LIMIT_BYTES: usize = 64 * 1024;
/// Default total read timeout used while receiving a request.
pub const DEFAULT_READ_TIMEOUT_SECS: u64 = 5;

#[derive(Clone, Copy, Debug)]
pub struct RequestLimits {
  pub body_bytes: usize,
  pub header_bytes: usize,
  pub read_timeout: std::time::Duration,
}

impl Default for RequestLimits {
  fn default() -> Self {
    Self {
      body_bytes: DEFAULT_BODY_LIMIT_BYTES,
      header_bytes: DEFAULT_HEADER_LIMIT_BYTES,
      read_timeout: std::time::Duration::from_secs(DEFAULT_READ_TIMEOUT_SECS),
    }
  }
}

fn request_error(
  status: crate::core::status_code::StatusCode,
) -> (
  crate::core::request::Request,
  Option<crate::core::response::Response>,
) {
  (
    crate::core::request::Request::default(),
    Some(crate::core::response::Response {
      status: status.to_string(),
      headers: vec![],
      body: Vec::new(),
    }),
  )
}

/// Generates a `parse_stream` function for a specific async runtime.
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
      routes: &std::collections::HashMap<(crate::core::request_type::Rt, String), crate::core::route::RouteEntry>,
      file_bases: &[String],
      limits: &crate::core::request::RequestLimits,
    ) -> (crate::core::request::Request, Option<crate::core::response::Response>) {
      use $async_read_ext;
      use $async_buf_read_ext;

      let mut reader = <$buf_reader>::new(stream);
      let mut raw: Vec<u8> = Vec::new();
      let header_started = std::time::Instant::now();

      loop {
        let remaining = match limits.read_timeout.checked_sub(header_started.elapsed()) {
          Some(value) if !value.is_zero() => value,
          _ => return crate::core::request::request_error(crate::core::status_code::StatusCode::RequestTimeout),
        };
        let mut line = String::new();

        #[cfg(feature = "async_tokio")]
        let n = {
          let read_fut = reader.read_line(&mut line);
          let sleep = tokio::time::sleep(remaining);
          futures::pin_mut!(read_fut, sleep);
          match futures::future::select(read_fut, sleep).await {
            futures::future::Either::Left((Ok(n), _)) => n,
            futures::future::Either::Left((Err(_), _)) => {
              return crate::core::request::request_error(crate::core::status_code::StatusCode::BadRequest);
            }
            futures::future::Either::Right(_) => {
              return crate::core::request::request_error(crate::core::status_code::StatusCode::RequestTimeout);
            }
          }
        };

        #[cfg(all(feature = "async_std", not(feature = "async_tokio")))]
        let n = {
          let read_fut = reader.read_line(&mut line);
          let sleep = async_std::task::sleep(remaining);
          futures::pin_mut!(read_fut, sleep);
          match futures::future::select(read_fut, sleep).await {
            futures::future::Either::Left((Ok(n), _)) => n,
            futures::future::Either::Left((Err(_), _)) => {
              return crate::core::request::request_error(crate::core::status_code::StatusCode::BadRequest);
            }
            futures::future::Either::Right(_) => {
              return crate::core::request::request_error(crate::core::status_code::StatusCode::RequestTimeout);
            }
          }
        };

        #[cfg(all(feature = "async_smol", not(any(feature = "async_tokio", feature = "async_std"))))]
        let n = {
          let read_fut = reader.read_line(&mut line);
          let sleep = smol::Timer::after(remaining);
          futures::pin_mut!(read_fut, sleep);
          match futures::future::select(read_fut, sleep).await {
            futures::future::Either::Left((Ok(n), _)) => n,
            futures::future::Either::Left((Err(_), _)) => {
              return crate::core::request::request_error(crate::core::status_code::StatusCode::BadRequest);
            }
            futures::future::Either::Right(_) => {
              return crate::core::request::request_error(crate::core::status_code::StatusCode::RequestTimeout);
            }
          }
        };

        if n == 0 {
          break;
        }
        raw.extend_from_slice(line.as_bytes());
        if raw.len() > limits.header_bytes {
          return crate::core::request::request_error(
            crate::core::status_code::StatusCode::RequestHeaderFieldsTooLarge,
          );
        }
        if line == "\r\n" || line == "\n" {
          break;
        }
      }

      let (method, content_length, has_transfer_encoding) = {
        let head = String::from_utf8_lossy(&raw);
        let method = head
          .lines()
          .next()
          .and_then(|line| line.split_whitespace().next())
          .unwrap_or("")
          .to_string();
        let (content_length, has_transfer_encoding) =
          match crate::core::request::extract_body_headers(&head) {
            Ok(value) => value,
            Err(status) => return crate::core::request::request_error(status),
          };
        (method, content_length, has_transfer_encoding)
      };

      if content_length > limits.body_bytes {
        return crate::core::request::request_error(crate::core::status_code::StatusCode::PayloadTooLarge);
      }

      if content_length > 0 {
        let mut body = Vec::with_capacity(content_length);
        let body_started = std::time::Instant::now();

        #[cfg(feature = "async_tokio")]
        {
          let mut limited = reader.take(content_length as u64);
          let read_fut = limited.read_to_end(&mut body);
          let sleep = tokio::time::sleep(limits.read_timeout);
          futures::pin_mut!(read_fut, sleep);
          match futures::future::select(read_fut, sleep).await {
            futures::future::Either::Left((Ok(_), _)) => {}
            futures::future::Either::Left((Err(_), _)) => {
              return crate::core::request::request_error(crate::core::status_code::StatusCode::BadRequest);
            }
            futures::future::Either::Right(_) => {
              return crate::core::request::request_error(crate::core::status_code::StatusCode::RequestTimeout);
            }
          }
        }

        #[cfg(all(feature = "async_std", not(feature = "async_tokio")))]
        {
          let mut limited = reader.take(content_length as u64);
          let read_fut = limited.read_to_end(&mut body);
          let sleep = async_std::task::sleep(limits.read_timeout);
          futures::pin_mut!(read_fut, sleep);
          match futures::future::select(read_fut, sleep).await {
            futures::future::Either::Left((Ok(_), _)) => {}
            futures::future::Either::Left((Err(_), _)) => {
              return crate::core::request::request_error(crate::core::status_code::StatusCode::BadRequest);
            }
            futures::future::Either::Right(_) => {
              return crate::core::request::request_error(crate::core::status_code::StatusCode::RequestTimeout);
            }
          }
        }

        #[cfg(all(feature = "async_smol", not(any(feature = "async_tokio", feature = "async_std"))))]
        {
          let mut limited = reader.take(content_length as u64);
          let read_fut = limited.read_to_end(&mut body);
          let sleep = smol::Timer::after(limits.read_timeout);
          futures::pin_mut!(read_fut, sleep);
          match futures::future::select(read_fut, sleep).await {
            futures::future::Either::Left((Ok(_), _)) => {}
            futures::future::Either::Left((Err(_), _)) => {
              return crate::core::request::request_error(crate::core::status_code::StatusCode::BadRequest);
            }
            futures::future::Either::Right(_) => {
              return crate::core::request::request_error(crate::core::status_code::StatusCode::RequestTimeout);
            }
          }
        }

        let _ = body_started;
        if body.len() != content_length {
          return crate::core::request::request_error(crate::core::status_code::StatusCode::BadRequest);
        }
        raw.extend_from_slice(&body);
      } else if matches!(method.as_str(), "POST" | "PUT" | "DELETE" | "PATCH" | "QUERY") {
        let mut body: Vec<u8> = Vec::new();

        if has_transfer_encoding {
          let take_limit = limits.body_bytes.saturating_add(1) as u64;

          #[cfg(feature = "async_tokio")]
          {
            let mut limited = reader.take(take_limit);
            let read_fut = limited.read_to_end(&mut body);
            let sleep = tokio::time::sleep(limits.read_timeout);
            futures::pin_mut!(read_fut, sleep);
            let _ = futures::future::select(read_fut, sleep).await;
          }

          #[cfg(all(feature = "async_std", not(feature = "async_tokio")))]
          {
            let mut limited = reader.take(take_limit);
            let read_fut = limited.read_to_end(&mut body);
            let sleep = async_std::task::sleep(limits.read_timeout);
            futures::pin_mut!(read_fut, sleep);
            let _ = futures::future::select(read_fut, sleep).await;
          }

          #[cfg(all(feature = "async_smol", not(any(feature = "async_tokio", feature = "async_std"))))]
          {
            let mut limited = reader.take(take_limit);
            let read_fut = limited.read_to_end(&mut body);
            let sleep = smol::Timer::after(limits.read_timeout);
            futures::pin_mut!(read_fut, sleep);
            let _ = futures::future::select(read_fut, sleep).await;
          }
        } else {
          let body_started = std::time::Instant::now();
          let mut chunk = [0u8; 1024];

          loop {
            let remaining = match limits.read_timeout.checked_sub(body_started.elapsed()) {
              Some(value) if !value.is_zero() => value,
              _ => break,
            };

            #[cfg(feature = "async_tokio")]
            let read = {
              let read_fut = reader.read(&mut chunk);
              let sleep = tokio::time::sleep(remaining);
              futures::pin_mut!(read_fut, sleep);
              match futures::future::select(read_fut, sleep).await {
                futures::future::Either::Left((result, _)) => result,
                futures::future::Either::Right(_) => break,
              }
            };

            #[cfg(all(feature = "async_std", not(feature = "async_tokio")))]
            let read = {
              let read_fut = reader.read(&mut chunk);
              let sleep = async_std::task::sleep(remaining);
              futures::pin_mut!(read_fut, sleep);
              match futures::future::select(read_fut, sleep).await {
                futures::future::Either::Left((result, _)) => result,
                futures::future::Either::Right(_) => break,
              }
            };

            #[cfg(all(feature = "async_smol", not(any(feature = "async_tokio", feature = "async_std"))))]
            let read = {
              let read_fut = reader.read(&mut chunk);
              let sleep = smol::Timer::after(remaining);
              futures::pin_mut!(read_fut, sleep);
              match futures::future::select(read_fut, sleep).await {
                futures::future::Either::Left((result, _)) => result,
                futures::future::Either::Right(_) => break,
              }
            };

            match read {
              Ok(0) => break,
              Ok(n) => {
                body.extend_from_slice(&chunk[..n]);
                if body.len() > limits.body_bytes {
                  return crate::core::request::request_error(
                    crate::core::status_code::StatusCode::PayloadTooLarge,
                  );
                }
              }
              Err(_) => break,
            }
          }
        }

        if body.len() > limits.body_bytes {
          return crate::core::request::request_error(crate::core::status_code::StatusCode::PayloadTooLarge);
        }
        raw.extend_from_slice(&body);
      }

      crate::core::request::Request::parse_bytes_async(raw, routes, file_bases).await
    }
  };
}

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
fn extract_body_headers(raw: &str) -> Result<(usize, bool), StatusCode> {
  let mut content_length: Option<usize> = None;
  let mut has_transfer_encoding = false;

  for line in raw.lines().skip(1) {
    let Some((name, value)) = line.split_once(':') else {
      continue;
    };
    if name.eq_ignore_ascii_case("content-length") {
      if content_length.is_some() {
        return Err(StatusCode::BadRequest);
      }
      let length = value.trim().parse::<usize>().map_err(|_| StatusCode::BadRequest)?;
      content_length = Some(length);
    } else if name.eq_ignore_ascii_case("transfer-encoding") {
      has_transfer_encoding = true;
    }
  }

  if content_length.is_some() && has_transfer_encoding {
    return Err(StatusCode::BadRequest);
  }

  Ok((content_length.unwrap_or(0), has_transfer_encoding))
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
  pub fn parse_stream_sync(
    stream: &TcpStream,
    routes: &HashMap<(Rt, String), RouteEntry>,
    file_bases: &[String],
    limits: &RequestLimits,
  ) -> (Self, Option<Response>) {
    use std::io::{BufRead, BufReader, Read};
    use std::time::Instant;

    let mut reader = BufReader::new(stream);
    let mut raw: Vec<u8> = Vec::new();
    let header_started = Instant::now();

    loop {
      let remaining = match limits.read_timeout.checked_sub(header_started.elapsed()) {
        Some(value) if !value.is_zero() => value,
        _ => return request_error(StatusCode::RequestTimeout),
      };
      let _ = stream.set_read_timeout(Some(remaining));

      let mut line = String::new();
      match reader.read_line(&mut line) {
        Ok(0) => break,
        Ok(_) => {
          raw.extend_from_slice(line.as_bytes());
          if raw.len() > limits.header_bytes {
            let _ = stream.set_read_timeout(None);
            return request_error(StatusCode::RequestHeaderFieldsTooLarge);
          }
          if line == "\r\n" || line == "\n" {
            break;
          }
        }
        Err(err)
          if err.kind() == std::io::ErrorKind::WouldBlock
            || err.kind() == std::io::ErrorKind::TimedOut =>
        {
          let _ = stream.set_read_timeout(None);
          return request_error(StatusCode::RequestTimeout);
        }
        Err(_) => {
          let _ = stream.set_read_timeout(None);
          return request_error(StatusCode::BadRequest);
        }
      }
    }

    let (method, content_length, has_transfer_encoding) = {
      let head = String::from_utf8_lossy(&raw);
      let method = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or("")
        .to_string();
      let (content_length, has_transfer_encoding) = match extract_body_headers(&head) {
        Ok(value) => value,
        Err(status) => {
          let _ = stream.set_read_timeout(None);
          return request_error(status);
        }
      };
      (method, content_length, has_transfer_encoding)
    };

    if content_length > limits.body_bytes {
      let _ = stream.set_read_timeout(None);
      return request_error(StatusCode::PayloadTooLarge);
    }

    if content_length > 0 {
      let _ = stream.set_read_timeout(Some(limits.read_timeout));
      let mut body = Vec::with_capacity(content_length);
      let mut limited = reader.take(content_length as u64);
      match limited.read_to_end(&mut body) {
        Ok(_) if body.len() == content_length => raw.extend_from_slice(&body),
        Ok(_) => {
          let _ = stream.set_read_timeout(None);
          return request_error(StatusCode::BadRequest);
        }
        Err(err)
          if err.kind() == std::io::ErrorKind::WouldBlock
            || err.kind() == std::io::ErrorKind::TimedOut =>
        {
          let _ = stream.set_read_timeout(None);
          return request_error(StatusCode::RequestTimeout);
        }
        Err(_) => {
          let _ = stream.set_read_timeout(None);
          return request_error(StatusCode::BadRequest);
        }
      }
    } else if matches!(method.as_str(), "POST" | "PUT" | "DELETE" | "PATCH" | "QUERY") {
      let _ = stream.set_read_timeout(Some(limits.read_timeout));
      let mut body = Vec::new();

      if has_transfer_encoding {
        let mut limited = reader.take(limits.body_bytes.saturating_add(1) as u64);
        let _ = limited.read_to_end(&mut body);
      } else {
        let started = Instant::now();
        let mut chunk = [0u8; 1024];
        loop {
          let remaining = match limits.read_timeout.checked_sub(started.elapsed()) {
            Some(value) if !value.is_zero() => value,
            _ => break,
          };
          let _ = stream.set_read_timeout(Some(remaining));
          match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
              body.extend_from_slice(&chunk[..n]);
              if body.len() > limits.body_bytes {
                let _ = stream.set_read_timeout(None);
                return request_error(StatusCode::PayloadTooLarge);
              }
            }
            Err(err)
              if err.kind() == std::io::ErrorKind::WouldBlock
                || err.kind() == std::io::ErrorKind::TimedOut =>
            {
              break;
            }
            Err(_) => break,
          }
        }
      }

      if body.len() > limits.body_bytes {
        let _ = stream.set_read_timeout(None);
        return request_error(StatusCode::PayloadTooLarge);
      }
      raw.extend_from_slice(&body);
    }

    let _ = stream.set_read_timeout(None);
    Self::parse_bytes_sync(raw, routes, file_bases)
  }

  #[cfg(feature = "sync")]
  pub fn parse_raw_sync(
    raw: String,
    routes: &HashMap<(Rt, String), RouteEntry>,
    file_bases: &[String],
  ) -> (Self, Option<Response>) {
    Self::parse_bytes_sync(raw.into_bytes(), routes, file_bases)
  }

  #[cfg(feature = "sync")]
  fn parse_bytes_sync(
    raw: Vec<u8>,
    routes: &HashMap<(Rt, String), RouteEntry>,
    file_bases: &[String],
  ) -> (Self, Option<Response>) {
    match Self::parse_bytes_only(raw, routes) {
      Ok(mut request) => {
        let early = request.route_sync(routes, file_bases);
        (request, early)
      }
      Err(response) => (Self::default(), Some(response)),
    }
  }

  #[cfg(any(feature = "async_tokio", feature = "async_std", feature = "async_smol"))]
  pub async fn parse_raw_async(
    raw: String,
    routes: &HashMap<(Rt, String), RouteEntry>,
    file_bases: &[String],
  ) -> (Self, Option<Response>) {
    Self::parse_bytes_async(raw.into_bytes(), routes, file_bases).await
  }

  #[cfg(any(feature = "async_tokio", feature = "async_std", feature = "async_smol"))]
  async fn parse_bytes_async(
    raw: Vec<u8>,
    routes: &HashMap<(Rt, String), RouteEntry>,
    file_bases: &[String],
  ) -> (Self, Option<Response>) {
    match Self::parse_bytes_only(raw, routes) {
      Ok(mut request) => {
        let early = request.route_async(routes, file_bases).await;
        (request, early)
      }
      Err(response) => (Self::default(), Some(response)),
    }
  }

  fn parse_bytes_only(
    raw: Vec<u8>,
    routes: &HashMap<(Rt, String), RouteEntry>,
  ) -> Result<Self, Response> {
    let separator = raw
      .windows(4)
      .position(|window| window == b"\r\n\r\n")
      .ok_or_else(|| Response {
        status: StatusCode::BadRequest.to_string(),
        headers: vec![],
        body: Vec::new(),
      })?;

    let head = std::str::from_utf8(&raw[..separator]).map_err(|_| Response {
      status: StatusCode::BadRequest.to_string(),
      headers: vec![],
      body: Vec::new(),
    })?;
    let body = raw[separator + 4..].to_vec();

    let mut lines = head.split("\r\n");
    let request_line = lines.next().ok_or_else(|| Response {
      status: StatusCode::BadRequest.to_string(),
      headers: vec![],
      body: Vec::new(),
    })?;
    let parts: Vec<&str> = request_line.split_whitespace().collect();

    if parts.len() != 3 {
      return Err(Response {
        status: StatusCode::BadRequest.to_string(),
        headers: vec![],
        body: Vec::new(),
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
        status: StatusCode::MethodNotAllowed.to_string(),
        headers: vec![],
        body: Vec::new(),
      });
    }
    if version != "HTTP/1.1" {
      return Err(Response {
        status: StatusCode::HttpVersionNotSupported.to_string(),
        headers: vec![],
        body: Vec::new(),
      });
    }

    const MAX_URI: usize = 2000;
    if path_str.len() > MAX_URI {
      return Err(Response {
        status: StatusCode::UriTooLong.to_string(),
        headers: vec![],
        body: Vec::new(),
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
            status: StatusCode::Ok.to_string(),
            headers: vec![(
              "Content-Type".to_string(),
              crate::core::utils::get_content_type_quick(&real_path),
            )],
            body: data,
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

    let request = Request::parse_bytes_only(raw, &HashMap::new()).expect("valid request");

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
}
