use sqlx::postgres::PgPool;

use anyhow::{anyhow};
use anyhow::Error as AnyError;

use fang::asynk::async_queue::AsyncQueueable;
use fang::AsyncRunnable;


use crate::models::Feed;
use crate::tasks::RefreshFeed;

pub async fn update_stale_feeds(pool: &PgPool, queue: &mut dyn AsyncQueueable) -> Result<(), AnyError> {
  let feeds = Feed::stale(pool, 600, 5).await;
  match feeds {
    Ok(feeds) => {
      for feed in feeds {
        log::info!("update_stale_feed {:} {:} {:}", feed.id, feed.age(), feed.url);

        let _result = feed.mark_fresh(pool).await;

        let task = RefreshFeed { id: feed.id };
        let _result = queue
          .insert_task(&task as &dyn AsyncRunnable)
          .await
          .unwrap();
      };

      Ok(())
    },
    Err(why) => Err(anyhow!(why.to_string()))
  }
}


#[cfg(test)]
mod test {
  use std::env;
  use url::Url;
  use fang::NoTls;

  use sqlx::postgres::PgPool;
  use sqlx::{
    Postgres,
    postgres::{PgPoolOptions, PgConnectOptions}
  };
  use sqlx::pool::PoolOptions;

  use fang::AsyncQueue;

  use crate::utils::test_helpers::real_feed;

  use crate::services::loader::update_stale_feeds;

  async fn active_task_count(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM fang_tasks")
    .fetch_one(pool)
    .await
    .unwrap()
    .tally
    .unwrap();

    Ok(result)
  }

  #[sqlx::test]
  async fn test_update_stale_feeds(_pool_opts:PoolOptions<Postgres>, opts: PgConnectOptions) -> Result<(), sqlx::Error> {
    // grab the default db url from the environment, then update with the
    // test db name so that fang and sqlx have the same db backend
    let default_db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
    let mut parsed_uri = Url::parse(&default_db_uri).unwrap();
    parsed_uri.set_path(opts.get_database().unwrap());

    let pool = PgPoolOptions::new()
      .max_connections(5u32)
      .connect_with(opts)
      .await
      .expect("Failed to create pool");

    let max_pool_size: u32 = 3;
  
    let mut queue = AsyncQueue::builder()
      .uri(parsed_uri)
      .max_pool_size(max_pool_size)
      .build();
  
    queue.connect(NoTls).await.unwrap();

    let feed = real_feed(&pool).await.unwrap();

    let pre_count = active_task_count(&pool).await?;

    let _result = feed.mark_stale(&pool).await;
    let _result = update_stale_feeds(&pool, &mut queue).await;

    let post_count = active_task_count(&pool).await?;

    assert!(post_count - pre_count == 1);

    pool.close().await;

    Ok(())

  }
}
