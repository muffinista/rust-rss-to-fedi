use activitystreams::{
  base::{AsBase, Base, Extends},
  markers,
  object::{AsObject, Object},
  unparsed::*,
};

// https://docs.rs/activitystreams/0.7.0-alpha.24/activitystreams/unparsed/index.html

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanBeSensitive<Inner> {
  pub sensitive: bool,
  pub inner: Inner,
}


impl<Inner> Extends for CanBeSensitive<Inner>
where
Inner: Extends<Error=serde_json::Error> + UnparsedMut,
{
  type Kind = Inner::Kind;
  type Error = serde_json::Error;
  
  fn extends(base: Base<Self::Kind>) -> Result<Self, Self::Error> {
    let mut inner = Inner::extends(base)?;
    
    Ok(CanBeSensitive {
      sensitive: inner.unparsed_mut().remove("sensitive")?,
      inner,
    })
  }
  
  fn retracts(self) -> Result<Base<Self::Kind>, Self::Error> {
    let CanBeSensitive {
      sensitive,
      mut inner,
    } = self;
    
    inner.unparsed_mut().insert("sensitive", sensitive)?;
    
    inner.retracts()
  }
}


/// Auto-implement Base, Object, and Actor when Inner supports it
impl<Inner> markers::Base for CanBeSensitive<Inner> where Inner: markers::Base {}
impl<Inner> markers::Object for CanBeSensitive<Inner> where Inner: markers::Object {}
impl<Inner> markers::Actor for CanBeSensitive<Inner> where Inner: markers::Actor {}


/// If we want to easily access getters and setters for internal types, we'll need to forward
/// those, too.
///
/// Forward for base methods
///
impl<Inner> AsBase for CanBeSensitive<Inner>
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
impl<Inner> AsObject for CanBeSensitive<Inner>
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
///
/// Make it easy for downstreams to get an Unparsed
impl<Inner> UnparsedMut for CanBeSensitive<Inner>
where
Inner: UnparsedMut,
{
  fn unparsed_mut(&mut self) -> &mut Unparsed {
    self.inner.unparsed_mut()
  }
}



/// Create our own extensible trait
pub trait AsCanBeSensitive<Inner> {
  fn can_be_sensitive_ref(&self) -> &CanBeSensitive<Inner>;
  fn can_be_sensitive_mut(&mut self) -> &mut CanBeSensitive<Inner>;
}

/// Implement it
impl<Inner> AsCanBeSensitive<Inner> for CanBeSensitive<Inner> {
  fn can_be_sensitive_ref(&self) -> &Self {
    self
  }
  
  fn can_be_sensitive_mut(&mut self) -> &mut Self {
    self
  }
}

/// And now create helper methods
pub trait CanBeSensitiveExt<Inner>: AsCanBeSensitive<Inner> {
  /// grab sensitive setting
  fn sensitive<'a>(&'a self) -> &'a bool
  where
  Inner: 'a,
  {
    &self.can_be_sensitive_ref().sensitive
  }
  
  /// Set as sensitive
  fn set_sensitive(&mut self, val: bool) -> &mut Self {
    self.can_be_sensitive_mut().sensitive = val;
    self
  }
}


/// Finally, we'll automatically implement CanBeSensitiveExt for any type implementing AsCanBeSensitive
impl<T, Inner> CanBeSensitiveExt<Inner> for T where T: AsCanBeSensitive<Inner> {}

