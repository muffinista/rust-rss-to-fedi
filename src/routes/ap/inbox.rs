
use rocket::post;
use rocket::http::Status;
use rocket::State;

use sqlx::postgres::PgPool;

use crate::models::feed::Feed;
use crate::models::feed::AcceptedActivity;

use rocket::serde::json::Json;

/// The inbox stream contains all activities received by the actor. The server
/// SHOULD filter content according to the requester's permission. In general,
/// the owner of an inbox is likely to be able to access all of their inbox
/// contents. Depending on access control, some other content may be public,
/// whereas other content may require authentication for non-owner users, if
/// they can access the inbox at all. 
///
/// https://www.w3.org/TR/activitypub/#inbox
///
#[post("/feed/<username>/inbox", data="<activity>")]
pub async fn user_inbox(username: &str, activity: Json<AcceptedActivity>, db: &State<PgPool>) -> Result<(), Status> {
  let feed_lookup = Feed::find_by_name(&username.to_string(), db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          println!("***** {:?}", activity);
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

// #[cfg(test)]
// mod test {
//   use crate::server::build_server;
//   use rocket::local::asynchronous::Client;
//   use rocket::http::Status;
//   use rocket::uri;
//   use rocket::{Rocket, Build};
//   use chrono::Utc;

//   use sqlx::postgres::PgPool;

//   use crate::utils::test_helpers::{real_feed};

//   #[sqlx::test]
//   async fn test_user_inbox(pool: PgPool) -> sqlx::Result<()> {
//     let feed = real_feed(&pool).await.unwrap();

//     let actor = "https://activitypub.pizza/users/colin".to_string();
//     let json = format!(r#"{{"actor":"{}","object":"{}/feed","type":"Follow"}}"#, actor, actor).to_string();
    
//     let server:Rocket<Build> = build_server(pool).await;
//     let client = Client::tracked(server).await.unwrap();

//     let req = client.post(uri!(super::user_inbox(&feed.name))).body(json);
//     let response = req.dispatch().await;

//     assert_eq!(response.status(), Status::Ok);

//     Ok(())
//   }
// }
