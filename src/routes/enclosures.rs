use rocket::get;
use rocket::http::Status;
use rocket::response::Redirect;
use rocket::State;

use sqlx::postgres::PgPool;
use std::path::Path;
use crate::models::Enclosure;


#[get("/feed/<username>/items/<item_id>/enclosures/<file>", format = "any")]
pub async fn show_enclosure(username: &str, item_id: i32, file: String, db: &State<PgPool>) -> Result<Redirect, Status> {
  let filename_base = Path::new(&file).with_extension("").into_os_string().into_string();
  if filename_base.is_err() {
    return Err(Status::NotFound)
  }

  let id = filename_base.unwrap().parse::<i32>();
  
  if id.is_err() {
    return Err(Status::NotFound)
  }

  let id:i32 = id.unwrap();

  let enclosure = Enclosure::find_by_feed_and_item_and_id(username, item_id, id, db).await;

  match enclosure {
    Ok(enclosure) => {
      if enclosure.is_some() {
        let enclosure = enclosure.unwrap();
        Ok(Redirect::to(enclosure.url))
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
  use rocket::local::asynchronous::Client;
  use rocket::http::{Status};
  use rocket::uri;
  use rocket::{Rocket, Build};

  use crate::models::feed::Feed;
  use crate::models::item::Item;
  use crate::models::Enclosure;
  use crate::utils::test_helpers::{build_test_server, real_feed, real_item, real_enclosure};

  use sqlx::postgres::PgPool;

  #[sqlx::test]
  async fn test_show_enclosure(pool: PgPool) -> sqlx::Result<()> {
    let feed: Feed = real_feed(&pool).await?;
    let item: Item = real_item(&feed, &pool).await?;
    let enclosure: Enclosure = real_enclosure(&item, &pool).await?;

    let server: Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let filename = format!("{:}.mp3", enclosure.id);
    let req = client.get(uri!(super::show_enclosure(&feed.name, item.id, filename)));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::SeeOther);

    Ok(())
  }
}
