use std::fmt::{self, Display, Formatter};

pub type Rt = RequestType;

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum RequestType {
  GET,
  POST,
  PUT,
  DELETE,
  HEAD,
  OPTIONS,
  CONNECT,
  PATCH,
  TRACE,
}

impl Display for RequestType {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

impl RequestType {
  pub fn from_str(s: &str) -> Self {
    match s.to_uppercase().as_str() {
      "GET" => RequestType::GET,
      "POST" => RequestType::POST,
      "PUT" => RequestType::PUT,
      "DELETE" => RequestType::DELETE,
      "HEAD" => RequestType::HEAD,
      "OPTIONS" => RequestType::OPTIONS,
      "CONNECT" => RequestType::CONNECT,
      "PATCH" => RequestType::PATCH,
      "TRACE" => RequestType::TRACE,
      _ => RequestType::GET,
    }
  }
}
