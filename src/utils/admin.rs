use std::env;

use sqlx::postgres::PgPool;
use crate::models::user::User;
use crate::models::feed::Feed;

pub async fn create_admin_feed(pool: &PgPool) -> Result<(), sqlx::Error> {
  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  let check = Feed::for_admin(pool).await;

  match check {
    Ok(check) => {
      if check.is_some() {
        Ok(())
      } else {
        let name = "admin";
        let url = "fake";
        let user = User::find_or_create_by_email(
          &format!("{}@{}", name, instance_domain),
          &pool
        ).await;

        match user {
          Ok(user) => {
            let feed = if Feed::exists_by_name(&name.to_string(), &pool).await? {
              Feed::load_by_name(&name.to_string(), &pool).await
            } else {
              Feed::create(&user, &url.to_string(), &name.to_string(), &pool).await
            };

            match feed {
              Ok(mut feed) => {
                feed.admin = true;
                feed.title = Some("Admin account".to_string());
                feed.description = Some("This is the admin account for this instance. Send me a message to get a login URL".to_string());
                let image_url = format!("{}/assets/icon.png", instance_domain).to_string();
                feed.image_url = Some(image_url);
                // feed.mark_admin(&pool).await?;
                Ok(())
              },
              Err(why) => Err(why)
            }
          },
          Err(why) => Err(why)
        }
      }
    },
    Err(why) => Err(why)
  }
}

#[cfg(test)]
mod test {
  use std::env;
  use sqlx::postgres::PgPool;

  use crate::models::User;

  use crate::utils::admin::create_admin_feed;

  #[sqlx::test]
  async fn test_create_admin_feed_from_scratch(pool: PgPool) -> sqlx::Result<()> {
    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
    let email =format!("admin@{}", instance_domain).to_string();

    assert!(User::find_by_email(&email, &pool).await?.is_none());

    let result = create_admin_feed(&pool).await?;
    assert_eq!((), result);
    assert!(User::find_by_email(&email, &pool).await?.is_some());

    Ok(())
  }
}