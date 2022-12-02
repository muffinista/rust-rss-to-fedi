use sqlx::sqlite::SqlitePool;
use serde::{Serialize};

use reqwest;
use feed_rs::parser;

use std::{error::Error, fmt};

use crate::user::User;
use crate::item::Item;

use activitystreams::{
  context
};

use activitystreams::base::BaseExt;
use activitystreams::{actor::{ApActor, ApActorExt, AsApActor, Service}, iri};


use anyhow::Error as AnyError;


#[derive(Debug, Serialize)]
pub struct Feed {
  pub id: i64,
  pub user_id: i64,
  pub name: String,
  pub url: String
}

impl PartialEq for Feed {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

#[derive(Debug)]
pub struct FeedError;

impl Error for FeedError {}
impl fmt::Display for FeedError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Oh no, something bad went down")
  }
}

impl Feed {
  pub async fn find(id: i64, pool: &SqlitePool) -> Result<Feed, sqlx::Error> {
    sqlx::query_as!(Feed, "SELECT * FROM feeds WHERE id = ?", id)
    .fetch_one(pool)
    .await
  }
  
  pub async fn for_user(user: &User, pool: &SqlitePool) -> Result<Vec<Feed>, sqlx::Error> {
    sqlx::query_as!(Feed, "SELECT * FROM feeds WHERE user_id = ?", user.id)
    .fetch_all(pool)
    .await
  }
  
  pub async fn find_by_url(url: &String, pool: &SqlitePool) -> Result<Feed, sqlx::Error> {
    sqlx::query_as!(Feed, "SELECT * FROM feeds WHERE url = ?", url)
    .fetch_one(pool)
    .await
  }
  
  pub async fn find_by_name(name: &String, pool: &SqlitePool) -> Result<Feed, sqlx::Error> {
    sqlx::query_as!(Feed, "SELECT * FROM feeds WHERE name = ?", name)
    .fetch_one(pool)
    .await
  }
  
  pub async fn create(user: &User, url: &String, name: &String, pool: &SqlitePool) -> Result<Feed, sqlx::Error> {
    let feed_id = sqlx::query!("INSERT INTO feeds (user_id, url, name) VALUES($1, $2, $3)", user.id, url, name)
      .execute(pool)
      .await?
      .last_insert_rowid();
    
    Feed::find(feed_id, pool).await
  }
  
  pub async fn delete(user: &User, id: i64, pool: &SqlitePool) -> Result<Feed, sqlx::Error> {
    let old_feed = Feed::find(id, pool).await;
    
    sqlx::query!("DELETE FROM feeds WHERE user_id = $1 AND id = $2", user.id, id)
    .execute(pool)
    .await?;
    
    old_feed   
  }
  
  pub async fn load(&self) -> Result<String, reqwest::Error> {
    let res = reqwest::get(&self.url).await?;
    
    // Response: HTTP/1.1 200 OK
    // Headers: {
    //     "date": "Tue, 29 Nov 2022 00:48:07 GMT",
    //     "content-type": "application/xml",
    //     "content-length": "68753",
    //     "connection": "keep-alive",
    //     "last-modified": "Tue, 08 Nov 2022 13:54:18 GMT",
    //     "etag": "\"10c91-5ecf5e04f7680\"",
    //     "accept-ranges": "bytes",
    //     "strict-transport-security": "max-age=15724800; includeSubDomains",
    // }
    eprintln!("Response: {:?} {}", res.version(), res.status());
    eprintln!("Headers: {:#?}\n", res.headers());
      
    res.text().await
  }
    
  pub async fn feed_to_entries(&self, data: feed_rs::model::Feed, pool: &SqlitePool) -> Result<Vec<Item>, FeedError> {
    let mut result: Vec<Item> = Vec::new();
    for entry in data.entries.iter() {
      let exists = Item::exists_by_guid(&entry.id, &self, pool).await.unwrap();

      // only create new items
      // @todo update changed items
      if ! exists {
        let item = Item::create_from_entry(&entry, &self, pool).await;
        match item {
          Ok(item) => result.push(item),
          Err(_why) => return Err(FeedError)
        }
      }
    }
    Ok(result)
  }
      
  pub async fn parse(&self, pool: &SqlitePool) -> Result<Vec<Item>, FeedError> {        
    let body = Feed::load(self).await;
    match body {
      Ok(body) => {
        let data = parser::parse(body.as_bytes());
        
        match data {
          Ok(data) => {
            let result = Feed::feed_to_entries(self, data, pool).await;
            match result {
              Ok(result) => Ok(result),
              Err(_why) => return Err(FeedError)
            }
          },
          Err(_why) => return Err(FeedError)
        }
      },
      Err(_why) => return Err(FeedError)
    }
  }

  // @todo we might not need the db here?
  pub fn to_activity_pub(&self, domain: &String, pool: &SqlitePool) -> Result<ApActor<Service>, AnyError> {

    let mut svc:ApActor<Service> = ApActor::new(
      iri!("https://example.com/inbox"),
      Service::new(),
    );

    svc
      .set_context(context())
      .set_id(iri!(format!("https://{}/users/{}/feed", domain, self.name)))
      .set_inbox(iri!(format!("https://{}/users/{}/inbox", domain, self.name)))
      .set_outbox(iri!(format!("https://{}/users/{}/outbox", domain, self.name)));

      // "following": "https://botsin.space/users/crimeduo/following",
      // "followers": "https://botsin.space/users/crimeduo/followers",
      // "inbox": "https://botsin.space/users/crimeduo/inbox",
      // "outbox": "https://botsin.space/users/crimeduo/outbox",
      // "preferredUsername": "crimeduo",
      // "name": "They fight crime!",
      // "publicKey": {
      //   "id": "https://botsin.space/users/crimeduo#main-key",
      //   "owner": "https://botsin.space/users/crimeduo",
      //   "publicKeyPem": "-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA+1ikYWHk8JypXZHCJnI5\nuBIX5dosGJHhzu1neA0vMNknk7h1SVu1rSCkA0dl6RmAAxr7Ohv7Sy2zyaQA9N0v\nmRal0+G7OGTjbV57Qr1b9+BvG710zhSh9l3kAw/2Ml8WLZFVBMWvnlVK8h0Pbnk7\n111fsHF45hotl+QmNGMkkrJfDQ/p+tSKhrSGn5CObu4EsO0hNpMjvGdba1PqCbd3\nNvNdo9cbQ4QKsClxgmCoLpQB9sxw5jzRjIIKp8F1nond/T4T6wsm7mj64yeskCca\n9TKxQA89x8uQ4mQfNuWKRmvQJR2aQKqYT4+hTzTYZ+zGRDAl1BhgQD0b9pgjkSEv\nAQIDAQAB\n-----END PUBLIC KEY-----\n"
      // },
      // "icon": {
      //   "type": "Image",
      //   "mediaType": "image/jpeg",
      //   "url": "https://files.botsin.space/accounts/avatars/109/282/037/155/191/435/original/1884a0545a6fc1bd.jpeg"
      // },
      // "image": {
      //   "type": "Image",
      //   "mediaType": "image/jpeg",
      //   "url": "https://files.botsin.space/accounts/headers/109/282/037/155/191/435/original/372509d3e7032272.jpg"
      // }
      
      


    Ok(svc)
  }
}

  
#[sqlx::test]
async fn test_create(pool: SqlitePool) -> sqlx::Result<()> {
  let email:String = "foo@bar.com".to_string();
  let user = User::find_or_create_by_email(&email, &pool).await?;
  
  let url:String = "https://foo.com/rss.xml".to_string();
  let name:String = "testfeed".to_string();
  let feed = Feed::create(&user, &url, &name, &pool).await?;
  
  assert_eq!(feed.url, url);
  assert_eq!(feed.name, name);
  assert_eq!(feed.user_id, user.id);
  
  Ok(())
}
  
#[sqlx::test]
async fn test_find_by_url(pool: SqlitePool) -> sqlx::Result<()> {
  let email:String = "foo@bar.com".to_string();
  let user = User::find_or_create_by_email(&email, &pool).await?;
  
  let url:String = "https://foo.com/rss.xml".to_string();
  let feed = Feed::create(&user, &url, &pool).await?;
  
  let feed2 = Feed::find_by_url(&url, &pool).await?;
  
  assert_eq!(feed, feed2);
  assert_eq!(feed2.url, url);
  
  Ok(())
}
#[sqlx::test]
async fn test_find_by_name(pool: SqlitePool) -> sqlx::Result<()> {
  let email:String = "foo@bar.com".to_string();
  let user = User::find_or_create_by_email(&email, &pool).await?;
  
  let url:String = "https://foo.com/rss.xml".to_string();
  let feed = Feed::create(&user, &url, &pool).await?;
  
  let feed2 = Feed::find_by_url(&url, &pool).await?;
  
  assert_eq!(feed, feed2);
  assert_eq!(feed2.url, url);
  
  Ok(())
}

#[sqlx::test]
async fn test_find(pool: SqlitePool) -> sqlx::Result<()> {
  let email:String = "foo@bar.com".to_string();
  let user = User::find_or_create_by_email(&email, &pool).await?;
  
  let url:String = "https://foo.com/rss.xml".to_string();
  let feed = Feed::create(&user, &url, &pool).await?;
  
  let feed2 = Feed::find(feed.id, &pool).await?;
  
  assert_eq!(feed, feed2);
  assert_eq!(feed2.url, url);
  
  Ok(())
}

#[sqlx::test]
async fn test_for_user(pool: SqlitePool) -> sqlx::Result<()> {
  let email:String = "foo@bar.com".to_string();
  let user = User::find_or_create_by_email(&email, &pool).await?;
  
  let url:String = "https://foo.com/rss.xml".to_string();
  let _feed = Feed::create(&user, &url, &pool).await?;
  
  let url2:String = "https://foofoo.com/rss.xml".to_string();
  let _feed2 = Feed::create(&user, &url2, &pool).await?;
  
  let feeds = Feed::for_user(&user, &pool).await?; 
  assert_eq!(feeds.len(), 2);
  
  Ok(())
}

#[sqlx::test]
async fn test_delete(pool: SqlitePool) -> sqlx::Result<()> {
  let email:String = "foo@bar.com".to_string();
  let user = User::find_or_create_by_email(&email, &pool).await?;
  
  let url:String = "https://foo.com/rss.xml".to_string();
  let feed = Feed::create(&user, &url, &pool).await?;
  
  let deleted_feed = Feed::delete(&user, feed.id, &pool).await?;
  assert_eq!(feed, deleted_feed);
  
  let feeds = Feed::for_user(&user, &pool).await?; 
  assert_eq!(feeds.len(), 0);
  
  Ok(())
}

#[sqlx::test]
async fn test_feed_to_entries(pool: SqlitePool) -> sqlx::Result<()> {
  use std::fs;
  let feed:Feed = Feed {
    id: 1,
    user_id: 1,
    url: "https://foo.com/rss.xml".to_string()
  };

  let path = "fixtures/test_feed_to_entries.xml";
  let data = parser::parse(fs::read_to_string(path).unwrap().as_bytes()).unwrap();

  let result = Feed::feed_to_entries(&feed, data, &pool).await.unwrap();

  assert_eq!(result.len(), 49);

  // check that reloading the same feed doesn't create more records
  let data2 = parser::parse(fs::read_to_string(path).unwrap().as_bytes()).unwrap();
  let result2 = Feed::feed_to_entries(&feed, data2, &pool).await.unwrap();

  assert_eq!(result2.len(), 0);

  Ok(())
}
