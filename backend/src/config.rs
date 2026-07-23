// Config — loads platform settings from environment variables; used by lib and main.
#![allow(dead_code)]

use std::env;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub frontend_url: String,
    pub jwt_secret: String,
    pub access_token_expiry: i64,
    pub refresh_token_expiry: i64,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub discord_client_id: String,
    pub discord_client_secret: String,
    pub github_client_id: String,
    pub github_client_secret: String,
    pub gitlab_client_id: String,
    pub gitlab_client_secret: String,
    /// Payment rail selector (`PAYMENT_RAIL`). Default `mock` — deterministic,
    /// offline, zero external services. Any other value currently falls back to
    /// the mock rail with a warning (see TODO(chain) in services/payment.rs).
    pub payment_rail: String,
    /// Protocol fee in basis points (`PROTOCOL_FEE_BPS`). Default 0.
    pub protocol_fee_bps: u16,
    /// Operator wallet that receives hosting/subscription fees (`OPERATOR_WALLET_PUBKEY`).
    pub operator_wallet_pubkey: Option<String>,
    /// Placeholder chain config — unused by the mock rail.
    pub chain_rpc_url: Option<String>,
    /// Placeholder chain id — unused by the mock rail.
    pub chain_id: Option<String>,
    /// Placeholder stablecoin contract address — unused by the mock rail.
    pub stablecoin_address: Option<String>,
    /// Comms provider selector (`COMMS_PROVIDER`, §3.5): `builtin` (default) |
    /// `matrix` | `jitsi` | `livekit` | `owncast`. Anything unknown, or a
    /// provider whose service is unconfigured, falls back to `builtin` — the
    /// demoted in-house stack, which needs zero external services.
    pub comms_provider: String,
    /// Matrix homeserver base URL (`MATRIX_HOMESERVER`). Gates `MatrixProvider`.
    pub matrix_homeserver: Option<String>,
    /// Matrix server name used in room aliases (`MATRIX_SERVER_NAME`).
    pub matrix_server_name: Option<String>,
    /// Shared secret for minting Matrix login tokens (`MATRIX_SHARED_SECRET`).
    /// Without it clients log in to the homeserver themselves.
    pub matrix_shared_secret: Option<String>,
    /// Jitsi Meet domain (`JITSI_DOMAIN`). Gates `JitsiProvider`.
    pub jitsi_domain: Option<String>,
    /// Jitsi JWT app id / secret (`JITSI_APP_ID` / `JITSI_JWT_SECRET`). Optional —
    /// an open Jitsi deployment needs neither.
    pub jitsi_app_id: Option<String>,
    pub jitsi_jwt_secret: Option<String>,
    /// LiveKit server URL (`LIVEKIT_URL`). Gates `LiveKitProvider`.
    pub livekit_url: Option<String>,
    pub livekit_api_key: Option<String>,
    pub livekit_api_secret: Option<String>,
    /// Owncast instance URL (`OWNCAST_URL`). Gates `OwncastProvider`.
    pub owncast_url: Option<String>,
    /// This node's OWN media server, when it runs one (`MEDIA_SERVER_BASE_URL`).
    /// Per-node, not global: rooms and streams record their own host and that
    /// value wins over this default.
    pub media_server_base_url: Option<String>,
    /// 32-byte hex seed for the node's comms IdP key (`NODE_SIGNING_SEED`).
    /// Unset → an ephemeral key is generated at boot.
    pub node_signing_seed: Option<String>,
    pub email_provider: String,
    pub resend_api_key: Option<String>,
    pub app_name: String,
    pub app_env: String,
    pub app_url: String,
    pub redis_url: String,
    /// Base URL for this server's own game WebSocket, e.g. `ws://api.magnetite.gg`.
    /// Used by matchmaking to set `server_endpoint` on new sessions.
    /// Defaults to `ws://localhost:8080`.
    pub game_server_ws_base: String,
    /// Anti-cheat: maximum speed (units/s) before a velocity violation is flagged.
    /// Defaults to 50.0 (same as `MAX_VEHICLE_SPEED` in the detection logic).
    pub anticheat_max_velocity: f64,
    /// Anti-cheat: maximum input rate (inputs/second) before flagging high severity.
    /// Defaults to 50.0.
    pub anticheat_max_input_rate: f64,
}

impl Config {
    pub fn from_env() -> Self {
        let app_env = env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
        let _is_production = app_env == "production";

        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .expect("SERVER_PORT must be a valid port number"),
            frontend_url: env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            access_token_expiry: env::var("ACCESS_TOKEN_EXPIRY")
                .unwrap_or_else(|_| "900".to_string())
                .parse()
                .expect("ACCESS_TOKEN_EXPIRY must be a valid number"),
            refresh_token_expiry: env::var("REFRESH_TOKEN_EXPIRY")
                .unwrap_or_else(|_| "604800".to_string())
                .parse()
                .expect("REFRESH_TOKEN_EXPIRY must be a valid number"),
            google_client_id: env::var("GOOGLE_CLIENT_ID").unwrap_or_else(|_| "".to_string()),
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET")
                .unwrap_or_else(|_| "".to_string()),
            discord_client_id: env::var("DISCORD_CLIENT_ID").unwrap_or_else(|_| "".to_string()),
            discord_client_secret: env::var("DISCORD_CLIENT_SECRET")
                .unwrap_or_else(|_| "".to_string()),
            github_client_id: env::var("GITHUB_CLIENT_ID").unwrap_or_else(|_| "".to_string()),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET")
                .unwrap_or_else(|_| "".to_string()),
            gitlab_client_id: env::var("GITLAB_CLIENT_ID").unwrap_or_else(|_| "".to_string()),
            gitlab_client_secret: env::var("GITLAB_CLIENT_SECRET")
                .unwrap_or_else(|_| "".to_string()),
            payment_rail: env::var("PAYMENT_RAIL").unwrap_or_else(|_| "mock".to_string()),
            protocol_fee_bps: env::var("PROTOCOL_FEE_BPS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            operator_wallet_pubkey: env::var("OPERATOR_WALLET_PUBKEY").ok(),
            chain_rpc_url: env::var("CHAIN_RPC_URL").ok(),
            chain_id: env::var("CHAIN_ID").ok(),
            stablecoin_address: env::var("STABLECOIN_ADDRESS").ok(),
            comms_provider: env::var("COMMS_PROVIDER").unwrap_or_else(|_| "builtin".to_string()),
            matrix_homeserver: env::var("MATRIX_HOMESERVER").ok(),
            matrix_server_name: env::var("MATRIX_SERVER_NAME").ok(),
            matrix_shared_secret: env::var("MATRIX_SHARED_SECRET").ok(),
            jitsi_domain: env::var("JITSI_DOMAIN").ok(),
            jitsi_app_id: env::var("JITSI_APP_ID").ok(),
            jitsi_jwt_secret: env::var("JITSI_JWT_SECRET").ok(),
            livekit_url: env::var("LIVEKIT_URL").ok(),
            livekit_api_key: env::var("LIVEKIT_API_KEY").ok(),
            livekit_api_secret: env::var("LIVEKIT_API_SECRET").ok(),
            owncast_url: env::var("OWNCAST_URL").ok(),
            media_server_base_url: env::var("MEDIA_SERVER_BASE_URL").ok(),
            node_signing_seed: env::var("NODE_SIGNING_SEED").ok(),
            email_provider: env::var("EMAIL_PROVIDER").unwrap_or_else(|_| "resend".to_string()),
            resend_api_key: env::var("RESEND_API_KEY").ok(),
            app_name: env::var("APP_NAME").unwrap_or_else(|_| "Magnetite".to_string()),
            app_env,
            app_url: env::var("APP_URL").unwrap_or_else(|_| "http://localhost:8080".to_string()),
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost".to_string()),
            game_server_ws_base: env::var("GAME_SERVER_WS_BASE")
                .unwrap_or_else(|_| "ws://localhost:8080".to_string()),
            anticheat_max_velocity: env::var("ANTICHEAT_MAX_VELOCITY")
                .unwrap_or_else(|_| "50.0".to_string())
                .parse()
                .unwrap_or(50.0),
            anticheat_max_input_rate: env::var("ANTICHEAT_MAX_INPUT_RATE")
                .unwrap_or_else(|_| "50.0".to_string())
                .parse()
                .unwrap_or(50.0),
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.app_env == "production" {
            if self.jwt_secret.len() < 32 {
                return Err(ConfigError::Validation(
                    "JWT_SECRET must be at least 32 characters in production".to_string(),
                ));
            }
            if self.database_url.is_empty() {
                return Err(ConfigError::Validation(
                    "DATABASE_URL must be set in production".to_string(),
                ));
            }
            if self.app_url.is_empty() || self.app_url == "http://localhost:8080" {
                return Err(ConfigError::Validation(
                    "APP_URL must be set to a non-localhost URL in production".to_string(),
                ));
            }
        }
        Ok(())
    }

    pub fn is_production(&self) -> bool {
        self.app_env == "production"
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Environment variable error: {0}")]
    Env(#[from] env::VarError),
    #[error("Configuration validation error: {0}")]
    Validation(String),
}

pub fn get_jwt_secret() -> String {
    env::var("JWT_SECRET").expect("JWT_SECRET must be set")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        temp_env::with_var("DATABASE_URL", Some("postgres://localhost/test"), || {
            temp_env::with_var(
                "JWT_SECRET",
                Some("test-secret-at-least-32-characters-long"),
                || {
                    temp_env::with_var("APP_ENV", Some("development"), || {
                        let config = Config::from_env();
                        assert_eq!(config.server_port, 8080);
                        assert_eq!(config.server_host, "0.0.0.0");
                        assert_eq!(config.email_provider, "resend");
                        assert_eq!(config.app_name, "Magnetite");
                        assert_eq!(config.app_env, "development");
                    });
                },
            );
        });
    }

    #[test]
    fn test_production_validation_fails_short_jwt() {
        temp_env::with_var("DATABASE_URL", Some("postgres://localhost/test"), || {
            temp_env::with_var("JWT_SECRET", Some("short"), || {
                temp_env::with_var("APP_ENV", Some("production"), || {
                    temp_env::with_var("APP_URL", Some("https://example.com"), || {
                        let config = Config::from_env();
                        let result = config.validate();
                        assert!(result.is_err());
                    });
                });
            });
        });
    }

    #[test]
    fn test_production_validation_fails_localhost_url() {
        temp_env::with_var("DATABASE_URL", Some("postgres://localhost/test"), || {
            temp_env::with_var(
                "JWT_SECRET",
                Some("test-secret-at-least-32-characters-long"),
                || {
                    temp_env::with_var("APP_ENV", Some("production"), || {
                        temp_env::with_var("APP_URL", Some("http://localhost:8080"), || {
                            let config = Config::from_env();
                            let result = config.validate();
                            assert!(result.is_err());
                        });
                    });
                },
            );
        });
    }

    #[test]
    fn test_production_validation_passes() {
        temp_env::with_var("DATABASE_URL", Some("postgres://localhost/test"), || {
            temp_env::with_var(
                "JWT_SECRET",
                Some("test-secret-at-least-32-characters-long"),
                || {
                    temp_env::with_var("APP_ENV", Some("production"), || {
                        temp_env::with_var("APP_URL", Some("https://example.com"), || {
                            let config = Config::from_env();
                            assert!(config.validate().is_ok());
                            assert!(config.is_production());
                        });
                    });
                },
            );
        });
    }
}
