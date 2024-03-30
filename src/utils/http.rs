use reqwest_middleware::ClientBuilder;
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use reqwest::header::{HeaderValue, HeaderMap};

use httpdate::fmt_http_date;

use std::time::{Duration, SystemTime};
use std::env;

static BASE_USER_AGENT: &str = concat!(
  env!("CARGO_PKG_NAME"),
  "/",
  env!("CARGO_PKG_VERSION"),
);

///
/// Generate a user agent for the current version of the code and the running instance
///
pub fn user_agent() -> String {
  let domain_name = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  format!("{BASE_USER_AGENT}; +{domain_name})")
}

pub fn generate_request_headers() -> HeaderMap {
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

pub fn http_client() -> Result<reqwest_middleware::ClientWithMiddleware, reqwest::Error> {
  let request_timeout = Duration::from_secs(15);
    let base_client = reqwest::Client::builder()
        .timeout(request_timeout)
        .build()?;

  // Retry up to 3 times with increasing intervals between attempts.
  let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

  Ok(ClientBuilder::new(base_client)
    .with(RetryTransientMiddleware::new_with_policy(retry_policy))
    .build())
}
