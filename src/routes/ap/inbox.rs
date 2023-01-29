
use rocket::post;
use rocket::http::Status;
use rocket::State;

use sqlx::postgres::PgPool;

use crate::models::feed::Feed;
use crate::models::feed::AcceptedActivity;

use rocket::serde::json::Json;

use rocket::request::{self, FromRequest, Request};
use rocket::outcome::{Outcome};

use chrono::{Duration, NaiveDateTime, Utc};


#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SignatureValidity {
  Invalid,
  ValidNoDigest,
  Valid,
  Absent,
  Outdated,
}

impl SignatureValidity {
  pub fn is_secure(self) -> bool {
    self == SignatureValidity::Valid
  }
}

// // https://github.com/Plume-org/Plume/blob/8c098def6173797b3f36f3668ee8038e1048f6a5/plume-common/src/activity_pub/sign.rs#L137

#[rocket::async_trait]
impl<'r> FromRequest<'r> for SignatureValidity {
  type Error = std::convert::Infallible;
  
  async fn from_request(request: &'r Request<'_>) -> request::Outcome<SignatureValidity, Self::Error> {
    // let pool = request.rocket().state::<PgPool>().unwrap();
    // let cookie = request.cookies().get_private("access_token");
    let sig_header = request.headers().get_one("Signature");
    if sig_header.is_none() {
      return Outcome::Forward(());
      // return Outcome::Failure(Status::BadRequest); // (SignatureValidity::Absent, ()));
    }
    let sig_header = sig_header.expect("sign::verify_http_headers: unreachable");

    let mut _key_id = None;
    let mut _algorithm = None;
    let mut headers = None;
    let mut signature = None;
    for part in sig_header.split(',') {
        match part {
            part if part.starts_with("keyId=") => _key_id = Some(&part[7..part.len() - 1]),
            part if part.starts_with("algorithm=") => _algorithm = Some(&part[11..part.len() - 1]),
            part if part.starts_with("headers=") => headers = Some(&part[9..part.len() - 1]),
            part if part.starts_with("signature=") => signature = Some(&part[11..part.len() - 1]),
            _ => {}
        }
    }

    if signature.is_none() || headers.is_none() {
      //missing part of the header
      // return Outcome::Failure(SignatureValidity::Invalid);
      return Outcome::Forward(());
    }
    let headers = headers
        .expect("sign::verify_http_headers: unreachable")
        .split_whitespace()
        .collect::<Vec<_>>();
    let signature = signature.expect("sign::verify_http_headers: unreachable");
    let h = headers
        .iter()
        .map(|header| (header, request.headers().get_one(header)))
        .map(|(header, value)| format!("{}: {}", header.to_lowercase(), value.unwrap_or("")))
        .collect::<Vec<_>>()
        .join("\n");

    // if !sender
    //     .verify(&h, &base64::decode(signature).unwrap_or_default())
    //     .unwrap_or(false)
    // {
    //     return SignatureValidity::Invalid;
    // }

    // @todo digest check
    // if !headers.contains(&"digest") {
    //   // signature is valid, but body content is not verified
    //   // return SignatureValidity::ValidNoDigest;
    //   return Outcome::Forward(());
    // }

    // let digest = request.headers().get_one("digest").unwrap_or("");
    // let digest = request::Digest::from_header(digest);

    // // @todo get digest of body content
    // if !digest.map(|d| d.verify_header(data)).unwrap_or(false) {
    //   // signature was valid, but body content does not match its digest
    //   // return SignatureValidity::Invalid;
    //   return Outcome::Forward(());
    // }

    // if !headers.contains(&"date") {
    //     return SignatureValidity::Valid; //maybe we shouldn't trust a request without date?
    // }

    let date = request.headers().get_one("date");
    if date.is_none() {
      // return SignatureValidity::Outdated;
      return Outcome::Forward(());
    }

    let date = NaiveDateTime::parse_from_str(date.unwrap(), "%a, %d %h %Y %T GMT");
    if date.is_err() {
      // return SignatureValidity::Outdated;
      return Outcome::Forward(());
    }
    let diff = Utc::now().naive_utc() - date.unwrap();
    let future = Duration::hours(12);
    let past = Duration::hours(-12);
    if diff < future && diff > past {

      // https://blog.joinmastodon.org/amp/2018/07/how-to-make-friends-and-verify-requests/
      // We need to read the Signature header, split it into its parts (keyId, headers and signature), fetch the public key linked from keyId, create a comparison string from the plaintext headers we got in the same order as was given in the signature header, and then verify that string using the public key and the original signature.

      // https://github.com/Plume-org/Plume/blob/8c098def6173797b3f36f3668ee8038e1048f6a5/plume-common/src/activity_pub/sign.rs#L72


      // SignatureValidity::Valid
      return Outcome::Success(SignatureValidity::Valid);

    } else {
        // SignatureValidity::Outdated
      return Outcome::Forward(());
    }
  }
}

/// The inbox stream contains all activities received by the actor. The server
/// SHOULD filter content according to the requester's permission. In general,
/// the owner of an inbox is likely to be able to access all of their inbox
/// contents. Depending on access control, some other content may be public,
/// whereas other content may require authentication for non-owner users, if
/// they can access the inbox at all. 
///
/// https://www.w3.org/TR/activitypub/#inbox
///
#[post("/feed/<username>/inbox", data="<activity>")]
pub async fn user_inbox(digest: SignatureValidity, username: &str, activity: Json<AcceptedActivity>, db: &State<PgPool>) -> Result<(), Status> {
  let feed_lookup = Feed::find_by_name(&username.to_string(), db).await;

  match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          println!("***** {:?}", activity);
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
  
  Ok(())
}

#[cfg(test)]
mod test {
  use crate::server::build_server;
  use rocket::local::asynchronous::Client;
  use rocket::http::Status;
  use rocket::uri;
  use rocket::{Rocket, Build};

  use sqlx::postgres::PgPool;

  use crate::utils::test_helpers::{real_feed};

  #[sqlx::test]
  async fn test_user_inbox(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();

    let actor = "https://activitypub.pizza/users/colin".to_string();
    let json = format!(r#"{{"actor":"{}","object":"{}/feed","type":"Follow"}}"#, actor, actor).to_string();
    
    let server:Rocket<Build> = build_server(pool).await;
    let client = Client::tracked(server).await.unwrap();

    let req = client.post(uri!(super::user_inbox(&feed.name))).body(json);
    let response = req.dispatch().await;

    assert_eq!(response.status(), Status::Ok);

    Ok(())
  }
}
