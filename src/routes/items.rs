use rocket::get;
use rocket::http::Status;
use rocket::response::Redirect;
use rocket::State;

use sqlx::postgres::PgPool;

use crate::models::feed::Feed;
use crate::models::item::Item;


#[get("/feed/<username>/items/<id>", format = "text/html", rank = 2)]
pub async fn show_item(username: &str, id: i32, db: &State<PgPool>) -> Result<Redirect, Status> {
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
                return Ok(Redirect::to(data.url.unwrap()))
              } else if feed.site_url.is_some() {
                return Ok(Redirect::to(feed.site_url.unwrap()))                
              }
            }

            Err(Status::NotFound)
          },
          Err(_why) => Err(Status::NotFound)
        }
      }
      else {
        Err(Status::NotFound)
      }
    },
    Err(_why) => Err(Status::NotFound)
  }
}


#[cfg(test)]
mod test {
  use crate::server::build_server;

  use rocket::local::asynchronous::Client;
  use rocket::http::Status;
  use rocket::uri;
  use rocket::{Rocket, Build};

  use crate::models::feed::Feed;
  use crate::models::item::Item;
  use crate::utils::test_helpers::{real_item, real_feed};

  use sqlx::postgres::PgPool;

  #[sqlx::test]
  async fn test_show_item(pool: PgPool) -> sqlx::Result<()> {
    let feed: Feed = real_feed(&pool).await?;
    let item: Item = real_item(&feed, &pool).await?;

    let server: Rocket<Build> = build_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::show_item(&feed.name, item.id)));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::SeeOther);

    Ok(())
  }
}
