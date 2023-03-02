use fang::async_trait;
use fang::asynk::async_queue::AsyncQueueable;
use fang::serde::{Deserialize, Serialize};
use fang::typetag;
use fang::AsyncRunnable;
use fang::FangError;

use url::Url;

use crate::services::mailer::*;
use crate::models::Feed;

use crate::utils::pool::worker_db_pool;


#[derive(Serialize, Deserialize)]
#[serde(crate = "fang::serde")]
pub struct DeliverMessage {
  pub feed_id: i32,
  pub actor_url: String,
  pub message: String
}

impl DeliverMessage {
  pub fn new(feed_id: i32, actor_url: String, message: String) -> Self {
    Self { feed_id, actor_url, message }
  }
}


#[async_trait]
#[typetag::serde]
impl AsyncRunnable for DeliverMessage {
  async fn run(&self, _queue: &mut dyn AsyncQueueable) -> Result<(), FangError> {
    let pool = worker_db_pool().await;
    let feed = Feed::find(self.feed_id, &pool).await;
    match feed {
      Ok(feed) => {
        let dest_url = &Url::parse(&self.actor_url).unwrap();
        let result = deliver_to_inbox(dest_url, &feed.ap_url(), &feed.private_key, &self.message).await;

        match result {
          Ok(result) => {
            println!("sent! {result:?}");
          },
          Err(why) => {
            println!("delivery failure! {why:?}");
          }
        }    
      },
      Err(why) => {
        println!("Something went wrong: {why:}");
      }   
    }

    Ok(())
  }

  // the maximum number of retries. Set it to 0 to make it not retriable
  // the default value is 20
  fn max_retries(&self) -> i32 {
    5
  }

  // backoff mode for retries
  fn backoff(&self, attempt: u32) -> u32 {
    u32::pow(2, attempt)
  }
}