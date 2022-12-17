use serde::{Serialize};

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
