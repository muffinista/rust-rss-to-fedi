use sqlx::postgres::PgPool;
use serde::Serialize;
use feed_rs::model::Entry;

use crate::models::Actor;
use crate::models::Enclosure;
use crate::models::Feed;
use crate::traits::content_map::*;

use crate::routes::enclosures::*;

use crate::utils::path_to_url;
use crate::DeliveryError;

use activitystreams::activity::*;
use activitystreams::object::ApObject;
use activitystreams::object::Document;
use activitystreams::iri;
use activitystreams::base::BaseExt;
use activitystreams::base::ExtendsExt;
use activitystreams::object::ObjectExt;
use activitystreams::link::Mention;
use activitystreams::link::LinkExt;
use activitystreams::time::OffsetDateTime;

use crate::activitystreams::Hashtag;

use fang::AsyncRunnable;
use fang::AsyncQueueable;

use crate::tasks::DeliverMessage;

use activitystreams::{
  security,
  context
};

use crate::utils::templates::{Context, render};

use sanitize_html::sanitize_str;
use sanitize_html::rules::predefined::RELAXED;

use chrono::{Duration, Utc};
use rocket::uri;

use std::env;

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
  pub language: Option<String>,
  
  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>
}

// NOTE: mastodon is going to allow: del, pre, blockquote, code, b, strong, u, i, em, ul, ol, li
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
    sqlx::query_as!(Item, "SELECT * FROM items WHERE feed_id = $1 ORDER by created_at DESC LIMIT $2", feed.id, limit)
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
                                (feed_id, guid, title, content, url, language, created_at, updated_at)
                                VALUES($1, $2, $3, $4, $5, $6, $7, $8)
                                RETURNING id",
                               feed.id,
                               entry.id,
                               title,
                               body,
                               item_url,
                               entry.language,
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
            Some(content.content_type.as_ref().unwrap().essence().to_string())
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
  
    feed.update_last_post_at(published_at, pool).await?;

    Item::find(item_id, pool).await
  }


  ///
  /// generate an HTML-ish version of this item suitable
  /// for adding to an AP message
  ///
  pub async fn to_html(&self, hashtag: Option<String>) -> String {
    let mut context = Context::new();
    context.insert("title", &self.title);
    context.insert("body", &self.content);

    if self.url.is_some() {
      context.insert("link", &self.url.as_ref().unwrap());
    }

    // tack on hashtag
    if let Some(ht) = hashtag  {
      if ! ht.is_empty() {
        let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
        let hashtag_url = format!("https://{instance_domain:}/tags/{ht:}");

        context.insert("hashtag", &ht);
        context.insert("hashtag_link", &hashtag_url);
      }
    };
   
    render("ap/feed-item", &context).unwrap()
  }

  pub fn language(&self, feed: &Feed) -> String {
    match &self.language {
      Some(l) => l.to_string(),
      None => feed.language()
    }
  }

  ///
  /// generate an AP version of this item
  ///
  pub async fn to_activity_pub(&self, feed: &Feed, pool: &PgPool) -> Result<ApObject<Create>, DeliveryError> {    

    let feed_url = feed.ap_url();
    let item_url = format!("{}/items/{}", feed_url, self.id);
    let ts = OffsetDateTime::from_unix_timestamp(self.created_at.timestamp()).unwrap();

    let content = self.to_html(feed.hashtag.clone()).await;

    let mut note: ContentMapNote = ContentMapNote::new();

    note
      .set_attributed_to(iri!(feed_url))
      .set_content_language_and_value(self.language(feed), content)
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
      // public items are sent _to_ activitystreams#Public
      // and cc'd to followers
      "public" => { 
        note
          .set_to(iri!("https://www.w3.org/ns/activitystreams#Public")) 
          .add_cc(iri!(feed.followers_url()))
      },

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

    //
    // add hashtag
    //
    if feed.hashtag.is_some() && !feed.hashtag.clone().unwrap().is_empty() {
      let mut hashtag = Hashtag::new();
      let guts = feed.hashtag.clone().unwrap();

      let output = format!("#{guts:}");

      let url_tag = feed.hashtag.clone().unwrap();
      let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
      let hashtag_url = format!("https://{instance_domain:}/tags/{url_tag:}");

      hashtag
        .set_href(iri!(hashtag_url))
        .set_name(output);
  
      note.add_tag(hashtag.into_any_base()?);  
    }



    //
    // add any enclosures
    // @todo think about excluding huge enclosures
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
      .add_context(iri!("as:Hashtag"))
      .add_context(security())
      .set_id(iri!(item_url))
      .set_published(ts);


    //
    // set destination according to desired publicity level
    //
    match item_publicity {
      // public items are sent _to_ activitystreams#Public
      // and cc'd to followers
      "public" => { 
        action
          .set_to(iri!("https://www.w3.org/ns/activitystreams#Public")) 
          .add_cc(iri!(feed.followers_url()))
      },

      // we'll handle some DM logic outside of message generation here
      "direct" => { &mut action },
      "followers" => { action.set_to(iri!(feed.followers_url())) },

      // unlisted/fallback
      // unlisted items are sent _to_ the feed followes
      // and cc'd to public
      _ => { 
        action
          .set_to(iri!(feed.followers_url()))
          .add_cc(iri!("https://www.w3.org/ns/activitystreams#Public"))
      },
    };


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
  pub async fn deliver(&self, feed: &Feed, pool: &PgPool, queue: &mut dyn AsyncQueueable) -> Result<(), DeliveryError> {
    let message = self.to_activity_pub(feed, pool).await.unwrap();
    let item_publicity = match &feed.status_publicity {
      Some(value) => value.as_str(),
      None => "unlisted"
    };

    // handle special case of sending DMs to feed owner
    // @TODO unify both parts of this if statement
    if item_publicity == "direct" {
      let user = feed.user(pool).await?;

      if user.actor_url.is_none() {
        log::debug!("Refusing to send because owner doesn't have an address");
        return Ok(());
      }

      let dest_actor = Actor::find_or_fetch(user.actor_url.as_ref().expect("No actor url!"), pool).await;
      match dest_actor {
        Ok(dest_actor) => {
          if dest_actor.is_none() {
            return Ok(());
          }

          let dest_actor = dest_actor.unwrap();
          let dest_url = dest_actor.inbox_url;
    
          let mut targeted = message.clone();
          targeted.set_to(iri!(dest_actor.url));
    
          let mut mention = Mention::new();

          mention
            .set_href(iri!(&dest_actor.url.to_string()))
            .set_name("en");
    
          targeted.add_tag(mention.into_any_base()?);
              
          let msg = serde_json::to_string(&targeted).unwrap();
          log::debug!("DM {msg}");
    
          let task = DeliverMessage { feed_id: feed.id, actor_url: dest_url, message: msg };
          let _result = queue
            .insert_task(&task as &dyn AsyncRunnable)
            .await
            .unwrap();
        },
        Err(why) => {
          log::debug!("couldnt find actor: {why:?}");
          return Err(why);
        }
      }    


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
              log::debug!("{msg}");     
      
              let task = DeliverMessage { feed_id: feed.id, actor_url: inbox, message: msg };
              let _result = queue
                .insert_task(&task as &dyn AsyncRunnable)
                .await
                .unwrap();      
            }
          },
          Err(why) => {
            log::info!("lookup failure! {why:?}");
            return Err(why);
          }
        }      
      }
      Ok(())
    }
  }

  pub async fn cleanup(pool: &PgPool, age:i64, limit: i64) -> Result<(), sqlx::Error> {
    let age = Utc::now() - Duration::days(age);
      
    sqlx::query!("DELETE FROM items WHERE id IN (select id FROM items WHERE created_at <= $1 ORDER BY created_at LIMIT $2)", age, limit)
        .execute(pool)
        .await?;

    Ok(())
  }
}


#[cfg(test)]
mod test {
  use sqlx::postgres::PgPool;
  use fang::NoTls;
  
  use crate::models::Feed;
  use crate::models::Item;
  use crate::models::Actor;
  use crate::utils::test_helpers::{real_item, real_feed, fake_item, real_item_with_enclosure};

  use crate::utils::queue::create_queue;

  use serde_json::Value;

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
  async fn test_to_html() -> Result<(), String> {
    let item: Item = fake_item();

    let result = item.to_html(Some("hashytime".to_string())).await;

    println!("{:}", result);

    assert!(result.contains("Hello!"));
    assert!(result.contains("<p>Hey!</p>"));
    assert!(result.contains("hashytime"));

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
        
        assert!(s.contains(r#"contentMap":{"en":"<a href=\"http:&#x2F;&#x2F;google.com\">Hello!</a><br />\n\n<p>Hey!</p>"#));
        Ok(())
      },
      Err(why) => Err(why.to_string())
    }
  }

  
  #[sqlx::test]
  async fn test_to_activity_pub_publicity_public(pool: PgPool) -> Result<(), String> {
    let mut feed: Feed = real_feed(&pool).await.unwrap();
    let item: Item = fake_item();

    feed.status_publicity = Some("public".to_string());
    let result = item.to_activity_pub(&feed, &pool).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();

        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["to"], "https://www.w3.org/ns/activitystreams#Public");
        assert!(v["cc"][0].to_string().contains("/followers"));
        assert!(s.contains(r#"contentMap":{"en":"<a href=\"http:&#x2F;&#x2F;google.com\">Hello!</a><br />\n\n<p>Hey!</p>"#));

        Ok(())
      },
      Err(why) => Err(why.to_string())
    }
  }
  
  #[sqlx::test]
  async fn test_to_activity_pub_publicity_unlisted(pool: PgPool) -> Result<(), String> {
    let mut feed: Feed = real_feed(&pool).await.unwrap();
    let item: Item = fake_item();

    feed.status_publicity = Some("unlisted".to_string());
    let result = item.to_activity_pub(&feed, &pool).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();
       
        let v: Value = serde_json::from_str(&s).unwrap();
        assert!(v["to"].to_string().contains("/followers"));
        assert_eq!(v["cc"][0], "https://www.w3.org/ns/activitystreams#Public");
        assert!(s.contains(r#"contentMap":{"en":"<a href=\"http:&#x2F;&#x2F;google.com\">Hello!</a><br />\n\n<p>Hey!</p>"#));

        Ok(())
      },
      Err(why) => Err(why.to_string())
    }
  }

  #[sqlx::test]
  async fn test_to_activity_pub_publicity_followers(pool: PgPool) -> Result<(), String> {
    use rocket::serde::json::Value::Null;
    
    let mut feed: Feed = real_feed(&pool).await.unwrap();
    let item: Item = fake_item();

    feed.status_publicity = Some("followers".to_string());
    let result = item.to_activity_pub(&feed, &pool).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();
       
        let v: Value = serde_json::from_str(&s).unwrap();
        assert!(v["to"].to_string().contains("/followers"));
        assert_eq!(v["cc"][0], Null);
        assert!(s.contains(r#"contentMap":{"en":"<a href=\"http:&#x2F;&#x2F;google.com\">Hello!</a><br />\n\n<p>Hey!</p>"#));

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
        assert!(s.contains("#hashy"));
        assert!(s.contains(r#"contentMap":{"en":"<a href=\"http:&#x2F;&#x2F;google.com\">Hello!</a><br />\n\n<p>Hey!</p>"#));

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
    let mut server = mockito::Server::new_async().await;
    let mut feed: Feed = real_feed(&pool).await.unwrap();
    let item: Item = fake_item();

    let actor = format!("{}/users/colin", &server.url());
    let inbox = format!("{}/inbox", &actor);

    let profile = format!("{{\"inbox\": \"{}/users/colin/inbox\"}}", &server.url());

    Actor::create(
      &actor.to_string(),
      &inbox,
      &"public_key_id".to_string(),
      &"public_key".to_string(),
      &"username".to_string(),
      &pool).await.unwrap();
  

    let _m = server.mock("GET", "/users/colin")
      .with_status(200)
      .with_header("Accept", "application/ld+json")
      .with_body(profile)
      .create_async()
      .await;

    let _m2 = server.mock("POST", "/users/colin/inbox")
      .with_status(202)
      .create_async()
      .await;



    let _follower = feed.add_follower(&pool, &actor).await;
    let mut queue = create_queue().await;

    queue.connect(NoTls).await.unwrap();


    feed.status_publicity = Some("unlisted".to_string());
    assert!(item.deliver(&feed, &pool, &mut queue).await.is_ok());

    feed.status_publicity = Some("public".to_string());
    assert!(item.deliver(&feed, &pool, &mut queue).await.is_ok());

    // skip for now @todo fix this
    // feed.status_publicity = Some("direct".to_string());
    // assert!(item.deliver(&feed, &pool, &mut queue).await.is_ok());

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
    assert_eq!(item.language.expect("No language!"), "en-us");

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


