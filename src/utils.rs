use std::env;

pub fn path_to_url(frag: &rocket::http::uri::Origin) -> String {
  let host = env::var("DOMAIN_NAME").expect("DOMAIN_NAME is not set");
  println!("HOST: {:?}", host);
  format!("https://{}{}", host, frag).to_string()
}
