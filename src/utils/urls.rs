use std::env;

use actix_web::HttpResponse;

pub fn redirect_to(url: &str) -> HttpResponse {
  let mut res = HttpResponse::TemporaryRedirect();
  res.insert_header((actix_web::http::header::LOCATION, url.as_bytes())).finish()
}


///
/// convert path to absolute URL
///
pub fn path_to_url(frag: &str) -> String {
  let host = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  format!("https://{host}{frag}")
}
