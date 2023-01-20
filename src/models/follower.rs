use serde::{Serialize};
use serde_json::Value;

use reqwest::header::{HeaderValue, HeaderMap};

use anyhow::Error as AnyError;
use url::Url;


#[derive(Debug, Serialize)]
pub struct Follower {
  pub id: i64,
  pub feed_id: i64,
  pub actor: String,
  pub created_at: chrono::NaiveDateTime,
  pub updated_at: chrono::NaiveDateTime
}

impl PartialEq for Follower {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl Follower {
  ///
  /// Ping the actor's profile data to get their inbox
  /// @todo -- cache this
  ///
  pub async fn find_inbox(&self) -> Result<String, AnyError> {

    let profile_url = Url::parse(&self.actor)?;
      
    let mut headers = HeaderMap::new();

    headers.insert(
      reqwest::header::ACCEPT,
      HeaderValue::from_str("application/ld+json").unwrap(),
    );

    // query that
    let client = reqwest::Client::new();
    let res = client
      .get(profile_url)
      .headers(headers)
      .send()
      .await?;


    let body = res.text().await?;

    let v: Value = serde_json::from_str(&body).unwrap();
    Ok(v["inbox"].as_str().unwrap().to_string())
  }
}


#[cfg(test)]
mod test {
  use crate::models::feed::Feed;
  use crate::models::follower::Follower;
  use crate::utils::test_helpers::{fake_feed, fake_follower};

  use mockito::mock;

  #[sqlx::test]
  async fn test_find_inbox() -> Result<(), String> {
    let feed: Feed = fake_feed();
    let follower: Follower = fake_follower(&feed);

    let _m = mock("GET", "/users/muffinista")
      .with_status(200)
      .with_header("Accept", "application/ld+json")
      .with_body("{\"inbox\": \"https://foo.com/users/muffinista/inbox\"}")
      .create();

    let result = follower.find_inbox().await.unwrap();
    assert!(result == "https://foo.com/users/muffinista/inbox");
    Ok(())
  }
}
