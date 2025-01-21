use std::str::FromStr;

use actix_web::{
    http::header::{HeaderName, HeaderValue}, web, HttpRequest
};
use chrono::{Duration, NaiveDateTime, Utc};

use base64::{Engine as _, engine::general_purpose};
use sqlx::PgPool;

use crate::{models::Actor, DeliveryError};

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

const SIGNATURE_HEADER: &str = "Signature";
const DATE_HEADER: &str = "date";


      // .map(|header| (header, header_data.get(HeaderName::from_static(header))))
      // .map(|(header, value)| format!("{}: {}", header.to_lowercase(), value.unwrap().to_str().expect("Invalid header!")))


fn header_to_payload_snippet(name: &str, header: &HeaderValue) -> String {
  format!("{}: {}", name.to_lowercase(), header.to_str().expect("Invalid header!"))
}

pub async fn validate_request(request: &HttpRequest) -> Result<SignatureValidity, DeliveryError> {

  if !request.headers().contains_key(SIGNATURE_HEADER) {
    // log::info!("no header!");
    return Ok(SignatureValidity::Absent);
  }

  let date = request.headers().get(DATE_HEADER);
  let sig = request.headers().get(SIGNATURE_HEADER).unwrap();

  // let sig_header = request_headers.get("Signature").unwrap().to_str().expect("Invalid header!");
  let pool = request.app_data::<web::Data<PgPool>>().unwrap();

  // let sig_check = from_request( &headers.clone(), &pool, request.uri()).await;
  let mut key_id = None;
  let mut _algorithm = None;
  let mut header_list = None;
  let mut signature = None;
//  let date = date.unwrap();

  let h = sig.to_str().unwrap();
  for part in h.split(',').by_ref() {
    match part {
      part if part.starts_with("keyId=") => key_id = Some(String::from(&part[7..part.len() - 1])),
      part if part.starts_with("algorithm=") => _algorithm = Some(String::from(&part[11..part.len() - 1])),
      part if part.starts_with("headers=") => header_list = Some(String::from(&part[9..part.len() - 1])),
      part if part.starts_with("signature=") => signature = Some(String::from(&part[11..part.len() - 1])),
      _ => {}
    }
  }

  if signature.is_none() || header_list.is_none() {
    // missing part of the header
    // log::info!("missing signature/header!");
    return Ok(SignatureValidity::Invalid)
  } 
  
  let mut header_data = request.headers().clone();
  if !header_data.contains_key("(request-target)") {
    header_data.insert(HeaderName::from_static("(request-target)"),
    HeaderValue::from_str(&format!("post {}", request.uri())).expect("Invalid request-target"));
  }

  let signature = signature.expect("sign::verify_http_headers: unreachable");
  let headers: Vec<String> = header_list
      .expect("sign::verify_http_headers: unreachable")
      .split_whitespace()
      .map(|val| val.into())
      .collect::<Vec<String>>();

  // .expect("sign::verify_http_headers: unreachable")
  let signature_verification_payload = headers
      .iter()
      .map(|header| header_to_payload_snippet(header, header_data.get(HeaderName::from_str(header).expect("Invalid header name!")).expect("Missing header!")))
      .collect::<Vec<_>>()
      .join("\n");

  let key_id = key_id.expect("Missing key_id??");
  let sender = Actor::find_or_fetch(&key_id, pool).await;

  match sender {
    Ok(sender) => {
      if sender.is_none() {
        // log::info!("unable to find sender!");
        Ok(SignatureValidity::InvalidActor(key_id))
      } else {
        let sender = sender.expect("Unable to load sender data!");

        // .verify_signature(&signature_verification_payload, &base64::decode(signature).unwrap_or_default())
        if !sender
          .verify_signature(&signature_verification_payload, &general_purpose::STANDARD.decode(signature).unwrap_or_default())
          .unwrap_or(false)
        {
          // log::info!("unable to verify signature!");
          Ok(SignatureValidity::InvalidSignature(key_id))
        } else {
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
    
          if date.is_none() {
            Ok(SignatureValidity::Outdated(key_id))
          } else {
            let date = NaiveDateTime::parse_from_str(date.unwrap().to_str().expect("Invalid date?"), "%a, %d %h %Y %T GMT");
            if date.is_err() {
              Ok(SignatureValidity::Outdated(key_id))
            } else {
              let diff = Utc::now().naive_utc() - date.unwrap();
              let future = Duration::hours(12);
              let past = Duration::hours(-12);
              if diff < future && diff > past {
                Ok(SignatureValidity::Valid(key_id))
              } else {
                Ok(SignatureValidity::Outdated(key_id))
              }

            }

          }
    
        }
      }
    },
    Err(_why) => {
      // log::info!("fetch failure? {why:?}");
      Ok(SignatureValidity::Invalid)
    }
  }

}
