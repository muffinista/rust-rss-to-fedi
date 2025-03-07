

use derive_more::Error;

pub struct FeedError {
  pub message: String
}

impl Error for FeedError {}
impl std::fmt::Display for FeedError {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "{:}", self.message)
  }
}

impl std::fmt::Debug for FeedError {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
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
  fn test_feed_error_from_string() {
    let err = FeedError { message: String::from("Boooo") };

    assert_eq!(err.message, String::from("Boooo"));
  }

  #[test]
  fn test_feed_error_from_sqlx() {
    let err = FeedError::from(sqlx::Error::RowNotFound);
    assert_eq!(err.message, String::from("no rows returned by a query that expected to return at least one row"));
  }
}
