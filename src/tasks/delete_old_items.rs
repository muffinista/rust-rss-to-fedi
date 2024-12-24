use fang::async_trait;
use fang::asynk::async_queue::AsyncQueueable;
use fang::serde::{Deserialize, Serialize};
use fang::typetag;
use fang::AsyncRunnable;
use fang::FangError;
use fang::Scheduled;

use crate::utils::pool::db_pool;


#[derive(Serialize, Deserialize)]
#[serde(crate = "fang::serde")]
pub struct DeleteOldItems {}

impl DeleteOldItems {
  pub fn new() -> Self {
    Self {}
  }
}

impl Default for DeleteOldItems {
  fn default() -> Self {
    Self::new()
  }
}


#[async_trait]
#[typetag::serde]
impl AsyncRunnable for DeleteOldItems {
  async fn run(&self, _queue: &mut dyn AsyncQueueable) -> Result<(), FangError> {
			let pool = db_pool().await;
			let result = crate::services::cleanup::cleanup_items(&pool).await;

			match result {
					Ok(result) => Ok(result),
					Err(_why) => {
							log::info!("DeleteOldItems failed!!");
							Err(FangError { description: "DeleteOldItems".to_string() })
					}   
			}
  }

  // If `uniq` is set to true and the task is already in the storage, it won't be inserted again
  // The existing record will be returned for for any insertions operation
  fn uniq(&self) -> bool {
    true
  }

  // This will be useful if you would like to schedule tasks.
  // default value is None (the task is not scheduled, it's just executed as soon as it's inserted)
  fn cron(&self) -> Option<Scheduled> {
      let expression = "* 0 * * * *";
      Some(Scheduled::CronPattern(expression.to_string()))
  }

  // the maximum number of retries. Set it to 0 to make it not retriable
  // the default value is 20
  fn max_retries(&self) -> i32 {
    0
  }
}
