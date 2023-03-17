use std::env;
use rocket::response::content;
use rocket::get;

#[get("/.well-known/host-meta")]
pub async fn host_meta() -> content::RawXml<String> {
  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

  let output: String = format!(
    r#"<?xml version="1.0" encoding="UTF-8"?>
  <XRD xmlns="http://docs.oasis-open.org/ns/xri/xrd-1.0">
    <Link rel="lrdd" template="https://{instance_domain:}/.well-known/webfinger?resource={{uri}}"/>
  </XRD>"#);

  content::RawXml(output)
}

#[cfg(test)]
mod test {
  use rocket::local::asynchronous::Client;
  use rocket::http::Status;
  use rocket::uri;
  use rocket::{Rocket, Build};
  use sqlx::postgres::PgPool;
  
  use crate::utils::test_helpers::{build_test_server};


  #[sqlx::test]
  async fn test_host_meta(pool: PgPool) {
    let server:Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::host_meta));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let output = response.into_string().await;
    match output {
      Some(output) => assert!(output.contains("well-known/webfinger")),
      None => panic!()
    }
  }
}