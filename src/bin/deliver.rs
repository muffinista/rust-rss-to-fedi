#![feature(proc_macro_hygiene, decl_macro)]

use sqlx::postgres::PgPoolOptions;
use std::env;

use rustypub::models::Actor;
use rustypub::models::Feed;
use rustypub::models::Item;
use rustypub::services::mailer::deliver_to_inbox;

use url::Url;

use clap::Parser;

use anyhow::Error as AnyError;

use activitystreams::iri;
use activitystreams::object::ObjectExt;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
   /// URL of the destination actor
   #[arg(short, long)]
   dest_url: String,

   /// ID of the item to deliver in the database
   #[arg(short, long)]
   item_id: i32,
}


#[tokio::main]
async fn main() -> Result<(), AnyError> {
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

  let dest_url = args.dest_url;
  let item_id = args.item_id;

  let item = Item::find(item_id, &pool).await?;
  let feed = Feed::for_item(item_id, &pool).await?;

  let mut message = item.to_activity_pub(&feed, &pool).await.unwrap();

  let dest_actor = Actor::find_or_fetch(&dest_url, &pool).await;

  match dest_actor {
    Ok(dest_actor) => {
      if dest_actor.is_none() {
        println!("Actor not found");
        return Ok(());
      }

      let dest_actor = dest_actor.unwrap();

      let inbox = dest_actor.inbox_url;
      println!("{dest_url:} -> {inbox:}");

      message.set_many_tos(vec![iri!(dest_actor.url)]);

      let msg = serde_json::to_string(&message).unwrap();
      println!("{msg}");

      let dest_url = &Url::parse(&inbox).unwrap();
      let result = deliver_to_inbox(dest_url, &feed.ap_url(), &feed.private_key, &message).await;
    
      match result {
        Ok(_result) => {
          println!("delivery to {inbox:} succeeded!");
        },
        Err(why) => {
          println!("delivery to {inbox:} failed: {why:}");
        }
      }
    
    },
    Err(why) => {
      println!("failed: {why:}");
    }
  }

  Ok(())
}
