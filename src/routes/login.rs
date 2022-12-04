use rocket::{FromForm, get, post};
use rocket::form::Form;
use rocket::http::Status;
use rocket::response::Redirect;
use rocket::State;
use rocket::http::{Cookie, CookieJar};

use sqlx::sqlite::SqlitePool;

use crate::user::User;

#[derive(FromForm)]
pub struct LoginForm {
  email: String
}


#[get("/user/auth/<login_token>")]
pub async fn attempt_login(db: &State<SqlitePool>, cookies: &CookieJar<'_>, login_token: &str) -> Result<Redirect, Status> {
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
pub async fn do_login(db: &State<SqlitePool>, form: Form<LoginForm>) -> Result<Redirect, Status> {
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

