use std::ops::Deref;

use rocket::post;
use rocket::http::Status;
use rocket::State;

use sqlx::postgres::PgPool;

use crate::models::Actor;

use crate::models::Feed;
use crate::models::feed::AcceptedActivity;
use crate::models::Message;

use rocket::serde::json::Json;

use rocket::request::{self, FromRequest, Request};
use rocket::outcome::{Outcome};

use chrono::{Duration, NaiveDateTime, Utc};

use std::env;

use base64::{Engine as _, engine::general_purpose};


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureValidity {
  Absent,
  Invalid,
  InvalidActor(String),
  InvalidSignature(String),
  ValidNoDigest(String),
  Valid(String),
  Outdated(String),
}

impl SignatureValidity {
  pub fn is_secure(self) -> bool {
    matches!(self, SignatureValidity::Valid(_))
  }
}

///
/// Rocket request guard for AP signed requests
///
///
/// This code is lightly modified from Plume:
///
/// https://github.com/Plume-org/Plume/blob/8c098def6173797b3f36f3668ee8038e1048f6a5/plume-common/src/activity_pub/sign.rs#L137
#[rocket::async_trait]
impl<'r> FromRequest<'r> for SignatureValidity {
  type Error = std::convert::Infallible;
  
  async fn from_request(request: &'r Request<'_>) -> request::Outcome<SignatureValidity, Self::Error> {
    let pool = request.rocket().state::<PgPool>().unwrap();
    let sig_header = request.headers().get_one("Signature");
    if sig_header.is_none() {
      log::info!("no header!");
      return Outcome::Success(SignatureValidity::Absent);
    }
    let sig_header = sig_header.unwrap();

    let mut key_id = None;
    let mut _algorithm = None;
    let mut headers = None;
    let mut signature = None;

    for part in sig_header.split(',') {
      match part {
        part if part.starts_with("keyId=") => key_id = Some(&part[7..part.len() - 1]),
        part if part.starts_with("algorithm=") => _algorithm = Some(&part[11..part.len() - 1]),
        part if part.starts_with("headers=") => headers = Some(&part[9..part.len() - 1]),
        part if part.starts_with("signature=") => signature = Some(&part[11..part.len() - 1]),
        _ => {}
      }
    }

    if signature.is_none() || headers.is_none() {
      // missing part of the header
      log::info!("missing signature/header!");
      return Outcome::Success(SignatureValidity::Invalid);
    }


    let mut header_data:rocket::http::HeaderMap<'_> = request.headers().clone();
    if !header_data.contains("(request-target)") {
      header_data.add_raw("(request-target)", format!("post {}", request.uri()));
    }

    let headers = headers
        .expect("sign::verify_http_headers: unreachable")
        .split_whitespace()
        .collect::<Vec<_>>();
    let signature = signature.expect("sign::verify_http_headers: unreachable");

    let signature_verification_payload = headers
        .iter()
        .map(|header| (header, header_data.get_one(header)))
        .map(|(header, value)| format!("{}: {}", header.to_lowercase(), value.unwrap_or("")))
        .collect::<Vec<_>>()
        .join("\n");

    let key_id = key_id.expect("Missing key_id??");
    let sender = Actor::find_or_fetch(key_id, pool).await;
    match sender {
      Ok(sender) => {
        if sender.is_none() {
          log::info!("unable to find sender!");
          return Outcome::Success(SignatureValidity::InvalidActor(String::from(key_id)));
        }

        let sender = sender.expect("Unable to load sender data!");

        // .verify_signature(&signature_verification_payload, &base64::decode(signature).unwrap_or_default())
        if !sender
          .verify_signature(&signature_verification_payload, &general_purpose::STANDARD.decode(signature).unwrap_or_default())
          .unwrap_or(false)
        {
          log::info!("unable to verify signature!");
          return Outcome::Success(SignatureValidity::InvalidSignature(String::from(key_id)));
        }

        // @todo digest check
        // if !headers.contains(&"digest") {
        //   // signature is valid, but body content is not verified
        //   // return SignatureValidity::ValidNoDigest;
        //   return Outcome::Forward(());
        // }

        // let digest = request.headers().get_one("digest").unwrap_or("");
        // let digest = request::Digest::from_header(digest);

        // @todo get/check digest of body content
        // if !digest.map(|d| d.verify_header(data)).unwrap_or(false) {
        //   // signature was valid, but body content does not match its digest
        //   // return SignatureValidity::Invalid;
        //   return Outcome::Forward(());
        // }

        let date = request.headers().get_one("date");
        if date.is_none() {
          return Outcome::Success(SignatureValidity::Outdated(String::from(key_id)));
        }

        let date = NaiveDateTime::parse_from_str(date.unwrap(), "%a, %d %h %Y %T GMT");
        if date.is_err() {
          return Outcome::Success(SignatureValidity::Outdated(String::from(key_id)));
        }
        let diff = Utc::now().naive_utc() - date.unwrap();
        let future = Duration::hours(12);
        let past = Duration::hours(-12);
        if diff < future && diff > past {
          return Outcome::Success(SignatureValidity::Valid(String::from(key_id)));
        } else {
          return Outcome::Success(SignatureValidity::Outdated(String::from(key_id)));
        }
      },
      Err(why) => {
        log::info!("fetch failure? {why:?}");
        Outcome::Success(SignatureValidity::Invalid)
      }        
    }
  }

}

/// The inbox stream contains all activities received by the actor. The server
/// SHOULD filter content according to the requester's permission.
/// 
/// In general,
/// the owner of an inbox is likely to be able to access all of their inbox
/// contents. Depending on access control, some other content may be public,
/// whereas other content may require authentication for non-owner users, if
/// they can access the inbox at all. 
///
/// https://www.w3.org/TR/activitypub/#inbox
///
#[post("/feed/<username>/inbox", data="<activity>")]
pub async fn user_inbox(digest: Option<SignatureValidity>, username: &str, activity: Json<AcceptedActivity>, db: &State<PgPool>) -> Result<(), Status> {
  let msg = serde_json::to_string(activity.deref()).unwrap();
  log::info!("{:}", msg);

  // get the actor from headers and check if the signature is valid
  let (actor, error) = if env::var("DISABLE_SIGNATURE_CHECKS").is_ok() {
    (None, None)
  } else {   
    match digest.as_ref().unwrap() {
      SignatureValidity::Absent => (None, Some(String::from("Absent"))),
      SignatureValidity::Invalid => (None, Some(String::from("Invalid"))),
      SignatureValidity::InvalidActor(value) => (Some(value), Some(String::from("InvalidActor"))),
      SignatureValidity::InvalidSignature(value) => (Some(value), Some(String::from("InvalidSignature"))),
      SignatureValidity::ValidNoDigest(value) => (Some(value), Some(String::from("ValidNoDigest"))),
      SignatureValidity::Valid(value) => (Some(value), None),
      SignatureValidity::Outdated(value) => (Some(value), Some(String::from("Outdated")))
    }
  };


  if env::var("DISABLE_SIGNATURE_CHECKS").is_ok() {
    log::info!("Skipping signature check because DISABLE_SIGNATURE_CHECKS is set");
  } else if digest.is_none() || !digest.clone().unwrap().is_secure() {
    log::info!("digest failure {digest:?}");

    let _log_result = Message::log(&username.to_string(), &msg, actor.cloned(), error, false, db).await;

    return Err(Status::NotFound)
  }


  let feed_lookup = Feed::find_by_name(&username.to_string(), db).await;

  let result = match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          let handle = feed.handle_activity(db, &activity).await;
          match handle {
            Ok(_handle) => Status::Accepted,
            Err(_why) => Status::NotFound
          }
        },
        None => return Err(Status::NotFound)
      }
    },
    Err(_why) => return Err(Status::NotFound)
  };

  let _log_result = Message::log(&username.to_string(), &msg, actor.cloned(), error, result == Status::Accepted, db).await;

  Ok(())
}

#[cfg(test)]
mod test {
  use rocket::local::asynchronous::Client;
  use rocket::http::Status;
  use rocket::uri;
  use rocket::{Rocket, Build};

  use sqlx::postgres::PgPool;
  use serde_json::json;

  use crate::utils::test_helpers::{build_test_server, real_feed};
  
  #[sqlx::test]
  async fn test_user_inbox(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();
    let json = json!({"actor":"https://feedsin.space/feed/nytus","object":{"id":"https://feedsin.space/feed/nytus/items/1283","type":"Note","content":"content goes here","attributedTo":"https://feedsin.space/feed/nytus","published":"2023-03-07T17:54:08Z","url":"https://feedsin.space/feed/nytus","attachment":[{"summary":"Joseph Zucchero in 2008. His restaurant Mr. Beef was used as the filming location for the popular FX television series “The Bear.”","type":"Document","url":"https://feedsin.space/feed/nytus/items/1283/enclosures/576"}],"tag":[{"href":"https://mastodon.social/tags/news","name":"#news","type":"Hashtag"}],"to":"https://feedsin.space/feed/nytus/followers","summary":""},"@context":["https://www.w3.org/ns/activitystreams","as:Hashtag","https://w3id.org/security/v1"],"type":"Create"});

    let server:Rocket<Build> = build_test_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.post(uri!(super::user_inbox(&feed.name))).json(&json);

    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    Ok(())
  }
}
