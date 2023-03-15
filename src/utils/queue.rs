use std::env;
use tokio::sync::OnceCell;

use fang::asynk::async_queue::AsyncQueue;
use fang::NoTls;

static QUEUE_POOL: OnceCell<AsyncQueue<NoTls>> = OnceCell::const_new();


async fn init_queue() -> AsyncQueue<NoTls> {
  let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
  let max_pool_size: u32 = 3;

  AsyncQueue::builder()
    .uri(db_uri)
    .max_pool_size(max_pool_size)
    .build()
}

pub async fn create_queue()  -> AsyncQueue<NoTls> {
  QUEUE_POOL.get_or_init(init_queue).await.clone()
}
