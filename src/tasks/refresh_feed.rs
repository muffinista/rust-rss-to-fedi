use fang::async_trait;
use fang::asynk::async_queue::AsyncQueueable;
use fang::serde::{Deserialize, Serialize};
use fang::typetag;
use fang::AsyncRunnable;
use fang::FangError;

use crate::models::Feed;
use crate::utils::pool::worker_db_pool;


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
    let pool = worker_db_pool().await;

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
