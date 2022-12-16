use sqlx::sqlite::SqlitePool;
use serde::{Serialize};

use feed_rs::model::Entry;

use crate::feed::Feed;

use activitystreams::activity::*;
use activitystreams::object::ApObject;
use activitystreams::object::Note;
use activitystreams::iri;
use activitystreams::base::BaseExt;
use activitystreams::base::ExtendsExt;
use activitystreams::object::ObjectExt;

use anyhow::Error as AnyError;

use activitystreams::{
  security,
  context
};


use rocket::futures::TryStreamExt;

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

  pub async fn exists_by_guid(guid: &String, feed: &Feed, pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM items WHERE feed_id = ? AND guid = ?", feed.id, guid)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally > 0),
      Err(why) => Err(why)
    }
  }

  pub async fn create_from_entry(entry: &Entry, feed: &Feed, pool: &SqlitePool) -> Result<Item, sqlx::Error> {
    let title = &entry.title.as_ref().unwrap().content;
    let body = &entry.content.as_ref().unwrap().body;
  
    // println!("Create: {:?}", entry.id);

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

  pub fn to_activity_pub(&self) -> Result<ApObject<Create>, AnyError> {    
    // we could return an object here instead of JSON so we can manipulate it if needed
    // pub fn to_activity_pub(&self) -> Result<ExtendedService, AnyError> {    

    let mut note: ApObject<Note> = ApObject::new(Note::new());

    note
      //.set_id(iri!(path_to_url(&uri!(render_feed(&self.name)))))
      .set_attributed_to(iri!("https://create.pizza/"))
      .set_content("Hello")
      .set_url(iri!("https://create.pizza/"))
      .set_cc(iri!("https://www.w3.org/ns/activitystreams#Public"));
      //.set_published(self.created_at)

    let mut action: ApObject<Create> = ApObject::new(Create::new(iri!("https://create.pizza/"), note.into_any_base()?));

    action
      .set_context(context())
      .add_context(security());

    Ok(action)


    // if returning an object makes sense we can do something like this:
    // let any_base = svc.into_any_base();
    // //    println!("any base: {:?}", any_base);
    
    // match any_base {
    //   Ok(any_base) => {
    //     let x = ExtendedService::from_any_base(any_base).unwrap();
        
    //     match x {
    //       Some(x) => {
    //         println!("JSON: {:?}", serde_json::to_string(&x).unwrap());
    //         Ok(x)
    //       },
    //       None => todo!()
    //     }
    //   },
    //   Err(_) => todo!()
    // }
    
  }

  pub async fn deliver(&self, feed: &Feed, pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let followers = feed.followers_list(pool).await?;
    for follwer in followers { 
      // generate and send
    };

    Ok(())
  }

  // pub async fn delete(feed: &Feed, id: i64, pool: &SqlitePool) -> Result<Item, sqlx::Error> {
  //   let old_item = Item::find(id, pool).await;
    
  //   sqlx::query!("DELETE FROM items WHERE feed_id = $1 AND id = $2", feed.id, id)
  //   .execute(pool)
  //   .await?;
    
  //   old_item   
  // }
}



#[cfg(test)]
mod test {
  // use sqlx::sqlite::SqlitePool;

  use crate::Item;

  #[sqlx::test]
  async fn test_to_activity_pub() -> Result<(), String> {
    let item:Item = Item { id: 1, feed_id: 1, guid: "12345".to_string(), title: Some("Hello!".to_string()), content: Some("Hey!".to_string()) };

    let result = item.to_activity_pub();
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();
        println!("{}", s);
        
        assert!(s.contains("Hello"));

        Ok(())
      },
      Err(why) => Err(why.to_string())
    }
  }
}