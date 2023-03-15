use std::env;
use std::str::FromStr;

use sqlx::{
  Pool,
  Postgres,
  ConnectOptions,
  postgres::{PgPoolOptions, PgConnectOptions}
};

use std::time::Duration;
use log::LevelFilter;
use tokio::sync::OnceCell;

const POOL_SIZE: u32 = 5;

static DB_POOL: OnceCell<Pool<Postgres>> = OnceCell::const_new();

pub fn pool_size() -> u32 {
  match env::var_os("POOL_SIZE") {
    Some(val) => {
      u32::from_str(&val.into_string().expect("Something went wrong setting the pool size")).unwrap()
    }
    None => POOL_SIZE
  }
}

fn connect_options() -> PgConnectOptions {
  let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
  PgConnectOptions::from_str(&db_uri)
    .expect("failed to parse DATABASE_URL")
    .log_statements(LevelFilter::Debug)
    .log_slow_statements(LevelFilter::Info, Duration::from_millis(200))
    .clone()
}

pub async fn init_db_pool() -> Pool<Postgres> {
  PgPoolOptions::new()
    .max_connections(pool_size())
    .connect_with(connect_options())
    .await
    .expect("Failed to create pool")
}

pub async fn db_pool() -> Pool<Postgres> {
  DB_POOL.get_or_init(init_db_pool).await.clone()
}
