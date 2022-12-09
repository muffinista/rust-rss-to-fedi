
use rocket::post;
use rocket::http::Status;
use rocket::State;

use sqlx::sqlite::SqlitePool;

use crate::feed::Feed;
use crate::feed::AcceptedActivity;

use rocket::serde::json::Json;


#[post("/feed/<username>/outbox", data="<activity>")]
pub async fn user_outbox(db: &State<SqlitePool>, username: &str, activity: Json<AcceptedActivity>) -> Result<(), Status> {
  let feed_lookup = Feed::find_by_name(&username.to_string(), db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          let handle = feed.handle_activity(db, &activity).await;
          match handle {
            Ok(_handle) => Status::Accepted,
            Err(_why) => Status::NotFound
          }
        },
        None => return Err(Status::NotFound)
      }
    },
    Err(_why) => return Err(Status::NotFound)
  };
  
  Ok(())
}

