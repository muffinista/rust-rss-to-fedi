
use actix_web::http::StatusCode;
use actix_web::{get, web, HttpResponse, Responder};

use sqlx::postgres::PgPool;

use crate::models::feed_error::AppError;
use crate::models::Feed;
use crate::routes::PageQuery;
use crate::ACTIVITY_JSON;


///  The outbox is discovered through the outbox property of an actor's profile.
///  The outbox MUST be an OrderedCollection.
///
/// The outbox stream contains activities the user has published, subject to the
/// ability of the requestor to retrieve the activity (that is, the contents of
/// the outbox are filtered by the permissions of the person reading it). If a
/// user submits a request without Authorization the server should respond with
/// all of the Public posts. This could potentially be all relevant objects
/// published by the user, though the number of available items is left to the
/// discretion of those implementing and deploying the server. 
///
#[get("/feed/{username}/outbox")]
pub async fn render_feed_outbox(
  path: web::Path<String>,
  query: web::Query<PageQuery>,
  db: web::Data<PgPool>,
  tmpl: web::Data<tera::Tera>) -> Result<impl Responder, AppError> {
  let tmpl = tmpl.as_ref();
  let db = db.as_ref();
  let username = path.into_inner();
  let feed_lookup = Feed::find_by_name(&username.to_string(), db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          // if we got a page param, return a page of outbox items
          // otherwise, return the summary
          let json = match query.page {
            Some(page) => {
              let result = feed.outbox_paged(page, db, tmpl).await;
              match result {
                Ok(result) => Ok(HttpResponse::build(StatusCode::OK).content_type(ACTIVITY_JSON).body(serde_json::to_string(&result).unwrap())),
                Err(_why) => Err(AppError::InternalError)
              }
            },
            None => {
              let result = feed.outbox(db).await;
              match result {
                Ok(result) => Ok(HttpResponse::build(StatusCode::OK).content_type(ACTIVITY_JSON).body(serde_json::to_string(&result).unwrap())),
                Err(_why) => Err(AppError::InternalError)
              }
            }
          };
      
          Ok(json.unwrap())
        },
        None => Err(AppError::NotFound)
      }
    },
    Err(_why) => Err(AppError::InternalError)
  }
}

#[cfg(test)]
mod test {
  use actix_web::{test, dev::Service};
  use actix_session::{SessionMiddleware, storage::CookieSessionStore};
  use sqlx::postgres::PgPool;

  use crate::build_test_server;
  use crate::assert_content_type;
  use crate::utils::test_helpers::real_feed;
  use crate::ACTIVITY_JSON;

  #[sqlx::test]
  async fn test_render_feed_outbox(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();
println!("visit! {}",feed.outbox_url());
    let server = test::init_service(build_test_server!(pool)).await;
    // let req = test::TestRequest::with_uri(&feed.outbox_url()).to_request();
    let req = test::TestRequest::with_uri(&format!("/feed/{}/outbox", feed.name)).to_request();
    let res = server.call(req).await.unwrap();

    assert_eq!(res.status(), actix_web::http::StatusCode::OK);
    assert_content_type!(res, ACTIVITY_JSON);

    Ok(())
  }
}
