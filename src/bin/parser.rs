#![feature(proc_macro_hygiene, decl_macro)]

use sqlx::postgres::PgPoolOptions;

use std::env;
use std::fs;

// use rustypub::models::user::User;
use rustypub::models::feed::Feed;
use rustypub::models::feed::*;


use activitystreams_ext::{Ext1};

use activitystreams::{
  activity::*,
  actor::{ApActor, ApActorExt, Service},
  base::Base,
  iri,
  iri_string::types::IriString,
  prelude::*,
  security,
  context,
  collection::{OrderedCollection, OrderedCollectionPage},
  link::Mention,
  object::ApObject,
  object::*,
  unparsed::*
};


#[tokio::main]
async fn main() -> Result<(), reqwest::Error>  {
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

  let json = fs::read_to_string("unlisted.json").unwrap();
  let act:AcceptedActivity = serde_json::from_str(&json).unwrap();

  let (actor, object, original) = act.clone().into_parts();


  // println!("{:?}", actor);
  // println!("==================================");

  println!("{:?}", object);
  println!("==================================");

  // println!("{:?}", original);
  // println!("==================================");

  // println!("{:?}", act.object().unwrap().as_one().unwrap().unparsed_mut());
  // //as_one().unwrap());

  let x = act.activity_object_ref().object_ref();

  // let x = object.as_one().unwrap();
  println!("{:?}", x);
  // println!("==================================");

  // println!("{:?}", x.clone().take_base().unwrap().into_generic().unwrap()["content"]);

  Ok(())
}
