use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;

pub const TEST_JWT_SECRET: &str = "test_jwt_secret_key_for_testing_only_at_least_32_chars";

pub fn get_test_jwt_secret() -> String {
    TEST_JWT_SECRET.to_string()
}

pub async fn create_test_pool() -> PgPool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost:5432/magnetite_test".to_string());

    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(30))
        .connect(&database_url)
        .await
        .expect("Failed to create test pool")
}

pub fn init_test_env() {
    std::env::set_var("JWT_SECRET", TEST_JWT_SECRET);
    std::env::set_var("APP_ENV", "test");
}

pub mod mock {
    use super::*;
    use argon2::{
        password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, SaltString},
        Argon2,
    };
    use chrono::{DateTime, Utc};
    use uuid::Uuid;

    pub struct MockUser {
        pub id: Uuid,
        pub username: String,
        pub email: String,
        pub password_hash: String,
        pub wallet_address: Option<String>,
        pub created_at: DateTime<Utc>,
    }

    impl MockUser {
        pub fn new(username: &str, email: &str, password: &str) -> Self {
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = Argon2::default();
            let password_hash = argon2
                .hash_password(password.as_bytes(), &salt)
                .map(|h| h.to_string())
                .unwrap_or_default();

            Self {
                id: Uuid::new_v4(),
                username: username.to_string(),
                email: email.to_string(),
                password_hash,
                wallet_address: None,
                created_at: Utc::now(),
            }
        }
    }

    pub struct MockSession {
        pub id: Uuid,
        pub user_id: Uuid,
        pub refresh_token_hash: String,
        pub user_agent: Option<String>,
        pub ip_address: Option<String>,
        pub expires_at: DateTime<Utc>,
        pub created_at: DateTime<Utc>,
    }

    impl MockSession {
        pub fn new(user_id: Uuid) -> Self {
            Self {
                id: Uuid::new_v4(),
                user_id,
                refresh_token_hash: String::new(),
                user_agent: None,
                ip_address: None,
                expires_at: Utc::now(),
                created_at: Utc::now(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_user_creation() {
        let user = mock::MockUser::new("testuser", "test@example.com", "password123");
        assert_eq!(user.username, "testuser");
        assert_eq!(user.email, "test@example.com");
        assert!(!user.password_hash.is_empty());
    }

    #[test]
    fn test_mock_session_creation() {
        let user_id = Uuid::new_v4();
        let session = mock::MockSession::new(user_id);
        assert_eq!(session.user_id, user_id);
    }

    #[test]
    fn test_init_test_env() {
        init_test_env();
        assert_eq!(std::env::var("JWT_SECRET").unwrap(), TEST_JWT_SECRET);
    }
}
