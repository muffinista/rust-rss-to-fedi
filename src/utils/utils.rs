use std::env;
use std::str::FromStr;

use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

const POOL_SIZE: u32 = 5;
const WORKER_POOL_SIZE: u32 = 3;

///
/// convert path to absolute URL
///
pub fn path_to_url(frag: &rocket::http::uri::Origin) -> String {
  let host = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  format!("https://{}{}", host, frag).to_string()
}

pub fn web_pool_size() -> u32 {
  match env::var_os("DATABASE_POOL_SIZE") {
    Some(val) => {
      u32::from_str(&val.into_string().expect("Something went wrong setting the pool size")).unwrap()
    }
    None => POOL_SIZE
  }
}

pub fn worker_pool_size() -> u32 {
  match env::var_os("WORKER_POOL_SIZE") {
    Some(val) => {
      u32::from_str(&val.into_string().expect("Something went wrong setting the pool size")).unwrap()
    }
    None => WORKER_POOL_SIZE
  }
}

pub async fn web_db_pool() -> Pool<Postgres> {
  let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");

  PgPoolOptions::new()
    .max_connections(web_pool_size())
    .connect(&db_uri)
    .await
    .expect("Failed to create pool")
}

pub async fn worker_db_pool() -> Pool<Postgres> {
  let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");

  PgPoolOptions::new()
    .max_connections(worker_pool_size())
    .connect(&db_uri)
    .await
    .expect("Failed to create pool")
}