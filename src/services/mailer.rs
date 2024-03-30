
use http_signature_normalization_reqwest::{
  Config,
  DefaultSpawner,
  Sign,
  digest::SignExt
};

use reqwest::Request;
use reqwest_middleware::RequestBuilder;
use reqwest::header::HeaderValue;

use sqlx::postgres::PgPool;

use crate::error::DeliveryError;

use crate::utils::http::*;
use crate::models::Feed;

use openssl::{
  hash::MessageDigest,
  pkey::PKey,
  sign::Signer
};

use url::Url;

use sha2::{Digest, Sha256};
use base64::{Engine as _, engine::general_purpose};

use serde::Serialize;

pub async fn admin_fetch_object(url: &str, pool: &PgPool) -> Result<Option<String>, DeliveryError> {
  let admin_feed = Feed::for_admin(pool).await?;

  if admin_feed.is_some() {
    let admin_feed = admin_feed.as_ref().unwrap();
    crate::services::mailer::fetch_object(url, Some(&admin_feed.ap_url()), Some(&admin_feed.private_key)).await
  } else {
    crate::services::mailer::fetch_object(url, None, None).await
  }
}

///
/// fetch an http object. Sign request with key if provided
///
pub async fn fetch_object(url: &str, key_id: Option<&str>, private_key: Option<&str>) -> Result<Option<String>, DeliveryError> {
  let client = reqwest::Client::new();
  let config: http_signature_normalization_reqwest::Config<DefaultSpawner> = Config::default().mastodon_compat();

  let response = if key_id.is_some() && private_key.is_some() {
    let key_id = key_id.unwrap();
    let private_key = private_key.unwrap();
    let private_key = PKey::private_key_from_pem(private_key.as_bytes())?;
    let mut signer = Signer::new(MessageDigest::sha256(), &private_key)?;

    let request = client
      .get(url)
      .header("Accept", "application/activity+json")
      .header("User-Agent", user_agent())
      .signature(&config, key_id, move |signing_string| {
        signer.update(signing_string.as_bytes())?;
        
        Ok(general_purpose::STANDARD.encode(signer.sign_to_vec()?)) as Result<_, DeliveryError>
      }).await?;
  
    client.execute(request).await
  } else {
    client
      .get(url)
      .header("Accept", "application/activity+json")
      .header("User-Agent", user_agent())
      .send()
      .await
  };

  match response {
    Ok(response) => {
      if !response.status().is_success() {
        return Ok(None)
      }


      let body = response
        .text()
        .await?;
  
      Ok(Some(body))  
    },
    Err(err) => Err(err.into())
  }
}



///
/// deliver a payload to an inbox
///
pub async fn deliver_to_inbox<T: Serialize + ?Sized>(inbox: &Url, key_id: &str, private_key: &str, json: &T) -> Result<(), DeliveryError> {
  let client = http_client()?;
  let mut heads = generate_request_headers();
  let payload = serde_json::to_vec(json).unwrap();
  // let printable_payload = String::from_utf8(payload.clone()).unwrap();

  log::info!("deliver to {inbox:}");
  // log::info!("message {printable_payload:}");

  // ensure we're sending proper content-type
  heads.insert(
    "Content-Type",
    HeaderValue::from_str("application/activity+json").unwrap(),
  );

  let request_builder = client
    .post(inbox.to_string())
    .headers(heads)
    .json(json);
  

  let request = sign_request(
    request_builder,
    format!("{key_id}#main-key"),
    private_key.to_string(),
    payload
  )
    .await?;

  log::info!("{:?}", request);
  println!("{request:?}");

  let response = client.execute(request).await;
  match response {
    Ok(response) => {
      if response.status().is_success() {
        Ok(())
      } else {
        let status = response.status().to_string();
        let text = response.text().await.unwrap();
        Err(DeliveryError::Error(format!("{status:} {text:}")))
      }
    },
    Err(why) => Err(DeliveryError::HttpMiddlewareError(why))
  }
}

pub async fn sign_request(
  request_builder: RequestBuilder,
  key_id: String,
  private_key: String,
  payload: Vec<u8>
) -> Result<Request, DeliveryError> {

  // https://docs.rs/http-signature-normalization-reqwest/0.7.1/http_signature_normalization_reqwest/struct.Config.html#method.mastodon_compat
  let config: http_signature_normalization_reqwest::Config<DefaultSpawner> = Config::default().mastodon_compat();
  let digest = Sha256::new();

  request_builder
    .signature_with_digest(
      config,
      key_id,
      digest,
      payload,
      move |signing_string| {
        let private_key = PKey::private_key_from_pem(private_key.as_bytes())?;
        let mut signer = Signer::new(MessageDigest::sha256(), &private_key)?;
        signer.update(signing_string.as_bytes())?;
        
        Ok(general_purpose::STANDARD.encode(signer.sign_to_vec()?)) as Result<_, DeliveryError>
      },
    )
    .await
}
