#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use chrono::Utc;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use tower::ServiceExt;

    fn get_test_jwt_secret() -> String {
        "test_jwt_secret_key_for_testing_only".to_string()
    }

    fn create_test_token(user_id: &str, email: &str, expired: bool) -> String {
        std::env::set_var("JWT_SECRET", get_test_jwt_secret());

        let exp_time = if expired {
            Utc::now().timestamp() - 3600
        } else {
            Utc::now().timestamp() + 3600
        };

        let payload = magnetite_backend::api::middleware::Claims {
            sub: user_id.to_string(),
            email: Some(email.to_string()),
            session_id: Some("test-session-id".to_string()),
            exp: exp_time,
            iat: Utc::now().timestamp(),
        };

        encode(
            &Header::default(),
            &payload,
            &EncodingKey::from_secret(get_test_jwt_secret().as_bytes()),
        )
        .unwrap()
    }

    mod health_endpoint_tests {
        use super::*;
        use tower::util::ServiceExt;

        #[tokio::test]
        async fn test_health_check_returns_ok() {
            let app = Router::new()
                .route("/health", get(|| async { "OK" }));

            let response = app
                .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    mod middleware_tests {
        use super::*;
        use magnetite_backend::api::middleware::{auth_middleware, extract_token_from_header, Claims};
        use axum::http::HeaderMap;

        #[test]
        fn test_extract_token_from_header_valid() {
            let mut headers = HeaderMap::new();
            headers.insert("Authorization", "Bearer test_token_123".parse().unwrap());

            let result = extract_token_from_header(&headers);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "test_token_123");
        }

        #[test]
        fn test_extract_token_from_header_missing() {
            let headers = HeaderMap::new();
            let result = extract_token_from_header(&headers);
            assert!(result.is_err());
        }

        #[test]
        fn test_extract_token_from_header_invalid_format() {
            let mut headers = HeaderMap::new();
            headers.insert("Authorization", "Basic test_token_123".parse().unwrap());

            let result = extract_token_from_header(&headers);
            assert!(result.is_err());
        }

        #[test]
        fn test_extract_token_from_header_empty_token() {
            let mut headers = HeaderMap::new();
            headers.insert("Authorization", "Bearer ".parse().unwrap());

            let result = extract_token_from_header(&headers);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "");
        }

        #[tokio::test]
        async fn test_auth_middleware_valid_token() {
            std::env::set_var("JWT_SECRET", get_test_jwt_secret());

            let user_id = uuid::Uuid::new_v4().to_string();
            let token = create_test_token(&user_id, "test@example.com", false);

            let mut headers = HeaderMap::new();
            headers.insert("Authorization", format!("Bearer {}", token).parse().unwrap());

            let user_id_result = magnetite_backend::api::middleware::auth_guard(headers).await;
            assert!(user_id_result.is_ok());
            assert_eq!(user_id_result.unwrap().to_string(), user_id);
        }

        #[tokio::test]
        async fn test_auth_middleware_invalid_token() {
            std::env::set_var("JWT_SECRET", get_test_jwt_secret());

            let mut headers = HeaderMap::new();
            headers.insert("Authorization", "Bearer invalid.token.here".parse().unwrap());

            let result = magnetite_backend::api::middleware::auth_guard(headers).await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_auth_middleware_expired_token() {
            std::env::set_var("JWT_SECRET", get_test_jwt_secret());

            let user_id = uuid::Uuid::new_v4().to_string();
            let token = create_test_token(&user_id, "test@example.com", true);

            let mut headers = HeaderMap::new();
            headers.insert("Authorization", format!("Bearer {}", token).parse().unwrap());

            let result = magnetite_backend::api::middleware::auth_guard(headers).await;
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn test_auth_middleware_missing_header() {
            let headers = HeaderMap::new();
            let result = magnetite_backend::api::middleware::auth_guard(headers).await;
            assert!(result.is_err());
        }

        #[test]
        fn test_validate_token_valid() {
            std::env::set_var("JWT_SECRET", get_test_jwt_secret());

            let user_id = uuid::Uuid::new_v4().to_string();
            let token = create_test_token(&user_id, "test@example.com", false);

            let result = magnetite_backend::api::middleware::validate_token(&token);
            assert!(result.is_ok());
            let claims = result.unwrap();
            assert_eq!(claims.sub, user_id);
            assert_eq!(claims.email, Some("test@example.com".to_string()));
        }

        #[test]
        fn test_validate_token_invalid() {
            std::env::set_var("JWT_SECRET", get_test_jwt_secret());

            let result = magnetite_backend::api::middleware::validate_token("invalid.token");
            assert!(result.is_err());
        }
    }

    mod claims_tests {
        use super::*;
        use magnetite_backend::api::middleware::Claims;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize)]
        struct TokenClaims {
            sub: String,
            email: Option<String>,
            session_id: Option<String>,
            exp: i64,
            iat: i64,
        }

        #[test]
        fn test_claims_deserialization() {
            let claims_json = r#"{
                "sub": "550e8400-e29b-41d4-a716-446655440000",
                "email": "test@example.com",
                "session_id": "session-123",
                "exp": 1234567890,
                "iat": 1234567800
            }"#;

            let claims: TokenClaims = serde_json::from_str(claims_json).unwrap();
            assert_eq!(claims.sub, "550e8400-e29b-41d4-a716-446655440000");
            assert_eq!(claims.email, Some("test@example.com".to_string()));
        }

        #[test]
        fn test_claims_serialization() {
            let claims = TokenClaims {
                sub: "user-123".to_string(),
                email: Some("test@example.com".to_string()),
                session_id: Some("session-456".to_string()),
                exp: 1234567890,
                iat: 1234567800,
            };

            let json = serde_json::to_string(&claims).unwrap();
            assert!(json.contains("user-123"));
            assert!(json.contains("test@example.com"));
        }
    }

    mod token_validation_tests {
        use super::*;

        #[test]
        fn test_token_with_malformed_header() {
            std::env::set_var("JWT_SECRET", get_test_jwt_secret());
            
            let result = magnetite_backend::api::middleware::validate_token("notavalidjwt");
            assert!(result.is_err());
        }

        #[test]
        fn test_token_with_wrong_algorithm() {
            std::env::set_var("JWT_SECRET", get_test_jwt_secret());

            use jsonwebtoken::{encode, EncodingKey, Header, Algorithm};
            
            let mut header = Header::default();
            header.alg = Algorithm::HS512;

            let payload = magnetite_backend::api::middleware::Claims {
                sub: "user-123".to_string(),
                email: Some("test@example.com".to_string()),
                session_id: Some("session-456".to_string()),
                exp: Utc::now().timestamp() + 3600,
                iat: Utc::now().timestamp(),
            };

            let token = encode(
                &header,
                &payload,
                &EncodingKey::from_secret(get_test_jwt_secret().as_bytes()),
            ).unwrap();

            let result = magnetite_backend::api::middleware::validate_token(&token);
            assert!(result.is_err());
        }
    }

    mod registration_tests {
        use super::*;

        #[test]
        fn test_register_request_deserialization() {
            let json = r#"{
                "username": "testuser",
                "email": "test@example.com",
                "password": "SecurePassword123!"
            }"#;

            let request: magnetite_backend::api::auth::RegisterRequest = 
                serde_json::from_str(json).unwrap();
            
            assert_eq!(request.username, "testuser");
            assert_eq!(request.email, "test@example.com");
            assert_eq!(request.password, "SecurePassword123!");
        }

        #[test]
        fn test_login_request_deserialization() {
            let json = r#"{
                "username": "testuser",
                "password": "SecurePassword123!"
            }"#;

            let request: magnetite_backend::api::auth::LoginRequest = 
                serde_json::from_str(json).unwrap();
            
            assert_eq!(request.username, "testuser");
            assert_eq!(request.password, "SecurePassword123!");
        }

        #[test]
        fn test_auth_response_serialization() {
            let response = magnetite_backend::api::auth::AuthResponse {
                access_token: "access_token_123".to_string(),
                refresh_token: "refresh_token_456".to_string(),
                expires_at: Utc::now(),
                user_id: uuid::Uuid::new_v4(),
            };

            let json = serde_json::to_string(&response).unwrap();
            assert!(json.contains("access_token_123"));
            assert!(json.contains("refresh_token_456"));
        }

        #[test]
        fn test_refresh_request_deserialization() {
            let json = r#"{"refresh_token": "refresh_token_123"}"#;

            let request: magnetite_backend::api::auth::RefreshRequest = 
                serde_json::from_str(json).unwrap();
            
            assert_eq!(request.refresh_token, "refresh_token_123");
        }
    }

    mod user_response_tests {
        use super::*;

        #[test]
        fn test_user_response_serialization() {
            let response = magnetite_backend::api::auth::UserResponse {
                id: uuid::Uuid::new_v4(),
                username: "testuser".to_string(),
                email: "test@example.com".to_string(),
                created_at: Utc::now(),
            };

            let json = serde_json::to_string(&response).unwrap();
            assert!(json.contains("testuser"));
            assert!(json.contains("test@example.com"));
        }
    }
}
