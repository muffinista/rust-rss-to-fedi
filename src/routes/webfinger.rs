use std::env;

use rocket::get;
use rocket::http::Status;
use rocket::State;
use rocket::uri;

use sqlx::sqlite::SqlitePool;

use crate::feed::Feed;

use webfinger::*;
use crate::routes::feeds::*;

use crate::utils::*;

#[get("/.well-known/webfinger?<resource>")]
pub async fn lookup_webfinger(resource: &str, db: &State<SqlitePool>) -> Result<String, Status> {
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
      subject: userstr.clone(),
      aliases: vec![userstr.clone()],
      links: vec![Link {
        rel: "http://webfinger.net/rel/profile-page".to_string(),
        mime_type: None,
        href: Some(href),
        template: None,
      }],
    }).unwrap())
  }
  else {
    Err(Status::NotFound)
  }
}


#[cfg(test)]
mod test {
  use crate::server::build_server;
  use rocket::local::asynchronous::Client;
  use rocket::http::Status;
  use rocket::uri;
  use rocket::{Rocket, Build};
  use crate::user::User;
  use crate::feed::Feed;
  use sqlx::sqlite::SqlitePool;
  use std::env;
  
  #[sqlx::test]
  async fn test_lookup_webfinger_404(pool: SqlitePool) {
    let server:Rocket<Build> = build_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.get(uri!(super::lookup_webfinger("acct:foo@bar.com")));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::NotFound);
  }
  
  #[sqlx::test]
  async fn test_lookup_webfinger_valid(pool: SqlitePool) -> sqlx::Result<()> {
    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

    let user = User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()) };

    let url: String = "https://foo.com/rss.xml".to_string();
    let name: String = "testfeed".to_string();

    Feed::create(&user, &url, &name, &pool).await?;
    
    let server: Rocket<Build> = build_server(pool).await;
    let client = Client::tracked(server).await.unwrap();
    
    let req = client.get(uri!(super::lookup_webfinger(format!("acct:{}@{}", name, instance_domain))));
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);
    
    let body = response.into_string().await.unwrap();
    assert!(body.contains(&format!(r#"href":"https://{}/feed/testfeed"#, instance_domain)));

    Ok(())
  }
}
