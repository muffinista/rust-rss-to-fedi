use activitystreams::{
  iri_string::types::IriString,
};

use openssl::{pkey::PKey, rsa::Rsa};

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicKey {
  pub public_key: PublicKeyInner,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicKeyInner {
  pub id: IriString,
  pub owner: IriString,
  pub public_key_pem: String,
}

pub fn generate_key() -> (String, String) {
  // generate keypair used for signing AP requests
  let rsa = Rsa::generate(2048).unwrap();
  let pkey = PKey::from_rsa(rsa).unwrap();
  let public_key = pkey.public_key_to_pem().unwrap();
  let private_key = pkey.private_key_to_pem_pkcs8().unwrap();

  let private_key_str = String::from_utf8(private_key).unwrap();
  let public_key_str = String::from_utf8(public_key).unwrap();

  (private_key_str, public_key_str)
}


