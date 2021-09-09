use std::any::TypeId;

use crate::parser::ParseError;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct CoreError {
  source_error: Box<dyn std::error::Error>,
}

impl CoreError {
  pub fn custom(message: &str) -> Self {
    tracing::error!("{}", &message);
    Self {
      source_error: message.into(),
    }
  }
}

impl Display for CoreError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    self.source_error.fmt(f)
  }
}

impl Error for CoreError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    Some(&*self.source_error)
  }
}

impl From<std::io::Error> for CoreError {
  fn from(err: std::io::Error) -> Self {
    tracing::error!("{}", err.to_string());

    Self {
      source_error: Box::new(err),
    }
  }
}

impl From<ParseError> for CoreError {
  fn from(parse_error: ParseError) -> Self {
    tracing::error!("{}", parse_error.to_string());
    Self {
      source_error: Box::new(parse_error),
    }
  }
}

impl From<serde_json::Error> for CoreError {
  fn from(err: serde_json::Error) -> Self {
    tracing::error!("{}", err.to_string());
    Self {
      source_error: Box::new(err),
    }
  }
}

impl From<CoreError> for String {
  fn from(error: CoreError) -> Self {
    error.to_string()
  }
}

impl From<&str> for CoreError {
  fn from(error: &str) -> Self {
    Self {
      source_error: Box::new(CoreError::custom(error)),
    }
  }
}
