use activitystreams::{
  base::{AnyBase, Base, Extends},
  kind,
  markers,
  unparsed::*,
};

use serde_json::json;
use tera::Context;

use crate::utils::templates::render;

kind!(AttachmentType, PropertyValue);

/// Generate the context required for ading PropertyValues
pub fn schema_property_context() -> Result<AnyBase, serde_json::Error> {
  let schema_property_context = json!({
    "schema": "http://schema.org#",
    "PropertyValue": "schema:PropertyValue",
    "value": "schema:value"
  });
  AnyBase::from_arbitrary_json(schema_property_context)  
}

pub fn to_profile_value_link(tmpl: &tera::Tera, url: String, title: String) -> String {
  // here's what Mastodon does for links
  //   <<~HTML.squish.html_safe # rubocop:disable Rails/OutputSafety
  //   <a href="#{h(url)}" target="_blank" rel="#{rel.join(' ')}" translate="no"><span class="invisible">#{h(prefix)}</span><span class="#{cutoff ? 'ellipsis' : ''}">#{h(display_url)}</span><span class="invisible">#{h(suffix)}</span></a>
  // HTML

  let mut template_context = Context::new();
  template_context.insert("url", &url);
  template_context.insert("title", &title);
  
  render("profile-value-link.html.tera", tmpl, &template_context).unwrap()
}

// this is mostly copied/modified from Link

pub trait AsAttachment: markers::Base {
  type Kind;

  fn attachment_ref(&self) -> &Attachment<Self::Kind>;
  fn attachment_mut(&mut self) -> &mut Attachment<Self::Kind>;
}

/// Helper methods for interacting with Attachment types
///
/// This trait represents methods valid for any ActivityStreams Attachment.
///
/// Documentation for the fields related to these methods can be found on the `Attachment` struct
// pub trait AttachmentExt: AsAttachment {
//   fn name<'a>(&'a self) -> &'a String
//   where
//     Self::Kind: 'a,
//   {
//     &self.attachment_ref().name //.as_ref()
//   }

//   fn set_name(&mut self, name: &str) -> &mut Self {
//     self.attachment_mut().name = name.to_string();
//     self
//   }

//   fn value<'a>(&'a self) -> &'a String
//   where
//     Self::Kind: 'a,
//   {
//     &self.attachment_ref().value //.as_ref()
//   }

//   fn set_value(&mut self, value: &str) -> &mut Self {
//     self.attachment_mut().value = value.to_string();
//     self
//   }
// }


#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment<Kind> {
  name: String,
  value: String,

  #[serde(flatten)]
  inner: Base<Kind>,
}

impl<Kind> Attachment<Kind> {
  pub fn new(name: &str, value: &str) -> Self
  where
    Kind: Default,
  {
    Attachment {
      name: name.to_string(),
      value: value.to_string(),
      inner: Base::new(),
    }
  }

  fn extending(mut inner: Base<Kind>) -> Result<Self, serde_json::Error> {
    Ok(Attachment {
      name: inner.remove("name")?,
      value: inner.remove("value")?,
      inner,
    })
  }

  fn retracting(self) -> Result<Base<Kind>, serde_json::Error> {
    let Attachment {
      name,
      value,
      mut inner,
    } = self;

    inner
      .insert("name", name)?
      .insert("value", value)?;

    Ok(inner)
  }
}

impl<Kind> markers::Base for Attachment<Kind> {}
impl<Kind> markers::Link for Attachment<Kind> {}

impl<Kind> Extends for Attachment<Kind> {
  type Kind = Kind;

  type Error = serde_json::Error;

  fn extends(base: Base<Self::Kind>) -> Result<Self, Self::Error> {
    Self::extending(base)
  }

  fn retracts(self) -> Result<Base<Self::Kind>, Self::Error> {
    self.retracting()
  }
}


// impl<Kind> TryFrom<Base<Kind>> for Attachment<Kind>
// where
//   Kind: serde::de::DeserializeOwned,
// {
//   type Error = serde_json::Error;

//   fn try_from(base: Base<Kind>) -> Result<Self, Self::Error> {
//     Self::extending(base)
//   }
// }

// impl<Kind> TryFrom<Attachment<Kind>> for Base<Kind>
// where
//   Kind: serde::ser::Serialize,
// {
//   type Error = serde_json::Error;

//   fn try_from(attachment: Attachment<Kind>) -> Result<Self, Self::Error> {
//     attachment.retracting()
//   }
// }

// impl<Kind> UnparsedMut for Attachment<Kind> {
//   fn unparsed_mut(&mut self) -> &mut Unparsed {
//     self.inner.unparsed_mut()
//   }
// }

// impl<Kind> AsBase for Attachment<Kind> {
//   type Kind = Kind;

//   fn base_ref(&self) -> &Base<Self::Kind> {
//     &self.inner
//   }

//   fn base_mut(&mut self) -> &mut Base<Self::Kind> {
//     &mut self.inner
//   }
// }

// impl<Kind> AsAttachment for Attachment<Kind> {
//   type Kind = Kind;

//   fn attachment_ref(&self) -> &Attachment<Self::Kind> {
//     self
//   }

//   fn attachment_mut(&mut self) -> &mut Attachment<Self::Kind> {
//     self
//   }
// }

// impl<T> AttachmentExt for T where T: AsAttachment {}


pub type PropertyValue = Attachment<AttachmentType>;

#[cfg(test)]
mod test {
  use super::PropertyValue;
  use activitystreams::base::ExtendsExt;
  use serde_json::Value;

  #[test]
  fn test_property_value() {
    let pv = PropertyValue::new("Powered by", "https://feedsin.space/").into_any_base().expect("tf?");

    // convert to generic json value for testing
    let s = serde_json::to_string(&pv).unwrap();

    let v: Value = serde_json::from_str(&s).unwrap();
    assert_eq!("PropertyValue", v["type"]);
    assert_eq!("Powered by", v["name"]);
    assert_eq!("https://feedsin.space/", v["value"]);
  }
}
