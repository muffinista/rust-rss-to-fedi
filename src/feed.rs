use activitystreams_ext::{Ext1};
use activitystreams::{actor::{ApActor, ApActorExt, Service}, iri};

use activitystreams::{
  prelude::*,
  security,
};

use sqlx::sqlite::SqlitePool;
use serde::{Serialize};

use reqwest;
use feed_rs::parser;

use std::{error::Error, fmt};

use crate::user::User;
use crate::item::Item;
use crate::follower::Follower;
use crate::keys::*;

use activitystreams::{
  context
};

use activitystreams::base::BaseExt;

use activitystreams::collection::OrderedCollection;
use activitystreams::collection::OrderedCollectionPage;
use activitystreams::object::ApObject;


use anyhow::Error as AnyError;

use rocket::uri;
use crate::utils::*;
use crate::routes::feeds::*;
use crate::routes::ap::outbox::*;

#[derive(Debug, Serialize)]
pub struct Feed {
  pub id: i64,
  pub user_id: i64,
  pub name: String,
  pub url: String,
  pub private_key: String,
  pub public_key: String,
  pub image_url: Option<String>,
  pub icon_url: Option<String>,

  pub title: Option<String>,
  pub description: Option<String>,
  pub site_url: Option<String>,
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

const PER_PAGE:u32 = 10u32;

// https://docs.rs/activitystreams/0.7.0-alpha.20/activitystreams/index.html#parse
// also examples/handle_incoming.rs

use activitystreams::activity::ActorAndObject;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum AcceptedTypes {
  Accept,
  Follow,
  Undo,
}

pub type AcceptedActivity = ActorAndObject<AcceptedTypes>;
pub type ExtendedService = Ext1<ApActor<Service>, PublicKey>;


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
  
  pub async fn find_by_name(name: &String, pool: &SqlitePool) -> Result<Option<Feed>, sqlx::Error> {
    sqlx::query_as!(Feed, "SELECT * FROM feeds WHERE name = ?", name)
      .fetch_optional(pool)
      .await
  }

  pub async fn exists_by_name(name: &String, pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("SELECT count(1) AS tally FROM feeds WHERE name = ?", name)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally > 0),
      Err(why) => Err(why)
    }
  }

  pub async fn exists_by_url(url: &String, pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("SELECT count(1) AS tally FROM feeds WHERE url = ?", url)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally > 0),
      Err(why) => Err(why)
    }
  }
  
  pub async fn create(user: &User,
      url: &String,
      name: &String, pool: &SqlitePool) -> Result<Feed, sqlx::Error> {

    // generate keypair used for signing AP requests
    let (private_key_str, public_key_str) = generate_key();

    let feed_id = sqlx::query!("INSERT INTO feeds (user_id, url, name, private_key, public_key)
                                VALUES($1, $2, $3, $4, $5)",
                               user.id, url, name, private_key_str, public_key_str)
      .execute(pool)
      .await?
      .last_insert_rowid();
    
    Feed::find(feed_id, pool).await
  }

  pub async fn save(&self, pool: &SqlitePool) -> Result<&Feed, sqlx::Error> {
    sqlx::query!("UPDATE feeds
      SET url = $1,
          name = $2,
          private_key = $3,
          public_key = $4,
          image_url = $5,
          icon_url = $6,
          title = $7,
          description = $8,
          site_url = $9
      WHERE id = $10",
      self.url,
      self.name,
      self.private_key,
      self.public_key,
      self.image_url,
      self.icon_url,
      self.title,
      self.description,
      self.site_url,
      self.id
    ).execute(pool)
      .await?;

    Ok(self)
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

  pub async fn entries_count(&self, pool: &SqlitePool)  -> Result<u64, AnyError>{
    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM items WHERE feed_id = ?", self.id)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally as u64),
      Err(_why) => todo!()
    }
  }

  pub async fn update_icon_url(&self, url:&str, pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let result = sqlx::query!("UPDATE feeds SET icon_url = $1 WHERE id = $2", url, self.id)
      .execute(pool)
      .await;

    match result {
      Ok(_result) => Ok(()),
      Err(why) => Err(why)
    }
  }

  pub async fn update_image_url(&self, url:&str, pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let result = sqlx::query!("UPDATE feeds SET image_url = $1 WHERE id = $2", url, self.id)
      .execute(pool)
      .await;

    match result {
      Ok(_result) => Ok(()),
      Err(why) => Err(why)
    }
  }

  pub async fn parse(&mut self, pool: &SqlitePool) -> Result<Vec<Item>, FeedError> {        
    let body = Feed::load(self).await;
    match body {
      Ok(body) => {
        let work = self.parse_from_data(body.to_string(), pool).await;
        match work {
          Ok(entries) => Ok(entries),
          Err(_why) => Err(FeedError)
        }
      },
      Err(_why) => Err(FeedError)
    }   
  }

  pub async fn parse_from_data(&mut self, body: String, pool: &SqlitePool) -> Result<Vec<Item>, FeedError> {        
    let data = parser::parse(body.as_bytes());
        
    match data {
      Ok(data) => {
        if data.title.is_some() {
          self.title = Some(data.title.as_ref().unwrap().content.clone());
        }
        if data.description.is_some() {
          self.description = Some(data.description.as_ref().unwrap().content.clone());
        }

        if data.icon.is_some() {
          self.icon_url = Some(data.icon.as_ref().unwrap().uri.clone());
          // self.update_icon_url(&data.icon.as_ref().unwrap().uri, pool).await.ok();
        }
        if data.logo.is_some() {
          self.image_url = Some(data.logo.as_ref().unwrap().uri.clone());
          // self.update_image_url(&data.logo.as_ref().unwrap().uri, pool).await.ok();
        }

        // todo snag link too

        let update = self.save(pool).await;
        match update {
          Ok(_update) => {
            let result = self.feed_to_entries(data, pool).await;
            match result {
              Ok(result) => Ok(result),
              Err(_why) => return Err(FeedError)
            }    
          }
          Err(_why) => return Err(FeedError)
        }
      },
      Err(_why) => return Err(FeedError)
    }
  }

  pub fn to_activity_pub(&self) -> Result<String, AnyError> {    
    // we could return an object here instead of JSON so we can manipulate it if needed
    // pub fn to_activity_pub(&self) -> Result<ExtendedService, AnyError> {    

    let mut svc = Ext1::new(
      ApActor::new(
        iri!("https://example.com/inbox"),
        Service::new(),
      ),
      PublicKey {
        public_key: PublicKeyInner {
          id: iri!(format!("{}#main-key", path_to_url(&uri!(render_feed(&self.name))))),
          owner: iri!(path_to_url(&uri!(render_feed(&self.name)))),
          public_key_pem: self.public_key.to_owned(),
        },
      },
    );

    svc
      .set_context(context())
      .add_context(security())
      .set_id(iri!(path_to_url(&uri!(render_feed(&self.name)))))
      .set_name(self.name.clone())
      .set_preferred_username(self.name.clone())
      .set_outbox(iri!(path_to_url(&uri!(user_outbox(&self.name)))))
      .set_followers(iri!(path_to_url(&uri!(render_feed_followers(&self.name, None::<u32>)))));

    if self.image_url.is_some() {
      svc.set_image(iri!(&self.image_url.clone().unwrap()));
    }
    if self.icon_url.is_some() {
      svc.set_icon(iri!(&self.icon_url.clone().unwrap()));
    }

    // generate JSON and return
    Ok(serde_json::to_string(&svc).unwrap())

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

  async fn follow(&self, pool: &SqlitePool, actor: &str) -> Result<(), sqlx::Error> {
    sqlx::query!("INSERT INTO followers (feed_id, actor) VALUES($1, $2)",
                 self.id, actor)
      .execute(pool)
      .await?;

    Ok(())
  }

  async fn unfollow(&self, pool: &SqlitePool, actor: &str) -> Result<(), sqlx::Error>  {
    sqlx::query!("DELETE FROM followers WHERE feed_id = ? AND actor = ?",
                 self.id, actor)
      .execute(pool)
      .await?;
    
    Ok(())
  }

  pub async fn handle_activity(&self, pool: &SqlitePool, activity: &AcceptedActivity)  -> Result<(), sqlx::Error>{
    let (actor, _object, act) = activity.clone().into_parts();

    let actor_id = actor.as_single_id().unwrap().to_string();
    
    match act.kind() {
      Some(AcceptedTypes::Follow) => self.follow(pool, &actor_id).await,
      Some(AcceptedTypes::Undo) => self.unfollow(pool, &actor_id).await,
      // we don't need to handle this but if we receive it, just move on
      Some(AcceptedTypes::Accept) => Ok(()),
      None => Ok(())
    }
  }

  pub async fn follower_count(&self, pool: &SqlitePool)  -> Result<u64, AnyError>{
    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = ?", self.id)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally as u64),
      Err(_why) => todo!()
    }
  }
  
  pub async fn followers(&self, pool: &SqlitePool)  -> Result<ApObject<OrderedCollection>, AnyError>{
    let count = self.follower_count(pool).await?;
    let total_pages = ((count / PER_PAGE as u64) + 1 ) as u32;

    let mut collection: ApObject<OrderedCollection> = ApObject::new(OrderedCollection::new());

    // The first, next, prev, last, and current properties are used
    // to reference other CollectionPage instances that contain 
    // additional subsets of items from the parent collection. 

    
    // in theory we can drop the first page of data in here
    // however, it's not required (mastodon doesn't do it)
    // and activitystreams might not be wired for it
    collection
      .set_context(context())
      .set_id(iri!(path_to_url(&uri!(render_feed(&self.name)))))
      .set_summary("A list of followers".to_string())
      .set_total_items(count)
      .set_first(iri!(path_to_url(&uri!(render_feed_followers(&self.name, Some(1))))))
      .set_last(iri!(path_to_url(&uri!(render_feed_followers(&self.name, Some(total_pages))))));

    Ok(collection)
  }

  pub async fn followers_paged(&self, page: u32, pool: &SqlitePool)  -> Result<ApObject<OrderedCollectionPage>, AnyError>{
    let count = self.follower_count(pool).await?;
    let total_pages = ((count / PER_PAGE as u64) + 1 ) as u32;
    let mut collection: ApObject<OrderedCollectionPage> = ApObject::new(OrderedCollectionPage::new());

    collection
      .set_context(context())
      .set_summary("A list of followers".to_string())
      .set_part_of(iri!(path_to_url(&uri!(render_feed(&self.name)))))
      .set_first(iri!(path_to_url(&uri!(render_feed_followers(&self.name, Some(1))))))
      .set_last(iri!(path_to_url(&uri!(render_feed_followers(&self.name, Some(total_pages))))))
      .set_current(iri!(path_to_url(&uri!(render_feed_followers(&self.name, Some(page))))));

    if page > 1 {
      collection.set_prev(iri!(path_to_url(&uri!(render_feed_followers(&self.name, Some(page - 1))))));
    }

    if page < total_pages {
      collection.set_next(iri!(path_to_url(&uri!(render_feed_followers(&self.name, Some(page + 1))))));
    }

    // return empty collection for invalid pages
    if page == 0 || page > total_pages {
      return Ok(collection)
    }

    // @todo handle page <= 0 and page > count
    
    let offset = (page - 1) * PER_PAGE;
    let result = sqlx::query_as!(Follower, "SELECT * FROM followers WHERE feed_id = ? LIMIT ? OFFSET ?", self.id, PER_PAGE, offset )
      .fetch_all(pool)
      .await;
  
    match result {
      Ok(result) => {
        let v: Vec<String> = result
          .into_iter()
          .filter_map(|follower| Some(follower.actor))
          .collect();
              
        // The first, next, prev, last, and current properties are used to 
        // reference other CollectionPage instances that contain additional 
        // subsets of items from the parent collection. 
        
        collection.set_many_items(v);
        
        Ok(collection)
          
      },
      Err(_why) => todo!()
    }
  }
}

#[cfg(test)]
mod test {
  use sqlx::sqlite::SqlitePool;
  use rocket::uri;
  use feed_rs::parser;

  use crate::user::User;
  use crate::Feed;
  use crate::feed::AcceptedActivity;
  use crate::utils::*;
  
  use crate::routes::feeds::*;

  fn fake_user() -> User {
    User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()) }
  }

  fn fake_feed() -> Feed {
    Feed {
      id: 1,
      user_id: 1,
      name: "testfeed".to_string(),
      url: "https://foo.com/rss.xml".to_string(),
      private_key: "private key".to_string(),
      public_key: "public key".to_string(),
      image_url: Some("https://foo.com/image.png".to_string()),
      icon_url: Some("https://foo.com/image.ico".to_string()),
      description: None,
      site_url: None,
      title: None
    }
  }

  async fn real_feed(pool: &SqlitePool) -> sqlx::Result<Feed> {
    let user = fake_user();
    
    let url:String = "https://foo.com/rss.xml".to_string();
    let name:String = "testfeed".to_string();
    let feed = Feed::create(&user, &url, &name, &pool).await?;
    
    Ok(feed)
  }


  #[sqlx::test]
  async fn test_create(pool: SqlitePool) -> sqlx::Result<()> {
    let user = fake_user();
    let feed:Feed = real_feed(&pool).await?;
    
    let url:String = "https://foo.com/rss.xml".to_string();
    let name:String = "testfeed".to_string();

    assert_eq!(feed.url, url);
    assert_eq!(feed.name, name);
    assert_eq!(feed.user_id, user.id);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_save(pool: SqlitePool) -> sqlx::Result<()> {
   
    let mut feed:Feed = real_feed(&pool).await?;
    
    let newname = "testfeed2".to_string();
    feed.name = newname.clone();

    let updated_feed = feed.save(&pool).await?;

    assert_eq!(updated_feed.name, newname);

    Ok(())
  }

  #[sqlx::test]
  async fn test_find_by_url(pool: SqlitePool) -> sqlx::Result<()> {
    let url: String = "https://foo.com/rss.xml".to_string();
    let name: String = "testfeed".to_string();

    let feed:Feed = real_feed(&pool).await?;
    let feed2 = Feed::find_by_url(&url, &pool).await?;
    
    assert_eq!(feed, feed2);
    assert_eq!(feed2.name, name);
    assert_eq!(feed2.url, url);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_find_by_name(pool: SqlitePool) -> sqlx::Result<()> {
    let feed:Feed = real_feed(&pool).await?;
    let feed2 = Feed::find_by_url(&feed.url, &pool).await?;
    
    assert_eq!(feed, feed2);
    assert_eq!(feed2.url, feed.url);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_find(pool: SqlitePool) -> sqlx::Result<()> {
    let feed:Feed = real_feed(&pool).await?;
    let feed2 = Feed::find(feed.id, &pool).await?;
    
    assert_eq!(feed, feed2);
    assert_eq!(feed2.url, feed.url);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_for_user(pool: SqlitePool) -> sqlx::Result<()> {
    let user = fake_user();
    
    let url: String = "https://foo.com/rss.xml".to_string();
    let name: String = "testfeed".to_string();
    let _feed = Feed::create(&user, &url, &name, &pool).await?;
    
    let url2: String = "https://foofoo.com/rss.xml".to_string();
    let name2: String = "testfeed2".to_string();
    let _feed2 = Feed::create(&user, &url2, &name2, &pool).await?;
    
    let feeds = Feed::for_user(&user, &pool).await?; 
    assert_eq!(feeds.len(), 2);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_delete(pool: SqlitePool) -> sqlx::Result<()> {
    let user = fake_user();
    let feed:Feed = real_feed(&pool).await?;

    let deleted_feed = Feed::delete(&user, feed.id, &pool).await?;
    assert_eq!(feed, deleted_feed);
    
    let feeds = Feed::for_user(&user, &pool).await?; 
    assert_eq!(feeds.len(), 0);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_parse_from_data(pool: SqlitePool) -> sqlx::Result<()> {
    use std::fs;
    let mut feed:Feed = real_feed(&pool).await?;

    let path = "fixtures/test_feed_to_entries.xml";
    let data = fs::read_to_string(path).unwrap();

    let result = feed.parse_from_data(data, &pool).await.unwrap();
    assert_eq!(result.len(), 3);

    let feed2 = Feed::find(feed.id, &pool).await?;
    assert_eq!(feed2.title, Some("muffinlabs.com".to_string()));

    Ok(())
  }
 

  #[sqlx::test]
  async fn test_feed_to_entries(pool: SqlitePool) -> sqlx::Result<()> {
    use std::fs;
    let feed:Feed = real_feed(&pool).await?;

    assert_eq!(feed.entries_count(&pool).await.unwrap(), 0);
    

    let path = "fixtures/test_feed_to_entries.xml";
    let data = parser::parse(fs::read_to_string(path).unwrap().as_bytes()).unwrap();

    let result = Feed::feed_to_entries(&feed, data, &pool).await.unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(feed.entries_count(&pool).await.unwrap(), 3);

    // check that reloading the same feed doesn't create more records
    let data2 = parser::parse(fs::read_to_string(path).unwrap().as_bytes()).unwrap();
    let result2 = Feed::feed_to_entries(&feed, data2, &pool).await.unwrap();

    assert_eq!(result2.len(), 0);
    assert_eq!(feed.entries_count(&pool).await.unwrap(), 3);

    // try with slightly more data
    let path2 = "fixtures/test_feed_to_entries_2.xml";
    let data2 = parser::parse(fs::read_to_string(path2).unwrap().as_bytes()).unwrap();
    let result2 = Feed::feed_to_entries(&feed, data2, &pool).await.unwrap();

    assert_eq!(result2.len(), 4);
    
    assert_eq!(feed.entries_count(&pool).await.unwrap(), 7);

    Ok(())
  }

  #[test]
  fn test_feed_to_activity_pub() {
    use std::env;

    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

    use serde_json::Value;
    let feed:Feed = fake_feed();

    let output = feed.to_activity_pub().unwrap();

    let v: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(v["name"], "testfeed");
    assert_eq!(v["publicKey"]["id"], format!("https://{}/feed/testfeed#main-key", instance_domain));
    assert_eq!(v["publicKey"]["publicKeyPem"], "public key");  
  }

  #[sqlx::test]
  async fn test_follow(pool: SqlitePool) -> sqlx::Result<()> {
    let actor = "https://activitypub.pizza/users/colin".to_string();
    let json = format!(r#"{{"actor":"{}","object":"{}/feed","type":"Follow"}}"#, actor, actor).to_string();
    let act:AcceptedActivity = serde_json::from_str(&json).unwrap();

    let feed:Feed = real_feed(&pool).await?;

    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = ? AND actor = ?", feed.id, actor)
      .fetch_one(&pool)
      .await
      .unwrap();

    assert!(result.tally == 0);

    feed.handle_activity(&pool, &act).await?;

    let result2 = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = ? AND actor = ?", feed.id, actor)
      .fetch_one(&pool)
      .await
      .unwrap();

    assert!(result2.tally > 0);
      
    Ok(())
  }

  #[sqlx::test]
  async fn test_unfollow(pool: SqlitePool) -> sqlx::Result<()> {
    let actor = "https://activitypub.pizza/users/colin".to_string();
    let json = format!(r#"{{"actor":"{}","object":"{}/feed","type":"Undo"}}"#, actor, actor).to_string();
    let act:AcceptedActivity = serde_json::from_str(&json).unwrap();
    
    let feed:Feed = real_feed(&pool).await?;

    sqlx::query!("INSERT INTO followers (feed_id, actor) VALUES($1, $2)", feed.id, actor)
      .execute(&pool)
      .await?;

    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = ? AND actor = ?", feed.id, actor)
      .fetch_one(&pool)
      .await
      .unwrap();

    assert!(result.tally == 1);

    feed.handle_activity(&pool, &act).await?;

    let post_result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = ? AND actor = ?", feed.id, actor)
      .fetch_one(&pool)
      .await
      .unwrap();

    assert!(post_result.tally == 0);
      
    Ok(())
  }

  #[sqlx::test]
  async fn test_followers(pool: SqlitePool) -> Result<(), String> {
    let feed:Feed = fake_feed();

    for i in 1..4 {
      let actor = format!("https://activitypub.pizza/users/colin{}", i);
      sqlx::query!("INSERT INTO followers (feed_id, actor) VALUES($1, $2)", feed.id, actor)
        .execute(&pool)
        .await
        .unwrap();
    }
    
    let result = feed.followers(&pool).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();
        println!("{:?}", s);

        assert!(s.contains("A list of followers"));
        Ok(())
      },

      Err(why) => Err(why.to_string())
    }
  }

  #[sqlx::test]
  async fn test_followers_paged(pool: SqlitePool) -> Result<(), String> {
    let feed:Feed = fake_feed();

    for i in 1..35 {
      let actor = format!("https://activitypub.pizza/users/colin{}", i);
      sqlx::query!("INSERT INTO followers (feed_id, actor) VALUES($1, $2)", feed.id, actor)
        .execute(&pool)
        .await
        .unwrap();
    }

    let result = feed.followers_paged(2, &pool).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();
        println!("{:?}", s);
        
        assert!(s.contains("OrderedCollectionPage"));
        assert!(s.contains("/colin11"));
        assert!(s.contains("/colin12"));
        assert!(s.contains("/colin13"));
        assert!(s.contains(&format!(r#"first":"{}"#, path_to_url(&uri!(render_feed_followers(feed.name.clone(), Some(1)))))));
        assert!(s.contains(&format!(r#"prev":"{}"#, path_to_url(&uri!(render_feed_followers(feed.name.clone(), Some(1)))))));      
        assert!(s.contains(&format!(r#"next":"{}"#, path_to_url(&uri!(render_feed_followers(feed.name.clone(), Some(3)))))));
        assert!(s.contains(&format!(r#"last":"{}"#, path_to_url(&uri!(render_feed_followers(feed.name.clone(), Some(4)))))));
        assert!(s.contains(&format!(r#"current":"{}"#, path_to_url(&uri!(render_feed_followers(feed.name.clone(), Some(2)))))));

        Ok(())
      },
      Err(why) => Err(why.to_string())
    }
  }

  #[sqlx::test]
  async fn test_follower_count(pool: SqlitePool) -> Result<(), String> {
    let feed:Feed = fake_feed();

    for i in 1..36{
      let actor = format!("https://activitypub.pizza/users/colin{}", i);
      sqlx::query!("INSERT INTO followers (feed_id, actor) VALUES($1, $2)", feed.id, actor)
        .execute(&pool)
        .await
        .unwrap();
    }
    
    let result = feed.follower_count(&pool).await;
    match result {
      Ok(result) => { 
        assert_eq!(35, result);
        Ok(())
      }
      Err(why) => Err(why.to_string())
    }
  }
}