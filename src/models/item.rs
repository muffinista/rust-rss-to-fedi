use sqlx::postgres::PgPool;
use serde::{Serialize};
use feed_rs::model::Entry;

use crate::models::Enclosure;
use crate::models::Feed;

use crate::routes::enclosures::*;

use crate::utils::path_to_url;

use activitystreams::activity::*;
use activitystreams::object::ApObject;
use activitystreams::object::Document;
use activitystreams::object::Note;
use activitystreams::iri;
use activitystreams::base::BaseExt;
use activitystreams::base::ExtendsExt;
use activitystreams::object::ObjectExt;
use activitystreams::link::Mention;
use activitystreams::link::LinkExt;

use crate::activitystreams::Hashtag;

use fang::AsyncRunnable;
use fang::AsyncQueueable;

use crate::tasks::{DeliverMessage};

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

use chrono::Utc;
use rocket::uri;


use activitystreams::mime::Mime;



///
/// Model for an item, which is the equivalent of an entry in an rss feed
///
#[derive(Debug, Serialize)]
pub struct Item {
  pub id: i32,
  pub feed_id: i32,
  pub guid: String,
  pub title: Option<String>,
  pub content: Option<String>,
  pub url: Option<String>,
  
  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>
}

fn sanitize_string(input: &str) -> String {
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
  pub async fn find(id: i32, pool: &PgPool) -> Result<Item, sqlx::Error> {
    sqlx::query_as!(Item, "SELECT * FROM items WHERE id = $1", id)
    .fetch_one(pool)
    .await
  }

  pub async fn find_by_feed_and_id(feed: &Feed, id: i32, pool: &PgPool) -> Result<Option<Item>, sqlx::Error> {
    sqlx::query_as!(Item, "SELECT * FROM items WHERE feed_id = $1 AND id = $2", feed.id, id)
    .fetch_optional(pool)
    .await
  }

  pub async fn find_by_guid(guid: &String, feed: &Feed, pool: &PgPool) -> Result<Item, sqlx::Error> {
    sqlx::query_as!(Item, "SELECT * FROM items WHERE feed_id = $1 AND guid = $2", feed.id, guid)
    .fetch_one(pool)
    .await
  }

  pub async fn exists_by_guid(guid: &String, feed: &Feed, pool: &PgPool) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM items WHERE feed_id = $1 AND guid = $2", feed.id, guid)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() > 0),
      Err(why) => Err(why)
    }
  }

  pub async fn for_feed(feed: &Feed, limit: i64, pool: &PgPool) -> Result<Vec<Item>, sqlx::Error> {
    sqlx::query_as!(Item, "SELECT * FROM items WHERE feed_id = $1 ORDER by created_at DESC, id ASC LIMIT $2", feed.id, limit)
    .fetch_all(pool)
    .await
  }

  pub async fn create_from_entry(entry: &Entry, feed: &Feed, pool: &PgPool) -> Result<Item, sqlx::Error> {
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

    let item_url = if !entry.links.is_empty() {
      Some(&entry.links[0].href)
    } else if entry.id.starts_with("http") {
      Some(&entry.id)
    } else {
      None
    };
    
    let now = Utc::now();

    let published_at = if entry.published.is_some() {
      entry.published.unwrap()
    } else {
      now
    };


    let item_id = sqlx::query!("INSERT INTO items 
                                (feed_id, guid, title, content, url, created_at, updated_at)
                                VALUES($1, $2, $3, $4, $5, $6, $7)
                                RETURNING id",
                               feed.id,
                               entry.id,
                               title,
                               body,
                               item_url,
                               published_at,
                               now
    )
      .fetch_one(pool)
      .await?
      .id;

    for media in &entry.media {      
      let description = if media.description.is_some() {
        Some(media.description.as_ref().unwrap().content.clone())
      } else {
        None
      };

      let credits = if !media.credits.is_empty() {
        Some(media.credits[0].entity.clone())
      } else {
        None
      };

      for content in &media.content {
        if content.url.is_some() {
          let url = Some(content.url.as_ref().unwrap().as_str());
    
          let content_type = if content.content_type.is_some() {
            Some(content.content_type.as_ref().unwrap().essence_str())
          } else {
            None
          };
    
          let size = if content.size.is_some() {
            Some(content.size.unwrap() as i32)
          } else {
            None
          };

          sqlx::query!("INSERT INTO enclosures 
            (item_id, url, content_type, size, description, credits, created_at, updated_at)
            VALUES($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id",
            item_id, url, content_type, size, description, credits, now, now)
          .fetch_one(pool)
          .await?;
        }
      } // for
    } // for
  

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
        println!("Parsing error(s): {e}");
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

  
  ///
  /// generate an AP version of this item
  ///
  pub async fn to_activity_pub(&self, feed: &Feed, pool: &PgPool) -> Result<ApObject<Create>, AnyError> {    
    let mut note: ApObject<Note> = ApObject::new(Note::new());

    let feed_url = feed.ap_url();
    let item_url = format!("{}/items/{}", feed_url, self.id);
    let ts = OffsetDateTime::from_unix_timestamp(self.created_at.timestamp()).unwrap();

    note
      .set_attributed_to(iri!(feed_url))
      .set_content(self.to_html())
      .set_url(iri!(feed_url))
      .set_id(iri!(item_url))
      .set_published(ts);

    let item_publicity = match &feed.status_publicity {
      Some(value) => value.as_str(),
      None => "unlisted"
    };

    //
    // set destination according to desired publicity level
    //
    match item_publicity {
      "public" => { note.set_cc(iri!("https://www.w3.org/ns/activitystreams#Public")) },

      // we'll handle some DM logic outside of message generation here
      "direct" => { &mut note },
      "followers" => { note.set_to(iri!(feed.followers_url())) },

      // unlisted/fallback
      _ => { 
        note
          .set_to(iri!(feed.followers_url()))
          .add_cc(iri!("https://www.w3.org/ns/activitystreams#Public"))
      },
    };

    //
    // add content warning as a summary
    // 
    if feed.content_warning.is_some() {
      let summary = feed.content_warning.as_ref().unwrap();
      note.set_summary(summary.to_string());
    }

    if feed.hashtag.is_some() {
      let mut hashtag = Hashtag::new();

      hashtag
        .set_name(feed.hashtag.clone().unwrap());
  
      note.add_tag(hashtag.into_any_base()?);  
    }



    //
    // add any enclosures
    //
    let enclosures = Enclosure::for_item(self, pool).await?;
    for enclosure in enclosures {
      let mut attachment = Document::new();

      let filename = enclosure.filename();
      let enclosure_url = path_to_url(&uri!(show_enclosure(&feed.name, self.id, filename)));
      
      attachment.set_url(iri!(&enclosure_url));

      if enclosure.content_type.is_some() {
        let content_type: &String = &enclosure.content_type.clone().unwrap();
        attachment.set_media_type(content_type.parse::<Mime>().unwrap());
      }

      if enclosure.description.is_some() {
        attachment.set_summary(enclosure.description.unwrap());
      }

      note.add_attachment(attachment.into_any_base()?);
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

  ///
  /// delete this item
  /// @todo -- add deletion notifications
  ///
  pub async fn delete(feed: &Feed, id: i32, pool: &PgPool) -> Result<Item, sqlx::Error> {
    let old_item = Item::find(id, pool).await;
    
    sqlx::query!("DELETE FROM items WHERE feed_id = $1 AND id = $2", feed.id, id)
    .execute(pool)
    .await?;
    
    old_item   
  }


  ///
  /// deliver this item to any followers of the parent feed
  ///
  pub async fn deliver(&self, feed: &Feed, pool: &PgPool, queue: &mut dyn AsyncQueueable) -> Result<(), AnyError> {
    let message = self.to_activity_pub(feed, pool).await.unwrap();
    let item_publicity = match &feed.status_publicity {
      Some(value) => value.as_str(),
      None => "unlisted"
    };

    // handle special case of sending DMs to feed owner
    // TODO unify both parts of this if statement
    if item_publicity == "direct" {
      let user = feed.user(pool).await?;

      if user.actor_url.is_none() {
        println!("Refusing to send because owner doesn't have an address");
        return Ok(());
      }

      let dest_url = user.actor_url.unwrap();

      let mut targeted = message.clone();
      targeted.set_to(iri!(dest_url));

      let mut mention = Mention::new();

      mention
        .set_href(iri!(dest_url))
        .set_name("en");

      targeted.add_tag(mention.into_any_base()?);
          
      let msg = serde_json::to_string(&targeted).unwrap();
      println!("{msg}");

      let task = DeliverMessage { feed_id: feed.id, actor_url: dest_url, message: msg };
      let _result = queue
        .insert_task(&task as &dyn AsyncRunnable)
        .await
        .unwrap();

        Ok(())

    } else {
      let followers = feed.followers_list(pool).await?;
      for follower in followers { 
        let inbox = follower.find_inbox(pool).await;
        match inbox {
          Ok(inbox) => {
            if inbox.is_some() {
              let inbox = inbox.unwrap();

              let mut targeted = message.clone();
              targeted.set_many_tos(vec![iri!(inbox)]);
                
              let msg = serde_json::to_string(&targeted).unwrap();
              println!("{msg}");
      
      
              let task = DeliverMessage { feed_id: feed.id, actor_url: inbox, message: msg };
              let _result = queue
                .insert_task(&task as &dyn AsyncRunnable)
                .await
                .unwrap();      
            }
          },
          Err(why) => {
            println!("lookup failure! {why:?}");
            // @todo retry! mark as undeliverable? delete user?
            // panic!("oops!");
            // Err(why)
          }
        }      
      }
      Ok(())
    }
  }
}


#[cfg(test)]
mod test {
  use std::env;
  use sqlx::postgres::PgPool;
  use crate::models::Feed;
  use crate::models::Item;
  use crate::models::Actor;
  use crate::utils::test_helpers::{real_item, real_feed, fake_item, real_item_with_enclosure};

  use mockito::mock;
  use fang::AsyncQueue;
  use fang::NoTls;

  #[sqlx::test]
  async fn test_find(pool: PgPool) -> sqlx::Result<()> {
    let feed: Feed = real_feed(&pool).await?;
    let item: Item = real_item(&feed, &pool).await?;

    let item2 = Item::find(item.id, &pool).await?;
    
    assert_eq!(item, item2);
    
    Ok(())
  }

  #[sqlx::test]
  async fn find_by_feed_and_id(pool: PgPool) -> sqlx::Result<()> {
    let feed: Feed = real_feed(&pool).await?;
    let item: Item = real_item(&feed, &pool).await?;

    let item2 = Item::find_by_feed_and_id(&feed, item.id, &pool).await?;
    
    assert_eq!(item, item2.unwrap());
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_find_by_guid(pool: PgPool) -> sqlx::Result<()> {
    let feed: Feed = real_feed(&pool).await?;
    let item: Item = real_item(&feed, &pool).await?;

    let item2 = Item::find_by_guid(&item.guid, &feed, &pool).await?;
    
    assert_eq!(item, item2);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_exists_by_guid(pool: PgPool) -> sqlx::Result<()> {
    let feed: Feed = real_feed(&pool).await?;
    let item: Item = real_item(&feed, &pool).await?;

    let result = Item::exists_by_guid(&item.guid, &feed, &pool).await?;   
    assert_eq!(true, result);

    let bad_guid = format!("{}sdfsdfsdf", item.guid);
    let result = Item::exists_by_guid(&bad_guid, &feed, &pool).await?;   
    assert_eq!(false, result);

    Ok(())
  }

  #[sqlx::test]
  pub async fn test_for_feed(pool: PgPool) -> sqlx::Result<()> {
    let feed: Feed = real_feed(&pool).await?;
    let _item: Item = real_item(&feed, &pool).await?;

    let result = Item::for_feed(&feed, 10, &pool).await?;
    assert_eq!(result.len(), 1);

    let _item2: Item = real_item(&feed, &pool).await?;
    let result2 = Item::for_feed(&feed, 10, &pool).await?;
    assert_eq!(result2.len(), 2);

    Ok(())
  }

  #[sqlx::test]
  async fn test_to_activity_pub(pool: PgPool) -> Result<(), String> {
    let feed: Feed = real_feed(&pool).await.unwrap();
    let item: Item = fake_item();

    let result = item.to_activity_pub(&feed, &pool).await;
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
  async fn test_to_activity_pub_with_hashtag(pool: PgPool) -> Result<(), String> {
    let mut feed: Feed = real_feed(&pool).await.unwrap();
    let item: Item = fake_item();

    feed.hashtag = Some("hashy".to_string());
    let result = item.to_activity_pub(&feed, &pool).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();
        println!("{:}", s);
        assert!(s.contains("hashy"));

        Ok(())
      },
      Err(why) => Err(why.to_string())
    }
  }

  #[sqlx::test]
  async fn test_to_activity_pub_with_enclosure(pool: PgPool) -> Result<(), String> {
    let feed: Feed = real_feed(&pool).await.unwrap();
    let item: Item = real_item_with_enclosure(&feed, &pool).await.unwrap();

    let result = item.to_activity_pub(&feed, &pool).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();

        println!("{:}", s);
        
        assert!(s.contains("/enclosures/"));
        assert!(s.contains("audio/mpeg"));

        Ok(())
      },
      Err(why) => Err(why.to_string())
    }
  }

  #[sqlx::test]
  async fn test_delete(pool: PgPool) -> Result<(), String> {
    let feed: Feed = real_feed(&pool).await.unwrap();
    let item: Item = real_item_with_enclosure(&feed, &pool).await.unwrap();

    assert!(Item::exists_by_guid(&item.guid, &feed, &pool).await.unwrap());
    assert_eq!(item, Item::delete(&feed, item.id, &pool).await.unwrap());
    assert!(!Item::exists_by_guid(&item.guid, &feed, &pool).await.unwrap());

    Ok(())
  }
  
  #[sqlx::test]
  async fn test_deliver(pool: PgPool) -> Result<(), String> {
    let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");

    let mut feed: Feed = real_feed(&pool).await.unwrap();
    let item: Item = fake_item();

    // let dest_actor: Actor = real_actor(&pool).await.unwrap();

    let actor = format!("{}/users/colin", &mockito::server_url());
    let inbox = format!("{}/inbox", &actor);

    let profile = format!("{{\"inbox\": \"{}/users/colin/inbox\"}}", &mockito::server_url());

    Actor::create(
      &actor.to_string(),
      &inbox,
      &"public_key_id".to_string(),
      &"public_key".to_string(),
      Some("username".to_string()),
      &pool).await.unwrap();
  

    let _m = mock("GET", "/users/colin")
      .with_status(200)
      .with_header("Accept", "application/ld+json")
      .with_body(profile)
      .create();

    let _m2 = mock("POST", "/users/colin/inbox")
      .with_status(202)
      .create();



    let _follower = feed.add_follower(&pool, &actor).await;

    let max_pool_size: u32 = 5;

    let mut queue:AsyncQueue<NoTls> = AsyncQueue::builder()
      // Postgres database url
      .uri(&db_uri)
      // Max number of connections that are allowed
      .max_pool_size(max_pool_size)
      .build();

    queue.connect(NoTls).await.unwrap();


    feed.status_publicity = Some("unlisted".to_string());
    assert!(item.deliver(&feed, &pool, &mut queue).await.is_ok());

    feed.status_publicity = Some("public".to_string());
    assert!(item.deliver(&feed, &pool, &mut queue).await.is_ok());

    feed.status_publicity = Some("direct".to_string());
    assert!(item.deliver(&feed, &pool, &mut queue).await.is_ok());

    Ok(())
  }


  #[sqlx::test]
  async fn test_create_from_entry(pool: PgPool) -> Result<(), String> {
    use std::fs;
    use feed_rs::parser;

    let feed: Feed = real_feed(&pool).await.unwrap();

    let path = "fixtures/test_feed_to_entries.xml";
    let data = parser::parse(fs::read_to_string(path).unwrap().as_bytes()).unwrap();

    let item:Item = Item::create_from_entry(&data.entries[0], &feed, &pool).await.unwrap();

    assert_eq!("http://muffinlabs.com/2022/09/10/how-i-maintain-botsin-space/", item.guid);
    assert_eq!("How I maintain botsin.space", item.title.unwrap());
    assert!(item.content.unwrap().contains("been meaning to write up some notes"));
    assert_eq!("http://muffinlabs.com/2022/09/10/how-i-maintain-botsin-space/", item.url.unwrap());

    Ok(())
  }

  #[sqlx::test]
  async fn test_create_from_entry_link_in_guid(pool: PgPool) -> Result<(), String> {
    use std::fs;
    use feed_rs::parser;

    let feed: Feed = real_feed(&pool).await.unwrap();

    let path = "fixtures/test_feed_link_in_guid.xml";
    let data = parser::parse(fs::read_to_string(path).unwrap().as_bytes()).unwrap();

    let item:Item = Item::create_from_entry(&data.entries[0], &feed, &pool).await.unwrap();

    assert_eq!("https://www.pbs.org/newshour/show/march-1-2023-pbs-newshour-full-episode", item.guid);
    assert_eq!("https://www.pbs.org/newshour/show/march-1-2023-pbs-newshour-full-episode", item.url.unwrap());

    Ok(())
  }
}


