use std::fmt;
use fang::FangError;
use http_signature_normalization_reqwest::SignError;

use crate::models::FeedError;

#[derive(Debug)]
pub enum DeliveryError {
    Error(String),
    HttpMiddlewareError(reqwest_middleware::Error),
    HttpError(reqwest::Error),
    ShaError(openssl::error::ErrorStack),
    SigningError(http_signature_normalization_reqwest::SignError),
    UrlParsingError(url::ParseError),
    JsonError(serde_json::Error),
    DbError(sqlx::Error),
    StringValidationError(iri_string::validate::Error),
    FeedError(FeedError),
}

impl std::fmt::Display for DeliveryError {
  fn fmt(&self, f: &mut fmt::Formatter) -> std::fmt::Result {
    write!(f, "An Error Occurred, Please Try Again!") // user-facing output
  }
}

impl From<&str> for DeliveryError {
  fn from(error: &str) ->  Self {
      DeliveryError::Error(String::from(error))
  }
}

impl From<DeliveryError> for FangError {
  fn from(error: DeliveryError) ->  Self {
      FangError { description: format!("{error:?}") }
  }
}

impl From<SignError> for DeliveryError {
  fn from(error: SignError) ->  Self {
    DeliveryError::SigningError(error)
  }
}

impl From<reqwest::Error> for DeliveryError {
  fn from(error: reqwest::Error) ->  Self {
    DeliveryError::HttpError(error)
  }
}

impl From<openssl::error::ErrorStack> for DeliveryError {
  fn from(error: openssl::error::ErrorStack) ->  Self {
    DeliveryError::ShaError(error)
  }
}

impl From<url::ParseError> for DeliveryError {
  fn from(error: url::ParseError) ->  Self {
    DeliveryError::UrlParsingError(error)
  }
}

impl From<serde_json::Error> for DeliveryError {
  fn from(error: serde_json::Error) ->  Self {
    DeliveryError::JsonError(error)
  }
}


impl From<sqlx::Error> for DeliveryError {
  fn from(error: sqlx::Error) ->  Self {
    DeliveryError::DbError(error)
  }
}

impl From<iri_string::validate::Error> for DeliveryError {
  fn from(error: iri_string::validate::Error) ->  Self {
    DeliveryError::StringValidationError(error)
  }
}

impl From<FeedError> for DeliveryError {
  fn from(error: FeedError) ->  Self {
    DeliveryError::FeedError(error)
  }
}


