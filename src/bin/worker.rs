use tokio::time::sleep;

use std::env;

use fang::asynk::async_queue::AsyncQueue;
use fang::asynk::async_queue::AsyncQueueable;
use fang::asynk::async_worker_pool::AsyncWorkerPool;
use fang::AsyncRunnable;
use fang::NoTls;

// use rustypub::tasks::DeliverItem;
// use rustypub::tasks::RefreshFeed;
use rustypub::tasks::UpdateStaleFeeds;

use std::time::Duration;

#[tokio::main]
async fn main() {
  if env::var("SENTRY_DSN").is_ok() {
    let sentry_dsn = env::var("SENTRY_DSN").expect("SENTRY_DSN is not set");
    let _guard = sentry::init((sentry_dsn, sentry::ClientOptions {
      release: sentry::release_name!(),
      ..Default::default()
    }));
  }

  let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");

  env_logger::init();

  log::info!("Starting...");
  let max_pool_size: u32 = 3;
  let mut queue = AsyncQueue::builder()
      .uri(db_uri)
      .max_pool_size(max_pool_size)
      .build();

  queue.connect(NoTls).await.unwrap();
  log::info!("Queue connected...");

  let mut pool: AsyncWorkerPool<AsyncQueue<NoTls>> = AsyncWorkerPool::builder()
      .number_of_workers(10_u32)
      .queue(queue.clone())
      .build();

  log::info!("Pool created ...");

  pool.start().await;
  log::info!("Workers started ...");

  let task = UpdateStaleFeeds {};
  queue
    .schedule_task(&task as &dyn AsyncRunnable)
    .await
    .unwrap();

  loop {
    sleep(Duration::from_secs(2)).await;
  }
}
