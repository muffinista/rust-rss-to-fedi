
use sqlx::postgres::PgPool;

use chrono::Utc;

use crate::{models::Item, utils::path_to_url};


///
/// Model for enclosures on RSS feeds. We'll attach enclosures to messages
///
pub struct Enclosure {
  pub id: i32,
  pub item_id: i32,
  pub url: String,
  pub content_type: Option<String>,
  pub size: Option<i32>,

  pub description: Option<String>,
  pub credits: Option<String>,

  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>
}

impl PartialEq for Enclosure {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id || (self.item_id == other.item_id && self.url == other.url)
  }
}

impl Enclosure {
  pub async fn find(id: i32, pool: &PgPool) -> Result<Enclosure, sqlx::Error> {
    sqlx::query_as!(Enclosure, "SELECT * FROM enclosures WHERE id = $1", id)
    .fetch_one(pool)
    .await
  }

  ///
  /// Query the db to get all the enclosures for the given item
  ///
  pub async fn for_item(item: &Item, pool: &PgPool) -> Result<Vec<Enclosure>, sqlx::Error> {
    sqlx::query_as!(Enclosure, "SELECT * FROM enclosures WHERE item_id = $1 ORDER by id", item.id)
    .fetch_all(pool)
    .await
  }

  pub async fn find_by_feed_and_item_and_id(username: &str, item_id: i32, id: i32, pool: &PgPool) -> Result<Option<Enclosure>, sqlx::Error> {
    sqlx::query_as!(Enclosure, "SELECT enclosures.* FROM enclosures
      INNER JOIN items ON enclosures.item_id = items.id
      INNER JOIN feeds ON items.feed_id = feeds.id
      WHERE feeds.name = $1 AND items.id = $2 AND enclosures.id = $3", username, item_id, id)
    .fetch_optional(pool)
    .await
  }

  pub fn filename(&self) -> String {
    format!("{:}", self.id)   
  }

  pub fn url(&self, feed_name:&String) -> String {
    path_to_url(&format!("/feed/{}/items/{}/enclosures/{}", feed_name, self.id, self.filename()))
  }
}

#[cfg(test)]
mod test {
  use sqlx::postgres::PgPool;
  use crate::models::Feed;
  use crate::models::Item;
  use crate::models::Enclosure;
  use crate::utils::test_helpers::{real_item, real_feed, real_item_with_enclosure};

  #[sqlx::test]
  async fn test_for_item(pool: PgPool) -> Result<(), String> {
    let feed: Feed = real_feed(&pool).await.unwrap();
    let item: Item = real_item_with_enclosure(&feed, &pool).await.unwrap();

    let result = Enclosure::for_item(&item, &pool).await.unwrap();
    assert_eq!(result.len(), 1);


    let item2: Item = real_item(&feed, &pool).await.unwrap();

    let result2 = Enclosure::for_item(&item2, &pool).await.unwrap();
    assert_eq!(result2.len(), 0);

    Ok(())
  }
}
