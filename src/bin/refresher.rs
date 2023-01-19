#![feature(proc_macro_hygiene, decl_macro)]

use sqlx::sqlite::SqlitePool;
use std::env;

#[tokio::main]
async fn main() -> Result<(), reqwest::Error>  {
  let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
  let pool = SqlitePool::connect(&db_uri)
    .await
    .expect("Failed to create pool");
  sqlx::migrate!("./migrations")
    .run(&pool)
    .await
    .ok();

  let result = rustypub::services::loader::update_stale_feeds(&pool).await;
  match result {
    Ok(_result) => {
      println!("It worked!");
    },
    Err(why) => {
      println!("Something went wrong: {:}", why.to_string());
    }
  }

  Ok(())
}
