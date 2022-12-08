#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use sqlx::sqlite::SqlitePool;

use std::env;

use rocket_dyn_templates::Template;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
  let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
  let _domain_name = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

  let pool = SqlitePool::connect(&db_uri)
    .await
    .expect("Failed to create pool");

  sqlx::migrate!("./migrations")
    .run(&pool)
    .await
    .ok();
  
  let _rocket = rocket::build()
    .manage(pool)
    .mount("/", routes![
      rustypub::routes::index::index,
      rustypub::routes::index::index_logged_in,
      rustypub::routes::login::do_login,
      rustypub::routes::login::attempt_login,
      rustypub::routes::feeds::add_feed,
      rustypub::routes::feeds::delete_feed,
      rustypub::routes::feeds::render_feed,
      rustypub::routes::feeds::show_feed,
      rustypub::routes::webfinger::lookup_webfinger,
      rustypub::routes::ap::outbox::user_outbox      
      ])
    .attach(Template::fairing())
    .launch()
    .await?;
  
  Ok(())
}
