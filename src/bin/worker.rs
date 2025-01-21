use tokio::time::sleep;

use std::env;
use std::str::FromStr;

use fang::asynk::async_queue::AsyncQueue;
use fang::asynk::async_queue::AsyncQueueable;
use fang::asynk::async_worker_pool::AsyncWorkerPool;
use fang::AsyncRunnable;
use fang::NoTls;

use rustypub::{
  tasks::{
    UpdateStaleFeeds,
    DeleteOldMessages,
    DeleteBadActors
  },
  utils::queue::create_queue
};

use std::time::Duration;


#[tokio::main]
async fn main() {
  env_logger::init();

  let worker_count = match env::var_os("WORKER_COUNT") {
    Some(val) => {
      u32::from_str(&val.into_string().expect("Something went wrong setting the worker count")).unwrap()
    }
    None => 10_u32
  };

  log::info!("Starting...");
  let mut queue = create_queue().await;

  queue.connect(NoTls).await.unwrap();
  log::info!("Queue connected...");

  let mut worker_pool: AsyncWorkerPool<AsyncQueue<NoTls>> = AsyncWorkerPool::builder()
      .number_of_workers(worker_count)
      .queue(queue.clone())
      .build();

  log::info!("Pool created ...");

  worker_pool.start().await;
  log::info!("Workers started ...");

  let task = UpdateStaleFeeds {};
  queue
    .schedule_task(&task as &dyn AsyncRunnable)
    .await
    .unwrap();

  let cleanup_task = DeleteOldMessages {};
  queue
    .schedule_task(&cleanup_task as &dyn AsyncRunnable)
    .await
    .unwrap();

  let cleanup_actors_task = DeleteBadActors {};
  queue
    .schedule_task(&cleanup_actors_task as &dyn AsyncRunnable)
    .await
    .unwrap();
    
  loop {
    sleep(Duration::from_secs(2)).await;
  }
}
