use rocket::get;
use rocket::http::Status;
use rocket::State;


use sqlx::sqlite::SqlitePool;

use std::env;

use crate::feed::Feed;

use webfinger::*;


// GET /.well-known/webfinger?resource=acct:crimeduo@botsin.space
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
    Err(Status::NotFound)
  } else {
    let userstr = user.to_string();
    print!("{}", userstr);
  
    let feed = Feed::find_by_name(&userstr, db).await;
    match feed {
      Ok(_feed) => Ok(serde_json::to_string(&Webfinger {
        subject: userstr.clone(),
        aliases: vec![userstr.clone()],
        links: vec![Link {
          rel: "http://webfinger.net/rel/profile-page".to_string(),
          mime_type: None,
          href: Some(format!("https://{}/feed/{}/", instance_domain, userstr)),
          template: None,
        }],
      }).unwrap()),
      Err(_why) => Err(Status::NotFound)
    }
  }
}
