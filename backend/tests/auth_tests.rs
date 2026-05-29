#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    fn get_test_jwt_secret() -> String {
        "test_jwt_secret_key_for_testing_only".to_string()
    }

    fn set_test_jwt_secret() {
        std::env::set_var("JWT_SECRET", get_test_jwt_secret());
    }

    fn clear_jwt_secret() {
        std::env::remove_var("JWT_SECRET");
    }

    mod password_tests {
        use super::*;

        #[test]
        fn test_hash_password_success() {
            let password = "SecurePassword123!";
            let hash = magnetite_backend::services::auth::hash_password(password);
            assert!(hash.is_ok());
            let hash_str = hash.unwrap();
            assert!(!hash_str.is_empty());
            assert_ne!(hash_str, password);
        }

        #[test]
        fn test_hash_password_different_hashes() {
            let password = "SecurePassword123!";
            let hash1 = magnetite_backend::services::auth::hash_password(password).unwrap();
            let hash2 = magnetite_backend::services::auth::hash_password(password).unwrap();
            assert_ne!(hash1, hash2);
        }

        #[test]
        fn test_verify_password_correct() {
            let password = "SecurePassword123!";
            let hash = magnetite_backend::services::auth::hash_password(password).unwrap();
            assert!(magnetite_backend::services::auth::verify_password(password, &hash));
        }

        #[test]
        fn test_verify_password_incorrect() {
            let password = "SecurePassword123!";
            let wrong_password = "WrongPassword456!";
            let hash = magnetite_backend::services::auth::hash_password(password).unwrap();
            assert!(!magnetite_backend::services::auth::verify_password(wrong_password, &hash));
        }

        #[test]
        fn test_verify_password_invalid_hash() {
            assert!(!magnetite_backend::services::auth::verify_password("password", "invalid_hash"));
        }

        #[test]
        fn test_verify_password_empty_password() {
            let hash = magnetite_backend::services::auth::hash_password("password").unwrap();
            assert!(!magnetite_backend::services::auth::verify_password("", &hash));
        }

        #[test]
        fn test_verify_password_empty_hash() {
            assert!(!magnetite_backend::services::auth::verify_password("password", ""));
        }
    }

    mod jwt_tests {
        use super::*;
        use magnetite_backend::services::session::{
            generate_access_token, decode_access_token, generate_refresh_token,
            generate_tokens, ACCESS_TOKEN_EXPIRY_SECS, REFRESH_TOKEN_EXPIRY_SECS,
        };

        #[test]
        fn test_generate_access_token() {
            temp_env::with_var("JWT_SECRET", Some(get_test_jwt_secret()), || {
                let user_id = Uuid::new_v4();
                let session_id = Uuid::new_v4();
                let email = "test@example.com";

                let token = generate_access_token(user_id, session_id, email);
                assert!(token.is_ok());
                let token_str = token.unwrap();
                assert!(!token_str.is_empty());
            });
        }

        #[test]
        fn test_decode_access_token() {
            temp_env::with_var("JWT_SECRET", Some(get_test_jwt_secret()), || {
                let user_id = Uuid::new_v4();
                let session_id = Uuid::new_v4();
                let email = "test@example.com";

                let token = generate_access_token(user_id, session_id, email).unwrap();
                let claims = decode_access_token(&token);
                assert!(claims.is_ok());

                let decoded = claims.unwrap();
                assert_eq!(decoded.sub, user_id.to_string());
                assert_eq!(decoded.email, email);
                assert_eq!(decoded.session_id, session_id.to_string());
            });
        }

        #[test]
        fn test_decode_access_token_invalid() {
            temp_env::with_var("JWT_SECRET", Some(get_test_jwt_secret()), || {
                let result = decode_access_token("invalid.token.here");
                assert!(result.is_err());
            });
        }

        #[test]
        fn test_decode_access_token_wrong_secret() {
            temp_env::with_var("JWT_SECRET", Some("secret1"), || {
                let user_id = Uuid::new_v4();
                let session_id = Uuid::new_v4();
                let email = "test@example.com";

                let token = generate_access_token(user_id, session_id, email).unwrap();

                temp_env::with_var("JWT_SECRET", Some("secret2"), || {
                    let result = decode_access_token(&token);
                    assert!(result.is_err());
                });
            });
        }

        #[test]
        fn test_generate_tokens() {
            temp_env::with_var("JWT_SECRET", Some(get_test_jwt_secret()), || {
                let user_id = Uuid::new_v4();
                let email = "test@example.com";

                let result = generate_tokens(user_id, email);
                assert!(result.is_ok());

                let (access_token, refresh_token) = result.unwrap();
                assert!(!access_token.token.is_empty());
                assert!(!refresh_token.token.is_empty());
                assert_eq!(access_token.expires_in, ACCESS_TOKEN_EXPIRY_SECS);
                assert_eq!(refresh_token.expires_in, REFRESH_TOKEN_EXPIRY_SECS);
            });
        }
    }

    mod token_expiry_tests {
        use super::*;
        use magnetite_backend::services::session::{generate_access_token, decode_access_token};
        use jsonwebtoken::{encode, EncodingKey, Header};
        use chrono::Utc;

        #[test]
        fn test_expired_token_rejected() {
            temp_env::with_var("JWT_SECRET", Some(get_test_jwt_secret()), || {
                let user_id = Uuid::new_v4();
                let session_id = Uuid::new_v4();
                let email = "test@example.com";

                let past_time = Utc::now().timestamp() - 3600;
                let payload = magnetite_backend::services::session::AccessTokenClaims {
                    sub: user_id.to_string(),
                    email: email.to_string(),
                    session_id: session_id.to_string(),
                    exp: past_time,
                    iat: past_time - 3600,
                };

                let expired_token = encode(
                    &Header::default(),
                    &payload,
                    &EncodingKey::from_secret(get_test_jwt_secret().as_bytes()),
                ).unwrap();

                let result = decode_access_token(&expired_token);
                assert!(result.is_err());
            });
        }

        #[test]
        fn test_future_token_accepted() {
            temp_env::with_var("JWT_SECRET", Some(get_test_jwt_secret()), || {
                let user_id = Uuid::new_v4();
                let session_id = Uuid::new_v4();
                let email = "test@example.com";

                let future_time = Utc::now().timestamp() + 3600;
                let payload = magnetite_backend::services::session::AccessTokenClaims {
                    sub: user_id.to_string(),
                    email: email.to_string(),
                    session_id: session_id.to_string(),
                    exp: future_time,
                    iat: Utc::now().timestamp(),
                };

                let future_token = encode(
                    &Header::default(),
                    &payload,
                    &EncodingKey::from_secret(get_test_jwt_secret().as_bytes()),
                ).unwrap();

                let result = decode_access_token(&future_token);
                assert!(result.is_ok());
            });
        }
    }

    mod refresh_token_tests {
        use super::*;
        use magnetite_backend::services::session::{
            generate_refresh_token, hash_refresh_token, verify_refresh_token,
        };

        #[test]
        fn test_generate_refresh_token() {
            let token = generate_refresh_token();
            assert!(!token.is_empty());
            assert!(token.len() >= 32);
        }

        #[test]
        fn test_generate_refresh_token_unique() {
            let token1 = generate_refresh_token();
            let token2 = generate_refresh_token();
            assert_ne!(token1, token2);
        }

        #[test]
        fn test_hash_refresh_token() {
            let token = generate_refresh_token();
            let hash = hash_refresh_token(&token);
            assert!(hash.is_ok());
            let hash_str = hash.unwrap();
            assert_ne!(hash_str, token);
        }

        #[test]
        fn test_verify_refresh_token() {
            let token = generate_refresh_token();
            let hash = hash_refresh_token(&token).unwrap();
            assert!(verify_refresh_token(&token, &hash));
        }

        #[test]
        fn test_verify_refresh_token_wrong_token() {
            let token = generate_refresh_token();
            let hash = hash_refresh_token(&token).unwrap();
            let wrong_token = generate_refresh_token();
            assert!(!verify_refresh_token(&wrong_token, &hash));
        }

        #[test]
        fn test_verify_refresh_token_invalid_hash() {
            assert!(!verify_refresh_token("token", "invalid_hash"));
        }
    }

    mod token_size_tests {
        use magnetite_backend::services::session::generate_secure_token;

        #[test]
        fn test_generate_secure_token_size() {
            for size in [16, 32, 64, 128] {
                let token = generate_secure_token(size);
                assert_eq!(token.len(), size * 8 / 6 + 1);
            }
        }

        #[test]
        fn test_generate_secure_token_randomness() {
            let token1 = generate_secure_token(32);
            let token2 = generate_secure_token(32);
            assert_ne!(token1, token2);
        }
    }
}
