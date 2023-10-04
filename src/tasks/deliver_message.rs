use fang::async_trait;
use fang::asynk::async_queue::AsyncQueueable;
use fang::serde::{Deserialize, Serialize};
use fang::typetag;
use fang::AsyncRunnable;
use fang::FangError;

use url::Url;

use crate::services::mailer::*;
use crate::models::Feed;

use crate::utils::pool::db_pool;
use serde_json::Value;

use tokio::time::timeout;
use std::time::Duration;

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

  async fn job(&self) -> Result<(), FangError> {
    let pool = db_pool().await;
    let feed = Feed::find(self.feed_id, &pool).await;
    match feed {
      Ok(feed) => {
        let dest_url = &Url::parse(&self.actor_url).unwrap();

        // we've gotten a JSON object. We'll deserialize it so we can send something that
        // is serializable to reqwest, since right now we can't manage deserializable objects
        // with fang
        let message_object:Value = serde_json::from_str(&self.message).unwrap();
        let result = deliver_to_inbox(dest_url, &feed.ap_url(), &feed.private_key, &message_object).await;

        match result {
          Ok(_result) => {
            log::info!("DeliverMessage: delivery to {dest_url:} succeeded!");
            Ok(())
          },
          Err(why) => {
            log::info!("DeliverMessage: delivery to {dest_url:} failed: {why:}");
            Err(FangError { description: why.to_string() })
          }
        }    
      },
      Err(why) => {
        log::info!("DeliverMessage failed: {why:}");
        Err(FangError { description: why.to_string() })
      }   
    }
  }
}


#[async_trait]
#[typetag::serde]
impl AsyncRunnable for DeliverMessage {
  async fn run(&self, _queue: &mut dyn AsyncQueueable) -> Result<(), FangError> {
    let result = timeout(Duration::from_secs(crate::JOB_TIMEOUT), self.job()).await;
    match result {
      Ok(_result) => Ok(()),
      Err(why) => {
        log::info!("DeliverMessage: timeout! {why:}");
        Err(FangError { description: why.to_string() })
      }
    }
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



#[cfg(test)]
mod test {
  use fang::asynk::async_queue::AsyncQueue;
  use fang::AsyncRunnable;
  use fang::NoTls;

  use sqlx::postgres::PgPool;
  use std::env;

  use crate::tasks::DeliverMessage;


  #[sqlx::test]
  async fn test_deliver_message_run(_pool: PgPool) {
    let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");

    let msg = DeliverMessage {
      feed_id: 1i32,
      actor_url: "https://muffin.pizza/".to_string(),
      message: "{}".to_string()   
    };

    let mut queue:AsyncQueue<NoTls> = AsyncQueue::builder()
      .uri(db_uri)
      .max_pool_size(1u32)
      .build();

    let result = msg.run(&mut queue).await;
    assert!(result.is_err());
  }
}
