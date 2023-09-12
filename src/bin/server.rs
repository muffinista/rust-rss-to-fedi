#![feature(proc_macro_hygiene, decl_macro)]

use std::env;

use rustypub::server::build_server;
use rustypub::utils::pool::db_pool;

#[rocket::main]
pub async fn main() -> Result<(), rocket::Error> {
  env_logger::init();

  let _domain_name = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

  rustypub::utils::templates::init_templating();

  let pool = db_pool().await;

  let server = build_server(pool)
    .await
    .launch()
    .await;

  match server {
    Ok(_server) => Ok(()),
    Err(why) => panic!("{}", why)
  }
}
