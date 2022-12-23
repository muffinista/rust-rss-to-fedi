#![feature(proc_macro_hygiene, decl_macro)]

use sqlx::sqlite::SqlitePool;
use chrono::prelude::*;

use std::env;

use rustypub::user::User;
use rustypub::feed::Feed;
use rustypub::Item;

#[tokio::main]
async fn main() -> Result<(), reqwest::Error>  {
  let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
  let pool = SqlitePool::connect(&db_uri)
    .await
    .expect("Failed to create pool");
  sqlx::migrate!("./migrations")
    .run(&pool)
    .await
    .ok();

  let feed = Feed::find(1, &pool).await.unwrap();

  let item = Item {
    id: 1,
    feed_id: feed.id,
    guid: "12345".to_string(),
    title: Some("Hello!".to_string()),
    content: Some("Here is some content".to_string()),
    url: Some("http://google.com".to_string()),
    created_at: Utc::now().naive_utc(),
    updated_at: Utc::now().naive_utc()
  };

//  let _follower = feed.follow(&pool, "muffinista@botsin.space").await;

  let result = item.deliver(&feed, &pool).await;
  match result {
    Ok(result) => {
      println!("{:?}", result);
      Ok(())
    },
    Err(why) => {
      println!("{}", why);
      Err(why.to_string())
    }
  };


  // let email:String = "foo@bar.com".to_string();
  // let user = User { id: 1, email: email, access_token: Some("".to_string()), login_token: "".to_string(), created_at: Utc::now().naive_utc(), updated_at: Utc::now().naive_utc() };
  // let feed = Feed::create(&user,
  //                         &"https://muffinlabs.com/atom.xml".to_string(),
  //                         &"muffinlabs".to_string(),
  //                         &pool).await.unwrap();

  // let feed = Feed::find(1, &pool).await.unwrap();
  // let result = Feed::parse(&feed, &pool).await;

  Ok(())
  // match result {
  //   Ok(_result) => Ok(()),
  //   Err(_why) => todo!()
  // }
}
