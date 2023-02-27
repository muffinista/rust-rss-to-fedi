use fang::async_trait;
use fang::asynk::async_queue::AsyncQueueable;
use fang::serde::{Deserialize, Serialize};
use fang::typetag;
use fang::AsyncRunnable;
use fang::FangError;

use crate::models::Feed;
use crate::models::Item;
use crate::models::Follower;

use crate::utils::pool::worker_db_pool;


#[derive(Serialize, Deserialize)]
#[serde(crate = "fang::serde")]
pub struct DeliverItem {
  pub feed_id: i32,
  pub item_id: i32,
  pub follower_id: i32,
}

impl DeliverItem {
  pub fn new(feed_id: i32, item_id: i32, follower_id: i32) -> Self {
    Self { feed_id, item_id, follower_id }
  }
}


#[async_trait]
#[typetag::serde]
impl AsyncRunnable for DeliverItem {
  async fn run(&self, _queue: &mut dyn AsyncQueueable) -> Result<(), FangError> {
    let pool = worker_db_pool().await;

    let feed = Feed::find(self.feed_id, &pool).await;
    match feed {
      Ok(feed) => {
        let item = Item::find(self.item_id, &pool).await;
        match item {
          Ok(item) => {
            let follower = Follower::find(self.follower_id, &pool).await;
            match follower {
              Ok(follower) => {
                if follower.is_none() {
                  return Ok(())
                }
                let follower = follower.unwrap();
                let result = item.deliver_to(&follower, &feed, &pool).await;
                match result {
                  Ok(_result) => (),
                  Err(why) => {
                    println!("{why:}");
                  }
                }
              },
              Err(why) => {
                println!("Something went wrong: {why:}");
              }   
            }
    
          },
          Err(why) => {
            println!("Something went wrong: {why:}");
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