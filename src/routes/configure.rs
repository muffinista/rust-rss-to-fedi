use std::env;

use crate::routes::*;
use actix_web::web::*;

pub fn get_secret_key() -> actix_web::cookie::Key {
  if env::var("SESSION_KEY").is_err() {
    println!("No SESSION_KEY set in environment!");
    actix_web::cookie::Key::generate()
  } else {
    let key = env::var("SESSION_KEY").expect("SESSION_KEY is not set???");
    actix_web::cookie::Key::from(key.as_bytes())
  }
}

pub fn apply(cfg: &mut ServiceConfig) {
  cfg.service(
    scope("")
    .service(index::index)
    .service(login::attempt_login)
    .service(login::do_logout)
    .service(enclosures::show_enclosure)
    .service(feeds::add_feed)
    .service(feeds::test_feed)
    .service(feeds::update_feed)
    .service(feeds::delete_feed)
    .service(feeds::show_feed)
    .service(feeds::render_feed_followers)
    .service(feeds::show_feed)
    .service(items::show_item)
    .service(webfinger::lookup_webfinger)
    .service(ap::inbox::user_inbox)
    .service(ap::outbox::render_feed_outbox)
    .service(admin::index_admin)
    .service(admin::show_feed_admin)
    .service(admin::update_settings_admin)
    .service(admin::delete_feed_admin)
    .service(well_known::host_meta)
    .service(nodeinfo::nodeinfo)
  );
}