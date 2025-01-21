use std::env;

use actix_web::{get, Responder};

use crate::models::feed_error::AppError;


#[get("/.well-known/host-meta")]
pub async fn host_meta() -> Result<impl Responder, AppError> {
  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

  let output: String = format!(
    r#"<?xml version="1.0" encoding="UTF-8"?>
  <XRD xmlns="http://docs.oasis-open.org/ns/xri/xrd-1.0">
    <Link rel="lrdd" template="https://{instance_domain:}/.well-known/webfinger?resource={{uri}}"/>
  </XRD>"#);

  Ok(output)
}

#[cfg(test)]
mod test {
  use actix_web::test;
  use actix_session::{SessionMiddleware, storage::CookieSessionStore};

  use sqlx::postgres::PgPool;
  
  use crate::build_test_server;


  #[sqlx::test]
  async fn test_host_meta(pool: PgPool) {
    let server = test::init_service(build_test_server!(pool)).await;

    // Create request object
    let req = test::TestRequest::with_uri("/.well-known/host-meta").to_request();

    let res = test::call_service(&server, req).await;
    assert!(res.status().is_success());

    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    assert!(std::str::from_utf8(&bytes).unwrap().contains("well-known/webfinger"));
  }
}