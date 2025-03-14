#![feature(proc_macro_hygiene, decl_macro)]

use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::{web, App, HttpServer, middleware::Logger};
use actix_files::Files;

use std::env;

use rustypub::{models::{Feed, User}, utils::pool::db_pool};
use http_signature_normalization_actix::digest::ring::Sha256;
use http_signature_normalization_actix::digest::middleware::VerifyDigest;

#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
  env_logger::init();

  let _domain_name = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

  let assets_dir = env::var("ASSETS_PATH").unwrap_or(String::from("./assets"));
  let templates_dir = env::var("TEMPLATES_PATH").unwrap_or(String::from("templates"));
  let bind_address = env::var("BIND_ADDRESS").unwrap_or(String::from("0.0.0.0"));
  let bind_port = env::var("BIND_PORT").unwrap_or(String::from("8080"));

  let bind_port: u16 = bind_port.parse::<u16>().expect("Base BIND_PORT value!");  

  let pool = db_pool().await;

  if User::for_admin(&pool).await.unwrap().is_none() {
    println!("Setting up admin user!");
    let u = User::create_by_actor_url(&String::from("fake"), &pool).await;
    match u {
      Ok(u) => {
        let f = Feed::create(&u, &String::from("fake"), &String::from("admin"), &pool).await.unwrap();
        let _result = f.mark_admin(&pool).await;
      },
      Err(why) => {
        println!("{:?}", why);
        panic!("weird that failed");
      }
    }
  }

  let tera =
    tera::Tera::new(&format!("{templates_dir:}/**/*")).expect("Parsing error while loading template folder");
  let secret_key = rustypub::routes::configure::get_secret_key();

  // Start the web application.
  // We'll need to transfer ownership of the AppState to the HttpServer via the `move`.
  // Then we can instantiate our controllers.
  HttpServer::new(move || {
    App::new()
      .wrap(VerifyDigest::new(Sha256::new()).optional())
      .service(Files::new("/assets", &assets_dir).prefer_utf8(true))
      .wrap(SessionMiddleware::new(CookieSessionStore::default(), secret_key.clone()))
      .wrap(Logger::default())
      .app_data(web::Data::new(pool.clone()))
      .app_data(web::Data::new(tera.clone()))
      .configure(|cfg| rustypub::routes::configure::apply(cfg))
    })
    .bind((bind_address, bind_port))?
    .run()
    .await
    // .bind(config.get_app_url())?;

}
