use actix_web::{
  error,
  http::{header::ContentType, StatusCode},
  HttpResponse,
};

use derive_more::{Display, Error};

#[derive(Debug, Display, Error, PartialEq)]
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



#[cfg(test)]
mod test {
  use super::AppError;

  #[test]
  fn test_app_error_from_sqlx() {
    let err = AppError::from(sqlx::Error::RowNotFound);
    assert_eq!(err, AppError::InternalError);
  }

  #[test]
  fn test_app_error_from_tera() {
    let err = AppError::from(tera::Error::msg("Hello"));
    assert_eq!(err, AppError::InternalError);
  }
}
