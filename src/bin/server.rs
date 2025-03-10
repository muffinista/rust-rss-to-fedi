#![feature(proc_macro_hygiene, decl_macro)]

use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::{web, App, HttpServer, middleware::Logger};
use actix_files::Files;

use std::env;

use rustypub::utils::pool::db_pool;

#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
  env_logger::init();

  let _domain_name = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

  let pool = db_pool().await;

  let assets_dir = env::var("ASSETS_PATH").unwrap_or(String::from("./assets"));
  let templates_dir = env::var("TEMPLATES_PATH").unwrap_or(String::from("templates"));

  let tera =
    tera::Tera::new(&format!("{templates_dir:}/**/*")).expect("Parsing error while loading template folder");
  let secret_key = rustypub::routes::configure::get_secret_key();

  // Start the web application.
  // We'll need to transfer ownership of the AppState to the HttpServer via the `move`.
  // Then we can instantiate our controllers.
  HttpServer::new(move || {
    App::new()
      .service(Files::new("/assets", &assets_dir).prefer_utf8(true))
      .wrap(SessionMiddleware::new(CookieSessionStore::default(), secret_key.clone()))
      .wrap(Logger::default())
      .app_data(web::Data::new(pool.clone()))
      .app_data(web::Data::new(tera.clone()))
      .configure(|cfg| rustypub::routes::configure::apply(cfg))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
    // .bind(config.get_app_url())?;

}
