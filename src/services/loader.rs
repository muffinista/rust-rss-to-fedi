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
        let task = RefreshFeed { id: feed.id };
        let _result = queue
          .insert_task(&task as &dyn AsyncRunnable)
          .await
          .unwrap();
  
        // log::info!("update_stale_feed {:}", feed.url);
        // feed.refresh(pool, queue).await?
      };

      Ok(())
    },
    Err(why) => Err(anyhow!(why.to_string()))
  }
}


// #[cfg(test)]
// mod test {
//   use fang::asynk::async_queue::AsyncQueue;
//   use fang::AsyncRunnable;
//   use fang::NoTls;

//   use sqlx::postgres::PgPool;
//   use std::env;

//   use crate::utils::test_helpers::real_feed;
//   use crate::utils::queue::create_queue;

//   use crate::services::loader::update_stale_feeds;

//   #[sqlx::test]
//   async fn test_update_stale_feeds(pool: PgPool) {
//     let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");

//     let mut queue = create_queue();

//     queue.connect(NoTls).await.unwrap();
//     // queue
//     //   .insert_task(&task as &dyn AsyncRunnable)
//     //   .await
//     //   .unwrap();


//     let feed = real_feed(&pool).await.unwrap();

//     let pre_count = sqlx::query!("SELECT COUNT(1) AS tally FROM fang_tasks")
//       .fetch_one(&pool)
//       .await
//       .unwrap()
//       .tally
//       .unwrap();

//     feed.mark_stale(&pool).await;

//     let result = update_stale_feeds(&pool, &mut queue).await;

//     let post_count = sqlx::query!("SELECT COUNT(1) AS tally FROM fang_tasks")
//       .fetch_one(&pool)
//       .await
//       .unwrap()
//       .tally
//       .unwrap();

//     println!("!!!! {post_count:} - {pre_count:}");
//     assert!(post_count - pre_count == 1);


//   }
// }
