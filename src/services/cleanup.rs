use sqlx::postgres::PgPool;
use fang::FangError;
use crate::models::Message;


use std::{
  env,
  str::FromStr
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



const ACTOR_ERROR_COUNT: i32 = 10;

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

