use sqlx::postgres::PgPool;
use rand::{distributions::Alphanumeric, Rng};

use rocket::request::{self, FromRequest, Request};
use rocket::outcome::{Outcome};

use rocket::uri;

use crate::routes::login::*;
use crate::utils::utils::*;

use rocket_dyn_templates::tera::Tera;
use rocket_dyn_templates::tera::Context;

use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

use anyhow::anyhow;
use anyhow::Error as AnyError;

use std::env;

use chrono::Utc;

#[derive(Debug)]
pub struct User {
  pub id: i32,
  pub email: Option<String>,
  pub actor_url: Option<String>,
  pub login_token: String,
  pub access_token: Option<String>,
  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>
  
}

impl PartialEq for User {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl User {
  ///
  /// Find user by ID. This assumes that the user exists!
  ///
  pub async fn find(id: i32, pool: &PgPool) -> Result<User, sqlx::Error> {
    sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
    .fetch_one(pool)
    .await
  }
  
  ///
  /// Find user by email
  ///
  pub async fn find_by_email(email: &String, pool: &PgPool) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(User, "SELECT * FROM users WHERE email = $1", email)
    .fetch_optional(pool)
    .await
  }
  
  ///
  /// Find user by email
  ///
  pub async fn find_by_actor_url(actor_url: &String, pool: &PgPool) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(User, "SELECT * FROM users WHERE actor_url = $1", actor_url)
    .fetch_optional(pool)
    .await
  }
  

  ///
  /// Find user by login
  ///
  pub async fn find_by_login(token: &String, pool: &PgPool) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(User, "SELECT * FROM users WHERE login_token = $1", token)
    .fetch_optional(pool)
    .await
  }
  
  ///
  /// Find user by access token
  ///
  pub async fn find_by_access(token: &String, pool: &PgPool) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(User, "SELECT * FROM users WHERE access_token = $1", token)
    .fetch_optional(pool)
    .await
  }
  
  ///
  /// generate and apply access token to the current object
  ///
  pub async fn apply_access_token(&self, pool: &PgPool) -> Result<String, sqlx::Error> {
    let token = User::generate_access_token();
    let query_check = sqlx::query!(
      "UPDATE users SET access_token = $1 WHERE id = $2", token, self.id)
      .execute(pool)
      .await;
      
    match query_check {
      Ok(_q) => return Ok(token),
      Err(why) => return Err(why)
    }
  }

  ///
  /// create a user with the given email address
  ///
  pub async fn create_by_email(email: &String, pool: &PgPool) -> Result<User, sqlx::Error> {
    let token = User::generate_login_token();
    let now = Utc::now();

    let user_id = sqlx::query!(
      "INSERT INTO users (email, login_token, created_at, updated_at)
      VALUES($1, $2, $3, $4)
      RETURNING id", email, token, now, now)
      .fetch_one(pool)
      .await?
      .id;
      
    User::find(user_id, pool).await
  }
    
  ///
  /// look for a user with the given email address. if none exists, create one
  ///
  pub async fn find_or_create_by_email(email: &String, pool: &PgPool) -> Result<User, sqlx::Error> {
    let user_check = sqlx::query_as!(User, "SELECT * FROM users WHERE email = $1", email)
    .fetch_one(pool)
    .await;
    
    match user_check {
      Ok(user) => return Ok(user),
      _ => return User::create_by_email(email, pool).await
    }
  }      

  ///
  /// create a user with the given URL
  ///
  pub async fn create_by_actor_url(actor_url: &String, pool: &PgPool) -> Result<User, sqlx::Error> {
    let token = User::generate_login_token();
    let now = Utc::now();

    let user_id = sqlx::query!(
      "INSERT INTO users (actor_url, login_token, created_at, updated_at)
      VALUES($1, $2, $3, $4)
      RETURNING id", actor_url, token, now, now)
      .fetch_one(pool)
      .await?
      .id;
      
    User::find(user_id, pool).await
  }

  ///
  /// look for a user with the given URL. if none exists, create one
  ///
  pub async fn find_or_create_by_actor_url(actor_url: &String, pool: &PgPool) -> Result<User, sqlx::Error> {
    let user_check = sqlx::query_as!(User, "SELECT * FROM users WHERE actor_url = $1", actor_url)
    .fetch_one(pool)
    .await;
    
    match user_check {
      Ok(user) => return Ok(user),
      _ => return User::create_by_actor_url(actor_url, pool).await
    }
  }

  pub fn generate_login_token() -> String {
    rand::thread_rng()
    .sample_iter(&Alphanumeric)
    .take(40)
    .map(char::from)
    .collect()
  }
  
  pub fn generate_access_token() -> String {
    rand::thread_rng()
    .sample_iter(&Alphanumeric)
    .take(40)
    .map(char::from)
    .collect()    
  }

  //
  // @todo add some code here to prevent abuse
  //
  pub fn should_send_login_email(&self) -> bool {
    !env::var("DISABLE_EMAIL").is_ok()
  }

  pub fn send_login_email(&self) -> Result<(), AnyError> {
    let auth_url = path_to_url(&uri!(attempt_login(&self.login_token)));
    println!("{:?}", auth_url);

    let tera = match Tera::new("templates/email/*.*") {
      Ok(t) => t,
      Err(e) => {
        println!("Parsing error(s): {}", e);
        ::std::process::exit(1);
      }
    };
    
    let mut context = Context::new();
    context.insert("link", &auth_url);
    
    let body = tera.render("send-login.text.tera", &context).unwrap();

    println!("{:}", body);

    if !env::var("SMTP_USERNAME").is_ok() ||
      !env::var("SMTP_PASSWORD").is_ok() ||
      !env::var("SMTP_HOST").is_ok() ||
      !env::var("SMTP_FROM").is_ok() {
      println!("Not sending mail because env vars are missing");
      return Ok(())        
    }

    let smtp_username = env::var("SMTP_USERNAME").expect("SMTP_USERNAME is not set");
    let smtp_password = env::var("SMTP_PASSWORD").expect("SMTP_PASSWORD is not set");
    let smtp_host = env::var("SMTP_HOST").expect("SMTP_HOST is not set");
    let mail_from = env::var("SMTP_FROM").expect("SMTP_FROM is not set");
    let target = self.email.clone().unwrap();

    let email = Message::builder()
      .from(mail_from.parse().unwrap()) // "NoBody <nobody@domain.tld>"
      .to(target.parse().unwrap())
      .subject("Your login email")
      .body(body)
      .unwrap();

    let creds = Credentials::new(smtp_username, smtp_password);

    // Open a remote connection to gmail
    let mailer = SmtpTransport::relay(&smtp_host)
        .unwrap()
        .credentials(creds)
        .build();

    // Send the email
    match mailer.send(&email) {
      Ok(_) => Ok(()),
      Err(e) => Err(anyhow!(format!("Could not send email: {:?}", e))),
    }
  }
}


#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
  type Error = std::convert::Infallible;
  
  async fn from_request(request: &'r Request<'_>) -> request::Outcome<User, Self::Error> {
    let pool = request.rocket().state::<PgPool>().unwrap();
    let cookie = request.cookies().get_private("access_token");
    
    match cookie {
      Some(cookie) => {
        let access_token = cookie.value();
        let user = User::find_by_access(&access_token.to_string(), &pool).await;
        match user {
          Ok(user) => {
            if user.is_some() {
              Outcome::Success(user.unwrap())
            }
            else {
              Outcome::Forward(())
            }
          },
          Err(_why) => Outcome::Forward(())
        }
      },
      None => {
        return Outcome::Forward(())
      }
    }
  }
}
    
#[cfg(test)]
mod test {
  use sqlx::postgres::PgPool;
  use crate::models::user::User;

  #[sqlx::test]
  async fn test_find_or_create_by_email(pool: PgPool) -> sqlx::Result<()> {
    let email:String = "foo@bar.com".to_string();
    let user = User::find_or_create_by_email(&email, &pool).await?;
    
    assert_eq!(user.email, email);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_find_by_login_token(pool: PgPool) -> sqlx::Result<()> {
    let email:String = "foo@bar.com".to_string();
    let user = User::find_or_create_by_email(&email, &pool).await?;
    let user_find = User::find_by_login(&user.login_token.to_string(), &pool).await?.unwrap();
    
    assert_eq!(user, user_find);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_find_by_access(pool: PgPool) -> sqlx::Result<()> {
    let email:String = "foo@bar.com".to_string();
    let user = User::find_or_create_by_email(&email, &pool).await?;
    let access_token = user.apply_access_token(&pool).await.unwrap().to_string();

    let user_find = User::find_by_access(&access_token, &pool).await?.unwrap();
    assert_eq!(user, user_find);
    
    let bad_token = format!("dfdfs{}fdsdf", access_token);
    let no_user = User::find_by_access(&bad_token, &pool).await;
    assert_eq!(false, no_user.unwrap().is_some());
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_find_by_email(pool: PgPool) -> sqlx::Result<()> {
    let email:String = "foo@bar.com".to_string();
    let user = User::find_or_create_by_email(&email, &pool).await?;
    let user_find = User::find_by_email(&user.email, &pool).await?.unwrap();
    
    assert_eq!(user, user_find);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_find_by_email_doesnt_exist(pool: PgPool) -> sqlx::Result<()> {
    let lookup:String = ("bar@baz.com").to_string();
    let user = User::find_by_email(&lookup, &pool).await;
    assert_eq!(false, user.unwrap().is_some());
    
    Ok(())
  }
}
