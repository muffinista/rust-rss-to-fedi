#![feature(proc_macro_hygiene, decl_macro)]

use std::env;

use sqlx::sqlite::SqlitePool;
use rustypub::server::build_server;

#[rocket::main]
pub async fn main() -> Result<(), rocket::Error> {
  let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
  let _domain_name = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

  let pool = SqlitePool::connect(&db_uri)
    .await
    .expect("Failed to create pool");

  let server = build_server(pool)
    .await
    .launch()
    .await;

  match server {
    Ok(_server) => Ok(()),
    Err(why) => panic!("{}", why)
  }
}
