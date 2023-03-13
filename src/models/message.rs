use sqlx::postgres::PgPool;

use chrono::Utc;


///
/// Model for an incoming message
///
#[derive(Debug)]
pub struct Message {
  pub id: i32,
  pub username: String,
  pub text: String,
  pub actor: Option<String>,
  pub error: Option<String>,
  pub handled: bool,
  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>
}

impl PartialEq for Message {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl Message {
  pub async fn find(id: i32, pool: &PgPool) -> Result<Option<Message>, sqlx::Error> {
    sqlx::query_as!(Message, "SELECT * FROM messages WHERE id = $1", id)
      .fetch_optional(pool)
      .await
  }

  pub async fn log(username: &String, text: &String, actor: Option<String>, error: Option<String>,  handled: bool, pool: &PgPool) -> Result<(), sqlx::Error> {
    let now = Utc::now();

    sqlx::query!("INSERT INTO messages 
      (username, text, actor, error, handled, created_at, updated_at)
      VALUES ($1, $2, $3, $4, $5, $6, $7)",
      username, text, actor, error, handled, now, now)
      .execute(pool)
      .await?;

    Ok(())

  }
}
