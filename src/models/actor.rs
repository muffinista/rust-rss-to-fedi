use anyhow::{anyhow};
use anyhow::Error as AnyError;

use url::Url;

use sqlx::postgres::PgPool;
use serde_json::Value;

use chrono::Utc;

use openssl::{
  hash::MessageDigest,
  pkey::PKey,
  rsa::Rsa,
  sign,
};

use crate::models::BlockedDomain;
use crate::models::Feed;

///
/// Model for an ActivityPub actor. This could be a remote user who also has
/// a User model. We'll track the inbox Url, public key info, and their username.
///
/// Then, when we need to communicate with someone, we have their inbox and key data
/// cached locally
///
pub struct Actor {
  pub url: String,
  pub inbox_url: String,
  pub public_key_id: String,
  pub public_key: String,

  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>,
  pub refreshed_at: chrono::DateTime::<Utc>,

  pub error: Option<String>,
  pub username: Option<String>
}

impl PartialEq for Actor {
  fn eq(&self, other: &Self) -> bool {
    self.url == other.url
  }
}

impl Actor {
  ///
  /// Query the DB for the actor with the given URL. If not found, fetch the data and cache it
  ///
  pub async fn find_or_fetch(url: &str, pool: &PgPool) -> Result<Option<Actor>, AnyError> {
    let mut clean_url = Url::parse(url).unwrap();
    clean_url.set_fragment(None);

    //
    // check if actor is on blocklist. if so, we won't do anything
    //
    let domain = clean_url.host().unwrap();
    let on_blocklist = BlockedDomain::exists(&domain.to_string(), pool).await?;
    if on_blocklist {
      return Ok(None);
    }

    let lookup_url = clean_url.as_str().to_string();

    let exists = Actor::exists_by_url(&lookup_url, pool).await?;
    if ! exists {
      let result = Actor::fetch(&lookup_url, pool).await;
      match result {
        Ok(_result) => {}
        Err(why) => {
          return Err(why)
        }
      }      
    }

    let result = sqlx::query_as!(Actor, "SELECT * FROM actors WHERE url = $1", &lookup_url)
      .fetch_optional(pool)
      .await;

    match result {
      Ok(result) => Ok(result),
      Err(why) => Err(why.into())
    }
  }

  ///
  /// Check if this Actor exists in the database
  ///
  pub async fn exists_by_url(url: &String, pool: &PgPool) -> Result<bool, sqlx::Error> {
    // look for an actor but exclude old data
    // let age = Utc::now() - Duration::seconds(3600);
    //  AND refreshed_at > $2
    let result = sqlx::query!("SELECT count(1) AS tally FROM actors WHERE url = $1", url)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() > 0),
      Err(why) => Err(why)
    }
  }

  ///
  /// Fetch the remote actor data and store it
  ///
  pub async fn fetch(url: &String, pool: &PgPool) -> Result<(), AnyError> {
    println!("FETCH ACTOR: {url:}");
    let admin_feed = Feed::for_admin(pool).await?;


    let resp = if admin_feed.is_some() {
      let admin_feed = admin_feed.unwrap();
      crate::services::mailer::fetch_object(url, Some(&admin_feed.ap_url()), Some(&admin_feed.private_key)).await
    } else {
      crate::services::mailer::fetch_object(url, None, None).await
    };

    match resp {
      Ok(resp) => {
        if resp.is_none() {
          return Err(anyhow!("User not found"))
        }

        let resp = resp.unwrap();
        let data:Value = serde_json::from_str(&resp).unwrap();
        if data["id"].is_string() && data["publicKey"].is_object() {
          let username = if data["preferredUsername"].is_string() {
            Some(data["preferredUsername"].as_str().unwrap().to_string())
          } else {
            None
          };

          Actor::create(&data["id"].as_str().unwrap().to_string(),
                        &data["inbox"].as_str().unwrap().to_string(),
                        &data["publicKey"]["id"].as_str().unwrap().to_string(),
                        &data["publicKey"]["publicKeyPem"].as_str().unwrap().to_string(),
                        username,
                        pool
          ).await?;
        } else {
          return Err(anyhow!("User not found"))
        }
      },
      Err(why) => {
        println!("fetch failed: {why:?}");
        return Err(why.into());
      }
    }

    Ok(())
  }
  
  ///
  /// Store actor data in the database
  ///
  pub async fn create(url: &String,
      inbox_url: &String,
      public_key_id: &String,
      public_key: &String,
      username: Option<String>,
      pool: &PgPool) -> Result<(), sqlx::Error> {

    let now = Utc::now();

    // create new row, or update existing row
    sqlx::query!("INSERT INTO actors
        (url, inbox_url, public_key_id, public_key, username, created_at, updated_at, refreshed_at)
        VALUES($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (url) DO UPDATE
          SET inbox_url = EXCLUDED.inbox_url,
            public_key_id = EXCLUDED.public_key_id,
            public_key = EXCLUDED.public_key,
            updated_at = EXCLUDED.updated_at,
            refreshed_at = EXCLUDED.updated_at",
        url, inbox_url, public_key_id, public_key, username, now, now, now)
      .execute(pool)
      .await?;

    Ok(())
  }

  ///
  /// Delete the specified actor
  ///
  pub async fn delete(url: &String, pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM actors WHERE url = $1", url)
      .execute(pool)
      .await?;
    
    Ok(())
  }

  // pub async fn mark_stale(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
  //   let old = Utc.with_ymd_and_hms(1900, 1, 1, 0, 0, 0).unwrap();
  //   let result = sqlx::query!("UPDATE actors SET refreshed_at = $1 WHERE url = $2", old, self.url)
  //     .execute(pool)
  //     .await;

  //   match result {
  //     Ok(_result) => Ok(()),
  //     Err(why) => Err(why)
  //   }
  // }

  // pub async fn mark_error(&self, err:&String, pool: &PgPool) -> Result<(), sqlx::Error> {
  //   let result = sqlx::query!("UPDATE actors SET error = $1 WHERE url = $2", err, self.url)
  //     .execute(pool)
  //     .await;

  //   match result {
  //     Ok(_result) => Ok(()),
  //     Err(why) => Err(why)
  //   }
  // }

  // pub async fn mark_fresh(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
  //   let now = Utc::now();
  //   let result = sqlx::query!("UPDATE actors SET refreshed_at = $1 WHERE url = $2", now, self.url)
  //     .execute(pool)
  //     .await;

  //   match result {
  //     Ok(_result) => Ok(()),
  //     Err(why) => Err(why)
  //   }
  // }

  ///
  /// Given a message payload and a signature, confirm that they came from this Actor
  ///
  pub fn verify_signature(&self, payload: &str, signature: &[u8]) -> Result<bool, AnyError> {
    // println!("{:}", payload);
    // println!("{:?}", signature);
    // println!("{:}", self.public_key);

    let key = PKey::from_rsa(Rsa::public_key_from_pem(self.public_key.as_ref()).unwrap()).unwrap();
    let mut verifier = sign::Verifier::new(MessageDigest::sha256(), &key)?;
    verifier.update(payload.as_bytes())?;
    Ok(verifier.verify(signature).unwrap()) //.map_err(SignError::from)
  }
}

#[cfg(test)]
mod test {
  use sqlx::postgres::PgPool;

  use crate::models::actor::Actor;
    
  #[sqlx::test]
  async fn test_find_or_fetch(pool: PgPool) -> Result<(), String> {
    let url = "https://botsin.space/users/muffinista".to_string();
    let actor = Actor::find_or_fetch(&url, &pool).await.unwrap().expect("Failed to load actor");
    // println!("{:?}", actor);

    assert_eq!(actor.url, url);
    assert_eq!(actor.public_key_id, "https://botsin.space/users/muffinista#main-key");

    Ok(())
  }

}
