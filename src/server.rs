use rocket::routes;

use sqlx::sqlite::SqlitePool;


use rocket_dyn_templates::Template;

use rocket::{Rocket, Build};

pub async fn build_server(pool: SqlitePool) -> Rocket<Build> {

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
      crate::routes::feeds::render_feed_followers,
      crate::routes::feeds::show_feed,
      crate::routes::webfinger::lookup_webfinger,
      crate::routes::ap::outbox::user_outbox    
    ])
    .attach(Template::fairing())
}
