use fang::async_trait;
use fang::asynk::async_queue::AsyncQueueable;
use fang::serde::{Deserialize, Serialize};
use fang::typetag;
use fang::AsyncRunnable;
use fang::FangError;
use fang::Scheduled;

use crate::models::utils::worker_db_pool;

#[derive(Serialize, Deserialize)]
#[serde(crate = "fang::serde")]
pub struct UpdateStaleFeeds {}

impl UpdateStaleFeeds {
  pub fn new() -> Self {
    Self {}
  }
}


#[async_trait]
#[typetag::serde]
impl AsyncRunnable for UpdateStaleFeeds {
  async fn run(&self, queue: &mut dyn AsyncQueueable) -> Result<(), FangError> {
    let pool = worker_db_pool().await;

    let result = crate::services::loader::update_stale_feeds(&pool, queue).await;
    match result {
      Ok(_result) => {
        println!("It worked!");
      },
      Err(why) => {
        println!("Something went wrong: {:}", why.to_string());
      }
    }
        
    Ok(())
  }

  // If `uniq` is set to true and the task is already in the storage, it won't be inserted again
  // The existing record will be returned for for any insertions operaiton
  fn uniq(&self) -> bool {
    true
  }

  // This will be useful if you would like to schedule tasks.
  // default value is None (the task is not scheduled, it's just executed as soon as it's inserted)
  fn cron(&self) -> Option<Scheduled> {
      let expression = "0 * * * * *";
      Some(Scheduled::CronPattern(expression.to_string()))
  }

  // the maximum number of retries. Set it to 0 to make it not retriable
  // the default value is 20
  fn max_retries(&self) -> i32 {
    1
    // 20
  }

  // // backoff mode for retries
  // fn backoff(&self, attempt: u32) -> u32 {
  //   u32::pow(2, attempt)
  // }

}
