use std::fmt::{Display, Formatter, Result};

use crate::core::status_code::StatusCode;

#[derive(Debug)]
pub struct Response {
  pub status: String,
  pub headers: Vec<(String, String)>,
  pub body: Vec<u8>,
}

impl Default for Response {
  fn default() -> Self {
    Response {
      status: StatusCode::NotFound.to_string(),
      headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
      body: b"404 Not Found".to_vec(),
    }
  }
}

impl Display for Response {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result {
    write!(f, "{:?}", self.body)
  }
}

impl Response {
  pub fn new() -> Self {
    Self::default()
  }
}
