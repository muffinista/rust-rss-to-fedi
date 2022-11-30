use sqlx::sqlite::SqlitePool;
use serde::{Deserialize, Serialize};

use std::{error::Error, fmt};

use feed_rs::model::Entry;

use crate::feed::Feed;


#[derive(Debug, Serialize)]
pub struct Item {
  pub id: i64,
  pub feed_id: i64,
  pub guid: String,
  pub title: Option<String>,
  pub content: Option<String>
}

impl PartialEq for Item {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id || (self.feed_id == other.feed_id && self.guid == other.guid)
  }
}

impl Item {
  pub async fn find(id: i64, pool: &SqlitePool) -> Result<Item, sqlx::Error> {
    sqlx::query_as!(Item, "SELECT * FROM items WHERE id = ?", id)
    .fetch_one(pool)
    .await
  }
  
  pub async fn for_feed(feed: &Feed, pool: &SqlitePool) -> Result<Vec<Item>, sqlx::Error> {
    sqlx::query_as!(Item, "SELECT * FROM items WHERE feed_id = ?", feed.id)
    .fetch_all(pool)
    .await
  }
  
  pub async fn find_by_guid(guid: &String, feed: &Feed, pool: &SqlitePool) -> Result<Item, sqlx::Error> {
    sqlx::query_as!(Item, "SELECT * FROM items WHERE feed_id = ? AND guid = ?", feed.id, guid)
    .fetch_one(pool)
    .await
  }
  
  pub async fn create_from_entry(entry: &Entry, feed: &Feed, pool: &SqlitePool) -> Result<Item, sqlx::Error> {
    let title = &entry.title.as_ref().unwrap().content;
    let body = &entry.content.as_ref().unwrap().body;
  
    println!("Got: {:?}", entry.id);

    let item_id = sqlx::query!("INSERT INTO items (feed_id, guid, title, content) VALUES($1, $2, $3, $4)",
      feed.id,
      entry.id,
      title,
      body)
      .execute(pool)
      .await?
      .last_insert_rowid();
    Item::find(item_id, pool).await
  }
  
  // pub async fn delete(feed: &Feed, id: i64, pool: &SqlitePool) -> Result<Item, sqlx::Error> {
  //   let old_item = Item::find(id, pool).await;
    
  //   sqlx::query!("DELETE FROM items WHERE feed_id = $1 AND id = $2", feed.id, id)
  //   .execute(pool)
  //   .await?;
    
  //   old_item   
  // }
}

#[sqlx::test]
async fn test_create(pool: SqlitePool) -> sqlx::Result<()> {
  let item:Item = Item {
    feed_id: 1,
    guid: Some("abcde".to_string()),
    title: Some("hello".to_string()),
    content: Some("hi there".to_string())
  };

  Item::save(&item, &pool).await?;
    
  Ok(())
}
