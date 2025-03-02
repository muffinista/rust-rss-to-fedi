use activitystreams::object::*;

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




#[cfg(test)]
mod test {
  use crate::models::SensitiveNote;

  #[test]
  fn test_sensitive_note() {
    let mut reply: SensitiveNote = SensitiveNote::new();
    reply.sensitive = true;
    assert!(reply.sensitive);
  }

  #[test]
  fn test_sensitive_note_default() {
    let reply: SensitiveNote = SensitiveNote::default();
    assert!(!reply.sensitive);
  }
}
