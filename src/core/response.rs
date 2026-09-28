use std::fmt::{Display, Formatter, Result};

use crate::core::status_code::StatusCode;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Body(Vec<u8>);

impl Body {
  pub fn len(&self) -> usize {
    self.0.len()
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  pub fn as_bytes(&self) -> &[u8] {
    &self.0
  }

  pub fn into_bytes(self) -> Vec<u8> {
    self.0
  }
}

impl AsRef<[u8]> for Body {
  fn as_ref(&self) -> &[u8] {
    self.as_bytes()
  }
}

impl From<&str> for Body {
  fn from(value: &str) -> Self {
    Self(value.as_bytes().to_vec())
  }
}

impl From<String> for Body {
  fn from(value: String) -> Self {
    Self(value.into_bytes())
  }
}

impl From<Vec<u8>> for Body {
  fn from(value: Vec<u8>) -> Self {
    Self(value)
  }
}

impl From<&[u8]> for Body {
  fn from(value: &[u8]) -> Self {
    Self(value.to_vec())
  }
}

impl<const N: usize> From<&[u8; N]> for Body {
  fn from(value: &[u8; N]) -> Self {
    Self(value.to_vec())
  }
}

#[derive(Debug)]
pub struct Response {
  pub status: StatusCode,
  pub headers: Vec<(String, String)>,
  pub body: Body,
}

impl Default for Response {
  fn default() -> Self {
    Response {
      status: StatusCode::NotFound,
      headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
      body: "404 Not Found".into(),
    }
  }
}

impl Display for Response {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result {
    write!(f, "{:?}", self.body.as_bytes())
  }
}

impl Response {
  pub fn new() -> Self {
    Self::default()
  }
}
