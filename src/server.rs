use rocket::routes;

use sqlx::sqlite::SqlitePool;

use std::env;

use rocket_dyn_templates::Template;

use rocket::{Rocket, Ignite};

pub async fn boot_server() -> Result<Rocket<Ignite>, rocket::Error> {
  let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
  let _domain_name = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

  let pool = SqlitePool::connect(&db_uri)
    .await
    .expect("Failed to create pool");

  sqlx::migrate!("./migrations")
    .run(&pool)
    .await
    .ok();
  
  rocket::build()
    .manage(pool)
    .mount("/", routes![
      crate::routes::index::index,
      crate::routes::index::index_logged_in,
      crate::routes::login::do_login,
      crate::routes::login::attempt_login,
      crate::routes::feeds::add_feed,
      crate::routes::feeds::delete_feed,
      crate::routes::feeds::render_feed,
      crate::routes::feeds::show_feed,
      crate::routes::webfinger::lookup_webfinger,
      crate::routes::ap::outbox::user_outbox    
    ])
    .attach(Template::fairing())
    .launch()
    .await
}
