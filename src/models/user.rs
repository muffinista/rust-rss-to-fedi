use sqlx::sqlite::SqlitePool;
use rand::{distributions::Alphanumeric, Rng};

#[derive(Debug)]
pub struct User {
  pub id: i64,
  pub email: String,
  pub login_token: String,
  pub access_token: Option<String>,
  pub created_at: chrono::NaiveDateTime,
  pub updated_at: chrono::NaiveDateTime
  
}

impl PartialEq for User {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl User {
  pub async fn find(id: i64, pool: &SqlitePool) -> Result<User, sqlx::Error> {
    sqlx::query_as!(User, "SELECT * FROM users WHERE id = ?", id)
    .fetch_one(pool)
    .await
  }
  
  pub async fn find_by_email(email: &String, pool: &SqlitePool) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(User, "SELECT * FROM users WHERE email = ?", email)
    .fetch_optional(pool)
    .await
  }
  
  pub async fn find_by_login(token: &String, pool: &SqlitePool) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(User, "SELECT * FROM users WHERE login_token = ?", token)
    .fetch_optional(pool)
    .await
  }
  
  pub async fn find_by_access(token: &String, pool: &SqlitePool) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(User, "SELECT * FROM users WHERE access_token = ?", token)
    .fetch_optional(pool)
    .await
  }
  
  pub async fn apply_access_token(&self, pool: &SqlitePool) -> Result<String, sqlx::Error> {
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
    
    pub async fn create_by_email(email: &String, pool: &SqlitePool) -> Result<User, sqlx::Error> {
      let token = User::generate_login_token();
      let user_id = sqlx::query!(
        "INSERT INTO users (email, login_token, created_at, updated_at)
        VALUES($1, $2, datetime(CURRENT_TIMESTAMP, 'utc'), datetime(CURRENT_TIMESTAMP, 'utc'))", email, token)
        .execute(pool)
        .await?
        .last_insert_rowid();
        
        User::find(user_id, pool).await
      }
      
      pub async fn find_or_create_by_email(email: &String, pool: &SqlitePool) -> Result<User, sqlx::Error> {
        let user_check = sqlx::query_as!(User, "SELECT * FROM users WHERE email = ?", email)
        .fetch_one(pool)
        .await;
        
        match user_check {
          Ok(user) => return Ok(user),
          _ => return User::create_by_email(email, pool).await
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
    }
    
    use rocket::request::{self, FromRequest, Request};
    use rocket::outcome::{Outcome};
    
    #[rocket::async_trait]
    impl<'r> FromRequest<'r> for User {
      type Error = std::convert::Infallible;
      
      async fn from_request(request: &'r Request<'_>) -> request::Outcome<User, Self::Error> {
        let pool = request.rocket().state::<SqlitePool>().unwrap();
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
    
    
    #[sqlx::test]
    async fn test_find_or_create_by_email(pool: SqlitePool) -> sqlx::Result<()> {
      let email:String = "foo@bar.com".to_string();
      let user = User::find_or_create_by_email(&email, &pool).await?;
      
      assert_eq!(user.email, email);
      
      Ok(())
    }
    
    #[sqlx::test]
    async fn test_find_by_login_token(pool: SqlitePool) -> sqlx::Result<()> {
      let email:String = "foo@bar.com".to_string();
      let user = User::find_or_create_by_email(&email, &pool).await?;
      let user_find = User::find_by_login(&user.login_token.to_string(), &pool).await?.unwrap();
      
      assert_eq!(user, user_find);
      
      Ok(())
    }
    
    #[sqlx::test]
    async fn test_find_by_access(pool: SqlitePool) -> sqlx::Result<()> {
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
    async fn test_find_by_email(pool: SqlitePool) -> sqlx::Result<()> {
      let email:String = "foo@bar.com".to_string();
      let user = User::find_or_create_by_email(&email, &pool).await?;
      let user_find = User::find_by_email(&user.email, &pool).await?.unwrap();
      
      assert_eq!(user, user_find);
      
      Ok(())
    }
    
    #[sqlx::test]
    async fn test_find_by_email_doesnt_exist(pool: SqlitePool) -> sqlx::Result<()> {
      let lookup:String = ("bar@baz.com").to_string();
      let user = User::find_by_email(&lookup, &pool).await;
      assert_eq!(false, user.unwrap().is_some());
      
      Ok(())
    }
    