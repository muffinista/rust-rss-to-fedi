use sqlx::postgres::PgPool;
use fang::FangError;
use crate::models::{Item, Message};


use std::{
  env,
  str::FromStr
};

const ACTOR_ERROR_COUNT: i32 = 10;

pub async fn cleanup_messages(pool: &PgPool) -> Result<(), FangError> {
  let result = Message::cleanup(pool, 365, 10000).await;
  match result {
    Ok(result) => Ok(result),
    Err(err) => {
      let description = format!("{err:?}");

      Err(FangError { description })
    }
  }
}

pub async fn cleanup_items(pool: &PgPool) -> Result<(), FangError> {
  let result = Item::cleanup(pool, 10000, 10000).await;
  match result {
    Ok(result) => Ok(result),
    Err(err) => {
      let description = format!("{err:?}");

      Err(FangError { description })
    }
  }
}


pub fn actor_max_error_count() -> i32 {
  match env::var_os("ACTOR_ERROR_COUNT") {
    Some(val) => {
      i32::from_str(&val.into_string().expect("Something went wrong setting the actor error count")).unwrap()
    }
    None => ACTOR_ERROR_COUNT
  }
}

pub async fn cleanup_actors(pool: &PgPool) -> Result<(), FangError> {
  let count = actor_max_error_count();

  let result = sqlx::query!("DELETE FROM followers WHERE id IN (SELECT followers.id from followers inner join actors on followers.actor = actors.url where actors.error_count > $1)", count)
    .execute(pool)
    .await;

    match result {
    Ok(_result) => {
      let actor_result = sqlx::query!("DELETE FROM actors WHERE error_count > $1", count)
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


#[cfg(test)]
mod test {
		use sqlx::postgres::PgPool;
		use chrono::{Duration, Utc};
		use crate::models::{Feed, Item, Message};
    use crate::utils::test_helpers::{real_feed, real_item};

  #[sqlx::test]
  async fn test_cleanup_messages(pool: PgPool) -> Result<(), String> {
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

  #[sqlx::test]
  async fn test_cleanup_items(pool: PgPool) -> sqlx::Result<()> {
      let feed: Feed = real_feed(&pool).await?;
      let item: Item = real_item(&feed, &pool).await?;
      let ts = chrono::Utc::now() - chrono::Duration::days(120);

      let _ = sqlx::query!("UPDATE items set created_at = $1 WHERE id = $2", ts, item.id)
        .execute(&pool)
        .await;


      let result = sqlx::query!("SELECT COUNT(1) AS tally FROM items")
          .fetch_one(&pool)
          .await
      .unwrap();

      assert!(result.tally.unwrap() == 1);

      Item::cleanup(&pool, 1, 10000).await.unwrap();
      
      let post_result = sqlx::query!("SELECT COUNT(1) AS tally FROM items")
          .fetch_one(&pool)
          .await
          .unwrap();
      
      assert!(post_result.tally.unwrap() == 0);
      
      Ok(())
  }

  #[sqlx::test]
  async fn test_cleanup_actors(pool: PgPool) -> Result<(), String> {
      let old = Utc::now() - Duration::seconds(10000);


      sqlx::query!("INSERT INTO actors (url, inbox_url, username, error_count, public_key_id, public_key, refreshed_at, created_at, updated_at) 
        VALUES('test', 'test', 'test', 100, 'foo', 'foo', $1, $2, $3)", old, old, old)
          .execute(&pool)
          .await
          .unwrap();

      
      let result = sqlx::query!("SELECT COUNT(1) AS tally FROM actors")
          .fetch_one(&pool)
          .await
      .unwrap();

      assert!(result.tally.unwrap() == 1);

      crate::services::cleanup::cleanup_actors(&pool).await.unwrap();
      
      let post_result = sqlx::query!("SELECT COUNT(1) AS tally FROM actors")
          .fetch_one(&pool)
          .await
          .unwrap();
      
      assert!(post_result.tally.unwrap() == 0);
      
      Ok(())
  }
}