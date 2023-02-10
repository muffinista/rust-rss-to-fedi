use fang::async_trait;
use fang::asynk::async_queue::AsyncQueueable;
use fang::serde::{Deserialize, Serialize};
use fang::typetag;
use fang::AsyncRunnable;
use fang::FangError;

use std::env;

use crate::models::Feed;

use sqlx::postgres::PgPoolOptions;


#[derive(Serialize, Deserialize)]
#[serde(crate = "fang::serde")]
pub struct RefreshFeed {
  pub id: i32,
}

impl RefreshFeed {
  pub fn new(id: i32) -> Self {
    Self { id }
  }
}


#[async_trait]
#[typetag::serde]
impl AsyncRunnable for RefreshFeed {
  async fn run(&self, queue: &mut dyn AsyncQueueable) -> Result<(), FangError> {
    let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");

    let pool = PgPoolOptions::new()
      .max_connections(5)
      .connect(&db_uri)
      .await
      .expect("Failed to create pool");

    let feed = Feed::find(self.id, &pool).await;
    match feed {
      Ok(mut feed) => {
        let result = feed.refresh(&pool, queue).await;
        match result {
          Ok(_result) => { println!("Done refreshing feeds"); },
          Err(why) => {
            println!("{:}", why);
          }
        }
      },
      Err(why) => {
        println!("{:}", why);
      }
    }
    
    Ok(())
  }

  // the maximum number of retries. Set it to 0 to make it not retriable
  // the default value is 20
  fn max_retries(&self) -> i32 {
    4
  }

  // backoff mode for retries
  fn backoff(&self, attempt: u32) -> u32 {
    u32::pow(2, attempt)
  }

}
