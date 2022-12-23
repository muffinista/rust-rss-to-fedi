
use rocket::post;
use rocket::http::Status;
use rocket::State;

use sqlx::sqlite::SqlitePool;

use crate::feed::Feed;
use crate::feed::AcceptedActivity;

use rocket::serde::json::Json;

#[post("/feed/<username>/inbox", data="<activity>")]
pub async fn user_inbox(username: &str, activity: Json<AcceptedActivity>, db: &State<SqlitePool>) -> Result<(), Status> {
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
//   use crate::user::User;
//   use crate::feed::Feed;
//   use chrono::Utc;

//   use sqlx::sqlite::SqlitePool;

//   #[sqlx::test]
//   async fn test_user_outbox(pool: SqlitePool) -> sqlx::Result<()> {
//     let user = User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()), created_at: Utc::now().naive_utc(), updated_at: Utc::now().naive_utc() };

//     let url: String = "https://foo.com/rss.xml".to_string();
//     let name: String = "testfeed".to_string();

//     let _feed = Feed::create(&user, &url, &name, &pool).await?;

//     let actor = "https://activitypub.pizza/users/colin".to_string();
//     let json = format!(r#"{{"actor":"{}","object":"{}/feed","type":"Follow"}}"#, actor, actor).to_string();
    
//     let server:Rocket<Build> = build_server(pool).await;
//     let client = Client::tracked(server).await.unwrap();

//     let req = client.post(uri!(super::user_outbox(&name))).body(json);
//     let response = req.dispatch().await;

//     assert_eq!(response.status(), Status::Ok);

//     Ok(())
//   }
// }
