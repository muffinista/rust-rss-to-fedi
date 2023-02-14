use sqlx::postgres::PgPool;
use chrono::Utc;

#[derive(Debug)]
pub struct Setting {
  pub name: String,
  pub value: String,
  pub created_at: chrono::DateTime::<Utc>,
  pub updated_at: chrono::DateTime::<Utc>
}

impl Setting {
  pub async fn find(name: &String, pool: &PgPool) -> Result<Option<Setting>, sqlx::Error> {
    sqlx::query_as!(Setting, "SELECT * FROM settings WHERE name = $1", name)
      .fetch_optional(pool)
      .await
  }

  pub async fn exists(name: &String, pool: &PgPool) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("SELECT count(1) AS tally FROM settings WHERE name = $1", name)
      .fetch_one(pool)
      .await;

    match result {
      Ok(result) => Ok(result.tally.unwrap() > 0),
      Err(why) => Err(why)
    }
  }

  pub async fn value_or(name: &String, default: &String, pool: &PgPool) -> Result<String, sqlx::Error> {
    let result = Setting::find(&name, pool).await;

    match result {
      Ok(result) => {
        if result.is_some() {
          Ok(result.unwrap().value)
        } else {
          Ok(default.to_owned())
        }
      }
      Err(why) => Err(why)
    }
  }

  pub async fn update(name: &String, value: &String, pool: &PgPool) -> Result<(), sqlx::Error> {
    let now = Utc::now();

    sqlx::query!("INSERT INTO settings (name, value, created_at, updated_at)
      VALUES ($1, $2, $3, $4)
      ON CONFLICT (name) DO UPDATE
        SET value = EXCLUDED.value,
        updated_at = EXCLUDED.updated_at",
      name,
      value,
      now,
      now
    ).execute(pool)
      .await?;

    Ok(())
  }

}


#[cfg(test)]
mod test {
  use sqlx::postgres::PgPool;
  use crate::models::Setting;

  #[sqlx::test]
  async fn test_value_or(pool: PgPool) -> Result<(), String> {
    let name = "signups".to_string();
    let result = Setting::value_or(&name, &"true".to_string(), &pool).await.unwrap();
    assert_eq!(result, "true");


    let name2 = "foobar".to_string();
    let result = Setting::value_or(&name2, &"bar".to_string(), &pool).await.unwrap();
    assert_eq!(result, "bar");
    Ok(())
  }
}
