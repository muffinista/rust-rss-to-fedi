use rocket_dyn_templates::{Template, context};

use rocket::get;
use rocket::State;

use sqlx::sqlite::SqlitePool;

use crate::user::User;
use crate::feed::Feed;

#[get("/")]
pub async fn index_logged_in(user: User, db: &State<SqlitePool>) -> Template {
  let feeds = Feed::for_user(&user, &db).await.unwrap();
  Template::render("home", context! { logged_in: true, feeds: feeds })
}

#[get("/", rank = 2)]
pub fn index() -> Template {
  Template::render("home", context! { logged_in: false })
}
