use sqlx::postgres::PgPool;

use anyhow::Error as AnyError;

use crate::models::Actor;
use chrono::Utc;

#[derive(Debug)]
pub struct Follower {
  pub id: i32,
  pub feed_id: i32,
  pub actor: String,
  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>
}

impl PartialEq for Follower {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl Follower {
  ///
  /// Ping the actor's profile data to get their inbox
  ///
  pub async fn find_inbox(&self, pool: &PgPool) -> Result<String, AnyError> {
    let actor = Actor::find_or_fetch(&self.actor.to_string(), pool).await?;
    Ok(actor.url)
  }
}


#[cfg(test)]
mod test {
  use sqlx::postgres::PgPool;
  use crate::models::feed::Feed;
  use crate::models::follower::Follower;
  use crate::utils::test_helpers::{fake_feed, fake_follower};

  use mockito::mock;

  #[sqlx::test]
  async fn test_find_inbox(pool: PgPool) -> Result<(), String> {
    let feed: Feed = fake_feed();
    let follower: Follower = fake_follower(&feed);

    let _m = mock("GET", "/users/muffinista")
      .with_status(200)
      .with_header("Accept", "application/ld+json")
      .with_body("{\"inbox\": \"https://foo.com/users/muffinista/inbox\"}")
      .create();

    let result = follower.find_inbox(&pool).await.unwrap();
    assert!(result == "https://foo.com/users/muffinista/inbox");
    Ok(())
  }
}
