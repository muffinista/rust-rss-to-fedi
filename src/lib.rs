pub mod utils;
pub mod services;
pub mod traits;
pub mod models;
pub mod routes;
pub mod server;
pub mod tasks;

pub mod activitystreams;

pub mod errors;
pub use errors::DeliveryError;

const PER_PAGE:i32 = 10i32;

mod constants {
  pub const REQUEST_TARGET: &str = "(request-target)";
  pub const ACTIVITY_JSON: &str = "application/activity+json";
}
