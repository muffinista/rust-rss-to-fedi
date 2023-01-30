use anyhow::Error as AnyError;

use url::Url;

use sqlx::postgres::PgPool;
use serde::{Serialize};
use serde_json::Value;

use chrono::{Utc, prelude::*};

use openssl::{
  hash::MessageDigest,
  pkey::PKey,
  rsa::Rsa,
  sign,
};

#[derive(Debug, Serialize)]
pub struct Actor {
  pub url: String,
  pub public_key_id: String,
  pub public_key: String,

  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>,
  pub refreshed_at: chrono::DateTime::<Utc>,

  pub error: Option<String>
}

impl PartialEq for Actor {
  fn eq(&self, other: &Self) -> bool {
    self.url == other.url
  }
}

impl Actor {
  pub async fn find_or_fetch(url: &String, pool: &PgPool) -> Result<Actor, sqlx::Error> {
    let mut clean_url = Url::parse(url).unwrap();
    clean_url.set_fragment(None);

    let lookup_url = clean_url.as_str().to_string();

    let exists = Actor::exists_by_url(&lookup_url, pool).await?;
    if ! exists {
      // @todo handle failure
      Actor::fetch(&lookup_url, pool).await.unwrap();
    }

    sqlx::query_as!(Actor, "SELECT * FROM actors WHERE url = $1", &lookup_url)
      .fetch_one(pool)
      .await
  }

  pub async fn exists_by_url(url: &String, pool: &PgPool) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("SELECT count(1) AS tally FROM actors WHERE url = $1", url)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() > 0),
      Err(why) => Err(why)
    }
  }

  pub async fn fetch(url: &String, pool: &PgPool) -> Result<(), AnyError> {
    let resp = crate::services::mailer::fetch_object(url).await?;
    let data:Value = serde_json::from_str(&resp).unwrap();

    Actor::create(&data["id"].as_str().unwrap().to_string(),
      &data["publicKey"]["id"].as_str().unwrap().to_string(),
      &data["publicKey"]["publicKeyPem"].as_str().unwrap().to_string(),
      pool
    ).await?;

    Ok(())
  }
  
  pub async fn create(url: &String,
      public_key_id: &String,
      public_key: &String,
      pool: &PgPool) -> Result<(), sqlx::Error> {

    let now = Utc::now();

    sqlx::query!("INSERT INTO actors
        (url, public_key_id, public_key, created_at, updated_at, refreshed_at)
        VALUES($1, $2, $3, $4, $5, $6)",
        url, public_key_id, public_key, now, now, now)
      .execute(pool)
      .await?;

    Ok(())
  }

  pub async fn delete(url: &String, pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM actors WHERE url = $1", url)
      .execute(pool)
      .await?;
    
    Ok(())
  }

  pub async fn mark_stale(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
    let old = Utc.with_ymd_and_hms(1900, 1, 1, 0, 0, 0).unwrap();
    let result = sqlx::query!("UPDATE actors SET refreshed_at = $1 WHERE url = $2", old, self.url)
      .execute(pool)
      .await;

    match result {
      Ok(_result) => Ok(()),
      Err(why) => Err(why)
    }
  }

  pub async fn mark_error(&self, err:&String, pool: &PgPool) -> Result<(), sqlx::Error> {
    let result = sqlx::query!("UPDATE actors SET error = $1 WHERE url = $2", err, self.url)
      .execute(pool)
      .await;

    match result {
      Ok(_result) => Ok(()),
      Err(why) => Err(why)
    }
  }

  pub async fn mark_fresh(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    let result = sqlx::query!("UPDATE actors SET refreshed_at = $1 WHERE url = $2", now, self.url)
      .execute(pool)
      .await;

    match result {
      Ok(_result) => Ok(()),
      Err(why) => Err(why)
    }
  }


  pub fn verify_signature(&self, payload: &str, signature: &[u8]) -> Result<bool, AnyError> {
    println!("{:}", payload);
    println!("{:?}", signature);
    println!("{:}", self.public_key);

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
    let actor = Actor::find_or_fetch(&url, &pool).await.unwrap();
    println!("{:?}", actor);

    assert_eq!(actor.url, url);
    assert_eq!(actor.public_key_id, "https://botsin.space/users/muffinista#main-key");


    // let _m = mock("GET", "/users/muffinista")
    //   .with_status(200)
    //   .with_header("Accept", "application/ld+json")
    //   .with_body("{\"inbox\": \"https://foo.com/users/muffinista/inbox\"}")
    //   .create();

    // let result = follower.find_inbox().await.unwrap();
    // assert!(result == "https://foo.com/users/muffinista/inbox");
    Ok(())
  }

}
