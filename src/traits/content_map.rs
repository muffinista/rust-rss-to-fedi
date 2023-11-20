use activitystreams::{
  base::{AsBase, Base, Extends},
  markers,
  object::*,
  unparsed::*,
};

use std::collections::HashMap;

// https://docs.rs/activitystreams/0.7.0-alpha.24/activitystreams/unparsed/index.html

pub type ContentMapValues = HashMap<String, String>;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentMap<Inner> {
  // note: i had this named content_map but the camelCase rename wasn't catching for some reason
  // and i didn't want to deal
  #[allow(non_snake_case)]
  contentMap: ContentMapValues,
  pub inner: Inner,
}


impl<Inner> Extends for ContentMap<Inner>
where
Inner: Extends<Error=serde_json::Error> + UnparsedMut,
{
  type Kind = Inner::Kind;
  type Error = serde_json::Error;
  
  fn extends(base: Base<Self::Kind>) -> Result<Self, Self::Error> {
    let mut inner = Inner::extends(base)?;
    
    Ok(ContentMap {
      contentMap: inner.unparsed_mut().remove("contentMap")?,
      inner,
    })
  }
  
  fn retracts(self) -> Result<Base<Self::Kind>, Self::Error> {
    let ContentMap {
      contentMap,
      mut inner,
    } = self;
    
    inner.unparsed_mut().insert("contentMap", contentMap)?;
    
    inner.retracts()
  }
}


/// Auto-implement Base, Object, and Actor when Inner supports it
impl<Inner> markers::Base for ContentMap<Inner> where Inner: markers::Base {}
impl<Inner> markers::Object for ContentMap<Inner> where Inner: markers::Object {}
impl<Inner> markers::Actor for ContentMap<Inner> where Inner: markers::Actor {}


/// If we want to easily access getters and setters for internal types, we'll need to forward
/// those, too.

/// Forward for base methods
///
/// This allows us to access methods related to `context`, `id`, `kind`, `name`,
/// `media_type`, and `preview` directly from the PublicKey struct
impl<Inner> AsBase for ContentMap<Inner>
where
Inner: AsBase,
{
  type Kind = Inner::Kind;
  
  fn base_ref(&self) -> &Base<Self::Kind> {
    self.inner.base_ref()
  }
  
  fn base_mut(&mut self) -> &mut Base<Self::Kind> {
    self.inner.base_mut()
  }
}

/// Forward for object methods
///
/// This allows us to access methods related to `url`, `generator`, `start_time`, `duration`,
/// and more directly from the PublicKey struct
impl<Inner> AsObject for ContentMap<Inner>
where
Inner: AsObject,
{
  type Kind = Inner::Kind;
  
  fn object_ref(&self) -> &Object<Self::Kind> {
    self.inner.object_ref()
  }
  
  fn object_mut(&mut self) -> &mut Object<Self::Kind> {
    self.inner.object_mut()
  }
}


/// If we want to be able to extend from our own type, we'll need to forward some
/// implementations, and create some traits

/// Make it easy for downstreams to get an Unparsed
impl<Inner> UnparsedMut for ContentMap<Inner>
where
Inner: UnparsedMut,
{
  fn unparsed_mut(&mut self) -> &mut Unparsed {
    self.inner.unparsed_mut()
  }
}



/// Create our own extensible trait
pub trait AsContentMap<Inner> {
  fn content_map_ref(&self) -> &ContentMap<Inner>;
  fn content_map_mut(&mut self) -> &mut ContentMap<Inner>;
}

/// Implement it
impl<Inner> AsContentMap<Inner> for ContentMap<Inner> {
  fn content_map_ref(&self) -> &Self {
    self
  }
  
  fn content_map_mut(&mut self) -> &mut Self {
    self
  }
}

/// And now create helper methods
pub trait AsContentMapExt<Inner>: AsContentMap<Inner> {
  /// Borrow the public key's ID
  fn content_map<'a>(&'a self) -> &'a ContentMapValues
  where
  Inner: 'a,
  {
    &self.content_map_ref().contentMap
  }
  
  /// Set the public key's ID
  fn set_content_map(&mut self, val: ContentMapValues) -> &mut Self {
    self.content_map_mut().contentMap = val;
    self
  }

  fn set_content_language_and_value(&mut self, key: String, value: String) -> &mut Self {
    self.content_map_mut().contentMap.insert(key, value);
    self
  }
}


/// Finally, we'll automatically implement PublicKeyExt for any type implementing AsPublicKey
impl<T, Inner> AsContentMapExt<Inner> for T where T: AsContentMap<Inner> {}

pub type ContentMapNote = ContentMap<ApObject<Note>>;

impl ContentMapNote {
  pub fn new() -> ContentMapNote {
    ContentMapNote {
      contentMap: ContentMapValues::new(),
      inner: ApObject::new(Note::new()),
    }
  }
}

impl Default for ContentMapNote {
  fn default() -> Self {
    Self::new()
  }
}
