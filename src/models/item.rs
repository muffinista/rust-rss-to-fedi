use sqlx::sqlite::SqlitePool;
use serde::{Serialize};
use feed_rs::model::Entry;

use crate::models::feed::Feed;
use crate::services::mailer::*;

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

use activitystreams::time::OffsetDateTime;

use rocket_dyn_templates::tera::Tera;
use rocket_dyn_templates::tera::Context;

use url::Url;


#[derive(Debug, Serialize)]
pub struct Item {
  pub id: i64,
  pub feed_id: i64,
  pub guid: String,
  pub title: Option<String>,
  pub content: Option<String>,
  pub url: Option<String>,
  pub created_at: chrono::NaiveDateTime,
  pub updated_at: chrono::NaiveDateTime

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

    let item_url = if entry.links.len() > 0 {
      Some(&entry.links[0].href)
    } else {
      None
    };
    
    // println!("Create: {:?}", entry.id);

    let item_id = sqlx::query!("INSERT INTO items (feed_id, guid, title, content, url, created_at, updated_at) VALUES($1, $2, $3, $4, $5, datetime(CURRENT_TIMESTAMP, 'utc'), datetime(CURRENT_TIMESTAMP, 'utc'))",
                               feed.id,
                               entry.id,
                               title,
                               body,
                               item_url
    )
      .execute(pool)
      .await?
      .last_insert_rowid();
    Item::find(item_id, pool).await
  }


  ///
  /// generate an HTML-ish version of this item suitable
  /// for adding to an AP message
  ///
  pub fn to_html(&self) -> String {
    let tera = match Tera::new("templates/ap/*.*") {
      Ok(t) => t,
      Err(e) => {
        println!("Parsing error(s): {}", e);
        ::std::process::exit(1);
      }
    };

    let mut context = Context::new();
    context.insert("title", &self.title);
    context.insert("body", &self.content);
    if self.url.is_some() {
      context.insert("link", &self.url.as_ref().unwrap());
    }
    
    tera.render("feed-item.html.tera", &context).unwrap()
  }

  
  pub fn to_activity_pub(&self, feed: &Feed) -> Result<ApObject<Create>, AnyError> {    
    let mut note: ApObject<Note> = ApObject::new(Note::new());

    let feed_url = feed.ap_url();
    let item_url = format!("{}/items/{}", feed_url, self.id);
    let ts = OffsetDateTime::from_unix_timestamp(self.created_at.timestamp()).unwrap();

    note
      .set_attributed_to(iri!(feed_url))
      .set_content(self.to_html())
      // @todo direct url to item
      .set_url(iri!(feed_url))
      .set_cc(iri!("https://www.w3.org/ns/activitystreams#Public"))
      .set_id(iri!(item_url))
      .set_published(ts);

    let mut action: ApObject<Create> = ApObject::new(
      Create::new(
        iri!(feed_url),
        note.into_any_base()?
      )
    );

    action
      .set_context(context())
      .add_context(security());

    Ok(action)
  }

  pub async fn delete(feed: &Feed, id: i64, pool: &SqlitePool) -> Result<Item, sqlx::Error> {
    let old_item = Item::find(id, pool).await;
    
    sqlx::query!("DELETE FROM items WHERE feed_id = $1 AND id = $2", feed.id, id)
    .execute(pool)
    .await?;
    
    old_item   
  }


  pub async fn deliver(&self, feed: &Feed, pool: &SqlitePool) -> Result<(), AnyError> {
    let message = self.to_activity_pub(feed).unwrap();
    let followers = feed.followers_list(pool).await?;
    for follower in followers { 
      let inbox = follower.find_inbox().await;
      match inbox {
        Ok(inbox) => {
          println!("INBOX: {}", inbox);
          // generate and send
          let mut targeted = message.clone();

          targeted.set_many_tos(vec![iri!(inbox)]);
          
          let msg = serde_json::to_string(&targeted).unwrap();
          println!("{}", msg);

          let result = deliver_to_inbox(&Url::parse(&inbox)?, &feed.ap_url(), &feed.private_key, &msg).await;

          match result {
            Ok(result) => println!("sent! {:?}", result),
            Err(why) => println!("failure! {:?}", why)
          }

        },
        Err(why) => {
          println!("failure! {:?}", why);
          // @todo retry! mark as undeliverable? delete user?
          // panic!("oops!");
        }
      }
    };

    Ok(())
  }
}


#[cfg(test)]
mod test {
  use sqlx::sqlite::SqlitePool;

  use crate::models::feed::Feed;
  use crate::models::item::Item;
  use crate::utils::keys::*;

  use chrono::Utc;
  use mockito::mock;
  use uuid::Uuid;

  fn fake_feed() -> Feed {
    let (private_key_str, public_key_str) = generate_key();

    Feed {
      id: 1,
      user_id: 1,
      name: "muffinfeed".to_string(),
      url: "https://foo.com/rss.xml".to_string(),
      private_key: private_key_str.to_string(),
      public_key: public_key_str.to_string(),
      image_url: Some("https://foo.com/image.png".to_string()),
      icon_url: Some("https://foo.com/image.ico".to_string()),
      description: None,
      site_url: None,
      title: None, created_at: Utc::now().naive_utc(), updated_at: Utc::now().naive_utc()
    }
  }

  fn fake_item() -> Item {
    Item {
      id: 1,
      feed_id: 1,
      guid: "12345".to_string(),
      title: Some("Hello!".to_string()),
      content: Some("Hey!".to_string()),
      url: Some("http://google.com".to_string()),
      created_at: Utc::now().naive_utc(),
      updated_at: Utc::now().naive_utc()
    }
  }

  async fn real_item(feed: &Feed, pool: &SqlitePool) -> sqlx::Result<Item> {
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

  #[sqlx::test]
  async fn test_find(pool: SqlitePool) -> sqlx::Result<()> {
    let feed: Feed = fake_feed();
    let item: Item = real_item(&feed, &pool).await?;

    let item2 = Item::find(item.id, &pool).await?;
    
    assert_eq!(item, item2);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_find_by_guid(pool: SqlitePool) -> sqlx::Result<()> {
    let feed: Feed = fake_feed();
    let item: Item = real_item(&feed, &pool).await?;

    let item2 = Item::find_by_guid(&item.guid, &feed, &pool).await?;
    
    assert_eq!(item, item2);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_exists_by_guid(pool: SqlitePool) -> sqlx::Result<()> {
    let feed: Feed = fake_feed();
    let item: Item = real_item(&feed, &pool).await?;

    let result = Item::exists_by_guid(&item.guid, &feed, &pool).await?;   
    assert_eq!(true, result);

    let bad_guid = format!("{}sdfsdfsdf", item.guid);
    let result = Item::exists_by_guid(&bad_guid, &feed, &pool).await?;   
    assert_eq!(false, result);

    Ok(())
  }


  #[sqlx::test]
  async fn test_to_activity_pub() -> Result<(), String> {
    let feed: Feed = fake_feed();
    let item: Item = fake_item();

    let result = item.to_activity_pub(&feed);
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();
        
        assert!(s.contains("Hello!"));
        assert!(s.contains("<p>Hey!</p>"));

        Ok(())
      },
      Err(why) => Err(why.to_string())
    }
  }
  
  #[sqlx::test]
  async fn test_deliver(pool: SqlitePool) -> Result<(), String> {
    let feed: Feed = fake_feed();
    let item: Item = fake_item();

    let actor = format!("{}/users/colin", &mockito::server_url());
    let profile = format!("{{\"inbox\": \"{}/users/colin/inbox\"}}", &mockito::server_url());

    let _m = mock("GET", "/users/colin")
      .with_status(200)
      .with_header("Accept", "application/ld+json")
      .with_body(profile)
      .create();

    let _m2 = mock("POST", "/users/colin/inbox")
      .with_status(202)
      .create();



    let _follower = feed.add_follower(&pool, &actor).await;

    let result = item.deliver(&feed, &pool).await;
    match result {
      Ok(_result) => {
        Ok(())
      },
      Err(why) => Err(why.to_string())
    }
  }
}
