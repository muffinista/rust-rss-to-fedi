use sqlx::sqlite::SqlitePool;
use serde::{Deserialize, Serialize};

use reqwest;
use feed_rs::parser;
//use feed_rs::model::Feed;
use feed_rs::parser::{ParseFeedError, ParseFeedResult};

use std::{error::Error, fmt};

use crate::user::User;

#[derive(Debug, Serialize)]
pub struct Feed {
  pub id: i64,
  pub user_id: i64,
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
  
  pub async fn create(user: &User, url: &String, pool: &SqlitePool) -> Result<Feed, sqlx::Error> {
    let feed_id = sqlx::query!("INSERT INTO feeds (user_id, url) VALUES($1, $2)", user.id, url)
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

    pub fn feed_to_entries(&self, data: feed_rs::model::Feed) -> Result<(), FeedError> {
        for entry in data.entries.iter() {
            println!("Got: {:?}", entry);
        }
        Ok(())
    }
    
    pub async fn parse_data(&self, body: String) -> Result<(), FeedError> {        
        // println!("{}", body);
        let data = parser::parse(body.as_bytes());

        match data {
            Ok(data) => Feed::feed_to_entries(self, data),
            Err(why) => return Err(FeedError)
        }
    }

    pub async fn parse(&self) -> Result<(), FeedError> {        
        let body = Feed::load(self).await;
        match body {
            Ok(body) => {
                // println!("{}", body);
                let data = parser::parse(body.as_bytes());
                
                match data {
                    Ok(data) => Feed::feed_to_entries(self, data),
                    Err(why) => return Err(FeedError)
                }
            },
            Err(why) => return Err(FeedError)
        }
            



        //data.entries

        //Ok(())
    }
}
  
  
#[sqlx::test]
async fn test_create(pool: SqlitePool) -> sqlx::Result<()> {
  let email:String = "foo@bar.com".to_string();
  let user = User::find_or_create_by_email(&email, &pool).await?;

  let url:String = "https://foo.com/rss.xml".to_string();
  let feed = Feed::create(&user, &url, &pool).await?;
  
  assert_eq!(feed.url, url);
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
