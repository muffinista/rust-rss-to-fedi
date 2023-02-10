use std::env;
use rocket_dyn_templates::{Template, context};

use rocket::{get, delete};
use rocket::State;

use rocket::http::Status;
use rocket::response::Redirect;

use sqlx::postgres::PgPool;

use crate::models::User;
use crate::models::Feed;
use crate::models::Item;

#[get("/admin")]
pub async fn index_admin(user: User, db: &State<PgPool>) -> Result<Template, Status> {
  if ! user.is_admin() {
    return Err(Status::NotFound)
  }

  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  let feeds = Feed::all(&db).await.unwrap();
  Ok(Template::render("admin", context! { 
    feeds: feeds,
    instance_domain: instance_domain
  }))
}

#[get("/admin/feed/<username>", format = "text/html", rank = 2)]
pub async fn show_feed_admin(user: User, username: &str, db: &State<PgPool>) -> Result<Template, Status> {
  if ! user.is_admin() {
    return Err(Status::NotFound)
  }

  let feed_lookup = Feed::find_by_name(&username.to_string(), db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          let logged_in = true; // user.is_some();
          let follow_url = feed.permalink_url();
          let items = Item::for_feed(&feed, 10, &db).await;

          match items {
            Ok(items) => {
              Ok(Template::render("feed-admin", context! {
                is_admin: feed.is_admin(),
                noindex: !feed.listed,
                logged_in: logged_in,
                username: user.full_username(),
                owned_by: true,
                feed: feed,
                items: items,
                follow_url: follow_url,
                instance_domain: env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set")
              }))    
            },
            Err(_why) => Err(Status::NotFound)        
          }

        },
        None => Err(Status::NotFound)
      }
    },
    Err(_why) => Err(Status::NotFound)
  }
}

#[delete("/admin/feed/<id>/delete")]
pub async fn delete_feed_admin(user: User, id: i32, db: &State<PgPool>) -> Result<Redirect, Status> {
  if ! user.is_admin() {
    return Err(Status::NotFound)
  }

  let feed = Feed::admin_delete(id, &db).await;
  
  match feed {
    Ok(_feed) => {
      Ok(Redirect::to("/admin"))
    },
    Err(why) => {
      print!("{}", why);
      Err(Status::NotFound)
    }
  }
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
  async fn index_admin_not_logged_in(pool: PgPool) {
    let server:Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::index_admin));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::NotFound);
  }

  #[sqlx::test]
  async fn index_admin_logged_in(pool: PgPool) {
    let user = real_user(&pool).await.unwrap();

    let server: Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    crate::models::test_helpers::login_user(&client, &user).await;   

    let req = client.get(uri!(super::index_admin));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let output = response.into_string().await;
    match output {
      Some(output) => assert!(output.contains("Feed admin!")),
      None => panic!()
    }
  }
}
