#![feature(proc_macro_hygiene, decl_macro)]

use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::{web, App, HttpServer};

use std::env;

// use rustypub::server::build_server;
use rustypub::utils::pool::db_pool;

#[actix_web::main]
pub async fn main() -> std::io::Result<()> {
  env_logger::init();

  let _domain_name = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

  let pool = db_pool().await;

  let tera =
    tera::Tera::new("templates/**/*").expect("Parsing error while loading template folder");
  let secret_key = rustypub::routes::configure::get_secret_key();

  // Start the web application.
  // We'll need to transfer ownership of the AppState to the HttpServer via the `move`.
  // Then we can instantiate our controllers.
  HttpServer::new(move || {
    App::new()
      .wrap(SessionMiddleware::new(CookieSessionStore::default(), secret_key.clone()))
      .app_data(web::Data::new(pool.clone()))
      .app_data(web::Data::new(tera.clone()))
      .configure(|cfg| rustypub::routes::configure::apply(cfg))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
    // .bind(config.get_app_url())?;

//       crate::routes::index::index,
//       crate::routes::index::index_logged_in,
//       crate::routes::login::do_login,
//       crate::routes::login::do_logout,
//       crate::routes::login::login_result,
//       crate::routes::login::attempt_login,
//       crate::routes::enclosures::show_enclosure,
//       crate::routes::feeds::add_feed,
//       crate::routes::feeds::test_feed,
//       crate::routes::feeds::update_feed,
//       crate::routes::feeds::delete_feed,
//       crate::routes::feeds::render_feed,
//       crate::routes::feeds::render_feed_followers,
//       crate::routes::feeds::show_feed,
//       crate::routes::items::show_item,
//       crate::routes::items::show_item_json,
//       crate::routes::webfinger::lookup_webfinger,
//       crate::routes::ap::inbox::user_inbox,
//       crate::routes::ap::outbox::render_feed_outbox,
//       crate::routes::admin::index_admin,
//       crate::routes::admin::show_feed_admin,
//       crate::routes::admin::update_settings_admin,
//       crate::routes::admin::delete_feed_admin,
//       crate::routes::well_known::host_meta,
//       crate::routes::nodeinfo::nodeinfo


  // println!("Listening on: {0}", config.get_app_url());

  // let server = build_server(pool)
  //   .await
  //   .launch()
  //   .await;

  // match server {
  //   Ok(_server) => Ok(()),
  //   Err(why) => panic!("{}", why)
  // }
}
