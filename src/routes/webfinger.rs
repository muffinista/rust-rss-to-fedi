use std::env;

use actix_web::http::StatusCode;
use actix_web::{HttpRequest, HttpResponse};
use actix_web::{get, web, Responder};

use sqlx::postgres::PgPool;

use webfinger::*;

use crate::models::feed_error::AppError;
use crate::models::Feed;
use crate::ACTIVITY_JSON;


///
/// Respond to webfinger requests
///
#[get("/.well-known/webfinger")]
pub async fn lookup_webfinger(req: HttpRequest, db: web::Data<PgPool>) -> Result<impl Responder, AppError> {
  let resource = req.query_string();
  let db = db.as_ref();
  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  
  // https://github.com/Plume-org/webfinger/blob/main/src/async_resolver.rs
  let mut parsed_query = resource.splitn(2, ':');
  let _res_prefix = Prefix::from(parsed_query.next().ok_or(AppError::NotFound)?);
  let res = parsed_query.next().ok_or(AppError::NotFound)?;
  
  let mut parsed_res = res.splitn(2, '@');
  let user = parsed_res.next().ok_or(AppError::NotFound)?;
  let domain = parsed_res.next().ok_or(AppError::NotFound)?;

  if domain != instance_domain {
    return Err(AppError::NotFound)
  }
  
  let userstr = user.to_string();

  // ensure feed exists
  let feed = Feed::find_by_name(&userstr, db).await?;

  if feed.is_some() {
    let href = feed.expect("Feed missing?").permalink_url();
    let results = serde_json::to_string(&Webfinger {
      subject: format!("acct:{}@{}", userstr.clone(), instance_domain),
      aliases: vec![userstr.clone()],
      links: vec![
        Link {
          rel: "http://webfinger.net/rel/profile-page".to_string(),
          mime_type: None,
          href: Some(href.clone()),
          template: None,
        },
        Link {
          rel: "self".to_string(),
          mime_type: Some(ACTIVITY_JSON.to_string()),
          href: Some(href),
          template: None,
        }
      ],
    }).unwrap();

    Ok(HttpResponse::build(StatusCode::OK).content_type("application/jrd+json").body(results))
  }
  else {
    Err(AppError::NotFound)
  }
}


#[cfg(test)]
mod test {
  use actix_web::{test, dev::Service};
  use actix_session::{SessionMiddleware, storage::CookieSessionStore};

  use sqlx::postgres::PgPool;
  use std::env;
  use crate::utils::test_helpers::real_feed;
  use crate::build_test_server;
  use crate::assert_content_type;

  
  #[sqlx::test]
  async fn test_lookup_webfinger_404(pool: PgPool) {
    let server = test::init_service(build_test_server!(pool)).await;
    let req = test::TestRequest::with_uri("/.well-known/webfinger?acct:foo@bar.com").to_request();
    let res = server.call(req).await.unwrap();
    assert_eq!(res.status(), actix_web::http::StatusCode::NOT_FOUND);
  }
  
  #[sqlx::test]
  async fn test_lookup_webfinger_valid(pool: PgPool) -> sqlx::Result<()> {
    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

    let feed = real_feed(&pool).await.unwrap();
    
    let server = test::init_service(build_test_server!(pool)).await;
    let req = test::TestRequest::with_uri(&format!("/.well-known/webfinger?acct:{}@{}", &feed.name, instance_domain)).to_request();
    let res = server.call(req).await.unwrap();
    assert!(res.status().is_success());

    assert_content_type!(res, "application/jrd+json");

    let bytes = actix_web::body::to_bytes(res.into_body()).await.unwrap();
    assert!(std::str::from_utf8(&bytes).unwrap().contains(&format!(r#"href":"https://{}/feed/{}"#, instance_domain, &feed.name)));
    
    Ok(())
  }
}
