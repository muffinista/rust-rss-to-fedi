#![feature(proc_macro_hygiene, decl_macro)]

use sqlx::postgres::PgPoolOptions;
use std::env;

use fang::NoTls;

use rustypub::utils::queue::create_queue;

use rustypub::models::Feed;
use rustypub::DeliveryError;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
   /// URL of the destination actor
   #[arg(short, long)]
   id: i32,
}


#[tokio::main]
async fn main() -> Result<(), DeliveryError> {
  let db_uri = env::var("DATABASE_URL").expect("DATABASE_URL is not set");

  let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&db_uri)
    .await
    .expect("Failed to create pool");

  sqlx::migrate!("./migrations")
    .run(&pool)
    .await
    .ok();

  let args = Args::parse();

  let id = args.id;

  let feed = Feed::find(id, &pool).await;
  match feed {
    Ok(mut feed) => {
      let mut queue = create_queue().await;
      queue.connect(NoTls).await.unwrap();
  
      let result = feed.refresh(&pool, &mut queue).await;
      match result {
        Ok(_result) => { 
          println!("RefreshFeed: Done refreshing feed {:}", feed.url);
          return Ok(());
        },
        Err(why) => println!("failure! {why:?}")
      }
    },
    Err(why) => println!("failure! {why:?}")
  }

  Ok(())
}
