#![feature(proc_macro_hygiene, decl_macro)]

use std::env;

use rustypub::server::build_server;
use rustypub::utils::pool::db_pool;

#[rocket::main]
pub async fn main() -> Result<(), rocket::Error> {
  if env::var("SENTRY_DSN").is_ok() {
    let sentry_dsn = env::var("SENTRY_DSN").expect("SENTRY_DSN is not set");
    let _guard = sentry::init((sentry_dsn, sentry::ClientOptions {
      release: sentry::release_name!(),
      ..Default::default()
    }));
  }

  env_logger::init();

  let _domain_name = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
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
