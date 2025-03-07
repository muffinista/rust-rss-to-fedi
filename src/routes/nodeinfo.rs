use actix_web::{get, web, web::Json, Responder};

use sqlx::postgres::PgPool;
use serde_json::json;

use crate::models::AppError;
use crate::models::NodeInfo;
use std::env;


#[get("/nodeinfo/2.0")]
pub async fn nodeinfo(db: web::Data<PgPool>) -> Result<impl Responder, AppError> {
  let db = db.as_ref();
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

    Ok(Json(results))
  } else {
    Err(AppError::NotFound)
  }
}

#[cfg(test)]
mod test {
  use actix_web::test;
  use actix_session::{SessionMiddleware, storage::CookieSessionStore};
  use sqlx::postgres::PgPool;
  
  use crate::build_test_server;

  #[sqlx::test]
  async fn test_nodeinfo(pool: PgPool) {
    let server = test::init_service(build_test_server!(pool)).await;

    // Create request object
    let req = test::TestRequest::with_uri("/nodeinfo/2.0").to_request();

    let res = test::call_service(&server, req).await;
    assert!(res.status().is_success());
    
    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    assert!(std::str::from_utf8(&bytes).unwrap().contains("localPosts"));
  }
}