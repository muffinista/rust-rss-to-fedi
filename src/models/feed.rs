use anyhow::{anyhow};
use anyhow::Error as AnyError;

use rocket::uri;
use url::Url;

use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;

use activitystreams_ext::{Ext1};

use activitystreams::{
  activity::*,
  actor::{ApActor, ApActorExt, Service},
  iri,
  iri_string::types::IriString,
  prelude::*,
  security,
  context,
  collection::{OrderedCollection, OrderedCollectionPage},
  link::Mention,
  object::ApObject,
  object::*,
};

use sqlx::postgres::PgPool;
use serde::{Serialize};

use reqwest;
use feed_rs::parser;

use chrono::{Duration, Utc, prelude::*};
use rocket_dyn_templates::tera::{Tera, Context};

use std::{env, error::Error, fmt};

use md5::{Md5, Digest};

use fang::AsyncQueueable;

use sanitize_html::sanitize_str;
use sanitize_html::rules::predefined::DEFAULT;


use crate::models::Actor;
use crate::models::User;
use crate::models::Item;
use crate::models::Follower;
use crate::utils::keys::*;
use crate::utils::path_to_url;
use crate::services::mailer::*;

use crate::routes::feeds::*;
use crate::routes::ap::inbox::*;
use crate::routes::ap::outbox::*;
use crate::routes::login::*;

use crate::traits::sensitive::*;

use crate::PER_PAGE;

///
/// Extend Notes with a 'sensitive' field which Mastodon uses
///
pub type SensitiveNote = CanBeSensitive<ApObject<Note>>;

impl SensitiveNote {
  pub fn new() -> SensitiveNote {
    CanBeSensitive {
      sensitive: false,
      inner: ApObject::new(Note::new()),
    }
  }
}

impl Default for SensitiveNote {
  fn default() -> Self {
    Self::new()
  }
}


///
/// The is the model for a feed. Most of the data we hold onto here is from attributes
/// in the RSS
///
#[derive(Debug, Serialize)]
pub struct Feed {
  pub id: i32,
  pub admin: bool,
  pub user_id: i32,
  pub name: String,
  pub url: String,
  pub private_key: String,
  pub public_key: String,
  pub image_url: Option<String>,
  pub icon_url: Option<String>,

  pub title: Option<String>,
  pub description: Option<String>,
  pub site_url: Option<String>,

  pub listed: bool,
  pub hashtag: Option<String>,
  pub content_warning: Option<String>,
  pub status_publicity: Option<String>,
  
  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>,
  pub refreshed_at: chrono::DateTime::<Utc>,
  pub last_post_at: Option<chrono::DateTime::<Utc>>,

  pub error: Option<String>
}

impl PartialEq for Feed {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

pub struct FeedError {
  message: String
}

impl Error for FeedError {}
impl fmt::Display for FeedError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "Oh no, something bad went down: {:}", self.message)
  }
}

impl fmt::Debug for FeedError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    // @todo does this work?
    let current_file = file!();
    let current_line = line!();

    write!(f, "FeedError {:} {{ file: {current_file:}, line: {current_line:} }}", self.message)
  }
}

impl From<sqlx::Error> for FeedError {
  fn from(error: sqlx::Error) -> Self {
    FeedError {
      message: error.to_string(),
    }
  }
}



///
/// This is a list of activity types that we want to handle
///
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum AcceptedTypes {
  Accept,
  Create,
  Delete,
  Follow,
  Undo,
}

pub type AcceptedActivity = ActorAndObject<AcceptedTypes>;

///
/// Extend Service with a public key
///
pub type ExtendedService = Ext1<ApActor<Service>, PublicKey>;


impl Feed {
  pub async fn find(id: i32, pool: &PgPool) -> Result<Feed, sqlx::Error> {
    sqlx::query_as!(Feed, "SELECT * FROM feeds WHERE id = $1", id)
    .fetch_one(pool)
    .await
  }

  pub async fn for_item(item_id: i32, pool: &PgPool) -> Result<Feed, sqlx::Error> {
    sqlx::query_as!(Feed, "SELECT feeds.* FROM feeds INNER JOIN items ON items.feed_id = feeds.id WHERE items.id = $1", item_id)
    .fetch_one(pool)
    .await
  }


  ///
  /// Return a page of feeds
  ///
  pub async fn paged(page: i32, pool: &PgPool) -> Result<Vec<Feed>, sqlx::Error> {
    let offset:i64 = ((page - 1) * PER_PAGE) as i64;

    sqlx::query_as!(Feed, "SELECT * FROM feeds ORDER BY id DESC LIMIT $1 OFFSET $2", PER_PAGE as i64, offset )
      .fetch_all(pool)
      .await
  }

  ///
  /// Return a page of feeds for a given user
  ///
  pub async fn paged_for_user(user: &User, page: i32, pool: &PgPool) -> Result<Vec<Feed>, sqlx::Error> {
    let offset:i64 = ((page - 1) * PER_PAGE) as i64;

    sqlx::query_as!(Feed, "SELECT * FROM feeds WHERE user_id = $1 ORDER BY id DESC LIMIT $2 OFFSET $3", user.id, PER_PAGE as i64, offset )
      .fetch_all(pool)
      .await
  }

  ///
  /// Get a count of how many items we have for this feed
  ///
  pub async fn count(pool: &PgPool)  -> Result<i32, AnyError> {
    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM feeds")
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() as i32),
      Err(why) => Err(why.into())
    }
  }
  
  ///
  /// Get a count of how many items we have for this feed
  ///
  pub async fn count_for_user(user: &User, pool: &PgPool)  -> Result<i32, AnyError> {
    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM feeds WHERE user_id = $1", user.id)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() as i32),
      Err(why) => Err(why.into())
    }
  }
  

  ///
  /// Query the db for a feed owned by this user with the given name
  ///
  pub async fn find_by_user_and_name(user: &User, name: &String, pool: &PgPool) -> Result<Option<Feed>, sqlx::Error> {
    sqlx::query_as!(Feed, "SELECT * FROM feeds WHERE name = $1 AND user_id = $2", name, user.id)
      .fetch_optional(pool)
      .await
  }

  ///
  /// Query the db for a maximum of _limit_ feeds older than _age_ seconds
  ///
  pub async fn stale(pool: &PgPool, age:i64, limit: i64) -> Result<Vec<Feed>, sqlx::Error> {
    let age = Utc::now() - Duration::seconds(age);
    sqlx::query_as!(Feed, "SELECT * FROM feeds WHERE admin = false AND refreshed_at < $1 ORDER BY refreshed_at LIMIT $2", age, limit)
    .fetch_all(pool)
    .await
  }

  ///
  /// Find the 'admin' feed. This is a special feed that will be used to
  /// send messages, handle authentications, etc
  ///
  pub async fn for_admin(pool: &PgPool) -> Result<Option<Feed>, sqlx::Error> {
    sqlx::query_as!(Feed, "SELECT * FROM feeds WHERE admin = true LIMIT 1")
    .fetch_optional(pool)
    .await
  }

  ///
  /// Find all the feeds for the given user
  ///
  pub async fn for_user(user: &User, pool: &PgPool) -> Result<Vec<Feed>, sqlx::Error> {
    sqlx::query_as!(Feed, "SELECT * FROM feeds WHERE user_id = $1", user.id)
    .fetch_all(pool)
    .await
  }
    
  pub async fn find_by_name(name: &String, pool: &PgPool) -> Result<Option<Feed>, sqlx::Error> {
    sqlx::query_as!(Feed, "SELECT * FROM feeds WHERE name = $1", name)
      .fetch_optional(pool)
      .await
  }

  pub async fn load_by_name(name: &String, pool: &PgPool) -> Result<Feed, sqlx::Error> {
    sqlx::query_as!(Feed, "SELECT * FROM feeds WHERE name = $1", name)
      .fetch_one(pool)
      .await
  }

  ///
  /// Check if a feed exists with the given name
  ///
  pub async fn exists_by_name(name: &String, pool: &PgPool) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("SELECT count(1) AS tally FROM feeds WHERE name = $1", name)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() > 0),
      Err(why) => Err(why)
    }
  }
  
  ///
  /// Create a feed
  ///
  pub async fn create(user: &User,
      url: &String,
      name: &String, pool: &PgPool) -> Result<Feed, sqlx::Error> {

    // generate keypair used for signing AP requests
    let (private_key_str, public_key_str) = generate_key();
    let old = Utc.with_ymd_and_hms(1900, 1, 1, 0, 0, 0).unwrap();

    let now = Utc::now();

    let status_publicity = Some("unlisted");

    let feed_id = sqlx::query!("INSERT INTO feeds
        (user_id, url, name, private_key, public_key, status_publicity, created_at, updated_at, refreshed_at)
        VALUES($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id",
        user.id, url, name, private_key_str, public_key_str, status_publicity, now, now, old)
      .fetch_one(pool)
      .await?
      .id;
    
    Feed::find(feed_id, pool).await
  }

  ///
  /// Save/update the feed
  ///
  pub async fn save(&self, pool: &PgPool) -> Result<&Feed, sqlx::Error> {
    let now = Utc::now();

    let clean_hashtag = if self.hashtag.is_some() && !self.hashtag.clone().unwrap().is_empty() {
      Some(self.hashtag.clone().unwrap().replace(['#', ' '], ""))
    } else {
      None
    };

    sqlx::query!("UPDATE feeds
      SET url = $1,
          name = $2,
          private_key = $3,
          public_key = $4,
          image_url = $5,
          icon_url = $6,
          title = $7,
          description = $8,
          site_url = $9,
          error = $10,
          updated_at = $11,
          hashtag = $12,
          content_warning = $13,
          status_publicity = $14,
          admin = $15,
          listed = $16
      WHERE id = $17",
      self.url,
      self.name,
      self.private_key,
      self.public_key,
      self.image_url,
      self.icon_url,
      self.title,
      self.description,
      self.site_url,
      self.error,
      now,
      clean_hashtag,
      self.content_warning,
      self.status_publicity,
      self.admin,
      self.listed,
      self.id
    ).execute(pool)
      .await?;

    Ok(self)
  }

  pub async fn delete(user: &User, id: i32, pool: &PgPool) -> Result<Feed, sqlx::Error> {
    let old_feed = Feed::find(id, pool).await;
    
    sqlx::query!("DELETE FROM feeds WHERE user_id = $1 AND id = $2", user.id, id)
      .execute(pool)
      .await?;
    
    old_feed   
  }

  pub async fn admin_delete(id: i32, pool: &PgPool) -> Result<Feed, sqlx::Error> {
    let old_feed = Feed::find(id, pool).await;
    
    sqlx::query!("DELETE FROM feeds WHERE id = $1", id)
      .execute(pool)
      .await?;
    
    old_feed   
  }


  ///
  /// Is this an admin feed?
  ///
  pub fn is_admin(&self) -> bool {
    self.admin
  }

  ///
  /// Return the number of seconds since this feed was refreshed
  ///
  pub fn age(&self) -> i64 {
    (Utc::now() - self.refreshed_at).num_seconds()
  }


  pub async fn mark_admin(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
    let result = sqlx::query!("UPDATE feeds SET admin = true WHERE id = $1", self.id)
    .execute(pool)
    .await;

    match result {
      Ok(_result) => Ok(()),
      Err(why) => Err(why)
    }
  }

  pub async fn mark_stale(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
    let old = Utc.with_ymd_and_hms(1900, 1, 1, 0, 0, 0).unwrap();
    let result = sqlx::query!("UPDATE feeds SET refreshed_at = $1 WHERE id = $2", old, self.id)
      .execute(pool)
      .await;

    match result {
      Ok(_result) => Ok(()),
      Err(why) => Err(why)
    }
  }

  pub async fn mark_error(&self, err:&String, pool: &PgPool) -> Result<(), sqlx::Error> {
    let result = sqlx::query!("UPDATE feeds SET error = $1 WHERE id = $2", err, self.id)
      .execute(pool)
      .await;

    match result {
      Ok(_result) => Ok(()),
      Err(why) => Err(why)
    }
  }

  pub async fn mark_fresh(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    let result = sqlx::query!("UPDATE feeds SET refreshed_at = $1 WHERE id = $2", now, self.id)
      .execute(pool)
      .await;

    match result {
      Ok(_result) => Ok(()),
      Err(why) => Err(why)
    }
  }

  ///
  /// Get a count of how many items we have for this feed
  ///
  pub async fn entries_count(&self, pool: &PgPool)  -> Result<i32, AnyError> {
    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM items WHERE feed_id = $1", self.id)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() as i32),
      Err(why) => Err(why.into())
    }
  }

  ///
  /// Find the user who owns this feed
  ///
  pub async fn user(&self, pool: &PgPool) -> Result<User, sqlx::Error> {
    sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", self.user_id)
      .fetch_one(pool)
      .await
  }


  ///
  /// load the contents of the feed
  ///
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
    // eprintln!("Response: {:?} {}", res.version(), res.status());
    // eprintln!("Headers: {:#?}\n", res.headers());
      
    res.text().await
  }


  ///
  /// check parsed feed data for any entries we should convert into new items
  ///
  pub async fn feed_to_entries(&self, data: feed_rs::model::Feed, pool: &PgPool) -> Result<Vec<Item>, sqlx::Error> {
    let mut result: Vec<Item> = Vec::new();
    for entry in data.entries.iter() {
      let exists = Item::exists_by_guid(&entry.id, self, pool).await.unwrap();

      // only create new items
      if ! exists {
        let item = Item::create_from_entry(entry, self, pool).await;
        match item {
          Ok(item) => result.push(item),
          Err(why) => return Err(why)
        };
      }
    }

    Ok(result)
  }

  pub async fn update_last_post_at(&self, published_at: chrono::DateTime::<Utc>, pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!(
      "UPDATE feeds SET last_post_at = $1 WHERE id = $2 AND last_post_at < $1",
      published_at,
      self.id
    ).execute(pool).await?;

    Ok(())
  }


  ///
  /// grab new data for this feed, and deliver any new entries to followers
  ///
  pub async fn refresh(&mut self, pool: &PgPool, queue: &mut dyn AsyncQueueable) -> Result<(), AnyError> {
    // skip processing for admin accounts
    if self.is_admin() {
      self.mark_fresh(pool).await?;
      return Ok(())
    }

    let items = self.parse(pool).await;
    match items {
      Ok(items) => {
        if !items.is_empty() {
          log::info!("delivering {} items", items.len());
          for item in items {
            item.deliver(self, pool, queue).await?;
          }  
        }

        self.mark_fresh(pool).await?;

        Ok(())
      },
      Err(why) => {
        // we mark as fresh even though this failed so we don't get stuck on bad feeds
        // @todo mark as erroring
        Err(anyhow!(why.to_string()))
      }
    }
  }

  ///
  /// load and parse feed
  /// returns a list of any new items
  ///
  pub async fn parse(&mut self, pool: &PgPool) -> Result<Vec<Item>, FeedError> {        
    // skip processing for admin accounts
    if self.is_admin() {
      self.mark_fresh(pool).await.unwrap();
      return Ok(Vec::<Item>::new())
    }

    let body = Feed::load(self).await;
    match body {
      Ok(body) => {
        let work = self.parse_from_data(body.to_string(), pool).await;
        match work {
          Ok(entries) => Ok(entries),
          Err(why) => {
            self.mark_error(&why.to_string(), pool).await.unwrap();
            Err(FeedError { message: why.to_string() })
          }
        }
      },
      Err(why) => {
        self.mark_error(&why.to_string(), pool).await.unwrap();
        Err(FeedError { message: why.to_string() })
      }
    }   
  }

  ///
  /// update our stored data from the downloaded feed data
  ///
  pub async fn parse_from_data(&mut self, body: String, pool: &PgPool) -> Result<Vec<Item>, FeedError> {        
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
        }
        if data.logo.is_some() {
          self.image_url = Some(data.logo.as_ref().unwrap().uri.clone());
        }

        // parse out a likely site link
        if !data.links.is_empty() {
          let query:Option<feed_rs::model::Link> = data.links
            .clone()
            .into_iter()
            .find(|link| 
              (link.media_type.is_none() && link.rel.is_none()) ||
              (link.media_type.is_some() && link.media_type.as_ref().unwrap() == "text/html") ||
              (link.rel.is_some() && link.rel.as_ref().unwrap() == "self")
            );
      
          self.site_url = if query.is_some() {
            Some(query.unwrap().href)
          } else {
            None
          }
        }

        // todo snag link too

        let update = self.save(pool).await;
        match update {
          Ok(_update) => {
            let result = self.feed_to_entries(data, pool).await;
            match result {
              Ok(result) => Ok(result),
              Err(why) => Err(FeedError { message: why.to_string() })
            }    
          }
          Err(why) => Err(FeedError { message: why.to_string() })
        }
      },
      Err(why) => Err(FeedError { message: why.to_string() })
    }
  }


  ///
  /// Return URL to use in ActivityPub output for this feed
  ///
  pub fn ap_url(&self) -> String {
    path_to_url(&uri!(render_feed(&self.name)))
  }

  ///
  /// Return URL to use in HTML output for this feed
  ///
  pub fn permalink_url(&self) -> String {
    path_to_url(&uri!(show_feed(&self.name, None::<i32>)))
  }

  ///
  /// URL for the followers route
  ///
  pub fn followers_url(&self) -> String {
    path_to_url(&uri!(render_feed_followers(&self.name, None::<i32>)))
  }

  ///
  /// return the email-style address for this feed
  ///
  pub fn address(&self) -> String {
    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
    format!("@{}@{}", self.name, instance_domain)
  }

  pub fn description_with_footer(&self) -> String {
    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
    let footer = format!("Powered by @admin@{instance_domain:}");

    if self.description.is_some() {
      format!("{:}\n\n{footer:}", self.description.clone().unwrap())
    } else {
      footer
    }
  }


  ///
  /// Generate valid ActivityPub data for this feed
  ///
  pub fn to_activity_pub(&self) -> Result<String, AnyError> {
    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
    let feed_url = self.ap_url();
    let mut svc = Ext1::new(
      ApActor::new(
        iri!(feed_url),
        Service::new(),
      ),
      PublicKey {
        public_key: PublicKeyInner {
          id: iri!(format!("{feed_url}#main-key")),
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
      .set_inbox(iri!(path_to_url(&uri!(user_inbox(&self.name)))))
      .set_outbox(iri!(path_to_url(&uri!(render_feed_outbox(&self.name, None::<i32>)))))
      .set_followers(iri!(self.followers_url()));

    if self.is_admin() {
      svc.set_summary(format!("Admin account for {instance_domain}"));

      let mut icon = Image::new();
      icon.set_url(iri!(format!("https://{instance_domain}/assets/icon.png")));
      svc.set_icon(icon.into_any_base()?);

      let mut image = Image::new();
      image.set_url(iri!(format!("https://{instance_domain}/assets/image.png")));
      svc.set_image(image.into_any_base()?);

    } else {
      svc.set_summary(self.description_with_footer());
  
      if self.icon_url.is_some() {
        let mut icon = Image::new();
        icon.set_url(iri!(self.icon_url.clone().unwrap()));
        svc.set_icon(icon.into_any_base()?);
      } else if self.image_url.is_some() {
        let mut icon = Image::new();
        icon.set_url(iri!(self.image_url.clone().unwrap()));
        svc.set_icon(icon.into_any_base()?);
      } else {
        let mut icon = Image::new();
        icon.set_url(iri!(format!("https://{instance_domain}/assets/icon.png")));
        svc.set_icon(icon.into_any_base()?);
      }
  
      if self.image_url.is_some() {
        let mut image = Image::new();
        image.set_url(iri!(self.image_url.clone().unwrap()));
        svc.set_image(image.into_any_base()?);
      }
    }

    // in theory we could return an object here instead of JSON so we can
    // manipulate it if needed but i had trouble getting that to work because of
    // assorted traits throwing issues when calling into_any_base()

    // generate JSON and return
    Ok(serde_json::to_string(&svc).unwrap())   
  }

  ///
  /// add follower to feed
  ///
  pub async fn add_follower(&self, pool: &PgPool, actor: &str) -> Result<(), AnyError> {
    let now = Utc::now();


    let result = sqlx::query!("INSERT INTO followers 
        (feed_id, actor, created_at, updated_at) 
        VALUES($1, $2, $3, $4)
        ON CONFLICT (feed_id, actor) DO UPDATE
        SET updated_at = EXCLUDED.updated_at",
                 self.id, actor, now, now)
      .execute(pool)
      .await;

    match result {
      Ok(_result) => Ok(()),
      Err(why) => Err(anyhow!(why.to_string()))
    } 
  }

  ///
  /// handle an actor following the feed by adding them to the db and sending an Accept message back
  ///
  pub async fn follow(&self, pool: &PgPool, actor: &str, activity: &AcceptedActivity) -> Result<(), AnyError> {
    // store follower in the db
    self.add_follower(pool, actor).await?;

    // now let's deliver an Accept message

    // reconstruct original follow activity
    let (_actor, _object, original_follow) = activity.clone().into_parts();

    let mut follow = Follow::new(actor, self.ap_url());

    let inbox = format!("{actor}/inbox");
    let follow_id: &IriString = original_follow.id_unchecked().unwrap();
    follow.set_id(follow_id.clone());

    // generate accept message for follow activity
    let mut accept = Accept::new(self.ap_url(), follow.into_any_base()?);
    accept.set_context(context());

    // deliver to the user
    deliver_to_inbox(&Url::parse(&inbox)?, &self.ap_url(), &self.private_key, &accept).await
  }

  ///
  /// handle an incoming message. we mostly ignore these except a user can message the admin
  /// feed to login to the site to add/manage feeds
  ///
  pub async fn incoming_message(&self, pool: &PgPool, actor_url: &str, activity: &AcceptedActivity) -> Result<(), AnyError> {

    log::info!("ACTOR: {actor_url:}");

    // THIS GETS THE CONTENT OF THE STATUS and is clearly
    // a bit of a hack, but it's hard to get the content of the
    // note to not end up in an unparsed bit of modelling anyway 
    // @todo figure out the polite way to do this
    let s = serde_json::to_string(&activity.object()?).unwrap();
    log::info!("MESSAGE: {s:}");


    let note: ApObject<Note> = serde_json::from_str(&s).unwrap();
    let content = note.content().unwrap();
    let message = content.as_single_xsd_string();

    // let message = activity.object()?.as_single_base();
    // println!("MESSAGE: {:?}", message);

    // ignore messages that aren't to admin feed
    if ! self.is_admin() || message.is_none() {
      return Ok(())
    }

    let clean_message = sanitize_str(&DEFAULT, message.unwrap()).unwrap().to_lowercase();

    // println!("MESSAGE: {clean_message:}");
    let matches: Vec<_> = clean_message.match_indices("help").collect();

    // check for the word 'help' in the beginning of the message
    if matches.is_empty() || matches.first().unwrap().0 > 100 {
      log::info!("User didn't ask for help in the beginning of the message");
      return Ok(());      
    }

    // grab the actor information for the sender
    let dest_actor = Actor::find_or_fetch(actor_url, pool).await;
    match dest_actor {
      Ok(dest_actor) => {
        if dest_actor.is_none() {
          // println!("Actor not found");
          return Ok(());
        }
        let dest_actor = dest_actor.unwrap();

        // generate a login message for this user
        let message = self.generate_login_message(Some(activity), &dest_actor, pool).await?;
        let msg = serde_json::to_string(&message).unwrap();
        log::info!("{msg}");
    
        let my_url = self.ap_url();

        // send the message!
        let result = deliver_to_inbox(&Url::parse(&dest_actor.inbox_url)?, &my_url, &self.private_key, &message).await;
    
        match result {
          Ok(result) => log::info!("sent! {result:?}"),
          Err(why) => log::info!("failure! {why:?}")
        }    
      },
      Err(why) => {
        log::info!("couldnt find actor: {why:?}");
      }
    }

    Ok(())
  }

  ///
  /// generate a login message to send to the user
  ///
  pub async fn generate_login_message(&self, activity: Option<&AcceptedActivity>, dest_actor: &Actor, pool: &PgPool) -> Result<ApObject<Create>, AnyError> {

    let mut reply: SensitiveNote = SensitiveNote::new();

    let my_url = self.ap_url();

    let source_id;
    let uniq_hash;
    let mut hasher = Md5::new();

    if activity.is_some() {
      let (_, object, _) = activity.unwrap().clone().into_parts();
      let source_value = object.as_single_id().unwrap().to_string();
  
      // generate a hash of the incoming actor id. we'll tack
      // this on the end of the ID for the reply to make it
      // vaguely unique to the conversation
      hasher.update(&source_value);
      uniq_hash = format!("{:X}", hasher.finalize());        
      source_id = Some(source_value);
    } else {
      uniq_hash = format!("{:X}", hasher.finalize());  
      source_id = None;
    }


    // lookup the user in the db. if they don't exist, add them
    let user = User::find_or_create_by_actor_url(&dest_actor.url, pool).await.unwrap();

    // update with actor information
    user.apply_actor(dest_actor, pool).await.unwrap();

    let auth_url = path_to_url(&uri!(attempt_login(&user.login_token)));

    let mut mention = Mention::new();
    mention
      .set_href(iri!(dest_actor.url.to_string()))
      .set_name("en");

    let tera = match Tera::new("templates/email/*.*") {
      Ok(t) => t,
      Err(e) => {
        println!("Parsing error(s): {e}");
        ::std::process::exit(1);
      }
    };
  
    let mut template_context = Context::new();
    template_context.insert("link", &auth_url);
    
    let body = tera.render("send-login-status.text.tera", &template_context).unwrap();

    reply
      .set_sensitive(true)
      .set_attributed_to(iri!(my_url))
      .set_content(body)
      .set_url(iri!(my_url))
      .set_id(iri!(format!("{my_url}/{uniq_hash}")))
      .set_to(iri!(dest_actor.url))
      .set_tag(mention.into_any_base()?);

    if source_id.is_some() {
      reply.set_in_reply_to(iri!(source_id.expect("")));
    }


    let mut action: ApObject<Create> = ApObject::new(
      Create::new(
        iri!(my_url),
        reply.into_any_base()?
      )
    );

    action
      .set_context(context())
      .add_context(security())
      .add_context("as:sensitive".to_string());

    Ok(action) 
  }

  
  ///
  /// handle unfollow activity
  ///
  pub async fn unfollow(&self, pool: &PgPool, actor: &str) -> Result<(), AnyError>  {
    sqlx::query!("DELETE FROM followers WHERE feed_id = $1 AND actor = $2",
                 self.id, actor)
      .execute(pool)
      .await?;
    
    Ok(())
  }

  ///
  /// handle any incoming events
  ///
  pub async fn handle_activity(&self, pool: &PgPool, activity: &AcceptedActivity)  -> Result<(), AnyError> {
    let s = serde_json::to_string(&activity).unwrap();
    log::info!("{s:}");

    let (actor, _object, act) = activity.clone().into_parts();

    let actor_id = actor.as_single_id().unwrap().to_string();
    
    match act.kind() {
      Some(AcceptedTypes::Follow) => self.follow(pool, &actor_id, activity).await,
      Some(AcceptedTypes::Undo) => self.unfollow(pool, &actor_id).await,
      Some(AcceptedTypes::Delete) => self.unfollow(pool, &actor_id).await,
      Some(AcceptedTypes::Create) => self.incoming_message(pool, &actor_id, activity).await,
      // we don't need to handle this but if we receive it, just move on
      Some(AcceptedTypes::Accept) => Ok(()),
      None => Ok(())
    }
  }

  ///
  /// generate an AP message to this user with a link to this feed
  ///
  pub async fn link_to_feed_message(&self, actor: &Actor) -> Result<ApObject<Create>, AnyError> {
    let mut reply: SensitiveNote = SensitiveNote::new();

    let my_url = self.permalink_url();

    let random_id: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();

    // mention the creator so they get pinged
    let mut mention = Mention::new();
    mention
      .set_href(iri!(&actor.url))
      .set_name("en");

    // mention the new feed account so it gets hyperlinked
    let mut feed_mention = Mention::new();
    feed_mention
      .set_href(iri!(&self.permalink_url()))
      .set_name(self.address());
  

    let tera = match Tera::new("templates/email/*.*") {
      Ok(t) => t,
      Err(e) => {
        println!("Parsing error(s): {e}");
        ::std::process::exit(1);
      }
    };
  
    let mut template_context = Context::new();
    template_context.insert("link", &self.permalink_url());
    template_context.insert("address", &self.address());
    
    let body = tera.render("send-creation-status.text.tera", &template_context).unwrap();

    reply
      .set_sensitive(true)
      .set_attributed_to(iri!(my_url))
      .set_content(body)
      .set_url(iri!(my_url))
      .set_id(iri!(format!("{my_url}/{random_id}")))
      .set_to(iri!(&actor.url))
      .add_tag(mention.into_any_base()?)
      .add_tag(feed_mention.into_any_base()?);

    let mut action: ApObject<Create> = ApObject::new(
      Create::new(
        iri!(my_url),
        reply.into_any_base()?
      )
    );

    action
      .set_context(context())
      .add_context(security())
      .add_context("as:sensitive".to_string());

    Ok(action) 
  }

  ///
  /// figure out how many people are following the feed
  ///
  pub async fn follower_count(&self, pool: &PgPool)  -> Result<i32, AnyError>{
    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = $1", self.id)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() as i32),
      Err(why) => Err(why.into())
    }
  }

  ///
  /// get a list of all followers
  ///
  pub async fn followers_list(&self, pool: &PgPool)  -> Result<Vec<Follower>, sqlx::Error> {
    sqlx::query_as!(Follower, "SELECT * FROM followers WHERE feed_id = $1", self.id)
      .fetch_all(pool)
      .await
  }
  
  ///
  /// generate AP data to represent follower information
  ///
  pub async fn followers(&self, pool: &PgPool)  -> Result<ApObject<OrderedCollection>, AnyError> {
    let count = self.follower_count(pool).await?;
    let total_pages = (count / PER_PAGE) + 1;

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
      .set_total_items(count as u64)
      .set_first(iri!(path_to_url(&uri!(render_feed_followers(&self.name, Some(1))))))
      .set_last(iri!(path_to_url(&uri!(render_feed_followers(&self.name, Some(total_pages))))));

    Ok(collection)
  }

  ///
  /// generate actual AP page of followes 
  ///
  pub async fn followers_paged(&self, page: i32, pool: &PgPool)  -> Result<ApObject<OrderedCollectionPage>, AnyError> {
    let count = self.follower_count(pool).await?;
    let total_pages:i32 = (count / PER_PAGE) + 1;
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
    
    let offset:i64 = ((page - 1) * PER_PAGE) as i64;
    let result = sqlx::query_as!(Follower, "SELECT * FROM followers WHERE feed_id = $1 LIMIT $2 OFFSET $3", self.id, PER_PAGE as i64, offset )
      .fetch_all(pool)
      .await;
  
    match result {
      Ok(result) => {
        let v: Vec<String> = result
          .into_iter()
          .map(|follower| follower.actor)
          .collect();
        
        collection.set_many_items(v);
        
        Ok(collection)
      },
      Err(why) => Err(why.into())
    }
  }

  ///
  /// generate AP data to represent outbox information
  ///
  pub async fn outbox(&self, pool: &PgPool)  -> Result<ApObject<OrderedCollection>, AnyError> {
    let count = self.entries_count(pool).await?;
    let total_pages = (count / PER_PAGE) + 1;

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
      .set_summary("A list of outbox items".to_string())
      .set_total_items(count as u64)
      .set_first(iri!(path_to_url(&uri!(render_feed_outbox(&self.name, Some(1))))))
      .set_last(iri!(path_to_url(&uri!(render_feed_outbox(&self.name, Some(total_pages))))));

    Ok(collection)
  }

  ///
  /// generate actual AP page of follows 
  ///
  pub async fn outbox_paged(&self, page: i32, pool: &PgPool)  -> Result<ApObject<OrderedCollectionPage>, AnyError>{
    let count = self.entries_count(pool).await?;
    let total_pages = (count / PER_PAGE) + 1;
    let mut collection: ApObject<OrderedCollectionPage> = ApObject::new(OrderedCollectionPage::new());

    collection
      .set_context(context())
      .set_summary("A list of outbox items".to_string())
      .set_part_of(iri!(path_to_url(&uri!(render_feed(&self.name)))))
      .set_first(iri!(path_to_url(&uri!(render_feed_outbox(&self.name, Some(1))))))
      .set_last(iri!(path_to_url(&uri!(render_feed_outbox(&self.name, Some(total_pages))))))
      .set_current(iri!(path_to_url(&uri!(render_feed_outbox(&self.name, Some(page))))));

    if page > 1 {
      collection.set_prev(iri!(path_to_url(&uri!(render_feed_outbox(&self.name, Some(page - 1))))));
    }

    if page < total_pages {
      collection.set_next(iri!(path_to_url(&uri!(render_feed_outbox(&self.name, Some(page + 1))))));
    }

    // return empty collection for invalid pages
    if page == 0 || page > total_pages {
      return Ok(collection)
    }
    
    let offset = (page - 1) * PER_PAGE;
    let result = sqlx::query_as!(Item, "SELECT * FROM items WHERE feed_id = $1 LIMIT $2 OFFSET $3",
      self.id as i32, PER_PAGE as i32, offset as i32)
      .fetch_all(pool)
      .await;

    match result {
      Ok(result) => {
        for item in result {
          let output = item.to_activity_pub(self, pool).await.unwrap();
          collection.add_item(output.into_any_base()?);    
        }

        Ok(collection)
      },
      Err(why) => Err(why.into())
    }
  }
}

#[cfg(test)]
mod test {
  use std::fs;
  use sqlx::postgres::PgPool;
  use rocket::uri;
  use feed_rs::parser;
  use chrono::Utc;

  use crate::models::Feed;
  use crate::models::feed::AnyError;
  use crate::models::feed::AcceptedActivity;
  use crate::models::Item;
  use crate::models::Enclosure;
  use crate::models::Actor;

  use crate::utils::test_helpers::{fake_user, fake_feed, real_feed, real_user, real_item, real_actor};
  use crate::utils::path_to_url;

  use crate::routes::feeds::*;
  use crate::routes::ap::outbox::*;


  use mockito::mock;

  #[sqlx::test]
  async fn test_create(pool: PgPool) -> sqlx::Result<()> {
    let user = fake_user();
    let feed:Feed = real_feed(&pool).await?;
    
    assert_eq!(feed.user_id, user.id);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_save(pool: PgPool) -> sqlx::Result<()> {
   
    let mut feed:Feed = real_feed(&pool).await?;
    
    let newname = "testfeed2".to_string();
    feed.name = newname.clone();

    let updated_feed = feed.save(&pool).await?;

    assert_eq!(updated_feed.name, newname);

    Ok(())
  }

  #[sqlx::test]
  async fn test_save_hashtag(pool: PgPool) -> sqlx::Result<()> {
   
    let mut feed:Feed = real_feed(&pool).await?;
    
    let hashtag = Some("#hello there".to_string());
    feed.hashtag = hashtag.clone();

    feed.save(&pool).await?;

    let updated_feed = Feed::find(feed.id, &pool).await?;

    assert_eq!(updated_feed.hashtag.clone().unwrap(), "hellothere".to_string());

    Ok(())
  }

  #[sqlx::test]
  async fn test_find(pool: PgPool) -> sqlx::Result<()> {
    let feed:Feed = real_feed(&pool).await?;
    let feed2 = Feed::find(feed.id, &pool).await?;
    
    assert_eq!(feed, feed2);
    assert_eq!(feed2.url, feed.url);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_stale(pool: PgPool) -> sqlx::Result<()> {
    let _feed: Feed = real_feed(&pool).await?;
    let feed2: Feed = real_feed(&pool).await?;
    

    let stale = Feed::stale(&pool, 100, 100).await?;
    assert_eq!(stale.len(), 2);

    feed2.mark_fresh(&pool).await?;

    let stale2 = Feed::stale(&pool, 100, 100).await?;
    assert_eq!(stale2.len(), 1);

    feed2.mark_stale(&pool).await?;

    let stale3 = Feed::stale(&pool, 100, 100).await?;
    assert_eq!(stale3.len(), 2);

    Ok(())
  }

  #[sqlx::test]
  async fn test_for_user(pool: PgPool) -> sqlx::Result<()> {
    let user = real_user(&pool).await?;
    
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
  async fn test_delete(pool: PgPool) -> sqlx::Result<()> {
    let user = fake_user();
    let feed:Feed = real_feed(&pool).await?;

    let deleted_feed = Feed::delete(&user, feed.id, &pool).await?;
    assert_eq!(feed, deleted_feed);
    
    let feeds = Feed::for_user(&user, &pool).await?; 
    assert_eq!(feeds.len(), 0);
    
    Ok(())
  }

  #[sqlx::test]
  async fn test_parse_from_data(pool: PgPool) -> sqlx::Result<()> {
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
  async fn test_feed_to_entries(pool: PgPool) -> sqlx::Result<()> {
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
 

  #[sqlx::test]
  async fn test_feed_with_enclosure_to_entries(pool: PgPool) -> sqlx::Result<()> {
    let feed:Feed = real_feed(&pool).await?;

    assert_eq!(feed.entries_count(&pool).await.unwrap(), 0);

    let path = "fixtures/test_enclosures.xml";
    let data = parser::parse(fs::read_to_string(path).unwrap().as_bytes()).unwrap();

    let result = Feed::feed_to_entries(&feed, data, &pool).await.unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(feed.entries_count(&pool).await.unwrap(), 1);

    // check that reloading the same feed doesn't create more records
    let data2 = parser::parse(fs::read_to_string(path).unwrap().as_bytes()).unwrap();
    let result2 = Feed::feed_to_entries(&feed, data2, &pool).await.unwrap();

    assert_eq!(result2.len(), 0);
    assert_eq!(feed.entries_count(&pool).await.unwrap(), 1);

    let items = Item::for_feed(&feed, 10, &pool).await?;

    let enclosures = Enclosure::for_item(&items[0], &pool).await?;
    assert_eq!(enclosures.len(), 1);

    let enclosure = &enclosures[0];
    assert_eq!(enclosure.url, "https://secretassets.colinlabs.com/podcasts/0232.mp3");
    assert_eq!(enclosure.content_type.as_ref().unwrap(), "audio/mp3");
    assert_eq!(enclosure.size, None);

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
    // assert_eq!(v["publicKey"]["publicKeyPem"], "public key");  
  }

  #[sqlx::test]
  fn test_admin_feed_to_activity_pub(pool: PgPool) -> Result<(), String> {
    use std::env;
    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

    use serde_json::Value;
    let tmpfeed:Feed = real_feed(&pool).await.unwrap();
    tmpfeed.mark_admin(&pool).await.unwrap();

    let feed = Feed::find(tmpfeed.id, &pool).await.unwrap();
    let output = feed.to_activity_pub().unwrap();

    let v: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(v["summary"], format!("Admin account for {}", instance_domain));
    assert_eq!(v["image"]["url"], format!("https://{}/assets/image.png", instance_domain));
    assert_eq!(v["icon"]["url"], format!("https://{}/assets/icon.png", instance_domain));

    Ok(())
  }

  #[sqlx::test]
  async fn test_follow(pool: PgPool) -> Result<(), String> {
    let actor = format!("{}/users/colin", &mockito::server_url());

    let json = format!(r#"{{"id": "{}/1/2/3", "actor":"{}","object":{{ "id": "{}" }} ,"type":"Follow"}}"#, &mockito::server_url(), actor, actor).to_string();
    let act:AcceptedActivity = serde_json::from_str(&json).unwrap();


    let _m = mock("GET", "/users/colin")
      .with_status(200)
      .with_header("Accept", "application/ld+json")
      .create();

    let _m2 = mock("POST", "/users/colin/inbox")
      .with_status(202)
      .create();


    let feed:Feed = real_feed(&pool).await.unwrap();

    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = $1 AND actor = $2", feed.id, actor)
      .fetch_one(&pool)
      .await
      .unwrap();

    assert!(result.tally.unwrap() == 0);

    let activity_result = feed.handle_activity(&pool, &act).await;

    match activity_result {
      Ok(_result) => {
        let result2 = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = $1 AND actor = $2", feed.id, actor)
        .fetch_one(&pool)
        .await
        .unwrap();
  
        assert!(result2.tally.unwrap() > 0);

        Ok(())
      },

      Err(why) => Err(why.to_string())
    }
  }

  #[sqlx::test]
  async fn test_unfollow(pool: PgPool) -> Result<(), String> {
    let actor = "https://activitypub.pizza/users/colin".to_string();
    let json = format!(r#"{{"actor":"{}","object":"{}/feed","type":"Undo"}}"#, actor, actor).to_string();
    let act:AcceptedActivity = serde_json::from_str(&json).unwrap();
    
    let feed:Feed = real_feed(&pool).await.unwrap();
    let now = Utc::now();

    sqlx::query!("INSERT INTO followers (feed_id, actor, created_at, updated_at) VALUES($1, $2, $3, $4)", feed.id, actor, now, now)
      .execute(&pool)
      .await
      .unwrap();

    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = $1 AND actor = $2", feed.id, actor)
      .fetch_one(&pool)
      .await
      .unwrap();

    assert!(result.tally.unwrap() == 1);

    feed.handle_activity(&pool, &act).await.unwrap();

    let post_result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = $1 AND actor = $2", feed.id, actor)
      .fetch_one(&pool)
      .await
      .unwrap();

    assert!(post_result.tally.unwrap() == 0);
      
    Ok(())
  }


  #[sqlx::test]
  async fn test_generate_login_message(pool: PgPool) -> Result<(), String> {
    let actor = format!("{}/users/colin", &mockito::server_url());

    let json = format!(r#"{{"id": "{}/1/2/3", "actor":"{}","object":{{ "id": "{}" }} ,"type":"Follow"}}"#, &mockito::server_url(), actor, actor).to_string();
    let act:AcceptedActivity = serde_json::from_str(&json).unwrap();

    let feed:Feed = real_feed(&pool).await.unwrap();
    let dest_actor:Actor = real_actor(&pool).await.unwrap();

    let message = feed.generate_login_message(Some(&act), &dest_actor, &pool).await.unwrap();

    let s = serde_json::to_string(&message).unwrap();
    println!("{}", s);

    assert!(s.contains(r#"sensitive":true"#));

    Ok(())
  }

  #[sqlx::test]
  async fn test_generate_login_message_no_activity(pool: PgPool) -> Result<(), String> {
    let feed:Feed = real_feed(&pool).await.unwrap();
    let dest_actor:Actor = real_actor(&pool).await.unwrap();

    let message = feed.generate_login_message(None, &dest_actor, &pool).await.unwrap();

    let s = serde_json::to_string(&message).unwrap();
    println!("{}", s);

    assert!(s.contains(r#"sensitive":true"#));

    Ok(())
  }

  #[sqlx::test]
  async fn test_link_to_feed_message(pool: PgPool) -> Result<(), String> {
    let actor = real_actor(&pool).await.unwrap();
    let feed: Feed = real_feed(&pool).await.unwrap();

    let message = feed.link_to_feed_message(&actor).await.unwrap();

    let s = serde_json::to_string(&message).unwrap();
    println!("{:}", s);

    assert!(s.contains(r#"sensitive":true"#));


    Ok(())
  }


  #[sqlx::test]
  async fn test_followers(pool: PgPool) -> Result<(), String> {
    let feed:Feed = fake_feed();
    let now = Utc::now();

    for i in 1..4 {
      let actor = format!("https://activitypub.pizza/users/colin{}", i);
      sqlx::query!("INSERT INTO followers (feed_id, actor, created_at, updated_at) VALUES($1, $2, $3, $4)", feed.id, actor, now, now)
        .execute(&pool)
        .await
        .unwrap();
    }
    
    let result = feed.followers(&pool).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();
        // println!("{:?}", s);

        assert!(s.contains("A list of followers"));
        Ok(())
      },

      Err(why) => Err(why.to_string())
    }
  }

  #[sqlx::test]
  async fn test_followers_paged(pool: PgPool) -> Result<(), String> {
    let feed:Feed = fake_feed();
    let now = Utc::now();

    for i in 1..35 {
      let actor = format!("https://activitypub.pizza/users/colin{}", i);
      sqlx::query!("INSERT INTO followers (feed_id, actor, created_at, updated_at) VALUES($1, $2, $3, $4)", feed.id, actor, now, now)
        .execute(&pool)
        .await
        .unwrap();
    }

    let result = feed.followers_paged(2, &pool).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();
        // println!("{:?}", s);
        
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
  async fn test_follower_count(pool: PgPool) -> Result<(), String> {
    let feed:Feed = fake_feed();
    let now = Utc::now();

    for i in 1..36 {
      let actor = format!("https://activitypub.pizza/users/colin{}", i);
      sqlx::query!("INSERT INTO followers (feed_id, actor, created_at, updated_at) VALUES($1, $2, $3, $4)", feed.id, actor, now, now)
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


  #[sqlx::test]
  async fn test_outbox(pool: PgPool) -> Result<(), AnyError> {
    let feed:Feed = real_feed(&pool).await?;

    for _i in 1..4 {
      real_item(&feed, &pool).await?;
    }
    
    let result = feed.outbox(&pool).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();
        // println!("{:?}", s);

        assert!(s.contains("A list of outbox items"));
        Ok(())
      },

      Err(why) => Err(why)
    }
  }

  #[sqlx::test]
  async fn test_outbox_paged(pool: PgPool) -> Result<(), AnyError> {
    let feed:Feed = real_feed(&pool).await?;

    for _i in 1..35 {
      real_item(&feed, &pool).await?;
    }

    let result = feed.outbox_paged(2, &pool).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();
        println!("{:}", s);

        assert!(s.contains("OrderedCollectionPage"));
        assert!(s.contains("/items/15"));
        assert!(s.contains("/items/16"));
        assert!(s.contains("/items/17"));
        assert!(s.contains(&format!(r#"first":"{}"#, path_to_url(&uri!(render_feed_outbox(feed.name.clone(), Some(1)))))));
        assert!(s.contains(&format!(r#"prev":"{}"#, path_to_url(&uri!(render_feed_outbox(feed.name.clone(), Some(1)))))));      
        assert!(s.contains(&format!(r#"next":"{}"#, path_to_url(&uri!(render_feed_outbox(feed.name.clone(), Some(3)))))));
        assert!(s.contains(&format!(r#"last":"{}"#, path_to_url(&uri!(render_feed_outbox(feed.name.clone(), Some(4)))))));
        assert!(s.contains(&format!(r#"current":"{}"#, path_to_url(&uri!(render_feed_outbox(feed.name.clone(), Some(2)))))));

        Ok(())
      },
      Err(why) => Err(why)
    }
  }
  
}
