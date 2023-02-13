use http_signature_normalization_reqwest::prelude::*;
use reqwest::Request;
use reqwest_middleware::RequestBuilder;
use reqwest::header::{HeaderValue, HeaderMap};

use crate::utils::http::http_client;

use openssl::{
  hash::MessageDigest,
  pkey::PKey,
  sign::Signer
};

use url::Url;
use httpdate::fmt_http_date;
use std::time::SystemTime;

use sha2::{Digest, Sha256};
use base64::{Engine as _, engine::general_purpose};

use anyhow::{anyhow};

use std::env;

static BASE_USER_AGENT: &str = concat!(
  env!("CARGO_PKG_NAME"),
  "/",
  env!("CARGO_PKG_VERSION"),
);


// ///
// /// query webfinger endpoint for actor and try and find data url
// ///
// pub async fn find_actor_url(actor: &str) -> Result<Option<Url>, WebfingerError> {
//   // println!("query webfinger for {}", actor);
//   let webfinger = resolve(format!("acct:{}", actor), true).await;

//   match webfinger {
//     Ok(webfinger) => Ok(parse_webfinger(webfinger)),
//     Err(why) => Err(why)
//   }
// }

pub async fn fetch_object(url: &str) -> Result<Option<String>, reqwest::Error> {
  let client = reqwest::Client::new();
  let response = client
    .get(url)
    .header("Accept", "application/activity+json")
    .header("User-Agent", user_agent())
    .send()
    .await;

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
    Err(err) => Err(err)
  }
}

// ///
// /// parse webfinger data for activity URL
// ///
// pub fn parse_webfinger(webfinger: Webfinger) -> Option<Url> {
//   let rel = "self".to_string();
//   let mime_type = Some("application/activity+json".to_string());

//   let query:Option<webfinger::Link> = webfinger
//     .links
//     .into_iter()
//     .find(|link| &link.rel == &rel && &link.mime_type == &mime_type);

//   match query {
//     Some(query) => Some(Url::parse(&query.href.unwrap()).unwrap()),
//     None => None
//   }
// }

///
/// deliver a payload to an inbox
///
pub async fn deliver_to_inbox(inbox: &Url, key_id: &str, private_key: &str, json: &str) -> Result<(), anyhow::Error> {
  let client = http_client();
  let heads = generate_request_headers(&inbox);

  println!("deliver to {:}", inbox);
  println!("message {:}", json);

  let request_builder = client
    .post(inbox.to_string())
    .headers(heads)
    .body(json.to_string());
  
  let request = sign_request(
    request_builder,
    format!("{}#main-key", key_id.to_string()),
    private_key.to_string(),
    json.to_string()
  )
    .await?;

  let response = client.execute(request).await;
  match response {
    // @todo check response code/etc
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

///
/// Generate a user agent for the current version of the code and the running instance
///
pub fn user_agent() -> String {
  let domain_name = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  format!("{}; +{})", BASE_USER_AGENT, domain_name)
}

fn generate_request_headers(_inbox: &Url) -> HeaderMap {
  let mut headers = HeaderMap::new();
  headers.insert(
    "user-agent",
    HeaderValue::from_str(&user_agent()).expect("Invalid user agent"),
  );
  headers.insert(
    "date",
    HeaderValue::from_str(&fmt_http_date(SystemTime::now())).expect("Date is valid"),
  );

  headers
}

pub async fn sign_request(
  request_builder: RequestBuilder,
  key_id: String,
  private_key: String,
  payload: String
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
