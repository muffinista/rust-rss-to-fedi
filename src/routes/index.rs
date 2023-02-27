use std::env;
use rocket_dyn_templates::{Template, context};

use rocket::get;
use rocket::State;

use sqlx::postgres::PgPool;

use crate::models::user::User;
use crate::models::feed::Feed;
use crate::models::Setting;

use crate::PER_PAGE;

#[get("/?<page>")]
pub async fn index_logged_in(user: User, page: Option<i32>, db: &State<PgPool>) -> Template {
  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  let signups_enabled = Setting::value_or(&"signups_enabled".to_string(), &"true".to_string(), db).await.unwrap();


  let page = if page.is_some() {
    page.unwrap()
  } else {
    1
  };

  let feeds = Feed::paged_for_user(&user, page, db).await.unwrap();
  let count = Feed::count_for_user(&user, db).await.unwrap();

  let total_pages:i32 = (count / PER_PAGE) + 1;

  Template::render("home", context! { 
    logged_in: true,
    username: user.full_username(),
    feeds: feeds,
    page: page,
    total_pages: total_pages,
    total: count,
    instance_domain: instance_domain,
    signups_enabled: signups_enabled == "true"
  })
}

#[get("/", rank = 2)]
pub fn index() -> Template {
  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  Template::render("home", context! {
    logged_in: false,
    instance_domain: instance_domain
  })
}

#[cfg(test)]
mod test {
  use rocket::local::asynchronous::Client;
  use rocket::http::Status;
  use rocket::uri;
  use rocket::{Rocket, Build};
  use sqlx::postgres::PgPool;
  
  use crate::utils::test_helpers::{build_test_server, real_user};


  #[sqlx::test]
  async fn index_not_logged_in(pool: PgPool) {
    let server:Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::index));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let output = response.into_string().await;
    match output {
      Some(output) => assert!(output.contains("To get started")),
      None => panic!()
    }
  }

  #[sqlx::test]
  async fn index_logged_in(pool: PgPool) {
    let user = real_user(&pool).await.unwrap();

    let server: Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    crate::utils::test_helpers::login_user(&client, &user).await;   

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
