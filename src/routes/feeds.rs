use std::env;

use rocket::{FromForm, get, post};
use rocket::form::Form;
use rocket::http::Status;
use rocket::response::Redirect;
use rocket::State;

use sqlx::sqlite::SqlitePool;

use crate::user::User;
use crate::feed::Feed;

#[derive(FromForm)]
pub struct FeedForm {
  name: String,
  url: String
}

#[post("/feed", data = "<form>")]
pub async fn add_feed(user: User, db: &State<SqlitePool>, form: Form<FeedForm>) -> Result<Redirect, Status> {
  let feed = Feed::create(&user, &form.url, &form.name, &db).await;
  
  match feed {
    Ok(_feed) => {
      Ok(Redirect::to("/"))
    },
    Err(why) => {
      print!("{}", why);
      Err(Status::NotFound)
    }
  }
}

#[get("/feed/<id>/delete")]
pub async fn delete_feed(user: User, id: i64, db: &State<SqlitePool>) -> Result<Redirect, Status> {
  let feed = Feed::delete(&user, id, &db).await;
  
  match feed {
    Ok(_feed) => {
      Ok(Redirect::to("/"))
    },
    Err(why) => {
      print!("{}", why);
      Err(Status::NotFound)
    }
  }
}

#[get("/feed/<username>")]
pub async fn render_feed(username: &str, db: &State<SqlitePool>) -> Result<String, Status> {
  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  let feed = Feed::find_by_name(&username.to_string(), db).await;

  match feed {
    Ok(feed) => {
      let ap = feed.to_activity_pub(&instance_domain);
      match ap {
        Ok(ap) => Ok(serde_json::to_string(&ap).unwrap()),
        Err(_why) => Err(Status::NotFound)
      }
      
    },
    Err(_why) => Err(Status::NotFound)
  }
}
