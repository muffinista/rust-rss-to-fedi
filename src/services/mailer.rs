use http_signature_normalization_reqwest::prelude::*;
use reqwest::Request;
use reqwest_middleware::RequestBuilder;

use crate::utils::http::*;

use openssl::{
  hash::MessageDigest,
  pkey::PKey,
  sign::Signer
};

use url::Url;

use sha2::{Digest, Sha256};
use base64::{Engine as _, engine::general_purpose};

use anyhow::{anyhow};

use serde::Serialize;


///
/// fetch an http object. Sign request with key if provided
///
pub async fn fetch_object(url: &str, key_id: Option<&str>, private_key: Option<&str>) -> Result<Option<String>, anyhow::Error> {
  let client = reqwest::Client::new();
  let config = Config::new().mastodon_compat();

  let response = if key_id.is_some() && private_key.is_some() {
    let key_id = key_id.unwrap();
    let private_key = private_key.unwrap();
  
    let request = client
      .get(url)
      .header("Accept", "application/activity+json")
      .header("User-Agent", user_agent())
      .signature(&config, key_id, move |signing_string| {
        let private_key = PKey::private_key_from_pem(private_key.as_bytes())?;
        let mut signer = Signer::new(MessageDigest::sha256(), &private_key)?;
        signer.update(signing_string.as_bytes())?;
        
        Ok(general_purpose::STANDARD.encode(signer.sign_to_vec()?)) as Result<_, anyhow::Error>
      })?;
  
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
pub async fn deliver_to_inbox<T: Serialize + ?Sized>(inbox: &Url, key_id: &str, private_key: &str, json: &T) -> Result<(), anyhow::Error> {
  let client = http_client();
  let heads = generate_request_headers();
  let payload = serde_json::to_vec(json).unwrap();
  // let printable_payload = String::from_utf8(payload).unwrap();

  log::info!("deliver to {inbox:}");
  // log::info!("message {printable_payload:}");

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

  let response = client.execute(request).await;
  match response {
    Ok(response) => {
      if response.status().is_success() {
        Ok(())
      } else {
        Err(anyhow!(response.status().to_string()))
      }
    },
    Err(why) => Err(why.into())
  }
}

pub async fn sign_request(
  request_builder: RequestBuilder,
  key_id: String,
  private_key: String,
  payload: Vec<u8>
) -> Result<Request, anyhow::Error> {

  // https://docs.rs/http-signature-normalization-reqwest/0.7.1/http_signature_normalization_reqwest/struct.Config.html#method.mastodon_compat
  let config = Config::new().mastodon_compat();
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
        
        Ok(general_purpose::STANDARD.encode(signer.sign_to_vec()?)) as Result<_, anyhow::Error>
      },
    )
    .await
}


// #[cfg(test)]
// mod test {
//   use url::Url;
//   use webfinger::Webfinger;

//   use crate::services::mailer::*;

//   #[tokio::test]
//   async fn test_parse_webfinger() {
//     let json = r#"
//       {
//           "subject": "acct:test@example.org",
//           "aliases": [
//               "https://example.org/@test/"
//           ],
//           "links": [
//               {
//                   "rel": "http://webfinger.net/rel/profile-page",
//                   "href": "https://example.org/@test/"
//               },
//               {
//                   "rel": "http://schemas.google.com/g/2010#updates-from",
//                   "type": "application/atom+xml",
//                   "href": "https://example.org/@test/feed.atom"
//               },
//               {
//                   "rel": "self",
//                   "type": "application/activity+json",
//                   "href": "https://example.org/@test/json"
//               }
//           ]
//       }"#;

//     let wf:Webfinger = serde_json::from_str::<Webfinger>(json).unwrap();

//     let inbox:Url = parse_webfinger(wf).unwrap();
//     assert_eq!("https://example.org/@test/json", inbox.to_string());
//   }
// }
