use rocket_dyn_templates::{Template, context};

use rocket::get;
use rocket::State;

use sqlx::sqlite::SqlitePool;

use crate::user::User;
use crate::feed::Feed;

#[get("/")]
pub async fn index_logged_in(user: User, db: &State<SqlitePool>) -> Template {
  let feeds = Feed::for_user(&user, &db).await.unwrap();
  Template::render("home", context! { logged_in: true, feeds: feeds })
}

#[get("/", rank = 2)]
pub fn index() -> Template {
  Template::render("home", context! { logged_in: false })
}

#[cfg(test)]
mod test {
  use crate::server::boot_server;
  use rocket::local::blocking::Client;
  use rocket::http::Status;
  use rocket::uri;
  
  #[sqlx::test]
  async fn index() {
    let server = boot_server().await;

    match server {
      Ok(server) => {    
        let client = Client::tracked(server).expect("valid rocket instance");
        let mut response = client.get(uri!(super::index)).dispatch();
        assert_eq!(response.status(), Status::Ok);
        assert_eq!(response.into_string().unwrap(), "Hello, world!");
      }
      Err(_why) => {
        println!("{:?}", _why);
        todo!()
      }
    }
  }
}
