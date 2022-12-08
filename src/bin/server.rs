#![feature(proc_macro_hygiene, decl_macro)]

//#[macro_use]
//extern crate rocket;

use rustypub::server::boot_server;

#[rocket::main]
pub async fn main() -> Result<(), rocket::Error> {
  let server = boot_server().await;
  match server {
    Ok(_server) => Ok(()),
    Err(why) => panic!("{}", why)
  }
}
