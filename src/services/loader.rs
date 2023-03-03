use sqlx::postgres::PgPool;

use anyhow::{anyhow};
use anyhow::Error as AnyError;

use fang::asynk::async_queue::AsyncQueueable;

use crate::models::Feed;

pub async fn update_stale_feeds(pool: &PgPool, queue: &mut dyn AsyncQueueable) -> Result<(), AnyError> {
  let feeds = Feed::stale(pool, 600, 5).await;
  match feeds {
    Ok(feeds) => {
      for mut feed in feeds {
        println!("{:}", feed.url);
        feed.refresh(pool, queue).await?
      };

      Ok(())
    },
    Err(why) => Err(anyhow!(why.to_string()))
  }
}
