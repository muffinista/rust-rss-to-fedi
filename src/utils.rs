use std::env;

pub fn path_to_url(frag: &str) -> String {
  let host = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  format!("https://{}{}", host, frag)
}
