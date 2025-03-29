use fang::async_trait;
use fang::asynk::async_queue::AsyncQueueable;
use fang::serde::{Deserialize, Serialize};
use fang::typetag;
use fang::AsyncRunnable;
use fang::FangError;

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
}


#[async_trait]
#[typetag::serde]
impl AsyncRunnable for RefreshFeed {
  async fn run(&self, queue: &mut dyn AsyncQueueable) -> Result<(), FangError> {
    let pool = db_pool().await;
    let tera =
    tera::Tera::new("templates/**/*").expect("Parsing error while loading template folder");

    let feed = Feed::find(self.id, &pool).await;
    match feed {
      Ok(mut feed) => {
        let result = feed.refresh(&pool, &tera, queue).await;
        match result {
          Ok(_result) => { 
            log::info!("RefreshFeed: Done refreshing feed {:}", feed.url);
            Ok(()) 
          },
          Err(why) => {
            println!("AAAA {:?}", why);
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


  /// Don't retry fetch issues, we'll just try again on the next go around
  fn max_retries(&self) -> i32 {
    0
  }

  // backoff mode for retries
  fn backoff(&self, attempt: u32) -> u32 {
    u32::pow(2, attempt)
  }

  // If `uniq` is set to true and the task is already in the storage, it won't be inserted again
  // The existing record will be returned for for any insertions operaiton
  fn uniq(&self) -> bool {
    true
  }
}


#[cfg(test)]
mod test {
  use fang::asynk::async_queue::AsyncQueue;
  use fang::AsyncRunnable;
  use fang::NoTls;

  use std::env;

  use crate::tasks::RefreshFeed;
  use crate::utils::pool::db_pool;
  use crate::utils::test_helpers::real_feed;


  #[sqlx::test]
  async fn test_refresh_feed_run_success() {
    let pool = db_pool().await;
    let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
    let mut server = mockito::Server::new_async().await;
    let mut feed = real_feed(&pool).await.unwrap();

    let url = format!("{}/rss.xml", &server.url()).to_string();
    feed.url = url;
    feed.save(&pool).await.unwrap();

    let path = "fixtures/test_rss.xml";
    let data = std::fs::read_to_string(path).unwrap();

    let m = server.mock("GET", "/rss.xml")
      .with_status(200)
      .with_body(data)
      .create_async()
      .await;

    let msg = RefreshFeed {
      id: feed.id
    };

    let mut queue:AsyncQueue<NoTls> = AsyncQueue::builder()
      .uri(db_uri)
      .max_pool_size(5u32)
      .build();

    let result = msg.run(&mut queue).await;
    assert!(result.is_ok());

    m.assert_async().await;

  }
  #[sqlx::test]
  async fn test_refresh_feed_run_error() {
    let pool = db_pool().await;
    let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
    let mut server = mockito::Server::new_async().await;
    let mut feed = real_feed(&pool).await.unwrap();

    let url = format!("{}/rss.xml", &server.url()).to_string();
    feed.url = url;
    feed.save(&pool).await.unwrap();

    let m = server.mock("GET", "/rss.xml")
      .with_status(404)
      .create_async()
      .await;

    let msg = RefreshFeed {
      id: feed.id
    };

    let mut queue:AsyncQueue<NoTls> = AsyncQueue::builder()
      .uri(db_uri)
      .max_pool_size(5u32)
      .build();

    let result = msg.run(&mut queue).await;
    assert!(!result.is_ok());

    m.assert_async().await;

  }
}
