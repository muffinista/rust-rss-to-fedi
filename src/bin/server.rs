#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use rocket_dyn_templates::{Template, context};
use rocket::response::{Redirect};
use rocket::http::{Cookie, CookieJar};

use rocket::form::Form;
use rocket::http::Status;

use rocket::State;

use sqlx::sqlite::SqlitePool;

use std::env;

use rustypub::user::User;
use rustypub::feed::Feed;

#[derive(FromForm)]
struct LoginForm {
  email: String
}


#[get("/")]
async fn index_logged_in(user: User, db: &State<SqlitePool>) -> Template {
  let feeds = Feed::for_user(&user, &db).await.unwrap();
  Template::render("home", context! { logged_in: true, feeds: feeds })
}

#[get("/", rank = 2)]
fn index() -> Template {
  Template::render("home", context! { logged_in: false })
}

#[get("/user/auth/<login_token>")]
async fn attempt_login(db: &State<SqlitePool>, cookies: &CookieJar<'_>, login_token: &str) -> Result<Redirect, Status> {
  let user = User::find_by_login(&login_token.to_string(), &**db).await;
  
  match user {
    Ok(user) => {
      let token = User::apply_access_token(user, db).await;
      match token {
        Ok(token) => {
          cookies.add_private(Cookie::new("access_token", token.to_string()));
          Ok(Redirect::to("/?yay=1"))
        },
        Err(why) => {
          print!("{}", why);
          Err(Status::NotFound)
        }
      }
    },
    Err(why) => {
      print!("{}", why);
      Err(Status::NotFound)
    }
  }
}

#[post("/login", data = "<form>")]
async fn do_login(db: &State<SqlitePool>, form: Form<LoginForm>) -> Result<Redirect, Status> {
  let user = User::find_or_create_by_email(&form.email, &**db).await;
  
  // generate login token
  // send email
  // redirect
  
  
  match user {
    Ok(user) => {
      print!("/user/auth/{}", user.login_token);
      // just log the user in for now
      //Ok(Redirect::to("/"))
      Ok(Redirect::to(format!("/user/auth/{}", user.login_token)))
    },
    Err(why) => {
      print!("{}", why);
      Err(Status::NotFound)
    }
  }
}


#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
  let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
  let _domain_name = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

  let pool = SqlitePool::connect(&db_uri)
    .await
    .expect("Failed to create pool");
  sqlx::migrate!("./migrations")
    .run(&pool)
    .await
    .ok();
  
  let _rocket = rocket::build()
    .manage(pool)
    .mount("/", routes![
      index,
      index_logged_in,
      do_login,
      attempt_login,
      rustypub::routes::feeds::add_feed,
      rustypub::routes::feeds::delete_feed,
      rustypub::routes::feeds::render_feed,
      rustypub::routes::webfinger::lookup_webfinger
      ])
    .attach(Template::fairing())
    .launch()
    .await?;
  
  Ok(())
}
