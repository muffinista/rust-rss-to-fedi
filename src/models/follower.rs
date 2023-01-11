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
  use crate::utils::keys::*;
  use chrono::Utc;
  use mockito::mock;

  fn fake_feed() -> Feed {
    let (private_key_str, public_key_str) = generate_key();

    Feed {
      id: 1,
      user_id: 1,
      name: "muffinfeed".to_string(),
      url: "https://foo.com/rss.xml".to_string(),
      private_key: private_key_str.to_string(),
      public_key: public_key_str.to_string(),
      image_url: Some("https://foo.com/image.png".to_string()),
      icon_url: Some("https://foo.com/image.ico".to_string()),
      description: None,
      site_url: None,
      title: None, created_at: Utc::now().naive_utc(), updated_at: Utc::now().naive_utc()
    }
  }

  fn fake_follower(feed: &Feed) -> Follower {
    Follower {
      id: 1,
      feed_id: feed.id,
      actor: format!("{}/users/muffinista", &mockito::server_url()),
      created_at: Utc::now().naive_utc(),
      updated_at: Utc::now().naive_utc()
    }
  }


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
