use std::collections::HashMap;

use actix_web::{
    http::header::HeaderMap, web, HttpRequest
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


fn header_to_payload_snippet(name: &str, header: &str) -> String {
  format!("{}: {}", name.to_lowercase(), header)
}

fn headers_to_hash(headers: &HeaderMap) -> HashMap<String, String> {
  let mut values:HashMap<String, String> = HashMap::<String, String>::new();

  for k in headers.keys() {
    values.insert(
      k.to_string(), 
      String::from(headers.get(k).unwrap().to_str().unwrap())
    );
  }

  values
}


pub async fn validate_request(request: &HttpRequest, payload: &str) -> Result<SignatureValidity, DeliveryError> {
  if !request.headers().contains_key(SIGNATURE_HEADER) {
    log::debug!("validate_request: no signature header!");
    return Ok(SignatureValidity::Absent);
  }

  log::debug!("validate_request: payload {payload:}");


  let date = request.headers().get(DATE_HEADER);
  let sig = request.headers().get(SIGNATURE_HEADER).unwrap();

  let pool = request.app_data::<web::Data<PgPool>>().unwrap();

  // let sig_check = from_request( &headers.clone(), &pool, request.uri()).await;
  let mut key_id = None;
  let mut _algorithm = None;
  let mut header_list = None;
  let mut signature = None;

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
    log::debug!("validate_request: missing signature/header!");
    return Ok(SignatureValidity::Invalid)
  } 
  
  // convert to a boring hashmap because the structure used by actix doesn't support
  // non-standard headers like "(request-target)"
  let mut header_data = headers_to_hash(request.headers());
  if !header_data.contains_key(crate::constants::REQUEST_TARGET) {
    header_data.insert(String::from(crate::constants::REQUEST_TARGET),format!("post {}", request.uri()));
  }

  let signature = signature.expect("sign::verify_http_headers: unreachable");
  let headers: Vec<String> = header_list
      .expect("sign::verify_http_headers: unreachable")
      .split_whitespace()
      .map(|val| val.into())
      .collect::<Vec<String>>();

  let signature_verification_payload = headers
      .iter()
      .map(|header| header_to_payload_snippet(header, header_data.get(header).unwrap_or_else(|| panic!("Missing header! {:}", header))))
      .collect::<Vec<_>>()
      .join("\n");
  let key_id = key_id.expect("Missing key_id??");
  let sender = Actor::find_or_fetch(&key_id, pool).await;
  match sender {
    Ok(sender) => {
      if sender.is_none() {
        log::debug!("validate_request: unable to find sender!");        
        Ok(SignatureValidity::InvalidActor(key_id))
      } else {
        let sender = sender.expect("Unable to load sender data!");
        // .verify_signature(&signature_verification_payload, &base64::decode(signature).unwrap_or_default())
        if !sender
          .verify_signature(&signature_verification_payload, &general_purpose::STANDARD.decode(signature).unwrap_or_default())
          .unwrap_or(false)
        {
          log::debug!("validate_request: unable to verify signature!");
          Ok(SignatureValidity::InvalidSignature(key_id))
        } else {    
          if date.is_none() {
            log::debug!("validate_request: no date!");
            Ok(SignatureValidity::Outdated(key_id))
          } else {
            let date = NaiveDateTime::parse_from_str(date.unwrap().to_str().expect("Invalid date?"), "%a, %d %h %Y %T GMT");
            if date.is_err() {
              log::debug!("validate_request: outdated err!");
              Ok(SignatureValidity::Outdated(key_id))
            } else {
              let diff = Utc::now().naive_utc() - date.unwrap();
              let future = Duration::hours(12);
              let past = Duration::hours(-12);
              if diff < future && diff > past {
                Ok(SignatureValidity::Valid(key_id))
              } else {
                log::debug!("validate_request: outdated");
                Ok(SignatureValidity::Outdated(key_id))
              }
            }
          }
        }
      }
    },
    Err(why) => {
      log::debug!("validate_request: fetch failure? {why:?}");
      Ok(SignatureValidity::Invalid)
    }
  }

}
