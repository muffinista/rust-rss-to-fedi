use std::env;

use sqlx::postgres::PgPool;
use crate::models::User;
use crate::models::Feed;

pub async fn create_admin_feed(pool: &PgPool) -> Result<(), sqlx::Error> {
  let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  let check = if env::var("FORCE_ADMIN_SETUP").is_ok() {
    Ok(None)
  } else {
    Feed::for_admin(pool).await
  };

  match check {
    Ok(check) => {
      if check.is_some() {
        Ok(())
      } else {
        println!("Creating admin user/feed");

        let name = "admin";
        let url = "fake";
        let user = User::find_or_create_by_actor_url(
          &format!("https://{instance_domain}/{name}"),
          pool
        ).await;

        match user {
          Ok(user) => {
            let feed = if Feed::exists_by_name(&name.to_string(), pool).await? {
              Feed::load_by_name(&name.to_string(), pool).await
            } else {
              Feed::create(&user, &url.to_string(), &name.to_string(), pool).await
            };

            match feed {
              Ok(mut feed) => {
                let image_url = format!("{instance_domain}/assets/icon.png");

                feed.image_url = Some(image_url);
                feed.user_id = user.id;
                feed.admin = true;
                feed.title = Some("Admin account".to_string());
                feed.description = Some("This is the admin account for this instance. Send me a message to get a login URL".to_string());

                feed.save(pool).await?;
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
  use crate::models::Feed;

  use crate::utils::admin::create_admin_feed;

  #[sqlx::test]
  async fn test_create_admin_feed_from_scratch(pool: PgPool) -> sqlx::Result<()> {
    let instance_domain = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
    let name = "admin";
    let actor_url = &format!("https://{}/{}", instance_domain, name);

    assert!(User::find_by_actor_url(&actor_url, &pool).await?.is_none());
    assert!(User::for_admin(&pool).await?.is_none());
    assert!(Feed::for_admin(&pool).await?.is_none());

    let result = create_admin_feed(&pool).await?;
    assert_eq!((), result);

    assert!(User::find_by_actor_url(&actor_url, &pool).await?.is_some());
    assert!(User::for_admin(&pool).await?.is_some());
    assert!(Feed::for_admin(&pool).await?.is_some());

    Ok(())
  }
}