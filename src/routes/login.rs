use actix_web::{get, web, Responder};
use actix_session::Session;

use sqlx::postgres::PgPool;

use crate::{models::user::User, utils::redirect_to};

const INDEX: &str = "/";


#[get("/user/auth/{login_token}")]
pub async fn attempt_login(session: Session, db: web::Data<PgPool>, path: web::Path<String>) -> Result<actix_web::HttpResponse,  actix_web::error::Error> {
  let db = db.as_ref();
  let login_token = path.into_inner();
  let user = User::find_by_login(&login_token, db).await;
  match user {
    Ok(user) => {
      if user.is_some() {
        let user = user.unwrap();
        let token = user.apply_access_token(db).await;
        match token {
          Ok(token) => {
            // @todo ensure this worked
            let _result = session.insert("access_token", token);

            Ok(crate::utils::redirect_to("/"))
          },
          Err(why) => {
            println!("session error!");
            log::info!("{why}");
            Ok(crate::utils::redirect_to(INDEX))
          }
        }
      }
      else {
        println!("no user!");
        Ok(crate::utils::redirect_to(INDEX))
      } 
    },
    Err(why) => {
      println!("error!");

      log::info!("{why}");
      Ok(crate::utils::redirect_to(INDEX))

    }
  }
}

#[get("/user/logout")]
pub async fn do_logout(session: Session) -> impl Responder {
  session.purge();
  redirect_to(INDEX)
}

