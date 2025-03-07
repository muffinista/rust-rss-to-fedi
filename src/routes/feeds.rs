use std::env;

use actix_web::http::header::{ContentType, HeaderValue};
use actix_web::{delete, put, HttpRequest};
use actix_web::{get, post, web, Responder, HttpResponse, http::StatusCode};
use actix_session::Session;
use serde::{Deserialize, Serialize};

use fang::AsyncRunnable;
use fang::AsyncQueueable;

use sqlx::postgres::PgPool;

use crate::models::AppError;
use crate::models::User;
use crate::models::Feed;
use crate::models::Item;
use crate::models::Setting;

use crate::services::url_to_feed::url_to_feed_url;

use crate::utils::queue::create_queue;
use crate::utils::templates;
use crate::tasks::RefreshFeed;
use crate::constants::ACTIVITY_JSON;
use crate::activity_json_response;

#[derive(Deserialize, Serialize)]
pub struct FeedForm {
  name: String,
  url: String
}

#[derive(Deserialize)]
pub struct FeedUpdateForm {
  url: String,
  listed: bool,
  status_publicity: Option<String>,
  content_warning: Option<String>,
  hashtag: Option<String>,
  title: Option<String>,
  description: Option<String>
}

#[derive(Serialize)]
pub struct FeedLookup {
  src: String,
  url: String,
  error: Option<String>
}

#[derive(Deserialize)]
struct PageQuery {
  page: Option<i32>,
}


///
/// After creating/updating a feed, let's refresh its data
/// 
async fn request_feed_update(feed: &Feed) -> Result<fang::Task, fang::AsyncQueueError> {
  let task = RefreshFeed { id: feed.id };
  let mut queue = create_queue().await;
  queue.connect(fang::NoTls).await.unwrap();

  queue
    .insert_task(&task as &dyn AsyncRunnable)
    .await
}

///
/// POST action to create a new feed
///
#[post("/feed")]
pub async fn add_feed(session: Session, db: web::Data<PgPool>, form: web::Form<FeedForm>, tmpl: web::Data<tera::Tera>) -> Result<impl Responder, AppError> {
  let tmpl = tmpl.as_ref();
  let db = db.as_ref();
  let Some(user) = User::from_session(&session, db).await? else {
    return Err(AppError::NotFound)      
  };

  let signups_enabled = Setting::value_or(&"signups_enabled".to_string(), &"true".to_string(), db).await.unwrap();

  if signups_enabled != "true" {
    return Err(AppError::InternalError)
  }

  let Some(_user) = User::from_session(&session, db).await? else {
    return Err(AppError::NotFound)      
  };

  //
  // follow the URL to make sure we add a valid RSS feed at this point
  //
  let url = url_to_feed_url(&form.url).await;
  match url {
    Err(_why) =>{
      Err(AppError::NotFound)
    },
    Ok(result) => {
      if result.is_some() {
        let url = result.unwrap();
        let feed = Feed::create(&user, &url, &form.name, db).await;
  
        match feed {
          Ok(feed) => {
            let _ = request_feed_update(&feed).await;
      
            let notify = user.send_link_to_feed(&feed, db, tmpl).await;
            match notify {
              Ok(_notify) => log::debug!("user notified!"),
              Err(why) => log::info!("something went wrong with notification: {why:?}")
            }
      
            let dest = feed.permalink_url();
            // Ok(FlashResponse::with_redirect("Feed created!".to_owned(), dest))
            Ok(crate::utils::redirect_to(&dest))
          },
          Err(why) => {
            log::info!("{why}");
            // Ok(FlashResponse::with_redirect("Sorry, something went wrong!".to_owned(), "/"))
            Ok(crate::utils::redirect_to("/"))
          }
        }
      } else {
        Err(AppError::NotFound)
      }
    }
  }
}

///
/// Update settings on a feed
///
#[put("/feed/{username}")]
pub async fn update_feed(session: Session, path: web::Path<String>, db: web::Data<PgPool>, form: web::Form<FeedUpdateForm>) -> Result<impl Responder, AppError> {
  let db = db.as_ref();
  let Some(user) = User::from_session(&session, db).await? else {
    return Err(AppError::NotFound)      
  };
  let username = path.into_inner();
  let feed_lookup = Feed::find_by_user_and_name(&user, &username.to_string(), db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(mut feed) => {
          feed.listed = form.listed;
          feed.hashtag = None;
          feed.content_warning = form.content_warning.clone();
          feed.hashtag = form.hashtag.clone();
          feed.status_publicity = form.status_publicity.clone();
          feed.url = form.url.clone();

          // user has tweaked title/description, let's mark that
          if form.title != feed.title || feed.description != form.description {
            feed.tweaked_profile_data = true;
          }

          feed.title = form.title.clone();
          feed.description = form.description.clone();

          let result = feed.save(db).await;
          let dest = feed.permalink_url();

          match result {
            Ok(_result) => {
              let _ = request_feed_update(&feed).await;

              // Ok(Flash::success(redirect_to(dest), "Feed updated!"))
              Ok(crate::utils::redirect_to(&dest))
            },
            // Err(_why) => Ok(Flash::error(redirect_to(dest), "Sorry, something went wrong!"))
            Err(_why) => Ok(crate::utils::redirect_to(&dest))
          }
        },
        None => Err(AppError::NotFound)
      }
    },
    Err(_why) => Err(AppError::NotFound)
  }
}

///
/// Take a potential URL/name for a feed and check if they are valid
///
#[post("/test-feed")]
pub async fn test_feed(session: Session,  db: web::Data<PgPool>, form: web::Json<FeedForm>) -> Result<impl Responder, AppError> {
  let db = db.as_ref();

  let Some(_user) = User::from_session(&session, db).await? else {
    return Ok(HttpResponse::build(actix_web::http::StatusCode::UNAUTHORIZED).finish());
  };

  // check if feed name is already in use
  let feed_exists = Feed::exists_by_name(&form.name, db).await;

  if feed_exists.is_ok() && feed_exists.unwrap() {
    let guts = serde_json::to_string(&FeedLookup {
      src: form.url.to_string(),
      url: form.url.to_string(),
      error: Some("Sorry, that username is already taken".to_string())
    }).unwrap();
    return Ok(HttpResponse::build(StatusCode::OK).content_type(ContentType::json()).body(guts))
  }

  let output_url = form.url.to_string();
  log::info!("Feed test: {output_url:}");
  
  // check if feed is valid
  let url = url_to_feed_url(&form.url).await;

  match url {
    Err(why) => {
      log::info!("Feed test: {output_url:} {why:}");
      Err(AppError::NotFound)
    },
    Ok(result) => {
      if let Some(result) = result {
        let guts = serde_json::to_string(&FeedLookup {
          src: form.url.to_string(),
          url: result,
          error: None
        }).unwrap();
        Ok(HttpResponse::build(StatusCode::OK).content_type(ContentType::json()).body(guts))
      } else {
        Err(AppError::NotFound)
      }
    }
  }

}

///
/// Delete a feed
///
#[delete("/feed/{id}/delete")]
pub async fn delete_feed(session: Session, path: web::Path<i32>, db: web::Data<PgPool>) -> Result<impl Responder, AppError> {
  let id = path.into_inner();
  let db = db.as_ref();
  let Some(user) = User::from_session(&session, db).await? else {
    return Err(AppError::NotFound)      
  };

  let feed = Feed::delete(&user, id, db).await;
  
  match feed {
    Ok(_feed) => {
      Ok(crate::utils::redirect_to("/"))
    },
    Err(why) => {
      print!("{why}");
      Err(AppError::NotFound)
    }
  }
}


///
/// show a feed's HTML output
///
#[get("/feed/{username}")]
pub async fn show_feed(
  request: HttpRequest,
  session: Session,
  path: web::Path<String>,
  tmpl: web::Data<tera::Tera>,
  db: web::Data<PgPool>) -> Result<HttpResponse, AppError> {


  let username = path.into_inner();
  let db = db.as_ref();
  let tmpl = tmpl.as_ref();

  let Some(feed) = Feed::find_by_name(&username.to_string(), db).await? else {
    return Err(AppError::NotFound)
  };

  let json_content_type = HeaderValue::from_static("application/json");
  let content_type = request.head().headers().get("content-type").unwrap_or(&json_content_type);

  if content_type.to_str().unwrap().contains("text/html") {
    let user = User::from_session(&session, db).await?;
    render_html_feed(&user, &feed, tmpl, db).await
  } else {
    render_json_feed(&feed, tmpl, db).await
  } // content_type != text/html
}

async fn render_json_feed(feed: &Feed, tera: &tera::Tera, db:&sqlx::Pool<sqlx::Postgres>) -> Result<HttpResponse, AppError> {
  let ap = feed.to_activity_pub(tera, db).await;
  match ap {
    Ok(ap) => Ok(activity_json_response!(ap)),
    Err(why) => {
      log::info!("{:?}", why);
      Err(AppError::NotFound)
    }
  }
}

async fn render_html_feed(user: &Option<User>, feed: &Feed, tmpl: &tera::Tera, db:&sqlx::Pool<sqlx::Postgres>) -> Result<HttpResponse, AppError> {
  let logged_in = user.is_some();
  let owned_by = logged_in && user.as_ref().unwrap().id == feed.user_id;
  let follow_url = feed.permalink_url();

  let items = if !owned_by && !feed.show_statuses_in_outbox() {
    Ok(Vec::<Item>::new())
  } else {
    Item::for_feed(feed, 10, db).await
  };
  
  let username = if let Some(user) = &user {
    user.full_username()
  } else {
    None
  };

  match items {
    Ok(items) => {
      let mut context = tera::Context::new();
      context.insert("instance_domain", &env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set"));
      context.insert("is_admin", &feed.is_admin());
      context.insert("noindex", &!feed.listed);
      context.insert("logged_in", &logged_in);
      context.insert("username", &username);
      context.insert("owned_by", &owned_by);
      context.insert("feed", &feed);
      context.insert("items", &items);
      context.insert("follow_url", &follow_url);
      context.insert("added", &false);
    
      let body = templates::render("feed.html.tera", tmpl, &context)?;

      Ok(HttpResponse::build(StatusCode::OK).content_type("text/html").body(body))
    },
    Err(_why) => Err(AppError::NotFound)
  }  

}

///
/// Render the AP data for a feed's followers
///
#[get("/feed/{username}/followers")]
pub async fn render_feed_followers(path: web::Path<String>, query: web::Query<PageQuery>, db: web::Data<PgPool>) -> Result<actix_web::HttpResponse, AppError> {
  let username = path.into_inner();
  let db = db.as_ref();

  let Some(feed) = Feed::find_by_name(&username.to_string(), db).await? else {
    return Err(AppError::NotFound)
  };

  // if we got a page param, return a page of followers
  // otherwise, return the summary
  let response = match query.page {
    Some(page) => {
      let result = feed.followers_paged(page, db).await;
      match result {
        Ok(result) => Ok(activity_json_response!(serde_json::to_string(&result).unwrap())),
        Err(_why) => Err(AppError::NotFound)
      }
    },
    None => {
      let result = feed.followers(db).await;
      match result {
        Ok(result) => Ok(activity_json_response!(serde_json::to_string(&result).unwrap())),
        Err(_why) => Err(AppError::NotFound)
      }
    }
  };
  
  Ok(response.unwrap())

}

#[cfg(test)]
mod test {
  use activitystreams::mime;
  use actix_web::http::header;
  use actix_web::{test, dev::Service};
  use actix_session::{SessionMiddleware, storage::CookieSessionStore};

  use chrono::Utc;

  use crate::{build_test_server, constants::ACTIVITY_JSON};
  use crate::utils::test_helpers::{real_user, real_feed, real_item};
  use crate::assert_ok_activity_json;

  use crate::models::Feed;

  use sqlx::postgres::PgPool;
  
  #[sqlx::test]
  async fn test_show_feed(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();

    for _i in 1..4 {
      real_item(&feed, &pool).await?;
    }

    let server = test::init_service(build_test_server!(pool)).await;
    let req = test::TestRequest::with_uri(&feed.ap_url()).insert_header(header::ContentType::html()).to_request();
    let res = server.call(req).await.unwrap();

    assert_eq!(res.status(), actix_web::http::StatusCode::OK);
    
    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();

    assert!(body.contains(&format!("Feed for {}", feed.name)));
    assert!(body.contains(&"Posted at"));

    Ok(())
  }
  
  #[sqlx::test]
  async fn test_show_feed_direct_publicity(pool: PgPool) -> sqlx::Result<()> {
    let mut feed:Feed = real_feed(&pool).await?;
    feed.status_publicity = Some("direct".to_string());
    feed.save(&pool).await?;

    for _i in 1..4 {
      real_item(&feed, &pool).await?;
    }

    let server = test::init_service(build_test_server!(pool)).await;
    let req = test::TestRequest::with_uri(&feed.ap_url()).insert_header(header::ContentType::html()).to_request();
    let res = server.call(req).await.unwrap();

    // let req = client.get(uri!(super::show_feed(&feed.name, None::<i32>))).header(Header::new("Accept", "text/html"));

    assert_eq!(res.status(), actix_web::http::StatusCode::OK);
    
    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();

    assert!(body.contains(&format!("Feed for {}", feed.name)));
    assert!(body.contains(&"No entries"));

    Ok(())
  }

  #[sqlx::test]
  async fn test_render_feed(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();


    let server = test::init_service(build_test_server!(pool)).await;
    let content_type: mime::Mime = ACTIVITY_JSON.parse().unwrap();
    let req = test::TestRequest::with_uri(&feed.ap_url()).insert_header(header::ContentType(content_type)).to_request();

    let res = server.call(req).await.unwrap();

    let name = feed.name;

    assert_ok_activity_json!(res);

    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();

    assert!(body.contains("-----BEGIN PUBLIC KEY-----"));
    assert!(body.contains(&name));

    Ok(())
  }

  #[sqlx::test]
  async fn test_render_feed_text_accept(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();

    let server = test::init_service(build_test_server!(pool)).await;
    let req = test::TestRequest::with_uri(&feed.ap_url()).insert_header(header::Accept(vec![
      header::QualityItem::max(mime::TEXT_PLAIN),
    ])).to_request();
    let res = server.call(req).await.unwrap();

    let name = feed.name;

    assert_ok_activity_json!(res);

    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();

    assert!(body.contains("-----BEGIN PUBLIC KEY-----"));
    assert!(body.contains(&name));

    Ok(())
  }

  #[sqlx::test]
  async fn test_render_feed_json_accept(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();

    let server = test::init_service(build_test_server!(pool)).await;
    let req = test::TestRequest::with_uri(&feed.ap_url()).insert_header(header::Accept(vec![
      header::QualityItem::max(mime::APPLICATION_JSON),
    ])).to_request();
    let res = server.call(req).await.unwrap();

    let name = feed.name;

    assert_ok_activity_json!(res);

    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    assert!(std::str::from_utf8(&bytes).unwrap().contains(&name));
    assert!(std::str::from_utf8(&bytes).unwrap().contains("-----BEGIN PUBLIC KEY-----"));

    Ok(())
  }

  #[sqlx::test]
  async fn test_test_feed(pool: PgPool) -> sqlx::Result<()> {
    let user = real_user(&pool).await.unwrap();

    let server = test::init_service(build_test_server!(pool)).await;

    let req = test::TestRequest::with_uri(&format!("/user/auth/{}", &user.login_token)).to_request();
    let res = server.call(req).await.unwrap();
    assert_eq!(res.status(), actix_web::http::StatusCode::TEMPORARY_REDIRECT);

    let session_cookies = res.response().cookies();

    let url: String = "https://muffinlabs.com/".to_string();
    let name: String = "testfeed".to_string();

    // let json = format!(r#"{{"name":"{}","url": "{}"}}"#, name, url).to_string();

    let json = crate::routes::feeds::FeedForm {
      name: name,
      url: url
    };
    let mut req = test::TestRequest::post().uri("/test-feed").set_json(json);
    for cookie in session_cookies {
      req = req.cookie(cookie.clone());
    } 

    let req = req.to_request();
    let res = server.call(req).await.unwrap();
    
    assert_eq!(res.status(), actix_web::http::StatusCode::OK);
    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();

    assert!(body.contains(r#"{"src":"https://muffinlabs.com/","url":"https://muffinlabs.com/atom.xml","error":null}"#));

    Ok(())
  }


  #[sqlx::test]
  async fn test_test_feed_not_logged_in(pool: PgPool) -> sqlx::Result<()> {
    let server = test::init_service(build_test_server!(pool)).await;

    let url: String = "https://muffinlabs.com/".to_string();
    let name: String = "testfeed".to_string();

    let json = crate::routes::feeds::FeedForm {
      name: name,
      url: url
    };
    let req = test::TestRequest::post().uri("/test-feed").set_json(json).to_request();

    let res = server.call(req).await.unwrap();

    assert_eq!(res.status(), actix_web::http::StatusCode::UNAUTHORIZED);

    Ok(())
  }

  #[sqlx::test]
  async fn test_render_feed_followers(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();
    let now = Utc::now();

    for i in 1..35 {
      let actor = format!("https://activitypub.pizza/users/colin{}", i);
      sqlx::query!("INSERT INTO followers (feed_id, actor, created_at, updated_at) VALUES($1, $2, $3, $4)", feed.id, actor, now, now)
        .execute(&pool)
        .await
        .unwrap();
    }
    
    let server = test::init_service(build_test_server!(pool)).await;
    let req = test::TestRequest::with_uri(&feed.followers_paged_url(2)).to_request();
    let res = server.call(req).await.unwrap();

    assert_ok_activity_json!(res);

    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();
 
    assert!(body.contains("OrderedCollectionPage"));
    assert!(body.contains("/colin11"));
    assert!(body.contains("/colin12"));
    assert!(body.contains("/colin13"));

    assert!(body.contains(&format!(r#"first":"{}"#, &feed.followers_paged_url(1))));
    assert!(body.contains(&format!(r#"prev":"{}"#, &feed.followers_paged_url(1))));
    assert!(body.contains(&format!(r#"next":"{}"#, &feed.followers_paged_url(3))));
    assert!(body.contains(&format!(r#"last":"{}"#, &feed.followers_paged_url(4))));
    assert!(body.contains(&format!(r#"current":"{}"#, &feed.followers_paged_url(2))));
    
    Ok(())
  }
}
