use std::env;
use rocket::response::content;
use rocket::get;

static HOST_META: &str = concat!(
  r#"<?xml version="1.0" encoding="UTF-8"?>
<XRD xmlns="http://docs.oasis-open.org/ns/xri/xrd-1.0">
  <Link rel="lrdd" template="https://"#, env!("DOMAIN_NAME"), r#"/.well-known/webfinger?resource={{uri}}"/>
</XRD>"#
);


#[get("/.well-known/host-meta")]
pub async fn host_meta() -> content::RawXml<&'static str> {
  content::RawXml(HOST_META)
}



#[cfg(test)]
mod test {
  use rocket::local::asynchronous::Client;
  use rocket::http::Status;
  use rocket::uri;
  use rocket::{Rocket, Build};
  use sqlx::postgres::PgPool;
  
  use crate::utils::test_helpers::{build_test_server, real_user};


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