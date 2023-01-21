use rocket::{FromForm, get, post};
use rocket::form::Form;
use rocket::http::Status;
use rocket::response::Redirect;
use rocket::State;
use rocket::http::{Cookie, CookieJar};
use rocket::uri;
use rocket_dyn_templates::{Template, context};

use sqlx::sqlite::SqlitePool;


use crate::models::user::User;

#[derive(FromForm)]
pub struct LoginForm {
  email: String
}


#[get("/user/auth/<login_token>")]
pub async fn attempt_login(db: &State<SqlitePool>, cookies: &CookieJar<'_>, login_token: &str) -> Result<Redirect, Status> {
  let user = User::find_by_login(&login_token.to_string(), &db).await;
  
  match user {
    Ok(user) => {
      if user.is_some() {
        let token = user.unwrap().apply_access_token(db).await;
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
      }
      else {
        Err(Status::NotFound)
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
  
  match user {
    Ok(user) => {
      if user.should_send_login_email() {
        let result = user.send_login_email();
        match result {
          Ok(_result) => {
            let dest = uri!(login_result());
            Ok(Redirect::to(dest))
          },
          Err(_why) => {
            let dest = uri!(login_result());
            Ok(Redirect::to(dest))
          }
        }
      } else {
        let dest = uri!(login_result());
        Ok(Redirect::to(dest))
      }
    },
    Err(why) => {
      print!("{}", why);
      Err(Status::NotFound)
    }
  }
}

#[get("/login/results")]
pub async fn login_result() -> Template {
  Template::render("login-after", context! { logged_in: false })
}
