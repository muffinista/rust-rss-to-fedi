use rocket_dyn_templates::{Template, context};

use rocket::get;
use rocket::State;

use sqlx::sqlite::SqlitePool;

use crate::models::user::User;
use crate::models::feed::Feed;

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
  use crate::server::build_server;
  use rocket::local::asynchronous::Client;
  use rocket::http::Status;
  use rocket::uri;
  use rocket::{Rocket, Build};
  use sqlx::sqlite::SqlitePool;
  
  #[sqlx::test]
  async fn index_not_logged_in(pool: SqlitePool) {
    let server:Rocket<Build> = build_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::index));
    let response = req.dispatch().await;

    // running multiple requests:
    // let (r1, r2) = rocket::tokio::join!(req.clone().dispatch(), req.dispatch());

    assert_eq!(response.status(), Status::Ok);
    let output = response.into_string().await;
    match output {
      Some(output) => assert!(output.contains("Email:")),
      None => panic!()
    }
  }
}
