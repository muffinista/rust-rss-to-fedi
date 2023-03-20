#![feature(once_cell)]

pub mod utils;
pub mod services;
pub mod traits;
pub mod models;

pub mod routes;
pub mod server;
pub mod tasks;

pub mod activitystreams;

const PER_PAGE:i32 = 10i32;
