use crate::core::cors::CorsPolicy;
use crate::core::request_type::RequestType;
use crate::core::response::Response;
use std::path::PathBuf;

pub fn print_server_info(addr: std::net::SocketAddr, _auto_close: bool) {
  // println!("Connection autoclose set to {:?}", _auto_close);

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

pub fn response_head(response: &Response, close: bool, cors: Option<&CorsPolicy>, origin: Option<&str>) -> String {
  let switching_protocols = response.status.starts_with("101 ");
  let mut header = format!("HTTP/1.1 {}\r\n", response.status);

  for (key, value) in &response.headers {
    if key.eq_ignore_ascii_case("content-length") {
      continue;
    }
    if key.eq_ignore_ascii_case("connection") {
      if close {
        continue;
      }
    }
    header.push_str(&format!("{}: {}\r\n", key, value));
  }

  if !switching_protocols {
    header.push_str(&format!("Content-Length: {}\r\n", response.body.len()));
  }
  if close {
    header.push_str("Connection: close\r\n");
  }
  if let Some(policy) = cors {
    for (key, value) in policy.header_lines(origin) {
      header.push_str(&format!("{}: {}\r\n", key, value));
    }
  }
  header.push_str("\r\n");
  header
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
      status: StatusCode::SwitchingProtocols.to_string(),
      headers: vec![
        ("Upgrade".into(), "websocket".into()),
        ("Connection".into(), "Upgrade".into()),
      ],
      body: Vec::new(),
    };

    let head = response_head(&response, false, None, None);

    assert!(head.contains("HTTP/1.1 101 Switching Protocols"));
    assert!(head.contains("Upgrade: websocket"));
    assert!(head.contains("Connection: Upgrade"));
    assert!(!head.contains("Content-Length"));
  }
}
