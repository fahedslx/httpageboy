use crate::core::cors::CorsPolicy;
use crate::core::request_type::RequestType;
use crate::core::response::Response;
use std::path::PathBuf;

pub fn print_server_info(addr: std::net::SocketAddr) {
  let url = format!("http://{}", addr);
  let _green_url = format!("\x1b[32m{}\x1b[0m", url);

  #[cfg(feature = "sync")]
  println!("Serving (sync) on {}", _green_url);

  #[cfg(feature = "async_tokio")]
  println!("Serving (async_tokio) on {}", _green_url);

  #[cfg(feature = "async_std")]
  println!("Serving (async_std) on {}", _green_url);

  #[cfg(feature = "async_smol")]
  println!("Serving (async_smol) on {}", _green_url);
}

pub fn file_source_path<S>(base: S) -> String
where
  S: Into<String>,
{
  let source = base.into();
  PathBuf::from(&source)
    .canonicalize()
    .map(|path| path.to_string_lossy().to_string())
    .unwrap_or(source)
}

pub const INTERNAL_SERVER_ERROR_HEAD: &str =
  "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

fn valid_header_part(value: &str) -> bool {
  !value.contains('\r') && !value.contains('\n')
}

fn status_forbids_content(status: crate::StatusCode) -> bool {
  let code = status as u16;
  (100..200).contains(&code)
    || status == crate::StatusCode::NoContent
    || status == crate::StatusCode::NotModified
}

pub fn response_has_content(method: Option<&RequestType>, response: &Response) -> bool {
  !matches!(method, Some(RequestType::HEAD)) && !status_forbids_content(response.status)
}

pub fn response_head(
  response: &Response,
  method: Option<&RequestType>,
  close: bool,
  cors: Option<&CorsPolicy>,
  origin: Option<&str>,
) -> Option<String> {
  let mut header = format!("HTTP/1.1 {}\r\n", response.status);

  for (key, value) in &response.headers {
    if !valid_header_part(key) || !valid_header_part(value) {
      return None;
    }
    if key.eq_ignore_ascii_case("content-length") {
      continue;
    }
    if key.eq_ignore_ascii_case("connection") && close {
      continue;
    }
    header.push_str(&format!("{}: {}\r\n", key, value));
  }

  if response_has_content(method, response) {
    header.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
  }
  if close {
    header.push_str("Connection: close\r\n");
  }
  if let Some(policy) = cors {
    for (key, value) in policy.header_lines(origin) {
      if !valid_header_part(&key) || !valid_header_part(&value) {
        return None;
      }
      header.push_str(&format!("{}: {}\r\n", key, value));
    }
  }
  header.push_str("\r\n");
  Some(header)
}

pub fn response_or_default(response: Option<Response>, method: &RequestType, cors: Option<&CorsPolicy>) -> Response {
  if let Some(response) = response {
    return response;
  }
  if *method == RequestType::OPTIONS {
    if let Some(policy) = cors {
      return policy.preflight_response();
    }
  }
  Response::new()
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::StatusCode;

  #[test]
  fn switching_protocols_preserves_upgrade_headers_without_content_length() {
    let response = Response {
      status: StatusCode::SwitchingProtocols,
      headers: vec![
        ("Upgrade".into(), "websocket".into()),
        ("Connection".into(), "Upgrade".into()),
      ],
      body: Vec::new().into(),
    };

    let head = response_head(&response, None, false, None, None).expect("valid response headers");

    assert!(head.contains("HTTP/1.1 101 Switching Protocols"));
    assert!(head.contains("Upgrade: websocket"));
    assert!(head.contains("Connection: Upgrade"));
    assert!(!head.contains("Content-Length"));
  }
  #[test]
  fn rejects_crlf_in_response_headers() {
    let response = Response {
      status: StatusCode::Ok,
      headers: vec![("X-Test".into(), "ok\r\nX-Injected: yes".into())],
      body: Vec::new().into(),
    };

    assert!(response_head(&response, None, true, None, None).is_none());
  }

  #[test]
  fn suppresses_content_for_head_and_bodyless_statuses() {
    let response = Response {
      status: StatusCode::Ok,
      headers: vec![],
      body: "body".into(),
    };
    assert!(!response_has_content(Some(&RequestType::HEAD), &response));
    let head = response_head(&response, Some(&RequestType::HEAD), false, None, None)
      .expect("valid response headers");
    assert!(!head.contains("Content-Length"));

    for status in [StatusCode::Continue, StatusCode::NoContent, StatusCode::NotModified] {
      let response = Response {
        status,
        headers: vec![],
        body: "body".into(),
      };
      assert!(!response_has_content(None, &response));
      let head = response_head(&response, None, false, None, None)
        .expect("valid response headers");
      assert!(!head.contains("Content-Length"));
    }
  }
}
