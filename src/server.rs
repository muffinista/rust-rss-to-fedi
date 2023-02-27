use sqlx::postgres::PgPool;

use rocket::{
  routes,
  Rocket,
  Build,
  fs::{FileServer, relative}
};
use rocket_dyn_templates::Template;

use crate::utils::admin::create_admin_feed;

pub async fn build_server(pool: PgPool) -> Rocket<Build> {
  sqlx::migrate!("./migrations")
    .run(&pool)
    .await
    .ok();

  // create an admin feed to handle interactions with server
  let result = create_admin_feed(&pool).await;
  match result {
    Ok(result) => println!("{result:?}"),
    Err(why) => panic!("{}", why)
  };
  
  rocket::build()
    .manage(pool)
    .mount("/assets", FileServer::from(relative!("assets")))
    .mount("/", routes![
      crate::routes::index::index,
      crate::routes::index::index_logged_in,
      crate::routes::login::do_login,
      crate::routes::login::do_logout,
      crate::routes::login::login_result,
      crate::routes::login::attempt_login,
      crate::routes::feeds::add_feed,
      crate::routes::feeds::test_feed,
      crate::routes::feeds::update_feed,
      crate::routes::feeds::delete_feed,
      crate::routes::feeds::render_feed,
      crate::routes::feeds::render_feed_followers,
      crate::routes::feeds::show_feed,
      crate::routes::items::show_item,
      crate::routes::items::show_item_json,
      crate::routes::webfinger::lookup_webfinger,
      crate::routes::ap::inbox::user_inbox,
      crate::routes::ap::outbox::render_feed_outbox,
      crate::routes::admin::index_admin,
      crate::routes::admin::show_feed_admin,
      crate::routes::admin::update_settings_admin,
      crate::routes::admin::delete_feed_admin
    ])
    .attach(Template::fairing())
}
