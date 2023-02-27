use std::env;

use rocket::get;
use rocket::http::Status;
use rocket::State;
use rocket::uri;

use sqlx::postgres::PgPool;

use webfinger::*;

use crate::models::feed::Feed;
use crate::routes::feeds::*;
use crate::utils::urls::path_to_url;


///
/// Respond to webfinger requests
///
#[get("/.well-known/webfinger?<resource>")]
pub async fn lookup_webfinger(resource: &str, db: &State<PgPool>) -> Result<String, Status> {
  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  
  // https://github.com/Plume-org/webfinger/blob/main/src/async_resolver.rs
  let mut parsed_query = resource.splitn(2, ':');
  let _res_prefix = Prefix::from(parsed_query.next().ok_or(Status::NotFound)?);
  let res = parsed_query.next().ok_or(Status::NotFound)?;
  
  let mut parsed_res = res.splitn(2, '@');
  let user = parsed_res.next().ok_or(Status::NotFound)?;
  let domain = parsed_res.next().ok_or(Status::NotFound)?;

  if domain != instance_domain {
    return Err(Status::NotFound)
  }
  
  let userstr = user.to_string();

  // ensure feed exists
  let feed_exists = Feed::exists_by_name(&userstr, db).await;

  if feed_exists.is_ok() && feed_exists.unwrap() {
    let href = path_to_url(&uri!(render_feed(&userstr)));

    Ok(serde_json::to_string(&Webfinger {
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
          mime_type: Some("application/activity+json".to_string()),
          href: Some(href),
          template: None,
        }
      ],
    }).unwrap())
  }
  else {
    Err(Status::NotFound)
  }
}


#[cfg(test)]
mod test {
  use rocket::local::asynchronous::Client;
  use rocket::http::Status;
  use rocket::uri;
  use rocket::{Rocket, Build};
  use sqlx::postgres::PgPool;
  use std::env;
  use crate::utils::test_helpers::{build_test_server, real_feed};

  
  #[sqlx::test]
  async fn test_lookup_webfinger_404(pool: PgPool) {
    let server:Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::lookup_webfinger("acct:foo@bar.com")));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::NotFound);
  }
  
  #[sqlx::test]
  async fn test_lookup_webfinger_valid(pool: PgPool) -> sqlx::Result<()> {
    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

    let feed = real_feed(&pool).await.unwrap();
    
    let server: Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();
    
    let req = client.get(uri!(super::lookup_webfinger(format!("acct:{}@{}", &feed.name, instance_domain))));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    
    let body = response.into_string().await.unwrap();
    assert!(body.contains(&format!(r#"href":"https://{}/feed/{}"#, instance_domain, &feed.name)));

    Ok(())
  }
}
