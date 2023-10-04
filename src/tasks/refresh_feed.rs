use fang::async_trait;
use fang::asynk::async_queue::AsyncQueueable;
use fang::serde::{Deserialize, Serialize};
use fang::typetag;
use fang::AsyncRunnable;
use fang::FangError;

use tokio::time::timeout;
use std::time::Duration;

use crate::models::Feed;
use crate::utils::pool::db_pool;


#[derive(Serialize, Deserialize)]
#[serde(crate = "fang::serde")]
pub struct RefreshFeed {
  pub id: i32,
}

impl RefreshFeed {
  pub fn new(id: i32) -> Self {
    Self { id }
  }

  async fn job(&self, queue: &mut dyn AsyncQueueable) -> Result<(), FangError> {
    let pool = db_pool().await;

    let feed = Feed::find(self.id, &pool).await;
    match feed {
      Ok(mut feed) => {
        let result = feed.refresh(&pool, queue).await;
        match result {
          Ok(_result) => { 
            log::info!("RefreshFeed: Done refreshing feed {:}", feed.url);
            Ok(()) 
          },
          Err(why) => {
            log::info!("RefreshFeed: Something went wrong: feed: {:} {why:}", feed.url);
            Err(FangError { description: why.to_string() })
          }
        }
      },
      Err(why) => {
        log::info!("RefreshFeed: Feed missing? {why:}");
        Err(FangError { description: why.to_string() })
      }
    }
  }
}


#[async_trait]
#[typetag::serde]
impl AsyncRunnable for RefreshFeed {
  async fn run(&self, queue: &mut dyn AsyncQueueable) -> Result<(), FangError> {
    let result = timeout(Duration::from_secs(crate::JOB_TIMEOUT), self.job(queue)).await;
    match result {
      Ok(_result) => Ok(()),
      Err(why) => {
        log::info!("RefreshFeed: timeout! {why:}");
        Err(FangError { description: why.to_string() })
      }
    }
  }

  /// Don't retry fetch issues, we'll just try again on the next go around
  fn max_retries(&self) -> i32 {
    0
  }

  // backoff mode for retries
  fn backoff(&self, attempt: u32) -> u32 {
    u32::pow(2, attempt)
  }

}
