use reqwest_middleware::ClientBuilder;
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};

pub fn http_client() -> reqwest_middleware::ClientWithMiddleware {
  // Retry up to 3 times with increasing intervals between attempts.
  let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);

  ClientBuilder::new(reqwest::Client::new())
    .with(RetryTransientMiddleware::new_with_policy(retry_policy))
    .build()
}
