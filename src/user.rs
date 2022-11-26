use sqlx::sqlite::SqlitePool;
use rand::{distributions::Alphanumeric, Rng};

#[derive(Debug)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub login_token: String,
    pub access_token: Option<String>
}

impl User {
    pub async fn find(id: i64, pool: &SqlitePool) -> Result<User, sqlx::Error> {
        sqlx::query_as!(User, "SELECT * FROM users WHERE id = ?", id)
            .fetch_one(pool)
            .await
    }


    pub async fn find_by_email(email: String, pool: &SqlitePool) -> Result<User, sqlx::Error> {
        sqlx::query_as!(User, "SELECT * FROM users WHERE email = ?", email)
            .fetch_one(pool)
            .await
    }

    pub async fn find_by_login(token: String, pool: &SqlitePool) -> Result<User, sqlx::Error> {
        sqlx::query_as!(User, "SELECT * FROM users WHERE login_token = ?", token)
            .fetch_one(pool)
            .await
    }

    pub async fn find_by_access(token: String, pool: &SqlitePool) -> Result<User, sqlx::Error> {
        sqlx::query_as!(User, "SELECT * FROM users WHERE access_token = ?", token)
            .fetch_one(pool)
            .await
    }

    pub async fn create_by_email(email: String, pool: &SqlitePool) -> Result<User, sqlx::Error> {
        let token = User::generate_login_token();
        let user_id = sqlx::query!(
            "INSERT INTO users (email, login_token)
                VALUES($1, $2)", email, token)
                .execute(pool)
            .await?
            .last_insert_rowid();

        User::find(user_id, pool).await
    }

    pub async fn find_or_create_by_email(email: String, pool: &SqlitePool) -> Result<User, sqlx::Error> {
        let user_check = sqlx::query_as!(User, "SELECT * FROM users WHERE email = ?", email)
            .fetch_one(pool)
            .await;

        match user_check {
            Ok(user) => return Ok(user),
            _ => return User::create_by_email(email, pool).await
        }
    }

    pub fn generate_login_token() -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(40)
            .map(char::from)
            .collect()
    }

    pub fn generate_access_token() -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(40)
            .map(char::from)
            .collect()

    }
}


#[sqlx::test]
async fn test_find_or_create_by_email(pool: SqlitePool) -> sqlx::Result<()> {
    let user = User::find_or_create_by_email(("foo@bar.com").to_string(), &pool).await?;

    assert_eq!(user.email, "foo@bar.com");
    
    Ok(())
}
