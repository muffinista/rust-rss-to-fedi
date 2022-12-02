#![feature(proc_macro_hygiene, decl_macro)]
//#![feature(async_fn_in_trait)]

#[macro_use]
extern crate rocket;

use rocket_dyn_templates::{Template, tera::Tera, context};
use rocket::response::{Redirect};
use rocket::http::{Cookie, CookieJar};

use rocket::form::Form;
use rocket::http::Status;

use rocket::State;

use sqlx::sqlite::SqlitePool;

use std::env;

use rustypub::user::User;
use rustypub::feed::Feed;

use webfinger::*;

#[derive(FromForm)]
struct LoginForm {
  email: String
}

#[derive(FromForm)]
struct FeedForm {
  name: String,
  url: String
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

#[get("/login")]
fn login() -> &'static str {
  "Hello, world!"
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


#[post("/feed", data = "<form>")]
async fn add_feed(user: User, db: &State<SqlitePool>, form: Form<FeedForm>) -> Result<Redirect, Status> {
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
async fn delete_feed(user: User, id: i64, db: &State<SqlitePool>) -> Result<Redirect, Status> {
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


#[get("/account")]
fn account() -> &'static str {
  "Hello, world!"
}

// GET /.well-known/webfinger?resource=acct:crimeduo@botsin.space
#[get("/.well-known/webfinger?<resource>")]
async fn lookup_webfinger(resource: &str, db: &State<SqlitePool>) -> Result<String, Status> {
  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  
  // https://github.com/Plume-org/webfinger/blob/main/src/async_resolver.rs
  let mut parsed_query = resource.splitn(2, ':');
  let res_prefix = Prefix::from(parsed_query.next().ok_or(Status::NotFound)?);
  let res = parsed_query.next().ok_or(Status::NotFound)?;
  
  let mut parsed_res = res.splitn(2, '@');
  let user = parsed_res.next().ok_or(Status::NotFound)?;
  let domain = parsed_res.next().ok_or(Status::NotFound)?;
  if domain != instance_domain {
    Err(Status::NotFound)
  } else {
    let userstr = user.to_string();
    print!("{}", userstr);
  
    let feed = Feed::find_by_name(&userstr, db).await;
    match feed {
      Ok(_feed) => Ok(serde_json::to_string(&Webfinger {
        subject: userstr.clone(),
        aliases: vec![userstr.clone()],
        links: vec![Link {
          rel: "http://webfinger.net/rel/profile-page".to_string(),
          mime_type: None,
          href: Some(format!("https://{}/@{}/", instance_domain, userstr)),
          template: None,
        }],
      }).unwrap()),
      Err(_why) => Err(Status::NotFound)
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
      login,
      do_login,
      attempt_login,
      account,
      add_feed,
      delete_feed,
      lookup_webfinger
      ])
    .attach(Template::fairing())
    .launch()
    .await?;
  
  Ok(())
}
