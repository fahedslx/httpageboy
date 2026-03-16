use std::fmt::{Display, Formatter, Result};

use crate::core::status_code::StatusCode;

#[derive(Debug)]
pub struct Response {
  pub status: String,
  pub content_type: String,
  pub content: Vec<u8>,
}

impl Default for Response {
  fn default() -> Self {
    Response {
      status: StatusCode::NotFound.to_string(),
      content_type: "text/plain".to_string(),
      content: b"404 Not Found".to_vec(),
    }
  }
}

impl Display for Response {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result {
    write!(f, "{:?}", self.content)
  }
}

impl Response {
  pub fn new() -> Self {
    Self::default()
  }
}
