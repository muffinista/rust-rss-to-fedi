
use actix_web::HttpRequest;
use actix_web::{post, web, Responder, HttpResponse};

use sqlx::postgres::PgPool;

use crate::errors::AppError;

use crate::models::{Feed, Message};
use crate::models::feed::AcceptedActivity;

use crate::utils::signature_check::{SignatureValidity, validate_request};


use std::env;

const ACTOR_ABSENT: &str = "Absent";
const ACTOR_INVALID: &str = "Invalid";
const ACTOR_INVALID_ACTOR: &str = "Invalid Actor";
const ACTOR_INVALID_SIGNATURE: &str = "Invalid Signature";
const ACTOR_VALID_NO_DIGEST: &str = "Valid, no digest";
const ACTOR_OUTDATED: &str = "Outdated";

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
/// digest: Option<SignatureValidity>, 
#[post("/feed/{username}/inbox")]
pub async fn user_inbox(
  request: HttpRequest,
  body: web::Bytes,
  path: web::Path<String>,
  db: web::Data<PgPool>,
  tmpl: web::Data<tera::Tera>) -> Result<impl Responder, AppError> {
  let tmpl = tmpl.as_ref();
  let db = db.as_ref();
  let username = path.into_inner();


  // we need to get the raw incoming message to do proper digest checks, if
  // we try to extract a payload right into an AcceptedActivity, we've lost
  // the original body!
  let raw = std::str::from_utf8(&body).unwrap();
  let activity: AcceptedActivity = serde_json::from_slice(&body).unwrap();

  let signature_validity = validate_request(&request, raw).await;

  if signature_validity.is_err() && env::var("DISABLE_SIGNATURE_CHECKS").is_err() {
    return Err(AppError::NotFound)
  }

  let signature_validity = signature_validity.unwrap();

  // get the actor from headers and check if the signature is valid
  let (_actor, error) = if env::var("DISABLE_SIGNATURE_CHECKS").is_ok() {
    (None, None)
  } else {   
    match signature_validity {
      SignatureValidity::Absent => (None, Some(ACTOR_ABSENT)),
      SignatureValidity::Invalid => (None, Some(ACTOR_INVALID)),
      SignatureValidity::InvalidActor(ref value) => (Some(value), Some(ACTOR_INVALID_ACTOR)),
      SignatureValidity::InvalidSignature(ref value) => (Some(value), Some(ACTOR_INVALID_SIGNATURE)),
      SignatureValidity::ValidNoDigest(ref value) => (Some(value), Some(ACTOR_VALID_NO_DIGEST)),
      SignatureValidity::Valid(ref value) => (Some(value), None),
      SignatureValidity::Outdated(ref value) => (Some(value), Some(ACTOR_OUTDATED))
    }
  };


  if env::var("DISABLE_SIGNATURE_CHECKS").is_ok() {
    log::info!("Skipping signature check because DISABLE_SIGNATURE_CHECKS is set");
  } else if !signature_validity.is_secure() {
    return Err(AppError::NotFound)
  }


  let feed_lookup = Feed::find_by_name(&username.to_string(), db).await;
  
  let result = match feed_lookup {
    Ok(feed_lookup) => {
      match feed_lookup {
        Some(feed) => {
          let handle = feed.handle_activity(db, tmpl, &activity).await;
          match handle {
            Ok(_handle) => actix_web::http::StatusCode::ACCEPTED,
            Err(_why) => {
              actix_web::http::StatusCode::NOT_FOUND
            }
          }
        },
        None => return Err(AppError::NotFound)
      }
    },
    Err(_why) => return Err(AppError::NotFound)
  };

  let _log_result = Message::log(&username.to_string(), raw, None, error, result == actix_web::http::StatusCode::ACCEPTED, db).await;

  Ok(HttpResponse::build(result)
    .finish())

}

#[cfg(test)]
mod test {
  use actix_web::{test, dev::Service};
  use actix_session::{SessionMiddleware, storage::CookieSessionStore};
  use sqlx::postgres::PgPool;
  use serde_json::Value;

  use crate::{
    build_test_server, 
    assert_accepted,
    utils::test_helpers::{
      sign_test_request,
      deformat_json_string,
      actor_json, 
      mock_ap_action, 
      real_feed
    }
  };

  #[sqlx::test]
  async fn test_user_inbox(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();

    // setup a mock server that responds requests for the ID in the object
    let mut object_server = mockito::Server::new_async().await;
    let path = "fixtures/create-note.json";
    let json = std::fs::read_to_string(path).unwrap().replace("SERVER_URL", &object_server.url());
    let json = deformat_json_string(&json);

    let actor_id = format!("{}/actor", object_server.url());

    let (private_key, public_key) = crate::utils::keys::generate_key();
    let actor_json = actor_json(&actor_id, &object_server.url(), &public_key);
  
    mock_ap_action(&mut object_server, "/feed/nytus/items/1283", &json).await;
    mock_ap_action(&mut object_server, "/actor", &serde_json::to_string(&actor_json).unwrap()).await;

    let payload: Value = serde_json::from_str(&json).unwrap();

    let mut req = test::TestRequest::post()
      .uri(&feed.inbox_url())
      .set_json(payload).to_request();

    sign_test_request(&mut req, &json, &actor_id, &private_key);

    let server = test::init_service(build_test_server!(pool)).await;
    let res = server.call(req).await.unwrap();
    assert_accepted!(res);

    Ok(())
  }
}
