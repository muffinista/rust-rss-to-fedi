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
pub struct DeleteBadActors {}

impl DeleteBadActors {
  pub fn new() -> Self {
    Self {}
  }
}

impl Default for DeleteBadActors {
  fn default() -> Self {
    Self::new()
  }
}


#[async_trait]
#[typetag::serde]
impl AsyncRunnable for DeleteBadActors {
  async fn run(&self, _queue: &mut dyn AsyncQueueable) -> Result<(), FangError> {
    let pool = db_pool().await;
    crate::services::cleanup::cleanup_actors(&pool).await?;

    Ok(())
  }

  // If `uniq` is set to true and the task is already in the storage, it won't be inserted again
  // The existing record will be returned for for any insertions operaiton
  fn uniq(&self) -> bool {
    true
  }

  fn cron(&self) -> Option<Scheduled> {
      let expression = "0 0 * * * *";
      Some(Scheduled::CronPattern(expression.to_string()))
  }

  // the maximum number of retries. Set it to 0 to make it not retriable
  // the default value is 20
  fn max_retries(&self) -> i32 {
    1
  }
}
