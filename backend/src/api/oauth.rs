use axum::{
    extract::{Query, State},
    http::{HeaderMap, HeaderValue},
    response::{IntoResponse, Redirect},
    routing::get,
    Router,
};
use base64;
use rand::Rng;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::services::session::AuthTokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthProvider {
    Google,
    Discord,
    GitHub,
    GitLab,
}

impl OAuthProvider {
    pub fn auth_url(&self) -> &'static str {
        match self {
            Self::Google => "https://accounts.google.com/o/oauth2/v2/auth",
            Self::Discord => "https://discord.com/api/oauth2/authorize",
            Self::GitHub => "https://github.com/login/oauth/authorize",
            Self::GitLab => "https://gitlab.com/oauth/authorize",
        }
    }

    pub fn token_url(&self) -> &'static str {
        match self {
            Self::Google => "https://oauth2.googleapis.com/token",
            Self::Discord => "https://discord.com/api/oauth2/token",
            Self::GitHub => "https://github.com/login/oauth/access_token",
            Self::GitLab => "https://gitlab.com/oauth/token",
        }
    }

    pub fn user_info_url(&self) -> &'static str {
        match self {
            Self::Google => "https://www.googleapis.com/oauth2/v2/userinfo",
            Self::Discord => "https://discord.com/api/users/@me",
            Self::GitHub => "https://api.github.com/user",
            Self::GitLab => "https://gitlab.com/api/v4/user",
        }
    }

    pub fn scope(&self) -> &'static str {
        match self {
            Self::Google => "openid email profile",
            Self::Discord => "identify email",
            Self::GitHub => "user:email read:user",
            Self::GitLab => "read_user",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    pub id: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DiscordUserInfo {
    pub id: String,
    pub email: String,
    pub username: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubUserInfo {
    pub id: i64,
    pub email: Option<String>,
    pub login: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GitLabUserInfo {
    pub id: i64,
    pub email: String,
    pub username: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<i32>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
}

pub fn create_oauth_url(provider: OAuthProvider, config: &OAuthConfig) -> (String, String) {
    let state: String = (0..32)
        .map(|_| {
            let idx = rand::thread_rng().gen_range(0..62);
            let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
            chars[idx] as char
        })
        .collect();

    let state_clone = state.clone();
    let redirect_uri = &config.redirect_uri;

    let mut url = format!("{}?", provider.auth_url());
    url.push_str(&format!("client_id={}", config.client_id));
    url.push_str(&format!(
        "&redirect_uri={}",
        urlencoding::encode(redirect_uri)
    ));
    url.push_str("&response_type=code");
    url.push_str(&format!("&scope={}", provider.scope()));
    url.push_str(&format!("&state={}", state));

    if provider == OAuthProvider::GitLab {
        url.push_str("&grant_type=authorization_code");
    }

    (url, state_clone)
}

pub async fn exchange_code(
    provider: OAuthProvider,
    config: &OAuthConfig,
    code: &str,
) -> Result<String> {
    let client = reqwest::Client::new();
    let redirect_uri = &config.redirect_uri;

    let params = match provider {
        OAuthProvider::Google => {
            let mut map = std::collections::HashMap::new();
            map.insert("code", code);
            map.insert("client_id", &config.client_id);
            map.insert("client_secret", &config.client_secret);
            map.insert("redirect_uri", redirect_uri);
            map.insert("grant_type", "authorization_code");
            map
        }
        OAuthProvider::Discord | OAuthProvider::GitHub | OAuthProvider::GitLab => {
            let mut map = std::collections::HashMap::new();
            map.insert("code", code);
            map.insert("client_id", &config.client_id);
            map.insert("client_secret", &config.client_secret);
            map.insert("redirect_uri", redirect_uri);
            map.insert("grant_type", "authorization_code");
            map
        }
    };

    let response = client
        .post(provider.token_url())
        .form(&params)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("OAuth token exchange failed: {}", e)))?;

    let text = response
        .text()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to read token response: {}", e)))?;

    let token_response: TokenResponse = if provider == OAuthProvider::GitHub {
        serde_json::from_str(&text)
            .map_err(|e| AppError::Internal(format!("Failed to parse GitHub token: {}", e)))?
    } else {
        serde_json::from_str(&text)
            .map_err(|e| AppError::Internal(format!("Failed to parse token response: {}", e)))?
    };

    Ok(token_response.access_token)
}

pub async fn fetch_user_info(
    provider: OAuthProvider,
    access_token: &str,
) -> Result<(String, String)> {
    let client = reqwest::Client::new();

    let headers = {
        let mut h = HeaderMap::new();
        h.insert(
            "Authorization",
            format!("Bearer {}", access_token).parse().unwrap(),
        );
        h
    };

    match provider {
        OAuthProvider::Google => {
            let user: GoogleUserInfo = client
                .get(provider.user_info_url())
                .headers(headers)
                .send()
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to fetch Google user info: {}", e))
                })?
                .json()
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to parse Google user info: {}", e))
                })?;
            Ok((user.email, user.name))
        }
        OAuthProvider::Discord => {
            let user: DiscordUserInfo = client
                .get(provider.user_info_url())
                .headers(headers)
                .send()
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to fetch Discord user info: {}", e))
                })?
                .json()
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to parse Discord user info: {}", e))
                })?;
            Ok((user.email, user.username))
        }
        OAuthProvider::GitHub => {
            let user: GitHubUserInfo = client
                .get(provider.user_info_url())
                .headers(headers.clone())
                .send()
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to fetch GitHub user info: {}", e))
                })?
                .json()
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to parse GitHub user info: {}", e))
                })?;

            let email = if let Some(email) = user.email {
                email
            } else {
                let emails: Vec<serde_json::Value> = client
                    .get("https://api.github.com/user/emails")
                    .headers(headers)
                    .send()
                    .await
                    .map_err(|e| {
                        AppError::Internal(format!("Failed to fetch GitHub emails: {}", e))
                    })?
                    .json()
                    .await
                    .map_err(|e| {
                        AppError::Internal(format!("Failed to parse GitHub emails: {}", e))
                    })?;
                emails
                    .iter()
                    .find(|e| e.get("primary").and_then(|v| v.as_bool()).unwrap_or(false))
                    .and_then(|e| e.get("email").and_then(|v| v.as_str()))
                    .unwrap_or("")
                    .to_string()
            };

            Ok((email, user.login))
        }
        OAuthProvider::GitLab => {
            let user: GitLabUserInfo = client
                .get(provider.user_info_url())
                .headers(headers)
                .send()
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to fetch GitLab user info: {}", e))
                })?
                .json()
                .await
                .map_err(|e| {
                    AppError::Internal(format!("Failed to parse GitLab user info: {}", e))
                })?;
            Ok((user.email, user.username))
        }
    }
}

pub async fn handle_callback(
    provider: OAuthProvider,
    config: &OAuthConfig,
    pool: &PgPool,
    code: &str,
) -> Result<crate::services::session::AuthTokens> {
    let access_token = exchange_code(provider, config, code).await?;
    let (email, username) = fetch_user_info(provider, &access_token).await?;

    let existing_user = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(pool)
        .await?;

    let user_id = if let Some((user_id,)) = existing_user {
        sqlx::query("UPDATE users SET username = $1 WHERE id = $2")
            .bind(&username)
            .bind(user_id)
            .execute(pool)
            .await?;
        user_id
    } else {
        let new_user_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users (id, username, email, created_at) VALUES ($1, $2, $3, NOW())",
        )
        .bind(new_user_id)
        .bind(&username)
        .bind(&email)
        .execute(pool)
        .await?;
        new_user_id
    };

    crate::services::session::create_session(pool, user_id, &email, None, None).await
}

pub async fn google_login() -> impl IntoResponse {
    let config = OAuthConfig {
        client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
        redirect_uri: std::env::var("GOOGLE_REDIRECT_URI").unwrap_or_default(),
    };
    let (url, state) = create_oauth_url(OAuthProvider::Google, &config);

    let cookie = format!(
        "oauth_state={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=600",
        state
    );

    let mut response = Redirect::to(&url).into_response();
    let headers = response.headers_mut();
    headers.insert(
        "Set-Cookie",
        HeaderValue::from_str(&cookie).expect("valid cookie header value"),
    );
    response
}

pub async fn google_callback(
    State(pool): State<PgPool>,
    Query(query): Query<CallbackQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let cookie_state = headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookie_str| {
            cookie_str
                .split(';')
                .find_map(|c| {
                    let mut parts = c.trim().splitn(2, '=');
                    if parts.next() == Some("oauth_state") {
                        parts.next().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
        });

    let state_valid = cookie_state
        .as_ref()
        .map(|s| s == &query.state)
        .unwrap_or(false);

    if !state_valid {
        let frontend_url =
            std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
        let redirect_url = format!("{}/auth/error?reason=invalid_state", frontend_url);
        return Redirect::to(&redirect_url).into_response();
    }

    let config = OAuthConfig {
        client_id: std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
        redirect_uri: std::env::var("GOOGLE_REDIRECT_URI").unwrap_or_default(),
    };

    match handle_google_callback(&config, &pool, &query.code).await {
        Ok(tokens) => {
            let frontend_url =
                std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
            let redirect_url = format!(
                "{}/auth/callback?access_token={}&refresh_token={}&expires_at={}",
                frontend_url,
                tokens.access_token,
                tokens.refresh_token,
                tokens.expires_at.timestamp()
            );
            Redirect::to(&redirect_url).into_response()
        }
        Err(e) => {
            let frontend_url =
                std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
            let redirect_url = format!("{}/auth/error?reason=callback_error", frontend_url);
            Redirect::to(&redirect_url).into_response()
        }
    }
}

pub async fn handle_google_callback(
    config: &OAuthConfig,
    pool: &PgPool,
    code: &str,
) -> Result<AuthTokens> {
    let access_token = exchange_google_code(config, code).await?;
    let user_info = decode_google_id_token(&access_token)?;

    let existing_user = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM users WHERE email = $1")
        .bind(&user_info.email)
        .fetch_optional(pool)
        .await?;

    let user_id = if let Some((user_id,)) = existing_user {
        sqlx::query("UPDATE users SET username = $1, avatar_url = $2 WHERE id = $3")
            .bind(&user_info.name)
            .bind(&user_info.picture)
            .bind(user_id)
            .execute(pool)
            .await?;
        user_id
    } else {
        let new_user_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users (id, username, email, avatar_url, created_at) VALUES ($1, $2, $3, $4, NOW())",
        )
        .bind(new_user_id)
        .bind(&user_info.name)
        .bind(&user_info.email)
        .bind(&user_info.picture)
        .execute(pool)
        .await?;
        new_user_id
    };

    crate::services::session::create_session(pool, user_id, &user_info.email, None, None).await
}

pub async fn exchange_google_code(config: &OAuthConfig, code: &str) -> Result<String> {
    let client = reqwest::Client::new();

    let mut params = std::collections::HashMap::new();
    params.insert("code", code);
    params.insert("client_id", &config.client_id);
    params.insert("client_secret", &config.client_secret);
    params.insert("redirect_uri", &config.redirect_uri);
    params.insert("grant_type", "authorization_code");

    let response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Google token exchange failed: {}", e)))?;

    let text = response
        .text()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to read Google token response: {}", e)))?;

    let token_response: TokenResponse = serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("Failed to parse Google token response: {}", e)))?;

    token_response
        .id_token
        .ok_or_else(|| AppError::Internal("No id_token in Google response".to_string()))
}

pub fn decode_google_id_token(id_token: &str) -> Result<GoogleUserInfo> {
    let parts: Vec<&str> = id_token.split('.').collect();
    if parts.len() != 3 {
        return Err(AppError::Internal("Invalid id_token format".to_string()));
    }

    let payload = parts[1];
    let decoded = base64::decode(payload)
        .map_err(|e| AppError::Internal(format!("Failed to decode id_token: {}", e)))?;

    let payload_str = String::from_utf8(decoded)
        .map_err(|e| AppError::Internal(format!("Invalid utf8 in id_token: {}", e)))?;

    #[derive(Deserialize)]
    struct Claims {
        email: String,
        name: String,
        picture: Option<String>,
        sub: String,
    }

    let claims: Claims = serde_json::from_str(&payload_str)
        .map_err(|e| AppError::Internal(format!("Failed to parse id_token claims: {}", e)))?;

    Ok(GoogleUserInfo {
        id: claims.sub,
        email: claims.email,
        name: claims.name,
        picture: claims.picture,
    })
}

pub fn create_google_url(config: &OAuthConfig) -> (String, String) {
    create_oauth_url(OAuthProvider::Google, config)
}

pub async fn get_google_user(access_token: &str) -> Result<GoogleUserInfo> {
    let client = reqwest::Client::new();
    let headers = {
        let mut h = HeaderMap::new();
        h.insert(
            "Authorization",
            format!("Bearer {}", access_token).parse().unwrap(),
        );
        h
    };

    let user: GoogleUserInfo = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .headers(headers)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch Google user info: {}", e)))?
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse Google user info: {}", e)))?;

    Ok(user)
}

pub async fn discord_login() -> impl IntoResponse {
    let config = OAuthConfig {
        client_id: std::env::var("DISCORD_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("DISCORD_CLIENT_SECRET").unwrap_or_default(),
        redirect_uri: std::env::var("DISCORD_REDIRECT_URI").unwrap_or_default(),
    };
    let (url, _) = create_oauth_url(OAuthProvider::Discord, &config);
    axum::response::Redirect::to(&url)
}

pub async fn discord_callback(
    State(pool): State<PgPool>,
    Query(query): Query<CallbackQuery>,
) -> impl IntoResponse {
    let config = OAuthConfig {
        client_id: std::env::var("DISCORD_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("DISCORD_CLIENT_SECRET").unwrap_or_default(),
        redirect_uri: std::env::var("DISCORD_REDIRECT_URI").unwrap_or_default(),
    };
    match handle_callback(OAuthProvider::Discord, &config, &pool, &query.code).await {
        Ok(tokens) => axum::response::Json(tokens).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn github_login() -> impl IntoResponse {
    let config = OAuthConfig {
        client_id: std::env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("GITHUB_CLIENT_SECRET").unwrap_or_default(),
        redirect_uri: std::env::var("GITHUB_REDIRECT_URI").unwrap_or_default(),
    };
    let (url, _) = create_oauth_url(OAuthProvider::GitHub, &config);
    axum::response::Redirect::to(&url)
}

pub async fn github_callback(
    State(pool): State<PgPool>,
    Query(query): Query<CallbackQuery>,
) -> impl IntoResponse {
    let config = OAuthConfig {
        client_id: std::env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("GITHUB_CLIENT_SECRET").unwrap_or_default(),
        redirect_uri: std::env::var("GITHUB_REDIRECT_URI").unwrap_or_default(),
    };
    match handle_callback(OAuthProvider::GitHub, &config, &pool, &query.code).await {
        Ok(tokens) => axum::response::Json(tokens).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn gitlab_login() -> impl IntoResponse {
    let config = OAuthConfig {
        client_id: std::env::var("GITLAB_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("GITLAB_CLIENT_SECRET").unwrap_or_default(),
        redirect_uri: std::env::var("GITLAB_REDIRECT_URI").unwrap_or_default(),
    };
    let (url, _) = create_oauth_url(OAuthProvider::GitLab, &config);
    axum::response::Redirect::to(&url)
}

pub async fn gitlab_callback(
    State(pool): State<PgPool>,
    Query(query): Query<CallbackQuery>,
) -> impl IntoResponse {
    let config = OAuthConfig {
        client_id: std::env::var("GITLAB_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("GITLAB_CLIENT_SECRET").unwrap_or_default(),
        redirect_uri: std::env::var("GITLAB_REDIRECT_URI").unwrap_or_default(),
    };
    match handle_callback(OAuthProvider::GitLab, &config, &pool, &query.code).await {
        Ok(tokens) => axum::response::Json(tokens).into_response(),
        Err(e) => e.into_response(),
    }
}

pub fn create_github_url() -> (String, String) {
    let config = OAuthConfig {
        client_id: std::env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("GITHUB_CLIENT_SECRET").unwrap_or_default(),
        redirect_uri: std::env::var("GITHUB_REDIRECT_URI").unwrap_or_default(),
    };
    create_oauth_url(OAuthProvider::GitHub, &config)
}

pub async fn exchange_github_code(code: &str) -> Result<String> {
    let config = OAuthConfig {
        client_id: std::env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("GITHUB_CLIENT_SECRET").unwrap_or_default(),
        redirect_uri: std::env::var("GITHUB_REDIRECT_URI").unwrap_or_default(),
    };
    exchange_code(OAuthProvider::GitHub, &config, code).await
}

pub async fn get_github_user(access_token: &str) -> Result<(String, String)> {
    fetch_user_info(OAuthProvider::GitHub, access_token).await
}

pub async fn handle_github_callback(pool: &PgPool, code: &str) -> Result<crate::services::session::AuthTokens> {
    let config = OAuthConfig {
        client_id: std::env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
        client_secret: std::env::var("GITHUB_CLIENT_SECRET").unwrap_or_default(),
        redirect_uri: std::env::var("GITHUB_REDIRECT_URI").unwrap_or_default(),
    };
    handle_callback(OAuthProvider::GitHub, &config, pool, code).await
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/google", get(google_login))
        .route("/google/callback", get(google_callback))
        .route("/discord", get(discord_login))
        .route("/discord/callback", get(discord_callback))
        .route("/github", get(github_login))
        .route("/github/callback", get(github_callback))
        .route("/gitlab", get(gitlab_login))
        .route("/gitlab/callback", get(gitlab_callback))
        .with_state(pool)
}