use sqlx::sqlite::SqlitePool;

use anyhow::{anyhow};
use anyhow::Error as AnyError;

use crate::models::feed::Feed;

pub async fn update_stale_feeds(pool: &SqlitePool) -> Result<(), AnyError>{
  let feeds = Feed::stale(pool, 3600, 5).await;
  match feeds {
    Ok(feeds) => {
      for mut feed in feeds { 
        feed.refresh(pool).await?
      };

      Ok(())
    },
    Err(why) => Err(anyhow!(why.to_string()))
  }
}