use sqlx::postgres::PgPool;
use rand::{distributions::Alphanumeric, Rng};

use chrono::{Duration, Utc};

use crate:: {
  models:: {
    Actor,
    Feed
  },
  services::mailer::deliver_to_inbox,
  DeliveryError
};

use url::Url;

#[derive(Debug)]
pub struct User {
  pub id: i32,
  pub email: Option<String>,
  pub actor_url: Option<String>,
  pub login_token: String,
  pub access_token: Option<String>,
  pub username: Option<String>,
  pub admin: bool,

  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>,
  pub login_token_updated_at: chrono::DateTime::<Utc>,
  pub access_token_updated_at: chrono::DateTime::<Utc>,
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
  /// Find the 'admin' user. This is a special feed that will be used to
  /// send messages, handle authentications, etc
  ///
  pub async fn for_admin(pool: &PgPool) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(User, "SELECT users.* FROM users INNER JOIN feeds ON users.id = feeds.user_id WHERE feeds.admin = true LIMIT 1")
    .fetch_optional(pool)
    .await
  }


  // ///
  // /// Find user by email
  // ///
  // pub async fn find_by_email(email: &String, pool: &PgPool) -> Result<Option<User>, sqlx::Error> {
  //   sqlx::query_as!(User, "SELECT * FROM users WHERE email = $1", email)
  //   .fetch_optional(pool)
  //   .await
  // }
  
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
    log::debug!("Find user: {token:}");
    sqlx::query_as!(User, "SELECT * FROM users WHERE access_token = $1", token)
      .fetch_optional(pool)
      .await
  }

  pub async fn reset_login_token(&self, pool: &PgPool) -> Result<String, sqlx::Error> {
    let token = User::generate_login_token();
    let query_check = sqlx::query!(
      "UPDATE users SET login_token = $1 WHERE id = $2", token, self.id)
      .execute(pool)
      .await;
      
    match query_check {
      Ok(_q) => Ok(token),
      Err(why) => Err(why)
    }
  }
  
  pub fn needs_new_access_token(&self) -> bool {
    if self.access_token.is_none() {
      return true;
    }

    let now = Utc::now();
    let diff = now.signed_duration_since(self.access_token_updated_at);
    println!("Total time: {}", diff.num_minutes());

    diff > Duration::hours(24)
  }

  ///
  /// generate and apply access token to the current object
  ///
  pub async fn apply_access_token(&self, pool: &PgPool) -> Result<String, sqlx::Error> {
    if self.needs_new_access_token() {
      let token = User::generate_access_token();
      // log::info!("generate token: {token:}");
  
      let query_check = sqlx::query!(
        "UPDATE users SET access_token = $1, access_token_updated_at = NOW() WHERE id = $2", token, self.id)
        .execute(pool)
        .await;
        
      match query_check {
        Ok(_q) => Ok(token),
        Err(why) => Err(why)
      }  
    } else {
      Ok(self.access_token.clone().unwrap())
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
      Ok(user) => Ok(user),
      _ => User::create_by_email(email, pool).await
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
      Ok(user) => Ok(user),
      _ => User::create_by_actor_url(actor_url, pool).await
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

  pub fn is_admin(&self) -> bool {
    self.admin
  }

  ///
  /// update user record with a few things from their actor
  ///
  pub async fn apply_actor(&self, actor: &Actor, pool: &PgPool) -> Result<(), sqlx::Error> {
    let query = sqlx::query!(
      "UPDATE users SET username = $1 WHERE id = $2", actor.username, self.id)
      .execute(pool)
      .await;
      
    match query {
      Ok(_q) => Ok(()),
      Err(why) => Err(why)
    }
  }

  pub fn full_username(&self) -> Option<String> {
    if self.username.is_none() || self.actor_url.is_none() {
      return None
    };

    let url = Url::parse(self.actor_url.as_ref().unwrap()).unwrap();
    let domain = url.host().unwrap();

    Some(format!("{}@{}", &self.username.as_ref().unwrap(), domain))
  }


  pub async fn send_link_to_feed(&self, feed: &Feed, pool: &PgPool) -> Result<(), DeliveryError> {
    let dest_actor = Actor::find_or_fetch(self.actor_url.as_ref().expect("No actor url!"), pool).await;

    match dest_actor {
      Ok(dest_actor) => {
        if dest_actor.is_none() {
          return Ok(());
        }
        let dest_actor = dest_actor.unwrap();

        let message = feed.link_to_feed_message(&dest_actor).await?;
        let msg = serde_json::to_string(&message).unwrap();
        log::debug!("{msg}");
    
        let feed_ap_url = feed.ap_url();
        let inbox = &Url::parse(&dest_actor.inbox_url)?;
    
        let result = deliver_to_inbox(
          inbox,
          &feed_ap_url,
          &feed.private_key,
          &message).await;
    
        match result {
          Ok(result) => log::debug!("sent! {result:?}"),
          Err(why) => {
            Actor::log_error(&inbox.to_string(), pool).await?;
            log::info!("failure! {why:?}")
          }
        }
      },
      Err(why) => {
        log::debug!("couldnt find actor: {why:?}");
      }
    }

    Ok(())
  }
}

   
#[cfg(test)]
mod test {
  use sqlx::postgres::PgPool;
  use crate::models::User;

  #[sqlx::test]
  async fn test_find_or_create_by_email(pool: PgPool) -> sqlx::Result<()> {
    let email:String = "foo@bar.com".to_string();
    let user = User::find_or_create_by_email(&email, &pool).await?;
    
    assert_eq!(user.email.unwrap(), email);
    
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
  async fn test_doesnt_regenerate_access_token_too_soon(pool: PgPool) -> sqlx::Result<()> {
    let email:String = "foo@bar.com".to_string();
    let user = User::find_or_create_by_email(&email, &pool).await?;
    let access_token = user.apply_access_token(&pool).await.unwrap().to_string();

    let user_2 = User::find_or_create_by_email(&email, &pool).await?;
    let access_token_2 = user_2.apply_access_token(&pool).await.unwrap().to_string();

    assert_eq!(access_token, access_token_2);
    
    Ok(())
  }
}
