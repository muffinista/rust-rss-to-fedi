use sqlx::postgres::PgPool;

use crate::models::Actor;
use crate::DeliveryError;

use chrono::Utc;

///
/// Model for a follower of a feed
///
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
  pub async fn find(id: i32, pool: &PgPool) -> Result<Option<Follower>, sqlx::Error> {
    sqlx::query_as!(Follower, "SELECT * FROM followers WHERE id = $1", id)
      .fetch_optional(pool)
      .await
  }

  ///
  /// Ping the actor's profile data to get their inbox
  ///
  pub async fn find_inbox(&self, pool: &PgPool) -> Result<Option<String>, DeliveryError> {
    let actor = Actor::find_or_fetch(&self.actor.to_string(), pool).await;
    match actor {
      Ok(actor) => {
        if let Some(actor) = actor {
          Ok(Some(actor.inbox_url))
        } else {
          Ok(None)
        }
      },
      Err(why) => Err(why)
    }
  }
}


#[cfg(test)]
mod test {
  use std::fs;
  use sqlx::postgres::PgPool;

  use crate::constants::ACTIVITY_JSON;
  use crate::models::Feed;
  use crate::models::Follower;
  use crate::utils::test_helpers::{fake_feed, fake_follower};

  #[sqlx::test]
  async fn test_find_inbox(pool: PgPool) -> Result<(), String> {
    let mut server = mockito::Server::new_async().await;
    let feed: Feed = fake_feed();
    let follower: Follower = fake_follower(&feed, &server);

    let path = "fixtures/muffinista.json";
    let data = fs::read_to_string(path).unwrap().replace("SERVER_URL", &server.url());
    

    let m = server.mock("GET", "/users/muffinista")
      .with_status(200)
      .with_header("Accept", ACTIVITY_JSON)
      .with_body(data)
      .create_async()
      .await;

    let result = follower.find_inbox(&pool).await.unwrap();

    m.assert_async().await;

    assert!(result.expect("Failed to find inbox") == "https://botsin.space/users/muffinista/inbox");
    Ok(())
  }
}
