use sqlx::postgres::PgPool;
use fang::FangError;
use crate::models::Message;


pub async fn cleanup_messages(pool: &PgPool) -> Result<(), FangError> {
		let result = Message::cleanup(pool, 10000, 10000).await;
		match result {
    Ok(result) => Ok(result),
    Err(err) => {
      let description = format!("{err:?}");

      Err(FangError { description })
		}
		}
}

