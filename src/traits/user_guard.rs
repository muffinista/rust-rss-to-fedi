use sqlx::postgres::PgPool;
use crate::models::User;

use rocket::request::{self, FromRequest, Request};
use rocket::outcome::{Outcome};


#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
  type Error = std::convert::Infallible;
  
  async fn from_request(request: &'r Request<'_>) -> request::Outcome<User, Self::Error> {
    let pool = request.rocket().state::<PgPool>().unwrap();
    let cookie = request.cookies().get_private("access_token");
    println!("{:?}", cookie);
    match cookie {
      Some(cookie) => {
        let access_token = cookie.value();
        println!("access token: {:}", access_token);
        let user = User::find_by_access(&access_token.to_string(), &pool).await;

        match user {
          Ok(user) => {
            if user.is_some() {
              println!("found user!");
              Outcome::Success(user.unwrap())
            }
            else {
              println!("no matching using!");
              Outcome::Forward(())
            }
          },
          Err(why) => {
            println!("ERR: {:?}", why);
            Outcome::Forward(())
          }
        }
      },
      None => {
        println!("No cookie to check");
        return Outcome::Forward(())
      }
    }
  }
}
 