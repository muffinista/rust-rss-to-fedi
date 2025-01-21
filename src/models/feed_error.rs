use actix_web::{
  error,
  http::{header::ContentType, StatusCode},
  HttpResponse,
};

use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
pub enum AppError {
  #[display("internal error")]
  InternalError,

  #[display("bad request")]
  BadClientData,

  #[display("timeout")]
  Timeout,

  #[display("not found")]
  NotFound,

  #[display("pool error")]
  PoolError
}

impl From<sqlx::Error> for AppError {
  fn from(_error: sqlx::Error) -> Self {
    AppError::InternalError
  }
}

impl From<tera::Error> for AppError {
  fn from(_error: tera::Error) -> Self {
    AppError::InternalError
  }
}

impl From<actix_session::SessionGetError> for AppError {
  fn from(_error: actix_session::SessionGetError) -> Self {
    AppError::InternalError
  }
}

impl error::ResponseError for AppError {
  fn error_response(&self) -> HttpResponse {
      HttpResponse::build(self.status_code())
          .insert_header(ContentType::html())
          .body(self.to_string())
  }

  fn status_code(&self) -> StatusCode {
      match *self {
        AppError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        AppError::BadClientData => StatusCode::BAD_REQUEST,
        AppError::Timeout => StatusCode::GATEWAY_TIMEOUT,
        AppError::NotFound => StatusCode::NOT_FOUND,
        AppError::PoolError => StatusCode::INTERNAL_SERVER_ERROR,
      }
  }
}


pub struct FeedError {
  pub message: String
}

impl Error for FeedError {}
impl std::fmt::Display for FeedError {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{:}", self.message)
  }
}

impl std::fmt::Debug for FeedError {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    // @todo does this work?
    let current_file = file!();
    let current_line = line!();

    write!(f, "FeedError {:} {{ file: {current_file:}, line: {current_line:} }}", self.message)
  }
}

impl From<sqlx::Error> for FeedError {
  fn from(error: sqlx::Error) -> Self {
    FeedError {
      message: error.to_string(),
    }
  }
}


#[cfg(test)]
mod test {
  use super::FeedError;

  #[test]
  fn test_feed_error() {
    let err = FeedError { message: String::from("Boooo") };

    assert_eq!(err.message, String::from("Boooo"));
  }
}
