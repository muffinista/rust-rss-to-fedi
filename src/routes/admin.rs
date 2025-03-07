use std::env;

use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use actix_web::{delete, get, put, web, Responder};
use actix_session::Session;

use serde::Deserialize;
use sqlx::postgres::PgPool;

use crate::errors::AppError;
use crate::models::User;
use crate::models::Feed;
use crate::models::Item;
use crate::models::Setting;

use crate::utils::templates;
use crate::PER_PAGE;

#[derive(Deserialize)]
pub struct AdminSettingsForm {
  signups_enabled: String
}

#[derive(Deserialize)]
struct PageQuery {
  page: Option<i32>,
}


#[get("/admin")]
pub async fn index_admin(session: Session, query: web::Query<PageQuery>, db: web::Data<PgPool>, tmpl: web::Data<tera::Tera>) -> Result<impl Responder, AppError> {
  let db = db.as_ref();
  let tmpl = tmpl.as_ref();

  let Some(user) = User::from_session(&session, db).await? else {
    return Err(AppError::NotFound)      
  };

  if ! user.is_admin() {
    return Err(AppError::NotFound)      
  }

  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  let page: i32 = query.page.unwrap_or(1);

  let feeds = Feed::paged(page, db).await.unwrap();
  let signups_enabled = Setting::value_or(&"signups_enabled".to_string(), &"true".to_string(), db).await.unwrap();

  let count = Feed::count(db).await.unwrap();
  let total_pages:i32 = (count / PER_PAGE) + 1;

  let mut context = tera::Context::new();
  context.insert("instance_domain", &instance_domain);
  context.insert("feeds", &feeds);
  context.insert("page", &page);
  context.insert("total_pages", &total_pages);
  context.insert("total", &count);
  context.insert("signups_enabled", &signups_enabled);
  context.insert("feed_link_prefix", &"/admin");


  let body = templates::render("admin.html.tera", tmpl, &context)?;

  Ok(HttpResponse::build(StatusCode::OK).content_type("text/html").body(body))
}


#[put("/admin/settings")]
pub async fn update_settings_admin(session: Session, db: web::Data<PgPool>, form: web::Form<AdminSettingsForm>) -> Result<impl Responder, AppError> {
  let db = db.as_ref();

  let Some(user) = User::from_session(&session, db).await? else {
    return Err(AppError::NotFound)      
  };

  if ! user.is_admin() {
    return Err(AppError::NotFound)
  }

  let result = Setting::update(&"signups_enabled".to_string(), &form.signups_enabled, db).await;

  let dest = "/admin?page=1";

  match result {
    Ok(_result) => Ok(crate::utils::redirect_to(dest)),
    Err(_why) => Ok(crate::utils::redirect_to(dest))
  }
}


#[get("/admin/feed/{username}")]
pub async fn show_feed_admin(session: Session, db: web::Data<PgPool>, path: web::Path<String>, tmpl: web::Data<tera::Tera>)  -> Result<impl Responder, AppError> {
  let db = db.as_ref();
  let tmpl = tmpl.as_ref();
  let username = path.into_inner();

  let Some(user) = User::from_session(&session, db).await? else {
    return Err(AppError::NotFound)      
  };

  if ! user.is_admin() {
    return Err(AppError::NotFound)
  }

  let feed_lookup = Feed::find_by_name(&username, db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          let logged_in = true; // user.is_some();
          let follow_url = feed.permalink_url();
          let items = Item::for_feed(&feed, 10, db).await;

          match items {
            Ok(items) => {
              let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
              let mut context = tera::Context::new();
              context.insert("instance_domain", &instance_domain);
              context.insert("is_admin", &feed.is_admin());
              context.insert("noindex", &!feed.listed);
              context.insert("logged_in", &logged_in);
              context.insert("username", &username);
              context.insert("owned_by", &true);
              context.insert("feed", &feed);
              context.insert("items", &items);
              context.insert("follow_url", &follow_url);
                    
              let body = templates::render("feed-admin.html.tera", tmpl, &context)?;

              Ok(HttpResponse::build(StatusCode::OK).content_type("text/html").body(body))
            },
            Err(_why) => Err(AppError::NotFound)        
          }

        },
        None => Err(AppError::NotFound)
      }
    },
    Err(_why) => Err(AppError::NotFound)
  }
}

#[delete("/admin/feed/{id}/delete")]
pub async fn delete_feed_admin(session: Session, path: web::Path<i32>, db: web::Data<PgPool>) -> Result<impl Responder, AppError> {
  let db = db.as_ref();
  let Some(user) = User::from_session(&session, db).await? else {
    return Err(AppError::NotFound)      
  };

  if ! user.is_admin() {
    return Err(AppError::NotFound)
  }

  let id = path.into_inner();
  let feed = Feed::admin_delete(id, db).await;
  
  match feed {
    Ok(_feed) => {
      Ok(crate::utils::redirect_to("/admin"))
    },
    Err(why) => {
      print!("{why}");
      Err(AppError::NotFound)
    }
  }
}

#[cfg(test)]
mod test {
  use actix_web::{test, dev::Service};
  use actix_session::{SessionMiddleware, storage::CookieSessionStore};
  use sqlx::postgres::PgPool;
  
  use crate::build_test_server;
  use crate::utils::test_helpers::{ real_user, real_admin_user};

  #[sqlx::test]
  async fn index_admin_not_logged_in(pool: PgPool) {
    let server = test::init_service(build_test_server!(pool)).await;
    let req = test::TestRequest::with_uri("/admin").to_request();
    let res = server.call(req).await.unwrap();

    assert_eq!(res.status(), actix_web::http::StatusCode::NOT_FOUND);
  }

  #[sqlx::test]
  async fn index_admin_non_admin_logged_in(pool: PgPool) {
    let user = real_user(&pool).await.unwrap();

    let server = test::init_service(build_test_server!(pool)).await;

    let req = test::TestRequest::with_uri(&format!("/user/auth/{}", &user.login_token)).to_request();
    let res = server.call(req).await.unwrap();
    assert_eq!(res.status(), actix_web::http::StatusCode::TEMPORARY_REDIRECT);

    let session_cookies = res.response().cookies();


    let mut req = test::TestRequest::with_uri("/admin");
    for cookie in session_cookies {
      req = req.cookie(cookie.clone());
    }
    let req = req.to_request();
    let res = server.call(req).await.unwrap();

    assert_eq!(res.status(), actix_web::http::StatusCode::NOT_FOUND);
  }

  #[sqlx::test]
  async fn index_admin_logged_in(pool: PgPool) {
    let user = real_admin_user(&pool).await.unwrap();

    let server = test::init_service(build_test_server!(pool)).await;
    let req = test::TestRequest::with_uri(&format!("/user/auth/{}", &user.login_token)).to_request();
    let res = server.call(req).await.unwrap();
    assert_eq!(res.status(), actix_web::http::StatusCode::TEMPORARY_REDIRECT);

    let session_cookies = res.response().cookies();


    let mut req = test::TestRequest::with_uri("/admin");
    for cookie in session_cookies {
      req = req.cookie(cookie.clone());
    }
    let req = req.to_request();
    let res = server.call(req).await.unwrap();

    assert_eq!(res.status(), actix_web::http::StatusCode::OK);
    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();
    assert!(body.contains("Feed admin!"));
  }
}
