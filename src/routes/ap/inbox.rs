
use actix_web::HttpRequest;
use actix_web::{post, web, Responder, HttpResponse};

// use rocket::post;
// use rocket::http::Status;
// use rocket::State;

use sqlx::postgres::PgPool;

use crate::models::feed_error::AppError;

use crate::models::Feed;
use crate::models::feed::AcceptedActivity;

use crate::utils::signature_check::{SignatureValidity, validate_request};

// use rocket::serde::json::Json;
// use rocket::request::{self, FromRequest, Request};
// use rocket::outcome::Outcome;


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
  activity: web::Json<AcceptedActivity>,
  path: web::Path<String>,
  db: web::Data<PgPool>,
  tmpl: web::Data<tera::Tera>) -> Result<impl Responder, AppError> {
  let tmpl = tmpl.as_ref();
  let db = db.as_ref();
  let username = path.into_inner();

  let signature_validity = validate_request(&request).await;

  // todo confirm behavior here
  if signature_validity.is_err() && env::var("DISABLE_SIGNATURE_CHECKS").is_err() {
    return Err(AppError::NotFound)
  }

  let signature_validity = signature_validity.unwrap();

  // get the actor from headers and check if the signature is valid
  let (_actor, _error) = if env::var("DISABLE_SIGNATURE_CHECKS").is_ok() {
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
//    log::debug!("digest failure {signature_validity:?}");

    // let _log_result = Message::log(&username.to_string(), &msg, actor.cloned(), error, false, db).await;

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
            Err(why) => {
              actix_web::http::StatusCode::NOT_FOUND
            }
          }
        },
        None => return Err(AppError::NotFound)
      }
    },
    Err(_why) => return Err(AppError::NotFound)
  };

  // let _log_result = Message::log(&username.to_string(), &msg, actor.cloned(), error, result == actix_web::http::StatusCode::ACCEPTED, db).await;

  Ok(HttpResponse::build(result)
    .finish())

}

#[cfg(test)]
mod test {
  use std::collections::HashMap;
  use std::time::SystemTime;

  use actix_web::http::header::HeaderValue;
  use actix_web::{test, dev::Service};
  use actix_session::{SessionMiddleware, storage::CookieSessionStore};
  use base64::engine::general_purpose;
  use base64::Engine;
  use httpdate::fmt_http_date;
  use openssl::hash::MessageDigest;
  use openssl::pkey::PKey;
  use openssl::sign::Signer;
  use sqlx::postgres::PgPool;
  use serde_json::Value;

  use actix_web::HttpMessage;
  use sha2::Digest;

  use crate::build_test_server;
  use crate::utils::test_helpers::{actor_json, mock_ap_action, real_feed};


  #[sqlx::test]
  async fn test_user_inbox(pool: PgPool) -> sqlx::Result<()> {
    let feed = real_feed(&pool).await.unwrap();

    // setup a mock server that responds requests for the ID in the object
    let mut object_server = mockito::Server::new_async().await;
    let path = "fixtures/create-note.json";
    let json = std::fs::read_to_string(path).unwrap().replace("SERVER_URL", &object_server.url());

    let actor_id = format!("{}/actor", object_server.url());

    let (private_key, public_key) = crate::utils::keys::generate_key();
    let actor_json = actor_json(&actor_id, &object_server.url(), &public_key);
  
    mock_ap_action(&mut object_server, "/feed/nytus/items/1283", &json).await;
    mock_ap_action(&mut object_server, "/actor", &serde_json::to_string(&actor_json).unwrap()).await;

    // note to future colin -- what you're doing here is testing that the inbox can
    // a) receive a message
    // b) parse the headers, etc
    // c) handle any issues with that gracefully
    // d) fetch the actor's signing key
    // e) ensure that the message is an authentic EMERGENCY ACTION MESSAGE
    // can actor be stubbed?


    let payload: Value = serde_json::from_str(&json).unwrap();
    let server = test::init_service(build_test_server!(pool)).await;
    let mut req = test::TestRequest::post()
      .uri(&feed.inbox_url())
      .set_json(payload).to_request();

    let headers: Vec<String> = req.headers().keys().map(|name| name.to_string()).collect();

    let sig_headers = vec![String::from(crate::constants::REQUEST_TARGET), String::from("host"), String::from("date"), String::from("digest")];
    let path_and_query = req.path();
    let method = req.method();
    let request_target = format!("{} https://test.com{}", method.to_string().to_lowercase(), path_and_query);
    
    let date = fmt_http_date(SystemTime::now());
    let mut values:HashMap<String, String> = HashMap::<String, String>::new();
    values.insert(String::from(crate::constants::REQUEST_TARGET), request_target);
    values.insert(String::from("host"), String::from("muffin.industries"));
    // values.insert(String::from("user-agent"), String::from("foobar industries"));
    values.insert(String::from("date"), date.clone());
    let mut digester = sha2::Sha256::new();
    digester.update(&json);
    // https://users.rust-lang.org/t/sha256-result-to-string/49391
    let digest = format!("{:X}", digester.finalize());
    values.insert(String::from("digest"),  digest.clone());
  
    for k in headers {
      values.insert(
        String::from(k.clone()), 
        String::from(req.headers().get(&k).unwrap().to_str().unwrap())
      );
    }

    let signing_string = sig_headers
        .iter()
        .map(|h| {
          let v = values.get(h).unwrap();
          format!("{}: {}", h, v)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let private_key = PKey::private_key_from_pem(private_key.as_bytes()).unwrap();
    let mut signer = Signer::new(MessageDigest::sha256(), &private_key).unwrap();
    signer.update(signing_string.as_bytes()).unwrap();
    let signature_value = general_purpose::STANDARD.encode(signer.sign_to_vec().unwrap());

    let key_id= format!("{}#main-key", actor_id);
    let final_signature = format!("keyId=\"{}\",headers=\"(request-target) host date digest\",signature=\"{}\"",
        key_id, signature_value);


    req.headers_mut().append(actix_web::http::header::HeaderName::from_lowercase(b"host").unwrap(), HeaderValue::from_str(&"muffin.industries").unwrap());
    req.headers_mut().append(actix_web::http::header::HeaderName::from_lowercase(b"date").unwrap(), HeaderValue::from_str(&date).unwrap());
    req.headers_mut().append(actix_web::http::header::HeaderName::from_lowercase(b"digest").unwrap(), HeaderValue::from_str(&digest).unwrap());
    req.headers_mut().append(actix_web::http::header::HeaderName::from_lowercase(b"signature").unwrap(), HeaderValue::from_str(&final_signature).unwrap());
    
    let res = server.call(req).await.unwrap();
    assert_eq!(res.status(), actix_web::http::StatusCode::ACCEPTED);

    Ok(())
  }
}
