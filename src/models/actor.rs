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

use crate::DeliveryError;
use crate::models::BlockedDomain;

///
/// Model for an ActivityPub actor. This could be a remote user who also has
/// a User model. We'll track the inbox Url, public key info, and their username.
///
/// Then, when we need to communicate with someone, we have their inbox and key data
/// cached locally
///

#[derive(Debug)]
pub struct Actor {
  pub url: String,
  pub inbox_url: String,
  pub public_key_id: String,
  pub public_key: String,

  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>,
  pub refreshed_at: chrono::DateTime::<Utc>,

  pub error: Option<String>,
  pub username: String,

  pub error_count:i32
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
  pub async fn find_or_fetch(url: &str, pool: &PgPool) -> Result<Option<Actor>, DeliveryError> {

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

    //
    // look for actor in db
    //
    let result = Actor::find(url, pool).await?;  
    if result.is_some() {
      return Ok(result);
    }

    //
    // fetch remote data
    //
    let fetch_result = Actor::fetch(&lookup_url, pool).await;
    match fetch_result {
      Ok(_fetch_result) => {
        // re-check db
        let result = Actor::find(url, pool).await?;
        if result.is_some() {
          return Ok(result);
        }
        Ok(None)
      }
      Err(why) => {
        Err(why)
      }
    }      
  }

  ///
  /// query the db for this actor
  ///
  pub async fn find(url: &str, pool: &PgPool) -> Result<Option<Actor>, sqlx::Error> {
    sqlx::query_as!(Actor, "SELECT * FROM actors WHERE url = $1 OR inbox_url = $2 OR public_key_id = $3", &url, &url, &url)
      .fetch_optional(pool)
      .await
  }

  ///
  /// Check if this Actor exists in the database
  ///
  pub async fn exists_by_url(url: &String, pool: &PgPool) -> Result<bool, sqlx::Error> {
    // look for an actor but exclude old data
    // let age = Utc::now() - Duration::seconds(3600);
    //  AND refreshed_at > $2
    let result = sqlx::query!("SELECT count(1) AS tally FROM actors WHERE url = $1 OR inbox_url = $2 OR public_key_id = $3", url, url, url)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() > 0),
      Err(why) => Err(why)
    }
  }

  pub async fn log_error(url: &String, pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!("UPDATE actors SET error_count = error_count + 1 WHERE url = $1 OR inbox_url = $1", url)
      .execute(pool)
      .await?;
    
    Ok(())
  }


  ///
  /// Fetch the remote actor data and store it
  ///
  pub async fn fetch(url: &String, pool: &PgPool) -> Result<(), DeliveryError> {
    log::debug!("FETCH ACTOR: {url:}");
    let resp = crate::services::mailer::admin_fetch_object(url, pool).await;

    match resp {
      Ok(resp) => {
        if resp.is_none() {
          return Err(DeliveryError::Error(String::from("User not found")))
        }

        let resp = resp.unwrap();
        log::debug!("ACTOR: {url:} -> {resp:}");

        let data:Value = serde_json::from_str(&resp).unwrap();

        if data["id"].is_string() && data["publicKey"].is_object() {
          let username: String = if data["preferredUsername"].is_string() {
            data["preferredUsername"].as_str().unwrap().to_string()
          } else {
            return Err(DeliveryError::Error(String::from("User has no preferredUsername")))
          };

          let inbox = if data["inbox"].is_string() {
            // log::info!("data has inbox key");
            data["inbox"].as_str().unwrap().to_string()
          } else if data["actor"].is_string() {
            // log::info!("data has actor key");
            data["actor"].as_str().unwrap().to_string()
          } else if data["publicKey"]["owner"].is_string() {
            // log::info!("data has owner key");

            // https://docs.gotosocial.org/en/latest/federation/federating_with_gotosocial/
            let owner_url = data["publicKey"]["owner"].as_str().unwrap();

            log::debug!("FETCH ACTOR OWNER: {owner_url:}");

            // Remote servers federating with GoToSocial should extract the
            // public key from the publicKey field. Then, they should use the
            // owner field of the public key to further dereference the full
            // version of the Actor, using a signed GET request.
            let resp = crate::services::mailer::admin_fetch_object(owner_url, pool).await;

            match resp {
              Ok(resp) => {
                if resp.is_none() {
                  return Err(DeliveryError::Error(String::from("User not found")))
                }
        
                let resp = resp.unwrap();
                log::debug!("ACTOR: {url:} -> {resp:}");
        
                let data:Value = serde_json::from_str(&resp).unwrap();       
                data["inbox"].as_str().unwrap().to_string()
              },
              Err(why) => {
                log::info!("fetch failed: {why:?}");
                return Err(why);        
              }
            }
          } else {
            log::debug!("data has neither????");

            return Err(DeliveryError::Error(String::from("User not found")))
          };

          log::debug!("actor create: {inbox:}");
          Actor::create(&data["id"].as_str().unwrap().to_string(),
                        &inbox,
                        &data["publicKey"]["id"].as_str().unwrap().to_string(),
                        &data["publicKey"]["publicKeyPem"].as_str().unwrap().to_string(),
                        &username,
                        pool
          ).await?;
        } else {
          return Err(DeliveryError::Error(String::from("User not found")))
        }
      },
      Err(why) => {
        log::info!("fetch failed: {why:?}");
        return Err(why);
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
      username: &String,
      pool: &PgPool) -> Result<(), sqlx::Error> {

    let now = Utc::now();

    // create new row, or update existing row
    sqlx::query!("INSERT INTO actors
        (url, inbox_url, public_key_id, public_key, username, refreshed_at, created_at, updated_at)
        VALUES($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (url) DO UPDATE
          SET inbox_url = EXCLUDED.inbox_url,
            public_key_id = EXCLUDED.public_key_id,
            public_key = EXCLUDED.public_key,
            username = EXCLUDED.username,
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


  ///
  /// generate a full username address for the actor, ie @username@domain
  ///
  pub fn full_username(&self) -> String {
    let url = Url::parse(&self.url).unwrap();
    let domain = url.host().unwrap();

    format!("@{}@{}", &self.username, domain)
  }


  ///
  /// Given a message payload and a signature, confirm that they came from this Actor
  ///
  pub fn verify_signature(&self, payload: &str, signature: &[u8]) -> Result<bool, DeliveryError> {
    let key = PKey::from_rsa(Rsa::public_key_from_pem(self.public_key.as_ref()).unwrap()).unwrap();
    let mut verifier = sign::Verifier::new(MessageDigest::sha256(), &key)?;
    verifier.update(payload.as_bytes())?;
    Ok(verifier.verify(signature).unwrap()) //.map_err(SignError::from)
  }
}

#[cfg(test)]
mod test {
  use sqlx::postgres::PgPool;
  use std::fs;

  use crate::constants::ACTIVITY_JSON;
  use crate::models::actor::Actor;
  use crate::utils::test_helpers::real_actor;

  #[sqlx::test]
  async fn test_find_or_fetch(pool: PgPool) -> Result<(), String> {
    let mut server = mockito::Server::new_async().await;
    let path = "fixtures/muffinista.json";
    let data = fs::read_to_string(path).unwrap().replace("SERVER_URL", &server.url());

    let m = server.mock("GET", "/users/muffinista")
      .with_status(200)
      .with_header("Accept", ACTIVITY_JSON)
      .with_body(data)
      .create_async()
      .await;

    let url = format!("{}/users/muffinista", &server.url()).to_string();

    let actor = Actor::find_or_fetch(&url, &pool).await.unwrap().expect("Failed to load actor");

    m.assert_async().await;

    assert_eq!(actor.url, url);
    assert_eq!(actor.public_key_id, "https://botsin.space/users/muffinista#main-key");

    Ok(())
  }

  #[sqlx::test]
  async fn test_find(pool: PgPool) -> Result<(), String> {
    let _actor:Actor = real_actor(&pool).await.unwrap();

    assert!(Actor::find("https://foo.com/users/user", &pool).await.unwrap().is_some());
    assert!(Actor::find("https://foo.com/users/user/inbox", &pool).await.unwrap().is_some());
    assert!(Actor::find("public_key_id", &pool).await.unwrap().is_some());
    assert!(Actor::find("random_string", &pool).await.unwrap().is_none());

    Ok(())
  }

  #[sqlx::test]
  async fn test_fetch(pool: PgPool) -> Result<(), sqlx::Error> {
    let mut server = mockito::Server::new_async().await;
    let path = "fixtures/muffinista.json";
    let data = fs::read_to_string(path).unwrap().replace("SERVER_URL", &server.url());

    let m = server.mock("GET", "/users/muffinista")
      .with_status(200)
      .with_header("Accept", ACTIVITY_JSON)
      .with_body(data)
      .create_async()
      .await;

    let url = format!("{}/users/muffinista", &server.url()).to_string();

    let exists = Actor::exists_by_url(&url, &pool).await?;
    assert!(!exists);

    let _actor = Actor::fetch(&url, &pool).await;

    m.assert_async().await;

    let exists = Actor::exists_by_url(&url, &pool).await?;
    assert!(exists);

    Ok(())
  }

  #[sqlx::test]
  async fn test_fetch_no_inbox(pool: PgPool) -> Result<(), sqlx::Error> {
    let mut server = mockito::Server::new_async().await;
    let path = "fixtures/muffinista-key.json";
    let data = fs::read_to_string(path).unwrap().replace("SERVER_URL", &server.url());

    let full_path = "fixtures/muffinista.json";
    let full_data = fs::read_to_string(full_path).unwrap().replace("SERVER_URL", &server.url());

    let m = server.mock("GET", "/users/muffinista/main-key")
      .with_status(200)
      .with_header("Accept", ACTIVITY_JSON)
      .with_body(data)
      .create_async()
      .await;

    let m2 = server.mock("GET", "/users/muffinista")
      .with_status(200)
      .with_header("Accept", ACTIVITY_JSON)
      .with_body(full_data)
      .create_async()
      .await;

    let url = format!("{}/users/muffinista/main-key", &server.url()).to_string();

    let exists = Actor::exists_by_url(&url, &pool).await?;
    assert!(!exists);

    let _actor = Actor::fetch(&url, &pool).await;

    m.assert_async().await;
    m2.assert_async().await;

    let url = format!("{}/users/muffinista", &server.url()).to_string();

    let exists = Actor::exists_by_url(&url, &pool).await?;
    assert!(exists);

    Ok(())
  }

  #[sqlx::test]
  async fn test_full_username(pool: PgPool) -> Result<(), String> {
    let actor:Actor = real_actor(&pool).await.unwrap();

    assert_eq!(actor.full_username(), "@username@foo.com");

    Ok(())
  }
}
