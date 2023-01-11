use std::env;

///
/// convert path to absolute URL
///
pub fn path_to_url(frag: &rocket::http::uri::Origin) -> String {
  let host = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  format!("https://{}{}", host, frag).to_string()
}
