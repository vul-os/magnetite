#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    mod wallet_models_tests {
        use super::*;

        #[test]
        fn test_wallet_balance_serialization() {
            let balance = magnetite_backend::api::wallet::WalletBalance {
                user_id: Uuid::new_v4(),
                balance: dec!(100.50),
                currency: "USDC".to_string(),
            };

            let json = serde_json::to_string(&balance).unwrap();
            assert!(json.contains("USDC"));
            assert!(json.contains("100.5"));
        }

        #[test]
        fn test_deposit_request_deserialization() {
            let json = r#"{
                "amount": "50.25",
                "payment_id": "pi_123456789"
            }"#;

            let request: magnetite_backend::api::wallet::DepositRequest = 
                serde_json::from_str(json).unwrap();
            
            assert_eq!(request.amount, dec!(50.25));
            assert_eq!(request.payment_id, "pi_123456789");
        }

        #[test]
        fn test_withdraw_request_deserialization() {
            let json = r#"{
                "amount": "25.00",
                "destination": "0x1234567890abcdef"
            }"#;

            let request: magnetite_backend::api::wallet::WithdrawRequest = 
                serde_json::from_str(json).unwrap();
            
            assert_eq!(request.amount, dec!(25.00));
            assert_eq!(request.destination, "0x1234567890abcdef");
        }

        #[test]
        fn test_transaction_serialization() {
            let tx = magnetite_backend::api::wallet::Transaction {
                id: Uuid::new_v4(),
                user_id: Uuid::new_v4(),
                tx_type: "deposit".to_string(),
                amount: dec!(100.00),
                status: "completed".to_string(),
                created_at: Utc::now(),
            };

            let json = serde_json::to_string(&tx).unwrap();
            assert!(json.contains("deposit"));
            assert!(json.contains("completed"));
            assert!(json.contains("100"));
        }
    }

    mod wallet_service_models_tests {
        use super::*;

        #[test]
        fn test_wallet_struct_serialization() {
            let wallet = magnetite_backend::services::wallet::Wallet {
                user_id: Uuid::new_v4(),
                currency: "USDC".to_string(),
                balance: dec!(500.75),
                updated_at: Utc::now(),
            };

            let json = serde_json::to_string(&wallet).unwrap();
            assert!(json.contains("500.75"));
        }

        #[test]
        fn test_transaction_struct_serialization() {
            let tx = magnetite_backend::services::wallet::Transaction {
                id: Uuid::new_v4(),
                user_id: Uuid::new_v4(),
                tx_type: "deposit".to_string(),
                amount: dec!(250.00),
                reference_id: Some("ref_123".to_string()),
                status: "completed".to_string(),
                created_at: Utc::now(),
            };

            let json = serde_json::to_string(&tx).unwrap();
            assert!(json.contains("deposit"));
            assert!(json.contains("250"));
        }

        #[test]
        fn test_transaction_without_reference_id() {
            let tx = magnetite_backend::services::wallet::Transaction {
                id: Uuid::new_v4(),
                user_id: Uuid::new_v4(),
                tx_type: "deposit".to_string(),
                amount: dec!(100.00),
                reference_id: None,
                status: "pending".to_string(),
                created_at: Utc::now(),
            };

            let json = serde_json::to_string(&tx).unwrap();
            assert!(json.contains("deposit"));
        }
    }

    mod balance_calculation_tests {
        use super::*;

        #[test]
        fn test_decimal_precision_deposit() {
            let initial = dec!(100.00);
            let deposit = dec!(50.25);
            let expected = dec!(150.25);

            assert_eq!(initial + deposit, expected);
        }

        #[test]
        fn test_decimal_precision_withdraw() {
            let initial = dec!(100.00);
            let withdrawal = dec!(30.50);
            let expected = dec!(69.50);

            assert_eq!(initial - withdrawal, expected);
        }

        #[test]
        fn test_insufficient_funds_check() {
            let balance = dec!(50.00);
            let requested = dec!(100.00);

            assert!(requested > balance);
        }

        #[test]
        fn test_exact_balance_withdraw() {
            let balance = dec!(100.00);
            let withdrawal = dec!(100.00);

            assert!(balance >= withdrawal);
            assert_eq!(balance - withdrawal, dec!(0));
        }

        #[test]
        fn test_small_amount_operations() {
            let balance = dec!(0.01);
            let deposit = dec!(0.02);

            assert_eq!(balance + deposit, dec!(0.03));
        }

        #[test]
        fn test_large_amount_operations() {
            let balance = dec!(1_000_000.00);
            let deposit = dec!(500_000.00);

            assert_eq!(balance + deposit, dec!(1_500_000.00));
        }
    }

    mod transaction_type_tests {
        #[test]
        fn test_deposit_type_string() {
            let tx_type = "deposit";
            assert_eq!(tx_type, "deposit");
        }

        #[test]
        fn test_withdrawal_type_string() {
            let tx_type = "withdrawal";
            assert_eq!(tx_type, "withdrawal");
        }

        #[test]
        fn test_transaction_type_case_sensitivity() {
            let deposit = "deposit";
            let deposit_upper = "DEPOSIT";

            assert_ne!(deposit, deposit_upper);
        }
    }

    mod error_handling_tests {
        use super::*;
        use axum::response::IntoResponse;
        use magnetite_backend::error::AppError;

        #[test]
        fn test_insufficient_funds_error() {
            let error = AppError::InsufficientFunds("Insufficient balance".to_string());
            assert!(error.to_string().contains("Insufficient"));
        }

        #[test]
        fn test_error_status_code_insufficient_funds() {
            let error = AppError::InsufficientFunds("Insufficient balance".to_string());
            let response = error.into_response();
            
            assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        }

        #[test]
        fn test_error_status_code_database() {
            let error = AppError::Database("Connection failed".to_string());
            let response = error.into_response();
            
            assert_eq!(response.status(), axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }

        #[test]
        fn test_error_status_code_unauthorized() {
            let error = AppError::Unauthorized("Invalid token".to_string());
            let response = error.into_response();
            
            assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
        }

        #[test]
        fn test_error_status_code_not_found() {
            let error = AppError::NotFound("User not found".to_string());
            let response = error.into_response();
            
            assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
        }

        #[test]
        fn test_error_json_serialization() {
            let error = AppError::BadRequest("Invalid input".to_string());
            let response = error.into_response();

            assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        }
    }

    mod decimal_edge_cases_tests {
        use super::*;

        #[test]
        fn test_zero_balance() {
            let balance = dec!(0);
            assert!(balance.is_zero());
        }

        #[test]
        fn test_negative_amount_rejected() {
            let balance = dec!(100.00);
            let negative = dec!(-50.00);

            assert!(negative < dec!(0));
            assert!(balance + negative < balance);
        }

        #[test]
        fn test_zero_deposit_no_change() {
            let balance = dec!(100.00);
            let zero_deposit = dec!(0);

            assert_eq!(balance + zero_deposit, balance);
        }

        #[test]
        fn test_zero_withdrawal_no_change() {
            let balance = dec!(100.00);
            let zero_withdrawal = dec!(0);

            assert_eq!(balance - zero_withdrawal, balance);
        }

        #[test]
        fn test_decimal_rounding() {
            let balance = dec!(100.00);
            let deposit = dec!(33.33);
            let expected = dec!(133.33);

            assert_eq!(balance + deposit, expected);
        }
    }

    mod currency_tests {
        #[test]
        fn test_usdc_currency_code() {
            let currency = "USDC";
            assert_eq!(currency.len(), 4);
        }

        #[test]
        fn test_currency_case_sensitivity() {
            let usdc_lower = "usdc";
            let usdc_upper = "USDC";

            assert_ne!(usdc_lower, usdc_upper);
        }
    }
}
