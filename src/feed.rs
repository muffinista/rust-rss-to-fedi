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
use activitystreams::collection::OrderedCollection;
use activitystreams::{actor::{ApActor, ApActorExt, Service}, iri};
use activitystreams::unparsed::*;

use activitystreams::{
  prelude::*,
  security,
  iri_string::types::IriString,
};

use anyhow::Error as AnyError;

use openssl::{pkey::PKey, rsa::Rsa};


#[derive(Debug, Serialize)]
pub struct Feed {
  pub id: i64,
  pub user_id: i64,
  pub name: String,
  pub url: String,
  pub private_key: String,
  pub public_key: String
}

impl PartialEq for Feed {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

#[derive(Debug, Serialize)]
pub struct Follower {
  pub id: i64,
  pub feed_id: i64,
  pub actor: String
}

impl PartialEq for Follower {
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

use activitystreams_ext::{Ext1, UnparsedExtension};

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicKey {
    public_key: PublicKeyInner,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicKeyInner {
    id: IriString,
    owner: IriString,
    public_key_pem: String,
}

impl<U> UnparsedExtension<U> for PublicKey
where
    U: UnparsedMutExt,
{
    type Error = serde_json::Error;

    fn try_from_unparsed(unparsed_mut: &mut U) -> Result<Self, Self::Error> {
        Ok(PublicKey {
            public_key: unparsed_mut.remove("publicKey")?,
        })
    }

    fn try_into_unparsed(self, unparsed_mut: &mut U) -> Result<(), Self::Error> {
        unparsed_mut.insert("publicKey", self.public_key)?;
        Ok(())
    }
}

pub type ExtendedService = Ext1<ApActor<Service>, PublicKey>;

// https://docs.rs/activitystreams/0.7.0-alpha.20/activitystreams/index.html#parse
// also examples/handle_incoming.rs

use activitystreams::activity::ActorAndObject;
// use activitystreams::activity::ActorAndObjectRef;
// use activitystreams::activity::ActorAndObjectRefExt;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum AcceptedTypes {
    // Accept,
    // Announce,
    // Create,
    // Delete,
    Follow,
  // Reject,
  //  Update,
    Undo,
}

pub type AcceptedActivity = ActorAndObject<AcceptedTypes>;


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
    // generate keypair used for signing AP requests
    let rsa = Rsa::generate(2048).unwrap();
    let pkey = PKey::from_rsa(rsa).unwrap();
    let public_key = pkey.public_key_to_pem().unwrap();
    let private_key = pkey.private_key_to_pem_pkcs8().unwrap();
    // let key_to_string = |key| match String::from_utf8(key) {
    //   Ok(s) => Ok(s),
    //   Err(e) => Err(Error::new(
    //     ErrorKind::Other,
    //     format!("Failed converting key to string: {}", e),
    //   )),
    // };

    let private_key_str = String::from_utf8(private_key).unwrap();
    let public_key_str = String::from_utf8(public_key).unwrap();

    let feed_id = sqlx::query!("INSERT INTO feeds (user_id, url, name, private_key, public_key)
                                VALUES($1, $2, $3, $4, $5)",
                               user.id, url, name, private_key_str, public_key_str)
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

  // Return an object here instead of JSON so we can manipulate it if needed
  pub fn to_activity_pub(&self, domain: &String) -> Result<ExtendedService, AnyError> {    
    let mut svc = Ext1::new(
        ApActor::new(
          iri!("https://example.com/inbox"),
          Service::new(),
        ),
        PublicKey {
            public_key: PublicKeyInner {
                id: iri!(format!("https://{}/users/{}/feed#main-key", domain, self.name)),
                owner: iri!(format!("https://{}/users/{}/feed", domain, self.name)),
                public_key_pem: self.public_key.to_owned(),
            },
        },
    );
    
    svc
      .set_context(context())
      .add_context(security())
      .set_id(iri!(format!("https://{}/users/{}/feed", domain, self.name)))
      .set_name(self.name.clone())
      .set_preferred_username(self.name.clone())
      .set_inbox(iri!(format!("https://{}/users/{}/inbox", domain, self.name)))
      .set_outbox(iri!(format!("https://{}/users/{}/outbox", domain, self.name)))
      .set_followers(iri!(format!("https://{}/users/{}/followers", domain, self.name)))
      .set_following(iri!(format!("https://{}/users/{}/following", domain, self.name)));
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

    let any_base = svc.into_any_base();

    match any_base {
      Ok(any_base) => {
        println!("any_base: {:#?}", any_base);
        let x = ExtendedService::from_any_base(any_base).unwrap();

        match x {
          Some(x) => Ok(x),
          None => todo!()
        }
      },
      Err(_) => todo!()
    }
  }

  async fn follow(&self, pool: &SqlitePool, actor: &str) -> Result<(), sqlx::Error> {
    sqlx::query!("INSERT INTO followers (feed_id, actor)
                                VALUES($1, $2)",
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
      None => Ok(())
    }
  }

  pub async fn followers(&self, pool: &SqlitePool)  -> Result<OrderedCollection, sqlx::Error>{
    let result = sqlx::query_as!(Follower, "SELECT * FROM followers WHERE feed_id = ?", self.id)
      .fetch_all(pool)
      .await;

      let v: Vec<String> = result
        .into_iter()
        .flat_map(|o| o.into_iter())
        .filter_map(|follower| Some(follower.actor))
        .collect();

      let mut collection = OrderedCollection::new();
      collection.set_many_items(v);

      Ok(collection)
    }
}

  
#[sqlx::test]
async fn test_create(pool: SqlitePool) -> sqlx::Result<()> {
  let user = User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()) };
  
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
  let user = User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()) };
  let url: String = "https://foo.com/rss.xml".to_string();
  let name: String = "testfeed".to_string();

  let feed = Feed::create(&user, &url, &name, &pool).await?;
  let feed2 = Feed::find_by_url(&url, &pool).await?;
  
  assert_eq!(feed, feed2);
  assert_eq!(feed2.url, url);
  
  Ok(())
}
#[sqlx::test]
async fn test_find_by_name(pool: SqlitePool) -> sqlx::Result<()> {
  let user = User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()) };

  let url: String = "https://foo.com/rss.xml".to_string();
  let name: String = "testfeed".to_string();

  let feed = Feed::create(&user, &url, &name, &pool).await?;
  let feed2 = Feed::find_by_url(&url, &pool).await?;
  
  assert_eq!(feed, feed2);
  assert_eq!(feed2.url, url);
  
  Ok(())
}

#[sqlx::test]
async fn test_find(pool: SqlitePool) -> sqlx::Result<()> {
  let user = User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()) };
  
  let url: String = "https://foo.com/rss.xml".to_string();
  let name: String = "testfeed".to_string();
  
  let feed = Feed::create(&user, &url, &name, &pool).await?;
  
  let feed2 = Feed::find(feed.id, &pool).await?;
  
  assert_eq!(feed, feed2);
  assert_eq!(feed2.url, url);
  
  Ok(())
}

#[sqlx::test]
async fn test_for_user(pool: SqlitePool) -> sqlx::Result<()> {
  let user = User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()) };
  
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
  let user = User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()) };

  let url: String = "https://foo.com/rss.xml".to_string();
  let name: String = "testfeed".to_string();
  let feed = Feed::create(&user, &url, &name, &pool).await?;
  
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
    name: "testfeed".to_string(),
    url: "https://foo.com/rss.xml".to_string(),
    private_key: "pk".to_string(),
    public_key: "pk".to_string()
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

#[test]
fn test_feed_to_activity_pub() {
  let feed:Feed = Feed {
    id: 1,
    user_id: 1,
    name: "testfeed".to_string(),
    url: "https://foo.com/rss.xml".to_string(),
    private_key: "private key".to_string(),
    public_key: "public key".to_string()
  };

  let result = feed.to_activity_pub(&"test.com".to_string()).unwrap();
  let output = serde_json::to_string(&result).unwrap();

  println!("{}", output);

  let v: Value = serde_json::from_str(&output).unwrap();
  assert_eq!(v["name"], "testfeed");
  assert_eq!(v["publicKey"]["id"], "https://test.com/users/testfeed/feed#main-key");
  assert_eq!(v["publicKey"]["publicKeyPem"], "public key");  
}


#[sqlx::test]
async fn test_follow(pool: SqlitePool) -> sqlx::Result<()> {
  use serde_json::Value;
  let json: &str = r#"{"actor":"https://activitypub.pizza/users/colin","object":"https://activitypub.pizza/users/colin/feed","type":"Follow"}"#;
  let act:AcceptedActivity = serde_json::from_str(json).unwrap();

  let user = User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()) };
  
  let url:String = "https://foo.com/rss.xml".to_string();
  let name:String = "testfeed".to_string();
  let feed = Feed::create(&user, &url, &name, &pool).await?;

  let actor = "https://activitypub.pizza/users/colin".to_string();

  let result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = ? AND actor = ?", feed.id, actor).fetch_one(&pool).await;

  match result {
    Ok(result) => Ok(result.tally == 0),
    Err(why) => Err(why)
  };

  feed.handle_activity(&pool, &act).await;

  let result2 = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = ? AND actor = ?", feed.id, actor).fetch_one(&pool).await;

  match result2 {
    Ok(result) => Ok(result.tally > 0),
    Err(why) => Err(why)
  };
    
  Ok(())
}

#[sqlx::test]
async fn test_unfollow(pool: SqlitePool) -> sqlx::Result<()> {
  use serde_json::Value;
  let json: &str = r#"{"actor":"https://activitypub.pizza/users/colin","object":"https://activitypub.pizza/users/colin/feed","type":"Follow"}"#;
  let act:AcceptedActivity = serde_json::from_str(json).unwrap();

  let user = User { id: 1, email: "foo@bar.com".to_string(), login_token: "lt".to_string(), access_token: Some("at".to_string()) };
  
  let url:String = "https://foo.com/rss.xml".to_string();
  let name:String = "testfeed".to_string();
  let feed = Feed::create(&user, &url, &name, &pool).await?;

  let actor = "https://activitypub.pizza/users/colin".to_string();

  sqlx::query!("INSERT INTO followers (feed_id, actor) VALUES($1, $2)", feed.id, actor)
    .execute(&pool)
    .await?;

  let result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = ? AND actor = ?", feed.id, actor).fetch_one(&pool).await;
  match result {
    Ok(result) => Ok(result.tally > 0),
    Err(why) => Err(why)
  };


  feed.handle_activity(&pool, &act).await;

  let result2 = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = ? AND actor = ?", feed.id, actor).fetch_one(&pool).await;
  match result2 {
    Ok(result) => Ok(result.tally == 0),
    Err(why) => Err(why)
  };

    
  Ok(())
}
