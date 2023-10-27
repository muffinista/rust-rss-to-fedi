use std::error::Error;
use std::fmt;

pub struct FeedError {
  pub message: String
}

impl Error for FeedError {}
impl fmt::Display for FeedError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:}", self.message)
  }
}

impl fmt::Debug for FeedError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    // @todo does this work?
    let current_file = file!();
    let current_line = line!();

    write!(f, "FeedError {:} {{ file: {current_file:}, line: {current_line:} }}", self.message)
  }
}

impl From<sqlx::Error> for FeedError {
  fn from(error: sqlx::Error) -> Self {
    FeedError {
      message: error.to_string(),
    }
  }
}


#[cfg(test)]
mod test {
  use super::FeedError;

  #[test]
  fn test_feed_error() {
    let err = FeedError { message: String::from("Boooo") };

    assert_eq!(err.message, String::from("Boooo"));
  }
}
