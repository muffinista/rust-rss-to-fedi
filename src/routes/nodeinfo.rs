use rocket::get;
// use rocket::serde::json::Json;
use rocket::State;
use rocket::http::Status;

use sqlx::postgres::PgPool;
use serde_json::json;

use crate::models::NodeInfo;
use std::env;


#[get("/nodeinfo/2.0")]
pub async fn nodeinfo(db: &State<PgPool>) -> Result<String, Status> {
  let data = NodeInfo::current(db).await;

  if data.is_ok() {
    let data = data.unwrap();

    let results = json!({
      "version": "2.0",
      "software": {
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION")
      },
      "protocols": [
        "activitypub"
      ],
      "services": {
        "outbound": [],
        "inbound": []
      },
      "usage": {
        "users": {
          "total": data.users,
          "activeMonth": data.users,
          "activeHalfyear": data.users
        },
        "localPosts": data.posts
      },
      "openRegistrations": true,
      "metadata": {}
    });

    Ok(results.to_string())
  } else {
    Err(Status::NotFound)
  }
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
  async fn test_nodeinfo(pool: PgPool) {
    let server:Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::nodeinfo));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let output = response.into_string().await;
    match output {
      Some(output) => assert!(output.contains("localPosts")),
      None => panic!()
    }
  }
}