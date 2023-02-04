
use sqlx::postgres::PgPool;

use chrono::Utc;

use crate::models::item::Item;

#[derive(Debug)]
pub struct Enclosure {
  pub id: i32,
  pub item_id: i32,
  pub url: String,
  pub content_type: Option<String>,
  pub size: Option<i32>,

  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>
}

impl PartialEq for Enclosure {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id || (self.item_id == other.item_id && self.url == other.url)
  }
}

impl Enclosure {
  pub async fn for_item(item: &Item, pool: &PgPool) -> Result<Vec<Enclosure>, sqlx::Error> {
    sqlx::query_as!(Enclosure, "SELECT * FROM enclosures WHERE item_id = $1 ORDER by id", item.id)
    .fetch_all(pool)
    .await
  }
}

#[cfg(test)]
mod test {
  use sqlx::postgres::PgPool;
  use crate::models::feed::Feed;
  use crate::models::item::Item;
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
