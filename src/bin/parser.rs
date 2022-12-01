#![feature(proc_macro_hygiene, decl_macro)]

use sqlx::sqlite::SqlitePool;

use std::env;

// use rustypub::user::User;
use rustypub::feed::Feed;

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

    // let email:String = "foo@bar.com".to_string();
    // let user = User { id: 1, email: email, access_token: Some("".to_string()), login_token: "".to_string() };
    // let feed = Feed::create(&user, &"https://muffinlabs.com/atom.xml".to_string(), &pool).await.unwrap();
    let feed = Feed::find(1, &pool).await.unwrap();
    Feed::parse(&feed, &pool).await;

    Ok(())
}
