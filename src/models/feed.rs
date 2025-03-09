use tera::Context;
use url::Url;

use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;

use activitystreams_ext::Ext1;

use activitystreams::{
  activity::*,
  actor::{ApActor, ApActorExt, Service},
  base::{AnyBase, BaseExt, ExtendsExt},
  collection::{CollectionExt, CollectionPageExt},
  iri,
  iri_string::types::IriString,
  link::LinkExt,
  security,
  context,
  collection::{OrderedCollection, OrderedCollectionPage},
  link::Mention,
  object::*,
  time::OffsetDateTime
};

use sqlx::postgres::PgPool;
use serde::Serialize;

use feed_rs::parser;

use chrono::{Duration, Utc, TimeZone};

use crate::utils::templates::render;

use std::{
  env,
  str::FromStr
};

use md5::{Md5, Digest};

use fang::AsyncQueueable;

use sanitize_html::sanitize_str;
use sanitize_html::rules::predefined::DEFAULT;

use crate::DeliveryError;

use crate::models::Actor;
use crate::models::User;
use crate::models::Item;
use crate::models::Follower;
use crate::models::SensitiveNote;
use crate::errors::FeedError;

use crate::utils::keys::*;
use crate::utils::path_to_url;
use crate::utils::http::*;

use crate::services::mailer::*;

use crate::traits::property_value::{
  schema_property_context,
  to_profile_value_link,
  PropertyValue
};

use crate::PER_PAGE;

use crate::traits::sensitive::CanBeSensitiveExt;


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

  pub tweaked_profile_data: bool,

  pub listed: bool,
  pub hashtag: Option<String>,
  pub content_warning: Option<String>,
  pub status_publicity: Option<String>,
  
  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>,
  pub refreshed_at: chrono::DateTime::<Utc>,
  pub last_post_at: Option<chrono::DateTime::<Utc>>,

  pub language: Option<String>,

  pub error: Option<String>,
  pub error_count: i32
}

impl PartialEq for Feed {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
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
  Update,
  Reject,
  Add,
  Remove,
  Like,
  Announce
}

pub type AcceptedActivity = ActorAndObject<AcceptedTypes>;

///
/// Extend Service with a public key
///
pub type ExtendedService = Ext1<ApActor<Service>, PublicKey>;

const MAX_FEED_ERROR_COUNT: i32 = 10;

pub fn feed_max_error_count() -> i32 {
  match env::var_os("FEED_ERROR_COUNT") {
    Some(val) => {
      i32::from_str(&val.into_string().expect("Something went wrong setting the feed error count")).unwrap()
    }
    None => MAX_FEED_ERROR_COUNT
  }
}

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
  /// Get a count of how many feeds we have
  ///
  pub async fn count(pool: &PgPool)  -> Result<i32, sqlx::Error> {
    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM feeds")
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() as i32),
      Err(why) => Err(why)
    }
  }
  
  ///
  /// Get a count of how many items we have for this feed
  ///
  pub async fn count_for_user(user: &User, pool: &PgPool)  -> Result<i32, sqlx::Error> {
    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM feeds WHERE user_id = $1", user.id)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() as i32),
      Err(why) => Err(why)
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
          listed = $16,
          error_count = $17,
          tweaked_profile_data = $18,
          language = $19
      WHERE id = $20",
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
      self.error_count,
      self.tweaked_profile_data,
      self.language,
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
  /// If specified, return the language. Otherwise, default to english
  /// 
  pub fn language(&self) -> String {
    match &self.language {
      Some(l) => l.to_string(),
      None => String::from("en")
    }
  }


  ///
  /// Is this an admin feed?
  ///
  pub fn is_admin(&self) -> bool {
    self.admin
  }


  ///
  /// Is this feed throwing an error?
  ///
  pub fn has_error(&self) -> bool {
    self.error_count > 0
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

  pub async fn mark_error(&self, err: &String, pool: &PgPool) -> Result<(), sqlx::Error> {
    let result = sqlx::query!("UPDATE feeds SET error = $1, error_count = error_count + 1 WHERE id = $2", Some(err), self.id)
      .execute(pool)
      .await;

    match result {
      Ok(_result) => Ok(()),
      Err(why) => Err(why)
    }
  }

  pub async fn mark_valid(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    let result = sqlx::query!("UPDATE feeds SET refreshed_at = $1, error_count = 0, error = NULL WHERE id = $2", now, self.id)
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
  pub async fn entries_count(&self, pool: &PgPool)  -> Result<i32, sqlx::Error> {
    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM items WHERE feed_id = $1", self.id)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() as i32),
      Err(why) => Err(why)
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
  pub async fn load(&self) -> Result<String, FeedError> {
    let client = reqwest::Client::new();
    let heads = generate_request_headers();
    let response = client
      .get(&self.url)
      .headers(heads)
      .send()
      .await;

    match response {
      Ok(response) => {
        if response.status().is_success() {
          let body = response
          .text()
          .await;

          if body.is_ok() {
            Ok(body.unwrap())
          } else {
            Err(FeedError { message: body.err().expect("weird").to_string() })
          }
    
        } else {
          Err(FeedError { message: String::from("404") })
        }
      },
      Err(err) => Err(FeedError { message: err.to_string() })
    }
  }


  ///
  /// check parsed feed data for any entries we should convert into new items
  ///
  pub async fn feed_to_entries(&self, data: feed_rs::model::Feed, pool: &PgPool) -> Result<Vec<Item>, sqlx::Error> {
    let mut result: Vec<Item> = Vec::new();
    for entry in data.entries.iter() {
      if entry.published.is_none() || entry.published >= self.last_post_at {
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
    }

    Ok(result)
  }

  pub async fn update_last_post_at(&self, published_at: chrono::DateTime::<Utc>, pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!(
      "UPDATE feeds SET last_post_at = $1 WHERE id = $2 AND (last_post_at IS NULL OR last_post_at < $1)",
      published_at,
      self.id
    ).execute(pool).await?;

    Ok(())
  }


  ///
  /// grab new data for this feed, and deliver any new entries to followers
  ///
  pub async fn refresh(&mut self, pool: &PgPool, tera: &tera::Tera, queue: &mut dyn AsyncQueueable) -> Result<(), DeliveryError> {
    // skip processing for admin accounts
    if self.is_admin() {
      self.mark_fresh(pool).await?;
      return Ok(())
    }
  
    if self.error_count > feed_max_error_count() {
      log::info!("Feed {} {} has too many errors {}, skipping", self.id, self.url, self.error_count);
      Ok(())
    } else {

      let items = self.parse(pool).await;
      match items {
        Ok(items) => {
          if !items.is_empty() {
            log::info!("delivering {} items", items.len());
            for item in items {
              item.deliver(self, pool, tera, queue).await?;
            }  
          }
  
          self.mark_valid(pool).await?;
  
          Ok(())
        },
        Err(why) => {
          // we mark as fresh even though this failed so we don't get stuck on bad feeds
          // @todo mark as erroring
          Err(DeliveryError::FeedError(why))
        }
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
        // only update title/description if user hasn't customized them
        if data.title.is_some() && !self.tweaked_profile_data {
          self.title = Some(sanitize_str(&DEFAULT, &data.title.as_ref().unwrap().content.clone()).unwrap());
        }
        if data.description.is_some() && !self.tweaked_profile_data {
          self.description = Some(sanitize_str(&DEFAULT, &data.description.as_ref().unwrap().content.clone()).unwrap());
        }
        if data.icon.is_some() {
          self.icon_url = Some(data.icon.as_ref().unwrap().uri.clone());
        }
        if data.logo.is_some() {
          self.image_url = Some(data.logo.as_ref().unwrap().uri.clone());
        }
        if data.language.is_some() {
          self.language = Some(sanitize_str(&DEFAULT, data.language.as_ref().unwrap()).unwrap());
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

          self.site_url = if let Some(query) = query {
            Some(query.href)
          } else {
            None
          };
        }

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
    path_to_url(&format!("/feed/{}", self.name))
  }

  ///
  /// Return inbox URL to use in ActivityPub output for this feed
  ///
  pub fn inbox_url(&self) -> String {
    path_to_url(&format!("/feed/{}/inbox", self.name))
  }

  ///
  /// Return outbox URL to use in ActivityPub output for this feed
  ///
  pub fn outbox_url(&self) -> String {
    path_to_url(&format!("/feed/{}/outbox", self.name))
  }

  ///
  /// Return outbox URL to use in ActivityPub output for this feed
  ///
  pub fn outbox_paged_url(&self, page: i32) -> String {
    path_to_url(&format!("/feed/{}/outbox?page={}", self.name, page))
  }

  ///
  /// Return URL to use in HTML output for this feed
  ///
  pub fn permalink_url(&self) -> String {
    path_to_url(&format!("/feed/{}", self.name))

    // path_to_url(&uri!(show_feed(&self.name, None::<i32>)))
  }

  ///
  /// URL for the followers route
  ///
  pub fn followers_url(&self) -> String {
    path_to_url(&format!("/feed/{}/followers", self.name))
//    path_to_url(&uri!(render_feed_followers(&self.name, None::<i32>)))
  }

  ///
  /// URL for the paged followers route
  ///
  pub fn followers_paged_url(&self, page:i32) -> String {
    path_to_url(&format!("/feed/{}/followers?page={}", self.name, page))
//    path_to_url(&uri!(render_feed_followers(&self.name, None::<i32>)))
  }

  ///
  /// return the email-style address for this feed
  ///
  pub fn address(&self) -> String {
    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
    format!("@{}@{}", self.name, instance_domain)
  }

  pub fn display_name(&self) -> &String {
    if self.title.is_some() {
      self.title.as_ref().unwrap()
    } else {
      &self.name
    }
  }


  pub async fn properties(&self, tera: &tera::Tera, pool: &PgPool) -> Result<Vec<AnyBase>, DeliveryError> {
    let mut results: Vec<AnyBase> = Vec::new();
    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
    let user = User::find(self.user_id, pool).await;

    if user.is_err() {
      return Ok(results);
    }

    let user = user.unwrap();
    let full_username = user.full_username();

    if self.site_url.is_some() {
      let guts = self.site_url.clone().unwrap();
      let value = to_profile_value_link(tera, guts.clone(), guts);
      results.push(PropertyValue::new("Homepage", &value).into_any_base().unwrap());
    }

    let actor_url = user.actor_url;
    if full_username.is_some() && actor_url.is_some() {
      let value = to_profile_value_link(tera, actor_url.unwrap(), full_username.unwrap());
      results.push(PropertyValue::new("Generated by", &value).into_any_base().unwrap());
    }


    let value = to_profile_value_link(tera, format!("https://{instance_domain:}/"), instance_domain);
    results.push(PropertyValue::new("Powered by", &value).into_any_base().unwrap());

    Ok(results)
  }
  

  ///
  /// Generate valid ActivityPub data for this feed
  ///
  pub async fn to_activity_pub(&self, tera: &tera::Tera, pool: &PgPool) -> Result<String, DeliveryError> {
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
          owner: iri!(self.ap_url()),
          public_key_pem: self.public_key.to_owned(),
        },
      },
    );

    svc
      .set_context(context())
      .add_context(security())
      .add_context(schema_property_context()?)
      .set_id(iri!(self.ap_url()))
      .set_name(self.display_name().clone())
      .set_preferred_username(self.name.clone())
      .set_inbox(iri!(self.inbox_url()))
      .set_outbox(iri!(self.outbox_url()))
      .set_followers(iri!(self.followers_url()))
      .set_many_attachments(self.properties(tera, pool).await?);
    
    if self.is_admin() {
      svc.set_summary(format!("Admin account for {instance_domain}"));

      let mut icon = Image::new();
      icon.set_url(iri!(format!("https://{instance_domain}/assets/icon.png")));
      svc.set_icon(icon.into_any_base()?);

      let mut image = Image::new();
      image.set_url(iri!(format!("https://{instance_domain}/assets/image.png")));
      svc.set_image(image.into_any_base()?);

    } else {
      if self.description.is_some() {
        svc.set_summary(self.description.clone().unwrap());
      }
  
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
  pub async fn add_follower(&self, pool: &PgPool, actor: &str) -> Result<(), sqlx::Error> {
    let now = Utc::now();

    sqlx::query!("INSERT INTO followers 
        (feed_id, actor, created_at, updated_at) 
        VALUES($1, $2, $3, $4)
        ON CONFLICT (feed_id, actor) DO UPDATE
        SET updated_at = EXCLUDED.updated_at",
                 self.id, actor, now, now)
      .execute(pool)
      .await?;

      Ok(())
  }

  ///
  /// handle an actor following the feed by adding them to the db and sending an Accept message back
  ///
  pub async fn follow(&self, pool: &PgPool, actor: &str, activity: &AcceptedActivity) -> Result<(), DeliveryError> {
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
    accept.set_id(follow_id.clone());
    accept.set_context(context());

    // deliver to the user
    let result = deliver_to_inbox(&Url::parse(&inbox)?, &self.ap_url(), &self.private_key, &accept).await;

    if result.is_err() {
      Actor::log_error(&inbox, pool).await?;
    }

    result

  }

  ///
  /// handle an incoming message. we mostly ignore these except a user can message the admin
  /// feed to login to the site to add/manage feeds
  ///
  pub async fn incoming_message(&self, pool: &PgPool, tera: &tera::Tera, actor_url: &str, activity: &AcceptedActivity) -> Result<(), DeliveryError> {
    // the underlying activitystream library will check that the URL exists/etc
    // via BaseExt::check_authority. we can skip that if we want
    let obj = if env::var("DISABLE_OBJECT_CHECKS").is_ok() {
      activity.object_unchecked()
    } else {
      // pull the Note out of the Activity
      let checked_obj = activity.object();
      if checked_obj.is_err() {
        return Err(DeliveryError::Error("Something went wrong".to_string()));
      }
      checked_obj.unwrap()
    };

    let note: Option<Note> = obj.as_one().unwrap().clone().extend().unwrap();

    if note.is_none() {
      return Ok(())
    }

    let note = note.unwrap();
    let content = note.content();

    if content.is_none() {
      return Ok(())
    }
    let content = content.unwrap();
    let message = content.as_single_xsd_string();

    // ignore messages that aren't to admin feed
    if ! self.is_admin() || message.is_none() {
      return Ok(())
    }

    let clean_message = sanitize_str(&DEFAULT, message.unwrap()).unwrap().to_lowercase();
    let matches: Vec<_> = clean_message.match_indices("help").collect();

    // check for the word 'help' in the beginning of the message
    if matches.is_empty() || matches.first().unwrap().0 > 100 {
      log::debug!("User didn't ask for help in the beginning of the message");
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
        let message = self.generate_login_message(Some(activity), &dest_actor, pool, tera).await?;
        let msg = serde_json::to_string(&message).unwrap();
        log::debug!("{msg}");
    
        let my_url = self.ap_url();

        // send the message!
        let result = deliver_to_inbox(&Url::parse(&dest_actor.inbox_url)?, &my_url, &self.private_key, &message).await;
    
        if result.is_err() {
          Actor::log_error(&dest_actor.inbox_url, pool).await?;
        }
    
        match result {
          Ok(result) => log::debug!("sent! {result:?}"),
          Err(why) => log::debug!("failure! {why:?}")
        }    
      },
      Err(why) => {
        log::debug!("couldnt find actor: {why:?}");
      }
    }

    Ok(())
  }

  ///
  /// generate a login message to send to the user
  ///
  pub async fn generate_login_message(&self, activity: Option<&AcceptedActivity>, dest_actor: &Actor, pool: &PgPool, tera: &tera::Tera) -> Result<ApObject<Create>, DeliveryError> {

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

    let auth_url = path_to_url(&format!("/user/auth/{}", &user.login_token));

    let mut mention = Mention::new();
    mention
      .set_href(iri!(dest_actor.url.to_string()))
      .set_name(dest_actor.full_username());


    let mut template_context = Context::new();
    template_context.insert("link", &auth_url);
    
    let body = render("email/send-login-status.text.tera", tera, &template_context).unwrap();
    let ts = OffsetDateTime::now_utc();

    reply
      .set_sensitive(true)
      .set_attributed_to(iri!(my_url))
      .set_content(body)
      .set_url(iri!(my_url))
      .set_id(iri!(format!("{my_url}/{uniq_hash}")))
      .set_to(iri!(dest_actor.url))
      .set_tag(mention.into_any_base()?)
      .set_published(ts);

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
      .add_context("as:sensitive".to_string())
      .set_id(iri!(format!("{my_url}/{uniq_hash}")))
      .set_to(iri!(dest_actor.url))
      .set_published(ts);


    Ok(action) 
  }

  
  ///
  /// handle unfollow activity
  ///
  pub async fn unfollow(&self, pool: &PgPool, actor: &str) -> Result<(), DeliveryError>  {
    sqlx::query!("DELETE FROM followers WHERE feed_id = $1 AND actor = $2",
                 self.id, actor)
      .execute(pool)
      .await?;
    
    Ok(())
  }

  ///
  /// handle any incoming events
  ///
  pub async fn handle_activity(&self, pool: &PgPool, tera: &tera::Tera, activity: &AcceptedActivity)  -> Result<(), DeliveryError> {
    let s = serde_json::to_string(&activity).unwrap();
    log::debug!("{s:}");
    

    let (actor, _object, act) = activity.clone().into_parts();

    let actor_id = actor.as_single_id().unwrap().to_string();
    
    match act.kind() {
      Some(AcceptedTypes::Follow) => self.follow(pool, &actor_id, activity).await,
      Some(AcceptedTypes::Undo) => self.unfollow(pool, &actor_id).await,
      Some(AcceptedTypes::Delete) => self.unfollow(pool, &actor_id).await,
      Some(AcceptedTypes::Create) => self.incoming_message(pool, tera, &actor_id, activity).await,
      // we don't need to handle this but if we receive it, just move on
      Some(AcceptedTypes::Accept) => Ok(()),
      None => Ok(()),

      // unknown activity type, just ignore quietly
      _ => Ok(())
    }
  }

  ///
  /// generate an AP message to this user with a link to this feed
  ///
  pub async fn link_to_feed_message(&self, tera: &tera::Tera, actor: &Actor) -> Result<ApObject<Create>, DeliveryError> {
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
  
    let mut template_context = Context::new();
    template_context.insert("link", &self.permalink_url());
    template_context.insert("address", &self.address());
    
    let body = render("email/send-creation-status.text.tera", tera, &template_context).unwrap();

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
  pub async fn follower_count(&self, pool: &PgPool)  -> Result<i32, sqlx::Error>{
    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = $1", self.id)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() as i32),
      Err(why) => Err(why)
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
  pub async fn followers(&self, pool: &PgPool)  -> Result<ApObject<OrderedCollection>, DeliveryError> {
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
      .set_id(iri!(&self.ap_url()))
      .set_summary("A list of followers".to_string())
      .set_total_items(count as u64)
      .set_first(iri!(&self.followers_paged_url(1)))
      .set_last(iri!(&self.followers_paged_url(total_pages)));

    Ok(collection)
  }

  ///
  /// generate actual AP page of followes 
  ///
  pub async fn followers_paged(&self, page: i32, pool: &PgPool)  -> Result<ApObject<OrderedCollectionPage>, DeliveryError> {
    let count = self.follower_count(pool).await?;
    let total_pages:i32 = (count / PER_PAGE) + 1;
    let mut collection: ApObject<OrderedCollectionPage> = ApObject::new(OrderedCollectionPage::new());

    collection
      .set_context(context())
      .set_summary("A list of followers".to_string())
      .set_part_of(iri!(&self.ap_url()))
      .set_first(iri!(&self.followers_paged_url(1)))
      .set_last(iri!(&self.followers_paged_url(total_pages)))
      .set_current(iri!(&self.followers_paged_url(page)));
   
    if page > 1 {
      collection.set_prev(iri!(&self.followers_paged_url(page - 1)));
    }

    if page < total_pages {
      collection.set_next(iri!(&self.followers_paged_url(page + 1)));
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
  pub async fn outbox(&self, pool: &PgPool)  -> Result<ApObject<OrderedCollection>, DeliveryError> {
    let count = if self.show_statuses_in_outbox() {
      self.entries_count(pool).await?
    } else {
      0
    };

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
      .set_id(iri!(&self.ap_url()))
      .set_summary("A list of outbox items".to_string())
      .set_total_items(count as u64)
      .set_first(iri!(self.outbox_paged_url(1)))
      .set_last(iri!(self.outbox_paged_url(total_pages)));

    Ok(collection)
  }

  pub fn show_statuses_in_outbox(&self) -> bool {
    self.status_publicity.is_some() && self.status_publicity.as_ref().unwrap() != "direct"
  }

  ///
  /// generate actual AP page of follows 
  ///
  pub async fn outbox_paged(&self, page: i32, pool: &PgPool, tera: &tera::Tera)  -> Result<ApObject<OrderedCollectionPage>, DeliveryError>{
    let count = if self.show_statuses_in_outbox() {
      self.entries_count(pool).await?
    } else {
      0
    };

    let total_pages = (count / PER_PAGE) + 1;
    let mut collection: ApObject<OrderedCollectionPage> = ApObject::new(OrderedCollectionPage::new());

    collection
      .set_context(context())
      .set_summary("A list of outbox items".to_string())
      .set_part_of(iri!(&self.ap_url()))
      .set_first(iri!(self.outbox_paged_url(1)))
      .set_last(iri!(self.outbox_paged_url(total_pages)))
      .set_current(iri!(self.outbox_paged_url(page)));

    if page > 1 {
      collection.set_prev(iri!(self.outbox_paged_url(page - 1)));
    }

    if page < total_pages {
      collection.set_next(iri!(self.outbox_paged_url(page + 1)));
    }

    // return empty collection for invalid pages
    if page == 0 || page > total_pages {
      return Ok(collection)
    }

    if self.show_statuses_in_outbox() {
      let offset = (page - 1) * PER_PAGE;
      let result = sqlx::query_as!(Item, "SELECT * FROM items WHERE feed_id = $1 LIMIT $2 OFFSET $3",
        self.id as i32, PER_PAGE as i32, offset as i32)
        .fetch_all(pool)
        .await;

      match result {
        Ok(result) => {
          for item in result {
            let output = item.to_activity_pub(self, pool, tera).await.unwrap();
            collection.add_item(output.into_any_base()?);    
          }

          Ok(collection)
        },
        Err(why) => Err(why.into())
      }
    } else {
      Ok(collection)
    }
  }
}

#[cfg(test)]
mod test {
  use std::fs;
  use sqlx::postgres::PgPool;
  use feed_rs::parser;
  use chrono::Utc;


  use crate::constants::ACTIVITY_JSON;
  use crate::models::Feed;
  use crate::models::feed::DeliveryError;
  use crate::models::feed::AcceptedActivity;
  use crate::models::Item;
  use crate::models::Enclosure;
  use crate::models::Actor;

  use crate::models::User;
use crate::utils::test_helpers::{fake_user, fake_feed, real_feed, real_user, real_item, real_actor, test_tera};


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
  async fn test_for_item(pool: PgPool) -> Result<(), String> {
    let feed: Feed = real_feed(&pool).await.unwrap();
    let item: Item = real_item(&feed, &pool).await.unwrap();
    let feed2 = Feed::for_item(item.id, &pool).await.unwrap();
    
    assert_eq!(feed, feed2);

    Ok(())
  }

  #[sqlx::test]
  async fn test_find_by_user_and_name(pool: PgPool) -> Result<(), String> {
    let feed: Feed = real_feed(&pool).await.unwrap();
    let user: User = User::find(feed.user_id, &pool).await.unwrap();
    let feed2: Feed = Feed::find_by_user_and_name(&user, &feed.name, &pool).await.unwrap().unwrap();
    
    assert_eq!(feed, feed2);

    Ok(())
  }
  
  #[sqlx::test]
  async fn test_load_by_name(pool: PgPool) -> Result<(), String> {
    let feed: Feed = real_feed(&pool).await.unwrap();
    let feed2: Feed = Feed::load_by_name(&feed.name, &pool).await.unwrap();
    
    assert_eq!(feed, feed2);

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
  async fn test_admin_delete(pool: PgPool) -> sqlx::Result<()> {
    let feed:Feed = real_feed(&pool).await?;

    let deleted_feed = Feed::admin_delete(feed.id, &pool).await?;
    assert_eq!(feed, deleted_feed);
      
    Ok(())
  }

  
  #[sqlx::test]
  async fn test_language() -> sqlx::Result<()> {
    let mut feed:Feed = fake_feed();
    feed.language = None;

    assert_eq!("en", feed.language());

    Ok(())
  }

  #[sqlx::test]
  async fn test_parse_atom_from_data(pool: PgPool) -> sqlx::Result<()> {
    use std::fs;
    let mut feed:Feed = real_feed(&pool).await?;

    let path = "fixtures/test_feed_to_entries.xml";
    let data = fs::read_to_string(path).unwrap();

    let result = feed.parse_from_data(data, &pool).await.unwrap();
    assert_eq!(result.len(), 3);

    let feed2 = Feed::find(feed.id, &pool).await?;
    assert_eq!(feed2.title, Some("muffinlabs.com".to_string()));
    assert_eq!(feed2.language, Some("es".to_string()));

    Ok(())
  }
 
  #[sqlx::test]
  async fn test_parse_rss_from_data(pool: PgPool) -> sqlx::Result<()> {
    use std::fs;
    let mut feed:Feed = real_feed(&pool).await?;

    let path = "fixtures/test_rss.xml";
    let data = fs::read_to_string(path).unwrap();

    let result = feed.parse_from_data(data, &pool).await.unwrap();
    assert_eq!(result.len(), 1);

    let feed2 = Feed::find(feed.id, &pool).await?;
    assert_eq!(feed2.title, Some("Latest Movie Trailers".to_string()));
    assert_eq!(feed2.language, Some("en-us".to_string()));

    Ok(())
  }
 
  #[sqlx::test]
  async fn test_is_admin(pool: PgPool) -> sqlx::Result<()> {
    let mut feed:Feed = real_feed(&pool).await?;

    assert_eq!(feed.is_admin(), false);

    feed.admin = true;
    assert_eq!(feed.is_admin(), true);

    Ok(())
  }
  
  #[sqlx::test]
  async fn test_errors(pool: PgPool) -> sqlx::Result<()> {
    let feed:Feed = real_feed(&pool).await?;

    assert_eq!(feed.has_error(), false);

    let err = "Something went wrong".to_string();
    feed.mark_error(&err, &pool).await?;

    let feed = Feed::find(feed.id, &pool).await?;
    assert_eq!(feed.has_error(), true);

    Ok(())
  }
 
  #[sqlx::test]
  async fn test_mark_valid(pool: PgPool) -> sqlx::Result<()> {
    let feed:Feed = real_feed(&pool).await?;

    let err = "Something went wrong".to_string();
    feed.mark_error(&err, &pool).await?;

    let feed = Feed::find(feed.id, &pool).await?;
    assert_eq!(feed.has_error(), true);

    feed.mark_valid(&pool).await?;

    let feed = Feed::find(feed.id, &pool).await?;
    assert_eq!(feed.has_error(), false);

    assert_eq!(feed.error, None);
    assert_eq!(feed.error_count, 0);

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

  #[sqlx::test]
  async fn test_feed_to_activity_pub(pool: PgPool) -> Result<(), String> {
    use std::env;

    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");

    use serde_json::Value;
    let feed:Feed = fake_feed();
    let tera = test_tera();

    let output = feed.to_activity_pub(&tera, &pool).await.unwrap();

    let v: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(v["name"], "testfeed");
    assert_eq!(v["publicKey"]["id"], format!("https://{}/feed/testfeed#main-key", instance_domain));

    Ok(())
  }

  #[sqlx::test]
  fn test_admin_feed_to_activity_pub(pool: PgPool) -> Result<(), String> {
    use std::env;
    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
    let tera = test_tera();

    use serde_json::Value;
    let tmpfeed:Feed = real_feed(&pool).await.unwrap();
    tmpfeed.mark_admin(&pool).await.unwrap();

    let feed = Feed::find(tmpfeed.id, &pool).await.unwrap();
    let output = feed.to_activity_pub(&tera, &pool).await.unwrap();

    let v: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(v["summary"], format!("Admin account for {}", instance_domain));
    assert_eq!(v["image"]["url"], format!("https://{}/assets/image.png", instance_domain));
    assert_eq!(v["icon"]["url"], format!("https://{}/assets/icon.png", instance_domain));

    Ok(())
  }

  #[sqlx::test]
  async fn test_follow(pool: PgPool) -> Result<(), String> {
    let mut server = mockito::Server::new_async().await;
    let actor = format!("{}/users/colin", &server.url());

    let json = format!(r#"{{"id": "{}/1/2/3", "actor":"{}","object":{{ "id": "{}" }} ,"type":"Follow"}}"#, &server.url(), actor, actor).to_string();
    let act:AcceptedActivity = serde_json::from_str(&json).unwrap();

    let _m = server.mock("GET", "/users/colin")
      .with_status(200)
      .with_header("Accept", "application/ld+json")
      .create_async()
      .await;

    let _m2 = server.mock("POST", "/users/colin/inbox")
      .with_status(202)
      .create_async()
      .await;


    let feed:Feed = real_feed(&pool).await.unwrap();
    let tera = test_tera();

    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = $1 AND actor = $2", feed.id, actor)
      .fetch_one(&pool)
      .await
      .unwrap();

    assert!(result.tally.unwrap() == 0);

    let activity_result = feed.handle_activity(&pool, &tera, &act).await;
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
    let tera = test_tera();

    sqlx::query!("INSERT INTO followers (feed_id, actor, created_at, updated_at) VALUES($1, $2, $3, $4)", feed.id, actor, now, now)
      .execute(&pool)
      .await
      .unwrap();

    let result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = $1 AND actor = $2", feed.id, actor)
      .fetch_one(&pool)
      .await
      .unwrap();

    assert!(result.tally.unwrap() == 1);

    feed.handle_activity(&pool, &tera, &act).await.unwrap();

    let post_result = sqlx::query!("SELECT COUNT(1) AS tally FROM followers WHERE feed_id = $1 AND actor = $2", feed.id, actor)
      .fetch_one(&pool)
      .await
      .unwrap();

    assert!(post_result.tally.unwrap() == 0);
      
    Ok(())
  }

  
  #[sqlx::test]
  async fn test_help(pool: PgPool) -> Result<(), String> {
    let mut server = mockito::Server::new_async().await;
    let actor = format!("{}/users/muffinista", &server.url());

    let admin_feed:Feed = real_feed(&pool).await.unwrap();
    admin_feed.mark_admin(&pool).await.unwrap();

    let admin_url = &admin_feed.ap_url();

    let json = format!(r#"{{
      "actor": "{actor}",
      "object": {{
          "id": "{actor}/statuses/113351263027388676",
          "type": "Note",
          "inReplyToAtomUri": "{actor}/statuses/113351161612620968",
          "sensitive": false,
          "directMessage": true,
          "inReplyTo": "{actor}/statuses/113351161612620968",
          "attachment": [],
          "tag": [
              {{
                  "href": "{admin_url}",
                  "name": "@admin@feedsin.space",
                  "type": "Mention"
              }}
          ],
          "summary": null,
          "contentMap": {{
              "en": "<p><span class=\"h-card\" translate=\"no\"><a href=\"{admin_url}\" class=\"u-url mention\">@<span>admin</span></a></span> help!!!!</p>"
          }},
          "atomUri": "{actor}/statuses/113351263027388676",
          "cc": [],
          "content": "<p><span class=\"h-card\" translate=\"no\"><a href=\"{admin_url}\" class=\"u-url mention\">@<span>admin</span></a></span> help!!!!</p>",
          "to": [
              "{admin_url}"
          ],
          "shares": {{
              "id": "{actor}/statuses/113351263027388676/shares",
              "totalItems": 0,
              "type": "Collection"
          }},
          "replies": {{
              "first": {{
                  "items": [],
                  "next": "{actor}/statuses/113351263027388676/replies?only_other_accounts=true&page=true",
                  "partOf": "{actor}/statuses/113351263027388676/replies",
                  "type": "CollectionPage"
              }},
              "id": "{actor}/statuses/113351263027388676/replies",
              "type": "Collection"
          }},
          "likes": {{
              "id": "{actor}/statuses/113351263027388676/likes",
              "totalItems": 0,
              "type": "Collection"
          }},
          "published": "2024-10-22T13:16:52Z",
          "url": "https://muffin.industries/@colin/113351263027388676",
          "attributedTo": "{actor}",
          "conversation": "tag:muffin.industries,2024-10-22:objectId=672404:objectType=Conversation"
      }},
      "published": "2024-10-22T13:16:52Z",
      "to": [
          "{admin_url}"
      ],
      "cc": [],
      "@context": [
          "https://www.w3.org/ns/activitystreams",
          {{
              "votersCount": "toot:votersCount",
              "ostatus": "http://ostatus.org#",
              "toot": "http://joinmastodon.org/ns#",
              "sensitive": "as:sensitive",
              "atomUri": "ostatus:atomUri",
              "conversation": "ostatus:conversation",
              "inReplyToAtomUri": "ostatus:inReplyToAtomUri",
              "litepub": "http://litepub.social/ns#",
              "directMessage": "litepub:directMessage"
          }}
      ],
      "id": "{actor}/statuses/113351263027388676/activity",
      "type": "Create"
  }}
    "#);

    let act: AcceptedActivity = serde_json::from_str(&json).unwrap();

    let path = "fixtures/helper.json";
    let data = fs::read_to_string(path).unwrap().replace("SERVER_URL", &server.url());

    let _m = server.mock("GET", "/users/muffinista")
      .with_status(200)
      .with_header("Accept", ACTIVITY_JSON)
      .with_body(data)
      .create_async()
      .await;

    let m2 = server.mock("POST", "/users/muffinista/inbox")
      .with_status(202)
      .create_async()
      .await;


    let feed:Feed = Feed::find(admin_feed.id, &pool).await.unwrap();
    let tera = test_tera();

    let activity_result = feed.handle_activity(&pool, &tera, &act).await;
    match activity_result {
      Ok(_result) => {
        m2.assert();
        Ok(())
      },

      Err(why) => Err(why.to_string())
    }
  }


  #[sqlx::test]
  async fn test_generate_login_message(pool: PgPool) -> Result<(), String> {
    let server = mockito::Server::new_async().await;

    let actor = format!("{}/users/colin", &server.url());

    let json = format!(r#"{{"id": "{}/1/2/3", "actor":"{}","object":{{ "id": "{}" }} ,"type":"Follow"}}"#, &server.url(), actor, actor).to_string();
    let act:AcceptedActivity = serde_json::from_str(&json).unwrap();

    let feed:Feed = real_feed(&pool).await.unwrap();
    let dest_actor:Actor = real_actor(&pool).await.unwrap();
    let tera = test_tera();

    let message = feed.generate_login_message(Some(&act), &dest_actor, &pool, &tera).await.unwrap();

    let s = serde_json::to_string(&message).unwrap();

    assert!(s.contains(r#"sensitive":true"#));

    Ok(())
  }

  #[sqlx::test]
  async fn test_generate_login_message_no_activity(pool: PgPool) -> Result<(), String> {
    let feed:Feed = real_feed(&pool).await.unwrap();
    let dest_actor:Actor = real_actor(&pool).await.unwrap();
    let tera = test_tera();

    let message = feed.generate_login_message(None, &dest_actor, &pool, &tera).await.unwrap();

    let s = serde_json::to_string(&message).unwrap();

    assert!(s.contains(r#"sensitive":true"#));

    Ok(())
  }

  #[sqlx::test]
  async fn test_link_to_feed_message(pool: PgPool) -> Result<(), String> {
    let actor = real_actor(&pool).await.unwrap();
    let feed: Feed = real_feed(&pool).await.unwrap();
    let tera = test_tera();

    let message = feed.link_to_feed_message(&tera, &actor).await.unwrap();

    let s = serde_json::to_string(&message).unwrap();

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
        
        assert!(s.contains("OrderedCollectionPage"));
        assert!(s.contains("/colin11"));
        assert!(s.contains("/colin12"));
        assert!(s.contains("/colin13"));
        assert!(s.contains(&format!(r#"first":"{}"#, feed.followers_paged_url(1))));
        assert!(s.contains(&format!(r#"prev":"{}"#, feed.followers_paged_url(1))));
        assert!(s.contains(&format!(r#"next":"{}"#, feed.followers_paged_url(3))));
        assert!(s.contains(&format!(r#"last":"{}"#, feed.followers_paged_url(4))));
        assert!(s.contains(&format!(r#"current":"{}"#, feed.followers_paged_url(2))));

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
  async fn test_outbox(pool: PgPool) -> Result<(), DeliveryError> {
    let feed:Feed = real_feed(&pool).await?;

    for _i in 0..4 {
      real_item(&feed, &pool).await?;
    }
    
    let result = feed.outbox(&pool).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();

        assert!(s.contains("A list of outbox items"));
        assert!(s.contains(r#""totalItems":4"#));
        Ok(())
      },

      Err(why) => Err(why)
    }
  }

  #[sqlx::test]
  async fn test_outbox_direct_status(pool: PgPool) -> Result<(), DeliveryError> {
    let mut feed:Feed = real_feed(&pool).await?;
    feed.status_publicity = Some("direct".to_string());

    for _i in 1..4 {
      real_item(&feed, &pool).await?;
    }
    
    let result = feed.outbox(&pool).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();

        assert!(s.contains("A list of outbox items"));
        assert!(s.contains(r#""totalItems":0"#));
        Ok(())
      },

      Err(why) => {
        assert!(false);
        Err(why)
      }
    }
  }

  #[sqlx::test]
  async fn test_outbox_paged(pool: PgPool) -> Result<(), DeliveryError> {
    let feed:Feed = real_feed(&pool).await?;

    for _i in 1..35 {
      real_item(&feed, &pool).await?;
    }

    let tera = test_tera();

    let result = feed.outbox_paged(2, &pool, &tera).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();

        assert!(s.contains("OrderedCollectionPage"));
        assert!(s.contains("/items/15"));
        assert!(s.contains("/items/16"));
        assert!(s.contains("/items/17"));

        assert!(s.contains(&format!(r#"first":"{}"#, feed.outbox_paged_url(1))));
        assert!(s.contains(&format!(r#"prev":"{}"#, feed.outbox_paged_url(1))));
        assert!(s.contains(&format!(r#"next":"{}"#, feed.outbox_paged_url(3))));
        assert!(s.contains(&format!(r#"last":"{}"#, feed.outbox_paged_url(4))));
        assert!(s.contains(&format!(r#"current":"{}"#, feed.outbox_paged_url(2))));

        Ok(())
      },
      Err(why) => Err(why)
    }
  }
  
  
  #[sqlx::test]
  async fn test_outbox_paged_direct_status(pool: PgPool) -> Result<(), DeliveryError> {
    let mut feed:Feed = real_feed(&pool).await?;
    feed.status_publicity = Some("direct".to_string());

    for _i in 1..35 {
      real_item(&feed, &pool).await?;
    }

    let tera = test_tera();


    let result = feed.outbox_paged(2, &pool, &tera).await;
    match result {
      Ok(result) => {
        let s = serde_json::to_string(&result).unwrap();

        assert!(s.contains("OrderedCollectionPage"));
        assert!(!s.contains("/items/15"));
        assert!(!s.contains("/items/16"));
        assert!(!s.contains("/items/17"));
        assert!(s.contains(&format!(r#"first":"{}"#, feed.outbox_paged_url(1))));
        assert!(s.contains(&format!(r#"prev":"{}"#, feed.outbox_paged_url(1))));
        assert!(s.contains(&format!(r#"last":"{}"#, feed.outbox_paged_url(1))));
        assert!(s.contains(&format!(r#"current":"{}"#, feed.outbox_paged_url(2))));

        Ok(())
      },
      Err(why) => Err(why)
    }
  }
 
  #[sqlx::test]
  async fn test_load(pool: PgPool) -> Result<(), DeliveryError> {
    let mut feed: Feed = real_feed(&pool).await?;
    let mut server = mockito::Server::new_async().await;

    let path = "fixtures/test_feed_to_entries.xml";
    let data = fs::read_to_string(path).unwrap().replace("SERVER_URL", &server.url());

    let url = format!("{}/test.xml", &server.url()).to_string();
    feed.url = url;

    let m = server.mock("GET", "/test.xml")
      .with_status(200)
      .with_body(&data)
      .create_async()
      .await;


    let output = feed.load().await.unwrap();
    m.assert_async().await;
    assert_eq!(&output, &data);    

    Ok(())
  }

  #[sqlx::test]
  async fn test_load_404(pool: PgPool) -> Result<(), DeliveryError> {
    let mut feed: Feed = real_feed(&pool).await?;
    let mut server = mockito::Server::new_async().await;

    let url = format!("{}/test.xml", &server.url()).to_string();
    feed.url = url;

    let m = server.mock("GET", "/test.xml")
      .with_status(404)
      .create_async()
      .await;


    let result = feed.load().await;
    m.assert_async().await;
    assert!(result.is_err());    

    Ok(())
  }

}
