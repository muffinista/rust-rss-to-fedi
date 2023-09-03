#![feature(proc_macro_hygiene, decl_macro)]

use sqlx::postgres::PgPoolOptions;
use std::env;

use rustypub::models::Actor;

use anyhow::Error as AnyError;


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

  let actors = Actor::stale(&pool).await;
  match actors {
    Ok(actors) => {
      for actor in actors {
        log::info!("update {:}", actor.url);
        let url = actor.url;

        let _result = Actor::fetch(&url, &pool).await;
        let actor = Actor::find_or_fetch(&url, &pool).await;
      
        match actor {
          Ok(actor) => {
            if actor.is_none() {
              println!("Actor not found");
            } else {
              let actor = actor.unwrap();
      
              let inbox = &actor.inbox_url;
              if actor.username.is_none() {
                println!("{url:} -> no username ;(");
              } else {
                let username = &actor.username.unwrap();
        
                println!("{url:} -> {inbox:}");
                println!("{url:} -> {username:}");    
              }
            }
          },
          Err(why) => {
            println!("failed: {why:}");
          }
        }
      };
    },
    Err(why) => {
      println!("failed: {why:}");
    }
  }


  Ok(())
}
