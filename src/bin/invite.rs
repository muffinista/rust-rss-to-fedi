#![feature(proc_macro_hygiene, decl_macro)]

use sqlx::postgres::PgPoolOptions;
use std::env;

use rustypub::models::Actor;
use rustypub::models::Feed;
use rustypub::services::mailer::deliver_to_inbox;
use rustypub::DeliveryError;

use url::Url;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
   /// URL of the destination actor
   #[arg(short, long)]
   dest_url: String,
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

  let dest_url = args.dest_url;

  let templates_dir = env::var("TEMPLATES_PATH").unwrap_or(String::from("templates"));

  let tera =
    tera::Tera::new(&format!("{templates_dir:}/**/*")).expect("Parsing error while loading template folder");

  let feed = Feed::for_admin(&pool).await?;

  let dest_actor = Actor::find_or_fetch(&dest_url, &pool).await;

  match dest_actor {
    Ok(dest_actor) => {
      if dest_actor.is_none() {
        println!("Actor not found");
        return Ok(());
      }

      if feed.is_none() {
        println!("Admin feed missing?!?!");
        return Ok(());
      }

      let dest_actor = dest_actor.unwrap();
      let feed = feed.unwrap();

      let inbox = &dest_actor.inbox_url;
      println!("{dest_url:} -> {inbox:}");

      let message = feed.generate_login_message(None, &dest_actor, &pool, &tera).await.unwrap();

      let msg = serde_json::to_string(&message).unwrap();
      println!("{msg}");
    
      let my_url = feed.ap_url();
    
      // send the message!
      let result = deliver_to_inbox(&Url::parse(inbox)?, &my_url, &feed.private_key, &message).await;
    
      match result {
        Ok(result) => println!("sent! {result:?}"),
        Err(why) => println!("failure! {why:?}")
      }    
    },
    Err(why) => {
      println!("failed: {why:}");
    }
  }

  Ok(())
}
