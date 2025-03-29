use serde::Deserialize;

pub mod admin;
pub mod ap;
pub mod configure;
pub mod feeds;
pub mod items;
pub mod index;
pub mod login;
pub mod webfinger;
pub mod enclosures;
pub mod well_known;
pub mod nodeinfo;


#[derive(Deserialize)]
pub(crate) struct PageQuery {
  pub(crate) page: Option<i32>,
}
