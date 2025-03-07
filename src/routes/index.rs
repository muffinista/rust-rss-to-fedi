use actix_web::{get, web, Responder, HttpResponse};
use actix_session::Session;

use std::env;

use sqlx::postgres::PgPool;

use crate::errors::AppError;
use crate::models::User;
use crate::models::Feed;
use crate::models::Setting;

use crate::routes::PageQuery;
use crate::utils::templates;
use crate::PER_PAGE;

#[get("/")]
pub async fn index(session: Session,  query: web::Query<PageQuery>, tmpl: web::Data<tera::Tera>, db: web::Data<PgPool>) -> Result<impl Responder, AppError> {
  let db = db.as_ref();
  let tmpl = tmpl.as_ref();
  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

  let mut context = tera::Context::new();
  context.insert("instance_domain", &instance_domain);

  let body = if let Some(user) = User::from_session(&session, db).await? {
    let signups_enabled = Setting::value_or(&"signups_enabled".to_string(), &"true".to_string(), db).await.unwrap();
    let page: i32 = query.page.unwrap_or(1);
  
    let feeds = Feed::paged_for_user(&user, page, db).await.unwrap();
    let count = Feed::count_for_user(&user, db).await.unwrap();
  
    let total_pages:i32 = (count / PER_PAGE) + 1;
  
    context.insert("logged_in", &true);
    context.insert("username", &user.full_username());
    context.insert("feeds", &feeds);
    context.insert("page", &page);
    context.insert("total_pages", &total_pages);
    context.insert("total", &count);
    context.insert("signups_enabled", &(signups_enabled == "true"));

    templates::render("home.html.tera", tmpl, &context)
  } else {
    context.insert("logged_in", &false);

    templates::render("home.html.tera", tmpl, &context)
  };

  match body {
    Ok(body) => Ok(HttpResponse::build(actix_web::http::StatusCode::OK).body(body)),
    Err(why) => {
      log::debug!("{:?}", why);
      Err(AppError::InternalError)
    }
  }
}

#[cfg(test)]
mod test {
  use actix_session::{SessionMiddleware, storage::CookieSessionStore};
  use actix_web::{test, dev::Service};
  use sqlx::postgres::PgPool;
  
  use crate::build_test_server;
  use crate::utils::test_helpers::real_user;


  #[sqlx::test]
  async fn index_not_logged_in(pool: PgPool) {
    let server = test::init_service(build_test_server!(pool)).await;
    let req = test::TestRequest::with_uri("/").to_request();
    let res = server.call(req).await.unwrap();

    assert_eq!(res.status(), actix_web::http::StatusCode::OK);

    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    assert!(std::str::from_utf8(&bytes).unwrap().contains("To get started"));
  }

  #[sqlx::test]
  async fn index_logged_in(pool: PgPool) {
    let user = real_user(&pool).await.unwrap();
    let server = test::init_service(build_test_server!(pool)).await;

    let req = test::TestRequest::with_uri(&format!("/user/auth/{}", &user.login_token)).to_request();
    let res = server.call(req).await.unwrap();
    assert_eq!(res.status(), actix_web::http::StatusCode::TEMPORARY_REDIRECT);

    let session_cookies = res.response().cookies();

    let mut req = test::TestRequest::with_uri("/");
    for cookie in session_cookies {
      req = req.cookie(cookie.clone());
    } 

    let req = req.to_request();
  
    let res = server.call(req).await.unwrap();
    assert_eq!(res.status(), actix_web::http::StatusCode::OK);

    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    assert!(std::str::from_utf8(&bytes).unwrap().contains("Add a new feed:"));
  }
}
