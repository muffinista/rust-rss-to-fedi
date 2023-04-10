use activitystreams::{
  object::ApObject,
  object::*,
};

use crate::traits::sensitive::*;


///
/// Extend Notes with a 'sensitive' field which Mastodon uses
///
pub type SensitiveNote = CanBeSensitive<ApObject<Note>>;

impl SensitiveNote {
  pub fn new() -> SensitiveNote {
    CanBeSensitive {
      sensitive: false,
      inner: ApObject::new(Note::new()),
    }
  }
}

impl Default for SensitiveNote {
  fn default() -> Self {
    Self::new()
  }
}
