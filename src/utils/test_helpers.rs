use sqlx::postgres::PgPool;

use crate::models::User;
use crate::models::Feed;
use crate::models::Follower;
use crate::models::Item;
use crate::models::Actor;
use crate::models::Enclosure;
use crate::utils::keys::generate_key;

use crate::server::build_server;

use chrono::Utc;
use uuid::Uuid;

use rocket::uri;
use rocket::{Rocket, Build};


pub async fn build_test_server(pool: PgPool) -> Rocket<Build> {
  build_server(pool).await
}

pub async fn login_user(client: &rocket::local::asynchronous::Client, user: &User) {
  // login the user
  let post = client.get(uri!(crate::routes::login::attempt_login(&user.login_token)));
  post.dispatch().await;
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
    updated_at: Utc::now() 
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



pub async fn real_actor(pool: &PgPool) -> sqlx::Result<Actor> {
  Actor::create(
    &"https://foo.com/users/user".to_string(),
    &"https://foo.com/users/user/inbox".to_string(),
    &"public_key_id".to_string(),
    &"public_key".to_string(),
    Some("username".to_string()),
    &pool).await?;
  
  let actor: Actor = Actor::find_or_fetch(&"https://foo.com/users/user".to_string(), &pool).await.unwrap().unwrap();

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
    error: None,
    error_count: 0
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
    updated_at: Utc::now()
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

