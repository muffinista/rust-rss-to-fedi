use actix_web::{get, web, Responder};
use sqlx::postgres::PgPool;
use std::path::Path;
use crate::models::feed_error::AppError;
use crate::models::Enclosure;


// , format = "any"
#[get("/feed/{username}/items/{item_id}/enclosures/{file}")]
pub async fn show_enclosure(path: web::Path<(String, i32, String)>, db: web::Data<PgPool>) -> Result<impl Responder, AppError> {
  let db = db.as_ref();
  let (username, item_id, file) = path.into_inner();

  let filename_base = Path::new(&file).with_extension("").into_os_string().into_string();
  if filename_base.is_err() {
    return Err(AppError::NotFound)
  }

  let id = filename_base.unwrap().parse::<i32>();
  
  if id.is_err() {
    return Err(AppError::NotFound)
  }

  let id:i32 = id.unwrap();

  let enclosure = Enclosure::find_by_feed_and_item_and_id(&username, item_id, id, db).await;

  match enclosure {
    Ok(enclosure) => {
      if enclosure.is_some() {
        let enclosure = enclosure.unwrap();
        Ok(crate::utils::redirect_to(&enclosure.url))
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
  use crate::models::Enclosure;
  use crate::utils::test_helpers::{ real_feed, real_item, real_enclosure};
  use crate::build_test_server;

  use sqlx::postgres::PgPool;

  #[sqlx::test]
  async fn test_show_enclosure(pool: PgPool) -> sqlx::Result<()> {
    let feed: Feed = real_feed(&pool).await?;
    let item: Item = real_item(&feed, &pool).await?;
    let enclosure: Enclosure = real_enclosure(&item, &pool).await?;

    let server = test::init_service(build_test_server!(pool)).await;
    let req = test::TestRequest::with_uri(&enclosure.url(&feed.name)).to_request();
    let res = server.call(req).await.unwrap();
    assert_eq!(res.status(), actix_web::http::StatusCode::TEMPORARY_REDIRECT);

    Ok(())
  }
}
