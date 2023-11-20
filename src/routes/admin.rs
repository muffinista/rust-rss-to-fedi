use std::env;
use rocket_dyn_templates::{Template, context};

use rocket::{FromForm, get, put, delete};
use rocket::form::Form;
use rocket::State;
use rocket::uri;
use rocket::response::{Flash, Redirect};
use rocket::http::Status;

use sqlx::postgres::PgPool;

use crate::models::User;
use crate::models::Feed;
use crate::models::Item;
use crate::models::Setting;

use crate::PER_PAGE;

#[derive(FromForm, serde::Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AdminSettingsForm {
  signups_enabled: String
}


#[get("/admin?<page>")]
pub async fn index_admin(user: User, page: Option<i32>, db: &State<PgPool>) -> Result<Template, Status> {
  if ! user.is_admin() {
    return Err(Status::NotFound)
  }

  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  let page: i32 = if let Some(page) = page {
    page
  } else {
    1
  };

  let feeds = Feed::paged(page, db).await.unwrap();
  let signups_enabled = Setting::value_or(&"signups_enabled".to_string(), &"true".to_string(), db).await.unwrap();

  let count = Feed::count(db).await.unwrap();
  let total_pages:i32 = (count / PER_PAGE) + 1;


  Ok(Template::render("admin", context! { 
    feeds: feeds,
    page: page,
    total_pages: total_pages,
    total: count,
    signups_enabled: signups_enabled,
    instance_domain: instance_domain,
    feed_link_prefix: "/admin"
  }))
}


#[put("/admin/settings", data = "<form>")]
pub async fn update_settings_admin(user: User, db: &State<PgPool>, form: Form<AdminSettingsForm>) -> Result<Flash<Redirect>, Status> {
  if ! user.is_admin() {
    return Err(Status::NotFound)
  }

  let result = Setting::update(&"signups_enabled".to_string(), &form.signups_enabled, db).await;

  let dest = uri!(index_admin(Some(1)));

  match result {
    Ok(_result) => Ok(Flash::success(Redirect::to(dest), "Settings updated!")),
    Err(_why) => Ok(Flash::error(Redirect::to(dest), "Sorry, something went wrong!"))
  }
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
          let items = Item::for_feed(&feed, 10, db).await;

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

  let feed = Feed::admin_delete(id, db).await;
  
  match feed {
    Ok(_feed) => {
      Ok(Redirect::to("/admin"))
    },
    Err(why) => {
      print!("{why}");
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
  
  use crate::utils::test_helpers::{build_test_server, real_user, real_admin_user};


  #[sqlx::test]
  async fn index_admin_not_logged_in(pool: PgPool) {
    let server:Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::index_admin(Some(1))));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Unauthorized);
  }

  #[sqlx::test]
  async fn index_admin_non_admin_logged_in(pool: PgPool) {
    let user = real_user(&pool).await.unwrap();

    let server:Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    crate::utils::test_helpers::login_user(&client, &user).await;   

    let req = client.get(uri!(super::index_admin(Some(1))));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::NotFound);
  }

  #[sqlx::test]
  async fn index_admin_logged_in(pool: PgPool) {
    let user = real_admin_user(&pool).await.unwrap();

    let server: Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    crate::utils::test_helpers::login_user(&client, &user).await;   

    let req = client.get(uri!(super::index_admin(Some(1))));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    let output = response.into_string().await;
    match output {
      Some(output) => assert!(output.contains("Feed admin!")),
      None => panic!()
    }
  }
}
