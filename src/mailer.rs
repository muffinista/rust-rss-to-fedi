use http_signature_normalization_reqwest::prelude::*;
use reqwest::Request;
use reqwest_middleware::RequestBuilder;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest::header::HeaderValue;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;



use webfinger::resolve;
use webfinger::WebfingerError;

use openssl::{
  hash::MessageDigest,
  pkey::PKey,
  rsa::Rsa,
  sign::{Signer, Verifier},
};

use url::Url;
use httpdate::fmt_http_date;
use std::time::SystemTime;

use sha2::{Digest, Sha256};
use base64;

pub async fn find_inbox(actor: &str) -> Result<Url, WebfingerError> {
  let webfinger = resolve(actor, true).await;
  let rel = "self".to_string();
  let mime_type = Some("application/activity+json".to_string());

  match webfinger {
    Ok(webfinger) => {
      let query:Option<webfinger::Link> = webfinger
        .links
        .into_iter()
        .find(|link| &link.rel == &rel && &link.mime_type == &mime_type);

      match query {
        Some(query) => Ok(Url::parse(&query.href.unwrap()).unwrap()),
        None => todo!()
      }
      
    },
    Err(why) => Err(why)
  }
}


/// deliver a payload to an inbox
pub async fn deliver_to_inbox(actor: &str, key_id: &str, private_key: &str, json: &str) -> Result<(), anyhow::Error> {
  let webfinger = find_inbox(actor).await;
  match webfinger {
    Ok(webfinger) => {
      println!("{:?}", webfinger);

      let client = ClientBuilder::new(reqwest::Client::new())
        .build();
      // // Trace HTTP requests. See the tracing crate to make use of these traces.
      // .with(TracingMiddleware::default())
      // // Retry failed requests.
      // .with(RetryTransientMiddleware::new_with_policy(retry_policy))
  
      let heads = generate_request_headers(&webfinger);
      
      let request_builder = client
          .post(webfinger)
          // .timeout(timeout)
          .headers(heads);
  
      let request = sign_request(
          request_builder,
          private_key.to_string(),
          json.to_string()
      )
      .await?;

      let response = client.execute(request).await;
      match response {
        // @todo check response code/etc
        Ok(response) => Ok(()),
        Err(_why) => todo!()
      }
    },
    Err(_why) => todo!()
  }
}


fn generate_request_headers(inbox: &Url) -> HeaderMap {
  let mut host = inbox.domain().expect("Domain is valid").to_string();
  if let Some(port) = inbox.port() {
      host = format!("{}:{}", host, port);
  }

  let mut headers = HeaderMap::new();
  headers.insert(
    HeaderName::from_static("content-type"),
    HeaderValue::from_static("application/activity+json"),
  );
  headers.insert(
    HeaderName::from_static("host"),
    HeaderValue::from_str(&host).expect("Hostname is valid"),
  );
  headers.insert(
    "date",
    HeaderValue::from_str(&fmt_http_date(SystemTime::now())).expect("Date is valid"),
  );

  headers
}

pub async fn sign_request(
  request_builder: RequestBuilder,
  private_key: String,
  payload: String
) -> Result<Request, anyhow::Error> {

  // https://docs.rs/http-signature-normalization-reqwest/0.7.1/http_signature_normalization_reqwest/struct.Config.html#method.mastodon_compat
  let config = Config::new().mastodon_compat();
  let digest = Sha256::new();

  request_builder
      .signature_with_digest(
          config,
          "a-key-id",
          digest,
          payload,
          move |signing_string| {
              let private_key = PKey::private_key_from_pem(private_key.as_bytes())?;
              let mut signer = Signer::new(MessageDigest::sha256(), &private_key)?;
              signer.update(signing_string.as_bytes())?;

              Ok(base64::encode(signer.sign_to_vec()?)) as Result<_, anyhow::Error>
          },
      )
      .await
  }


#[tokio::test]
async fn test_find_inbox() {
  let actor = "muffinista@botsin.space";
  let inbox:Url = find_inbox(&actor).await.unwrap();
  assert_eq!("https://botsin.space/users/muffinista", inbox.to_string());
}
