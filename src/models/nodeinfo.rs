use serde::Serialize;
use sqlx::postgres::PgPool;


#[derive(Debug, Serialize)]
pub struct NodeInfo {
  pub users: i64,
  pub posts: i64,
}

impl NodeInfo {
  pub async fn current(pool: &PgPool) -> Result<NodeInfo, sqlx::Error> {
    let user_query = sqlx::query!("SELECT COUNT(1) AS tally FROM users")
      .fetch_one(pool)
      .await;

    let post_query = sqlx::query!("SELECT COUNT(1) AS tally FROM items")
      .fetch_one(pool)
      .await;
    let user_count = user_query.unwrap().tally.unwrap();
    let post_count = post_query.unwrap().tally.unwrap();
    let output = NodeInfo{
      users: user_count,
      posts: post_count
    };

    Ok(output)
  }
}