use rocket_dyn_templates::{Template, context};

use rocket::get;
use rocket::State;

use sqlx::postgres::PgPool;

use crate::models::user::User;
use crate::models::feed::Feed;

#[get("/")]
pub async fn index_logged_in(user: User, db: &State<PgPool>) -> Template {
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
  use sqlx::postgres::PgPool;
  
  use crate::utils::test_helpers::{real_user};


  #[sqlx::test]
  async fn index_not_logged_in(pool: PgPool) {
    let server:Rocket<Build> = build_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::index));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let output = response.into_string().await;
    match output {
      Some(output) => assert!(output.contains("Email:")),
      None => panic!()
    }
  }

  #[sqlx::test]
  async fn index_logged_in(pool: PgPool) {
    let user = real_user(&pool).await.unwrap();

    let server: Rocket<Build> = build_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    crate::models::test_helpers::login_user(&client, &user).await;   

    let req = client.get(uri!(super::index));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let output = response.into_string().await;
    match output {
      Some(output) => assert!(output.contains("Add a new feed:")),
      None => panic!()
    }
  }
}
