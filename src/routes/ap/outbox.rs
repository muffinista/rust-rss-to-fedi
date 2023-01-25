
use rocket::get;
use rocket::http::Status;
use rocket::State;

use sqlx::sqlite::SqlitePool;

use crate::models::feed::Feed;


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
#[get("/feed/<username>/outbox?<page>")]
pub async fn render_feed_outbox(username: &str, page: Option<u32>, db: &State<SqlitePool>) -> Result<String, Status> {
  let feed_lookup = Feed::find_by_name(&username.to_string(), db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          // if we got a page param, return a page of outbox items
          // otherwise, return the summary
          let json = match page {
            Some(page) => {
              let result = feed.outbox_paged(page, db).await;
              match result {
                Ok(result) => Ok(serde_json::to_string(&result).unwrap()),
                Err(_why) => Err(Status::InternalServerError)
              }
            },
            None => {
              let result = feed.outbox(db).await;
              match result {
                Ok(result) => Ok(serde_json::to_string(&result).unwrap()),
                Err(_why) => Err(Status::InternalServerError)
              }
            }
          };
      
          Ok(json.unwrap())
        },
        None => Err(Status::NotFound)
      }
    },
    Err(_why) => Err(Status::InternalServerError)
  }
}

#[cfg(test)]
mod test {
  use crate::server::build_server;
  use rocket::local::asynchronous::Client;
  use rocket::http::Status;
  use rocket::uri;
  use rocket::{Rocket, Build};

  use sqlx::sqlite::SqlitePool;

  use crate::utils::test_helpers::{real_feed};

  #[sqlx::test]
  async fn test_render_feed_outbox(pool: SqlitePool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();

    let actor = "https://activitypub.pizza/users/colin".to_string();
    let json = format!(r#"{{"actor":"{}","object":"{}/feed","type":"Follow"}}"#, actor, actor).to_string();
    
    let server: Rocket<Build> = build_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.post(uri!(super::render_feed_outbox(&feed.name))).body(json);
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    Ok(())
  }
}
