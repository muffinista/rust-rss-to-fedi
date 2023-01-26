use sqlx::sqlite::SqlitePool;
use crate::models::user::User;
use crate::models::feed::Feed;
use crate::models::follower::Follower;
use crate::models::item::Item;
use crate::models::keys::generate_key;

use chrono::Utc;
use uuid::Uuid;

use rocket::uri;


pub async fn login_user(client: &rocket::local::asynchronous::Client, user: &User) {
  // login the user
  let post = client.get(uri!(crate::routes::login::attempt_login(&user.login_token)));
  post.dispatch().await;
}

pub fn fake_user() -> User {
  User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()), created_at: Utc::now().naive_utc(), updated_at: Utc::now().naive_utc() }
}

pub async fn real_user(pool: &SqlitePool) -> sqlx::Result<User> {
  let user:User = User::find_or_create_by_email(&"foo@bar.com".to_string(), &pool).await?;
  
  Ok(user)
}

pub async fn real_feed(pool: &SqlitePool) -> sqlx::Result<Feed> {
  let user = fake_user();
  
  let url:String = "https://foo.com/rss.xml".to_string();
  let name:String = "testfeed".to_string();
  let feed = Feed::create(&user, &url, &name, &pool).await?;
  
  Ok(feed)
}


pub fn fake_feed() -> Feed {
  let (private_key_str, public_key_str) = generate_key();

  Feed {
    id: 1,
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
    created_at: Utc::now().naive_utc(),
    updated_at: Utc::now().naive_utc(),
    refreshed_at: Utc::now().naive_utc(),
    error: None
  }
}

pub fn fake_follower(feed: &Feed) -> Follower {
  Follower {
    id: 1,
    feed_id: feed.id,
    actor: format!("{}/users/muffinista", &mockito::server_url()),
    created_at: Utc::now().naive_utc(),
    updated_at: Utc::now().naive_utc()
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
    enclosure_url: None,
    enclosure_content_type: None,
    enclosure_size: None,
    created_at: Utc::now().naive_utc(),
    updated_at: Utc::now().naive_utc()
  }
}

pub async fn real_item(feed: &Feed, pool: &SqlitePool) -> sqlx::Result<Item> {
  let id = Uuid::new_v4().to_string();
  let item_url = format!("https://foo.com/{}", id);

  let item_id = sqlx::query!("INSERT INTO items
                            (feed_id, guid, title, content, url, created_at, updated_at)
                            VALUES($1, $2, $3, $4, $5, datetime(CURRENT_TIMESTAMP, 'utc'), datetime(CURRENT_TIMESTAMP, 'utc'))",
                            feed.id,
                            id,
                            id,
                            id,
                            item_url
  )
    .execute(pool)
    .await?
    .last_insert_rowid();

  Item::find(item_id, &pool).await
}

