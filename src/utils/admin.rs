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
            let feed = Feed::create(&user, &url.to_string(), &name.to_string(), &pool).await;

            match feed {
              Ok(feed) => {
                sqlx::query!("UPDATE feeds SET admin = true WHERE id = $1", feed.id)
                .execute(pool)
                .await;

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