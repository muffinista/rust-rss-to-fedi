use sqlx::postgres::PgPool;

use chrono::{Duration, Utc};

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

  pub async fn cleanup(pool: &PgPool, age:i64, limit: i64) -> Result<(), sqlx::Error> {
    let age = Utc::now() - Duration::seconds(age);
      
    sqlx::query!("DELETE FROM messages WHERE id IN (select id FROM messages WHERE created_at <= $1 ORDER BY created_at LIMIT $2)", age, limit)
        .execute(pool)
        .await?;

    Ok(())
  }

}


#[cfg(test)]
mod test {
		use sqlx::postgres::PgPool;
		use chrono::{Duration, Utc};
		use crate::models::Message;

  #[sqlx::test]
  async fn test_cleanup(pool: PgPool) -> Result<(), String> {
			let old = Utc::now() - Duration::seconds(10000);
			
			sqlx::query!("INSERT INTO messages (username, text, handled, created_at, updated_at) VALUES('test', 'test', true, $1, $2)", old, old)
          .execute(&pool)
          .await
          .unwrap();

      
      let result = sqlx::query!("SELECT COUNT(1) AS tally FROM messages")
          .fetch_one(&pool)
          .await
      .unwrap();

			assert!(result.tally.unwrap() == 1);

			Message::cleanup(&pool, 100, 10000).await.unwrap();
			
			let post_result = sqlx::query!("SELECT COUNT(1) AS tally FROM messages")
					.fetch_one(&pool)
					.await
					.unwrap();
			
			assert!(post_result.tally.unwrap() == 0);
      
			Ok(())
  }
}
