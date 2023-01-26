use sqlx::sqlite::SqlitePool;
use serde::{Serialize};
use feed_rs::model::Entry;

use crate::models::feed::Feed;
use crate::services::mailer::*;

use activitystreams::activity::*;
use activitystreams::object::ApObject;
use activitystreams::object::Document;
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

use sanitize_html::sanitize_str;
use sanitize_html::rules::predefined::RELAXED;

use url::Url;
use chrono::Utc;

use activitystreams::mime::Mime;


#[derive(Debug, Serialize)]
pub struct Item {
  pub id: i64,
  pub feed_id: i64,
  pub guid: String,
  pub title: Option<String>,
  pub content: Option<String>,
  pub url: Option<String>,

  pub enclosure_url: Option<String>,
  pub enclosure_content_type: Option<String>,
  pub enclosure_size: Option<i64>,
  
  pub created_at: chrono::NaiveDateTime,
  pub updated_at: chrono::NaiveDateTime
}

fn sanitize_string(input: &String) -> String {
  // relaxed or basic makes sense here probably
  // https://docs.rs/sanitize_html/latest/sanitize_html/rules/predefined/index.html
  sanitize_str(&RELAXED, input).unwrap()
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

  pub async fn find_by_feed_and_id(feed: &Feed, id: i64, pool: &SqlitePool) -> Result<Option<Item>, sqlx::Error> {
    sqlx::query_as!(Item, "SELECT * FROM items WHERE feed_id = ? AND id = ?", feed.id, id)
    .fetch_optional(pool)
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

  pub async fn for_feed(feed: &Feed, limit: i64, pool: &SqlitePool) -> Result<Vec<Item>, sqlx::Error> {
    sqlx::query_as!(Item, "SELECT * FROM items WHERE feed_id = ? ORDER by id DESC LIMIT ?", feed.id, limit)
    .fetch_all(pool)
    .await
  }

  pub async fn create_from_entry(entry: &Entry, feed: &Feed, pool: &SqlitePool) -> Result<Item, sqlx::Error> {
    let title = if entry.title.is_some() {
      Some(&entry.title.as_ref().unwrap().content)
    } else {
      None
    };

    let clean_body;

    // default to summary if we have it
    let body = if entry.summary.is_some() {
      clean_body = sanitize_string(&entry.summary.as_ref().unwrap().content);
      Some(&clean_body)
    }
    else if entry.content.is_some() {
      clean_body = sanitize_string(entry.content.as_ref().unwrap().body.as_ref().unwrap());
      Some(&clean_body)
    } else {
      None
    };

    let item_url = if entry.links.len() > 0 {
      Some(&entry.links[0].href)
    } else {
      None
    };
    
    let enclosure_url;
    let enclosure_content_type;
    let enclosure_size;
    
    if entry.media.len() > 0 && entry.media[0].content.len() > 0 && entry.media[0].content[0].url.is_some() {
      enclosure_url = Some(entry.media[0].content[0].url.as_ref().unwrap().as_str());

      enclosure_content_type = if entry.media[0].content[0].content_type.is_some() {
        Some(entry.media[0].content[0].content_type.as_ref().unwrap().essence_str())
      } else {
        None
      };

      enclosure_size = if entry.media[0].content[0].size.is_some() {
        Some(entry.media[0].content[0].size.unwrap() as i64)
      } else {
        None
      }
      
    } else {
      enclosure_url = None;
      enclosure_content_type = None;
      enclosure_size = None;
    };

    let now = Utc::now().naive_utc();


    let item_id = sqlx::query!("INSERT INTO items 
                                (feed_id, guid, title, content, url, enclosure_url, enclosure_content_type, enclosure_size, created_at, updated_at)
                                VALUES($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
                               feed.id,
                               entry.id,
                               title,
                               body,
                               item_url,
                               enclosure_url,
                               enclosure_content_type,
                               enclosure_size,
                               now,
                               now
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

    if self.enclosure_url.is_some() {
      let mut attachment = Document::new();
      let enclosure_url = self.enclosure_url.clone().unwrap();

      attachment.set_url(iri!(&enclosure_url));

      if self.enclosure_content_type.is_some() {
        let content_type: &String = &self.enclosure_content_type.clone().unwrap();
        attachment.set_media_type(content_type.parse::<Mime>().unwrap());
      }

      note.set_attachment(attachment.into_any_base()?);
    }

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
  use crate::utils::test_helpers::{real_item, fake_feed, fake_item, fake_item_with_enclosure};

  use mockito::mock;

  #[sqlx::test]
  async fn test_find(pool: SqlitePool) -> sqlx::Result<()> {
    let feed: Feed = fake_feed();
    let item: Item = real_item(&feed, &pool).await?;

    let item2 = Item::find(item.id, &pool).await?;
    
    assert_eq!(item, item2);
    
    Ok(())
  }

  #[sqlx::test]
  async fn find_by_feed_and_id(pool: SqlitePool) -> sqlx::Result<()> {
    let feed: Feed = fake_feed();
    let item: Item = real_item(&feed, &pool).await?;

    let item2 = Item::find_by_feed_and_id(&feed, item.id, &pool).await?;
    
    assert_eq!(item, item2.unwrap());
    
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
  pub async fn test_for_feed(pool: SqlitePool) -> sqlx::Result<()> {
    let feed: Feed = fake_feed();
    let _item: Item = real_item(&feed, &pool).await?;

    let result = Item::for_feed(&feed, 10, &pool).await?;
    assert_eq!(result.len(), 1);

    let _item2: Item = real_item(&feed, &pool).await?;
    let result2 = Item::for_feed(&feed, 10, &pool).await?;
    assert_eq!(result2.len(), 2);

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
  async fn test_to_activity_pub_with_enclosure() -> Result<(), String> {
    let feed: Feed = fake_feed();
    let item: Item = fake_item_with_enclosure();

    let result = item.to_activity_pub(&feed);
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();

        println!("{:}", s);
        
        assert!(s.contains("Hello!"));
        assert!(s.contains("<p>Hey!</p>"));
        assert!(s.contains("file.mp3"));
        assert!(s.contains("audio/mpeg"));

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
