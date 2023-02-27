use std::env;

use sqlx::postgres::PgPool;
use crate::models::User;

use rocket::request::{self, FromRequest, Request};
use rocket::outcome::{Outcome};


fn user_to_outcome(user: Result<Option<User>, sqlx::Error>) -> request::Outcome<User, std::convert::Infallible> {
  match user {
    Ok(user) => {
      if let Some(existing_user) = user {
        Outcome::Success(existing_user)
      }
      else {
        Outcome::Forward(())
      }
    },
    Err(why) => {
      println!("ERR: {why:?}");
      Outcome::Forward(())
    }
  }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
  type Error = std::convert::Infallible;
  
  async fn from_request(request: &'r Request<'_>) -> request::Outcome<User, Self::Error> {
    let pool = request.rocket().state::<PgPool>().unwrap();

    if env::var("SINGLE_USER_MODE").is_ok() {
      let user = User::for_admin(pool).await;
      return user_to_outcome(user)
    }

    let cookie = request.cookies().get_private("access_token");

    match cookie {
      Some(cookie) => {
        let access_token = cookie.value();
        // println!("access token: {:}", access_token);
        let user = User::find_by_access(&access_token.to_string(), pool).await;
        user_to_outcome(user)
      },
      None => {
        println!("No cookie to check");
        return Outcome::Forward(())
      }
    }
  }
}
 