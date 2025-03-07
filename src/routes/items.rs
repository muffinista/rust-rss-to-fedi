use actix_web::Responder;
use actix_web::{get, web};

use sqlx::postgres::PgPool;

use crate::models::AppError;
use crate::models::Feed;
use crate::models::Item;


#[get("/feed/{username}/items/{id}")]
pub async fn show_item(path: web::Path<(String, i32)>, db: web::Data<PgPool>) -> Result<impl Responder, AppError> {
  let (username, id) = path.into_inner();
  let db = db.as_ref();
  let lookup_feed = Feed::find_by_name(&username.to_string(), db).await;

  match lookup_feed {
    Ok(lookup_feed) => {
      if lookup_feed.is_some() {
        let feed = lookup_feed.unwrap();
        let item = Item::find_by_feed_and_id(&feed, id, db).await;
        match item {
          Ok(item) => {
            if item.is_some() {
              let data = item.unwrap();
              if data.url.is_some() {
                return Ok(crate::utils::redirect_to(&data.url.unwrap()))
              } else if feed.site_url.is_some() {
                return Ok(crate::utils::redirect_to(&feed.site_url.unwrap()))                
              }
            }

            Err(AppError::NotFound)
          },
          Err(_why) => Err(AppError::NotFound)
        }
      }
      else {
        Err(AppError::NotFound)
      }
    },
    Err(_why) => Err(AppError::NotFound)
  }
}


#[cfg(test)]
mod test {
  use actix_web::{test, dev::Service};
  use actix_session::{SessionMiddleware, storage::CookieSessionStore};

  use crate::models::Feed;
  use crate::models::Item;
  use crate::utils::test_helpers::{ real_item, real_feed};
  use crate::build_test_server;

  use sqlx::postgres::PgPool;

  #[sqlx::test]
  async fn test_show_item(pool: PgPool) -> sqlx::Result<()> {
    let feed: Feed = real_feed(&pool).await?;
    let item: Item = real_item(&feed, &pool).await?;

    let server = test::init_service(build_test_server!(pool)).await;
    let req = test::TestRequest::with_uri(&format!("/feed/{}/items/{}", feed.name, item.id)).to_request();
    let res = server.call(req).await.unwrap();

    assert_eq!(res.status(), actix_web::http::StatusCode::TEMPORARY_REDIRECT);

    Ok(())
  }
}
