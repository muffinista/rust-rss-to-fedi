pub mod http;
pub mod keys;
pub mod urls;
pub mod admin;
pub mod queue;
pub mod pool;
pub mod templates;

pub use urls::*;

pub mod signature_check;

use sha2::Digest;

pub fn string_to_digest_string(data: &str) -> String {
  let mut digester = sha2::Sha256::new();
  digester.update(data);
  // https://users.rust-lang.org/t/sha256-result-to-string/49391
  format!("{:X}", digester.finalize())
}


#[cfg(test)]
pub mod test_helpers;
