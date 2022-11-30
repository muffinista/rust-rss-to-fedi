#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use sqlx::sqlite::SqlitePool;

use std::env;

use rustypub::user::User;
use rustypub::feed::Feed;

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

    let feed = Feed::find(2, &pool).await.unwrap();
    Feed::parse(&feed).await;

    Ok(())
}
