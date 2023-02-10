use std::env;
use fang::asynk::async_queue::AsyncQueue;
use fang::NoTls;

pub fn create_queue() -> AsyncQueue<NoTls> {
  let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
  let max_pool_size: u32 = 5;

  let queue:AsyncQueue<NoTls> = AsyncQueue::builder()
    // Postgres database url
    .uri(&db_uri)
    // Max number of connections that are allowed
    .max_pool_size(max_pool_size)
    .build();

  queue
}
