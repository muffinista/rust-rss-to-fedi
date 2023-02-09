use sqlx::postgres::PgPool;

use chrono::Utc;

///
/// Simple model to track blocked domains. If a domain is blocked,
/// we won't interact with it
///
pub struct BlockedDomain {
  pub name: String,
  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>
}

impl BlockedDomain {
  ///
  /// Check if the specfied domain is on the block list
  ///
  pub async fn exists(name: &String, pool: &PgPool) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("SELECT count(1) AS tally FROM blocked_domains WHERE name = $1", name)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() > 0),
      Err(why) => Err(why)
    }
  }

  pub async fn create(name: &String, pool: &PgPool) -> Result<(), sqlx::Error> {
    let now = Utc::now();

    sqlx::query!("INSERT INTO blocked_domains (name, created_at, updated_at) VALUES ($1, $2, $3)", name, now, now)
      .execute(pool)
      .await?;

    Ok(())
  }

  pub async fn delete(name: &String, pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM blocked_domains WHERE name = $1", name)
      .execute(pool)
      .await?;

    Ok(())
  }
}


#[cfg(test)]
mod test {
  use sqlx::postgres::PgPool;
  use crate::models::BlockedDomain;


  #[sqlx::test]
  async fn test_blocked_domain_exists(pool: PgPool) -> Result<(), sqlx::Error> {
    let name = "foo.com".to_string();
    
    assert!(!BlockedDomain::exists(&name, &pool).await.unwrap());

    BlockedDomain::create(&name, &pool).await?;
    assert!(BlockedDomain::exists(&name, &pool).await.unwrap());

    BlockedDomain::delete(&name, &pool).await?;
    assert!(!BlockedDomain::exists(&name, &pool).await.unwrap());

    Ok(())
  }
}
