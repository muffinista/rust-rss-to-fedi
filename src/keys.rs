use activitystreams_ext::{UnparsedExtension};
use activitystreams::unparsed::*;

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

impl<U> UnparsedExtension<U> for PublicKey where U: UnparsedMutExt, {
  type Error = serde_json::Error;

  fn try_from_unparsed(unparsed_mut: &mut U) -> Result<Self, Self::Error> {
    Ok(PublicKey {
      public_key: unparsed_mut.remove("publicKey")?,
    })
  }

  fn try_into_unparsed(self, unparsed_mut: &mut U) -> Result<(), Self::Error> {
    unparsed_mut.insert("publicKey", self.public_key)?;
    Ok(())
  }
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


