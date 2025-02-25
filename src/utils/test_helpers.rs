use std::future::Future;

use mockito::Mock;
use mockito::ServerGuard;
use serde_json::json;
use serde_json::Value;
use sqlx::postgres::PgPool;

use crate::models::User;
use crate::models::Feed;
use crate::models::Follower;
use crate::models::Item;
use crate::models::Actor;
use crate::models::Enclosure;
use crate::utils::keys::generate_key;

use std::collections::HashMap;
use std::time::SystemTime;

use actix_web::http::header::HeaderValue;
use base64::engine::general_purpose;
use base64::Engine;
use httpdate::fmt_http_date;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::sign::Signer;

use chrono::Utc;
use uuid::Uuid;

#[macro_export]
macro_rules! build_test_server {
  ($pool:expr) => {{
    let tera =
      tera::Tera::new("templates/**/*").expect("Parsing error while loading template folder");
    let secret_key = crate::routes::configure::get_secret_key();    
    actix_web::App::new()
      .wrap(SessionMiddleware::new(CookieSessionStore::default(), secret_key.clone()))
      .app_data(actix_web::web::Data::new($pool.clone()))
      .app_data(actix_web::web::Data::new(tera.clone()))
      .configure(|cfg| crate::routes::configure::apply(cfg))
  }}
}

#[macro_export]
macro_rules! assert_content_type {
  ($res:expr, $type:expr) => {
    assert_eq!($type, $res.headers().get("content-type").expect("missing content type!"));
  }
}

#[macro_export]
macro_rules! assert_accepted {
  ($res:expr) => {
    assert_eq!(actix_web::http::StatusCode::ACCEPTED, $res.status());
  }
}

#[macro_export]
macro_rules! assert_ok_activity_json {
  ($res:expr) => {
    assert_eq!(actix_web::http::StatusCode::OK, $res.status());
    assert_eq!(crate::constants::ACTIVITY_JSON, $res.headers().get("content-type").expect("missing content type!"));
  }
}


pub fn sign_test_request(req: &mut actix_http::Request, body: &str, actor_id: &str, private_key: &str) {
  let sig_headers = vec![String::from(crate::constants::REQUEST_TARGET), String::from("host"), String::from("date"), String::from("digest")];
  let path_and_query = req.path();
  let method = req.method();
  let request_target = format!("{} https://test.com{}", method.to_string().to_lowercase(), path_and_query);
  
  let date = fmt_http_date(SystemTime::now());
  let mut values: HashMap<String, String> = HashMap::<String, String>::new();
  values.insert(String::from(crate::constants::REQUEST_TARGET), request_target);
  values.insert(String::from("host"), String::from("muffin.industries"));
  values.insert(String::from("date"), date.clone());

  let digest = crate::utils::string_to_digest_string(body);
  values.insert(String::from("digest"),  digest.clone());

  let signing_string = sig_headers
      .iter()
      .map(|h| {
        let v = values.get(h).unwrap();
        format!("{}: {}", h, v)
      })
      .collect::<Vec<_>>()
      .join("\n");

  let private_key = PKey::private_key_from_pem(private_key.as_bytes()).unwrap();
  let mut signer = Signer::new(MessageDigest::sha256(), &private_key).unwrap();
  signer.update(signing_string.as_bytes()).unwrap();
  let signature_value = general_purpose::STANDARD.encode(signer.sign_to_vec().unwrap());

  let key_id= format!("{}#main-key", actor_id);
  let final_signature = format!("keyId=\"{}\",headers=\"(request-target) host date digest\",signature=\"{}\"",
      key_id, signature_value);


  req.headers_mut().append(actix_web::http::header::HeaderName::from_lowercase(b"host").unwrap(), HeaderValue::from_str(&"muffin.industries").unwrap());
  req.headers_mut().append(actix_web::http::header::HeaderName::from_lowercase(b"date").unwrap(), HeaderValue::from_str(&date).unwrap());
  req.headers_mut().append(actix_web::http::header::HeaderName::from_lowercase(b"digest").unwrap(), HeaderValue::from_str(&digest).unwrap());
  req.headers_mut().append(actix_web::http::header::HeaderName::from_lowercase(b"signature").unwrap(), HeaderValue::from_str(&final_signature).unwrap());
  
}

pub fn deformat_json_string(json: &str) -> String {
  // this is very silly, but by converting/de-converting back to a string, we ensure that we
  // use consistent spacing/formatting/etc when doing digest tests
  let json: Value = serde_json::from_str(&json).unwrap();
  serde_json::to_string(&json).unwrap()
}

pub fn fake_user() -> User {
  User { 
    id: 1, 
    admin: false,
    email: Some("foo@bar.com".to_string()), 
    actor_url: Some("http://foobar.com".to_string()), 
    login_token: "lt".to_string(), 
    access_token: Some("at".to_string()), 
    username: Some("username".to_string()),
    created_at: Utc::now(), 
    updated_at: Utc::now() ,
    access_token_updated_at: Utc::now(),
    login_token_updated_at: Utc::now(),
  }
}

pub async fn real_user(pool: &PgPool) -> sqlx::Result<User> {
  let user:User = User::find_or_create_by_actor_url(&"https:://muffin.pizza/users/test".to_string(), &pool).await?;
  
  Ok(user)
}

pub async fn real_admin_user(pool: &PgPool) -> sqlx::Result<User> {
  let user:User = User::find_or_create_by_actor_url(&"https:://muffin.pizza/users/test".to_string(), &pool).await?;

  sqlx::query!("UPDATE users SET admin = true WHERE id = $1", user.id)
    .execute(pool)
    .await?;
    
  Ok(user)
}

pub fn actor_json(actor_id: &str, server_url: &str, public_key: &str) -> serde_json::Value {
  json!({
    "id": actor_id,
    "type": "Person",
    "owner": format!("{}/actor", server_url),
    "inbox": format!("{}/actor/inbox", server_url),
    "outbox": format!("{}/actor/outbox", server_url),
    "preferredUsername": "actor",
    "name": "actor mcactor",
    "publicKey": {
        "id": format!("{}/actor#main-key", server_url),
        "owner": format!("{}/actor", server_url),
        "publicKeyPem": public_key
    }
  })
}

pub async fn real_actor(pool: &PgPool) -> sqlx::Result<Actor> {
  Actor::create(
    &"https://foo.com/users/user".to_string(),
    &"https://foo.com/users/user/inbox".to_string(),
    &"public_key_id".to_string(),
    &"public_key".to_string(),
    &"username".to_string(),
    &pool).await?;
  
  let actor:Actor = Actor::find_or_fetch(&"https://foo.com/users/user".to_string(), &pool).await.unwrap().unwrap();

  Ok(actor)
}

pub async fn real_feed(pool: &PgPool) -> sqlx::Result<Feed> {
  let user = real_user(&pool).await.unwrap();
  
  let url:String = "https://foo.com/rss.xml".to_string();
  let name = Uuid::new_v4().to_string();

  let feed = Feed::create(&user, &url, &name, &pool).await?;
  
  Ok(feed)
}


pub fn fake_feed() -> Feed {
  let (private_key_str, public_key_str) = generate_key();

  Feed {
    id: 1,
    admin: false,
    user_id: 1,
    name: "testfeed".to_string(),
    url: "https://foo.com/rss.xml".to_string(),
    private_key: private_key_str.to_string(),
    public_key: public_key_str.to_string(),
    image_url: Some("https://foo.com/image.png".to_string()),
    icon_url: Some("https://foo.com/image.ico".to_string()),
    description: None,
    site_url: None,
    title: None,
    listed: false,
    hashtag: None,
    content_warning: None,
    status_publicity: None,
    
    created_at: Utc::now(),
    updated_at: Utc::now(),
    refreshed_at: Utc::now(),
    last_post_at: None,
    language: None,
    error: None,
    error_count: 0,
    tweaked_profile_data: false
  }
}

pub fn fake_follower(feed: &Feed, server: &mockito::Server) -> Follower {
  Follower {
    id: 1,
    feed_id: feed.id,
    actor: format!("{}/users/muffinista", server.url()),
    created_at: Utc::now(),
    updated_at: Utc::now()
  }
}

pub fn fake_item() -> Item {
  Item {
    id: 1,
    feed_id: 1,
    guid: "12345".to_string(),
    title: Some("Hello!".to_string()),
    content: Some("Hey!".to_string()),
    url: Some("http://google.com".to_string()),
    created_at: Utc::now(),
    updated_at: Utc::now(),
    language: None
  }
}

pub async fn real_item(feed: &Feed, pool: &PgPool) -> sqlx::Result<Item> {
  let id = Uuid::new_v4().to_string();
  let item_url = format!("https://foo.com/{}", id);
  let now = Utc::now();

  let item_id = sqlx::query!("INSERT INTO items
                            (feed_id, guid, title, content, url, created_at, updated_at)
                            VALUES($1, $2, $3, $4, $5, $6, $7)
                            RETURNING id",
                            feed.id,
                            id,
                            id,
                            id,
                            item_url,
                            now,
                            now
  )
    .fetch_one(pool)
    .await?
    .id;

  Item::find(item_id, &pool).await
}

pub async fn real_item_with_enclosure(feed: &Feed, pool: &PgPool) -> sqlx::Result<Item> {
  let item = real_item(feed, pool).await?;

  let now = Utc::now();
  let url = "http://media.com/attachment.mp3";
  let content_type = "audio/mpeg";
  let size = 123456;

  sqlx::query!("INSERT INTO enclosures 
    (item_id, url, content_type, size, created_at, updated_at)
    VALUES($1, $2, $3, $4, $5, $6)
    RETURNING id",
      item.id, url, content_type, size, now, now)
    .fetch_one(pool)
    .await?;

  Ok(item)
}


pub async fn real_enclosure(item: &Item, pool: &PgPool) -> sqlx::Result<Enclosure> {
  let now = Utc::now();
  let url = "http://media.com/attachment.mp3";
  let content_type = "audio/mpeg";
  let size = 123456;

  let enclosure_id = sqlx::query!("INSERT INTO enclosures 
    (item_id, url, content_type, size, created_at, updated_at)
    VALUES($1, $2, $3, $4, $5, $6)
    RETURNING id",
      item.id, url, content_type, size, now, now)
    .fetch_one(pool)
    .await?
    .id;

  Enclosure::find(enclosure_id, &pool).await
}

pub fn test_tera() -> tera::Tera {
  tera::Tera::new("templates/**/*").expect("Parsing error while loading template folder")
}

pub fn mock_ap_action(object_server: &mut ServerGuard, path: &str, body: &str) -> impl Future<Output = Mock> {
   object_server.mock("GET", path)
    .with_status(200)
    .with_header("Accept", crate::constants::ACTIVITY_JSON)
    .with_body(body)
    .create_async()
}
