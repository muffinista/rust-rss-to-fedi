use sqlx::postgres::PgPool;
use fang::FangError;
use crate::models::{
  Message
};


pub async fn cleanup_messages(pool: &PgPool) -> Result<(), FangError> {
  let result = Message::cleanup(pool, 10000, 10000).await;
  match result {
    Ok(result) => Ok(result),
    Err(err) => {
      let description = format!("{err:?}");

      Err(FangError { description })
    }
  }
}


pub async fn cleanup_actors(pool: &PgPool) -> Result<(), FangError> {
  let result = sqlx::query!("DELETE FROM followers WHERE id IN (SELECT followers.id from followers inner join actors on followers.actor = actors.url where actors.error_count > 10)")
    .execute(pool)
    .await;

    match result {
    Ok(_result) => {
      let actor_result = sqlx::query!("DELETE FROM actors WHERE error_count > 10")
      .execute(pool)
      .await;

      match actor_result {
        Ok(_actor_result) => Ok(()),
        Err(err) => {
          let description = format!("{err:?}");
    
          Err(FangError { description })
        }
      }
    
    },
    Err(err) => {
      let description = format!("{err:?}");

      Err(FangError { description })
    }
  }
}

