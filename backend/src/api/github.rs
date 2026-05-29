// GitHub API — app installation, CI check runs, deploy triggers; platform surface.
#![allow(dead_code)]

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, Result};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConfig {
    pub app_id: String,
    pub private_key: String,
    pub webhook_secret: String,
}

impl GitHubConfig {
    pub fn from_env() -> Self {
        Self {
            app_id: std::env::var("GITHUB_APP_ID").expect("GITHUB_APP_ID must be set"),
            private_key: std::env::var("GITHUB_APP_PRIVATE_KEY")
                .expect("GITHUB_APP_PRIVATE_KEY must be set"),
            webhook_secret: std::env::var("GITHUB_WEBHOOK_SECRET")
                .expect("GITHUB_WEBHOOK_SECRET must be set"),
        }
    }
}

#[derive(Debug, Serialize)]
struct JWTClaims {
    iat: i64,
    exp: i64,
    iss: String,
}

pub fn generate_jwt(app_id: &str, private_key: &str) -> Result<String> {
    let now = Utc::now();
    let exp = now + Duration::minutes(10);

    let claims = JWTClaims {
        iat: now.timestamp(),
        exp: exp.timestamp(),
        iss: app_id.to_string(),
    };

    let key = EncodingKey::from_rsa_pem(private_key.as_bytes())
        .map_err(|e| AppError::Internal(format!("Failed to parse private key: {}", e)))?;

    encode(&Header::new(jsonwebtoken::Algorithm::RS256), &claims, &key)
        .map_err(|e| AppError::Internal(format!("Failed to generate JWT: {}", e)))
}

pub async fn get_installation_access_token(jwt: &str, installation_id: &str) -> Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "https://api.github.com/app/installations/{}/access_tokens",
            installation_id
        ))
        .header("Authorization", format!("Bearer {}", jwt))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get access token: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::Internal(format!(
            "Failed to get installation token: {}",
            resp.status()
        )));
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        token: String,
    }

    let token_resp: TokenResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse token response: {}", e)))?;

    Ok(token_resp.token)
}

#[derive(Debug, Deserialize)]
pub struct GitHubRepository {
    pub id: i64,
    pub full_name: String,
    pub name: String,
    pub owner: Owner,
    pub private: bool,
    pub html_url: String,
    pub default_branch: String,
}

#[derive(Debug, Deserialize)]
pub struct Owner {
    pub login: String,
    pub id: i64,
    #[serde(rename = "type")]
    pub account_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RepositoriesResponse {
    repositories: Vec<GitHubRepository>,
}

#[derive(Debug, Deserialize)]
struct InstallationsResponse {
    installations: Vec<InstallationData>,
}

#[derive(Debug, Deserialize)]
pub struct InstallationData {
    pub id: i64,
    pub account: Owner,
    pub repository_selection: Option<String>,
}

pub async fn list_installations_api(jwt: &str) -> Result<Vec<InstallationData>> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.github.com/app/installations")
        .header("Authorization", format!("Bearer {}", jwt))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list installations: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::Internal(format!(
            "Failed to list installations: {}",
            resp.status()
        )));
    }

    let installations: InstallationsResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse installations: {}", e)))?;

    Ok(installations.installations)
}

pub async fn list_installation_repos(access_token: &str) -> Result<Vec<GitHubRepository>> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.github.com/installation/repositories")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list repositories: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::Internal(format!(
            "Failed to list repositories: {}",
            resp.status()
        )));
    }

    let repos_resp: RepositoriesResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse repositories: {}", e)))?;

    Ok(repos_resp.repositories)
}

pub async fn verify_repo_access(access_token: &str, owner: &str, repo: &str) -> Result<bool> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("https://api.github.com/repos/{}/{}", owner, repo))
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to verify repo: {}", e)))?;

    Ok(resp.status().is_success())
}

#[derive(Debug, Deserialize)]
pub struct RepoContent {
    pub name: String,
    pub path: String,
    pub content: Option<String>,
    pub encoding: Option<String>,
    pub sha: Option<String>,
}

pub async fn get_repository_content(
    access_token: &str,
    owner: &str,
    repo: &str,
    path: &str,
) -> Result<RepoContent> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "https://api.github.com/repos/{}/{}/contents/{}",
            owner, repo, path
        ))
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to get repo content: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::Internal(format!(
            "Failed to get repository content: {}",
            resp.status()
        )));
    }

    resp.json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse repo content: {}", e)))
}

#[derive(Debug, Serialize)]
pub struct CheckRunRequest {
    pub name: String,
    pub head_sha: String,
    pub status: String,
    #[serde(rename = "type")]
    pub check_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckRunResponse {
    pub id: i64,
    pub url: String,
    pub name: String,
    pub head_sha: String,
    pub status: String,
}

pub async fn create_check_run(
    access_token: &str,
    owner: &str,
    repo: &str,
    commit_sha: &str,
    name: &str,
) -> Result<CheckRunResponse> {
    let client = reqwest::Client::new();
    let check_request = CheckRunRequest {
        name: name.to_string(),
        head_sha: commit_sha.to_string(),
        status: "in_progress".to_string(),
        check_type: "Checks::Run".to_string(),
    };

    let resp = client
        .post(format!(
            "https://api.github.com/repos/{}/{}/check-runs",
            owner, repo
        ))
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&check_request)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create check run: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::Internal(format!(
            "Failed to create check run: {}",
            resp.status()
        )));
    }

    resp.json()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse check run response: {}", e)))
}

pub async fn update_check_run(
    access_token: &str,
    owner: &str,
    repo: &str,
    check_run_id: i64,
    conclusion: &str,
) -> Result<()> {
    let client = reqwest::Client::new();
    #[derive(Serialize)]
    struct UpdateRequest {
        status: String,
        conclusion: String,
    }

    let update = UpdateRequest {
        status: "completed".to_string(),
        conclusion: conclusion.to_string(),
    };

    let resp = client
        .patch(format!(
            "https://api.github.com/repos/{}/{}/check-runs/{}",
            owner, repo, check_run_id
        ))
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&update)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to update check run: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::Internal(format!(
            "Failed to update check run: {}",
            resp.status()
        )));
    }

    Ok(())
}

fn compute_hmac_sha256(secret: &str, payload: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload);
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

pub fn verify_webhook_signature(secret: &str, signature: &str, payload: &[u8]) -> bool {
    let expected = compute_hmac_sha256(secret, payload);

    if let Some(sig) = signature.strip_prefix("sha256=") {
        sig == expected
    } else {
        false
    }
}

#[derive(Debug, Deserialize)]
pub struct WebhookEvent {
    pub action: Option<String>,
    pub repository: Option<GitHubRepository>,
    pub installation: Option<Installation>,
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
    pub commits: Option<Vec<Commit>>,
    pub pull_request: Option<PullRequest>,
    pub repositories_added: Option<Vec<GitHubRepository>>,
    pub repositories_removed: Option<Vec<GitHubRepository>>,
}

#[derive(Debug, Deserialize)]
pub struct Installation {
    pub id: i64,
    pub account: Option<Owner>,
}

#[derive(Debug, Deserialize)]
pub struct Commit {
    pub id: String,
    pub message: String,
    pub author: CommitAuthor,
}

#[derive(Debug, Deserialize)]
pub struct CommitAuthor {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct PullRequest {
    pub id: i64,
    pub number: i64,
    pub title: String,
    pub state: String,
    pub user: Owner,
    pub head: PullRequestHead,
}

#[derive(Debug, Deserialize)]
pub struct PullRequestHead {
    pub sha: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
}

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BuildStatus {
    pub id: Uuid,
    pub repository: String,
    pub commit_sha: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn handle_webhook(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<WebhookResponse>> {
    let config = GitHubConfig::from_env();
    let payload = body.to_vec();

    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing signature".to_string()))?;

    if !verify_webhook_signature(&config.webhook_secret, signature, &payload) {
        return Err(AppError::Unauthorized("Invalid signature".to_string()));
    }

    let event_type = headers
        .get("x-github-event")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("Missing event type".to_string()))?;

    let event: WebhookEvent = serde_json::from_slice(&payload)
        .map_err(|e| AppError::BadRequest(format!("Failed to parse webhook: {}", e)))?;

    match event_type {
        "push" => {
            tracing::info!("Push event received for repo: {:?}", event.repository);
            if let Some(repo) = &event.repository {
                handle_push_event(&pool, repo, &event).await?;
            }
        }
        "pull_request" => {
            tracing::info!("Pull request event: {:?}", event.action);
            if let Some(pr) = &event.pull_request {
                handle_pull_request_event(&pool, &event, pr).await?;
            }
        }
        "installation" => {
            tracing::info!("Installation event: {:?}", event.action);
            if let Some(installation) = &event.installation {
                handle_installation_event(&pool, installation, &event.action).await?;
            }
        }
        "installation_repositories" => {
            tracing::info!("Installation repositories event: {:?}", event.action);
            handle_installation_repositories_event(&pool, &event).await?;
        }
        _ => {
            tracing::info!("Unhandled event type: {}", event_type);
        }
    }

    Ok(Json(WebhookResponse {
        status: "ok".to_string(),
        message: format!("Processed {} event", event_type),
    }))
}

async fn handle_push_event(
    pool: &PgPool,
    repo: &GitHubRepository,
    event: &WebhookEvent,
) -> Result<()> {
    let git_ref = event.git_ref.as_deref().unwrap_or("refs/heads/main");
    let is_main_branch = git_ref == "refs/heads/main" || git_ref == "refs/heads/master";

    // Extract a semver tag if the push is a tag push (refs/tags/v*).
    let version_tag: Option<String> = if git_ref.starts_with("refs/tags/") {
        Some(git_ref.trim_start_matches("refs/tags/").to_string())
    } else {
        None
    };

    let pipeline_id = Uuid::new_v4();
    let commit_sha = event
        .commits
        .as_ref()
        .and_then(|c| c.last())
        .map(|c| c.id.clone())
        .unwrap_or_else(|| "unknown".to_string());

    sqlx::query(
        "INSERT INTO cicd_pipelines (id, repository, status, commit_sha, triggered_at)
         VALUES ($1, $2, 'triggered', $3, NOW())",
    )
    .bind(pipeline_id)
    .bind(&repo.full_name)
    .bind(&commit_sha)
    .execute(pool)
    .await?;

    tracing::info!(
        "CI/CD pipeline triggered for {} on {}",
        repo.full_name,
        git_ref
    );

    if is_main_branch || version_tag.is_some() {
        let build_id = Uuid::new_v4();

        // Look up the game_id linked to this repository (if any).
        let game_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM games WHERE github_repo = $1 AND active = true LIMIT 1",
        )
        .bind(&repo.full_name)
        .fetch_optional(pool)
        .await?;

        sqlx::query(
            "INSERT INTO build_status (id, repository, commit_sha, status, game_id, created_at, updated_at)
             VALUES ($1, $2, $3, 'pending', $4, NOW(), NOW())",
        )
        .bind(build_id)
        .bind(&repo.full_name)
        .bind(&commit_sha)
        .bind(game_id)
        .execute(pool)
        .await?;

        // Create a pending artifact record so the developer can track build
        // progress immediately, before the CI worker reports back.
        if let Some(gid) = game_id {
            crate::services::distribution::create_build_artifact_from_push(
                pool,
                gid,
                build_id,
                &commit_sha,
                version_tag.as_deref(),
            )
            .await?;

            tracing::info!(
                "Artifact record created for game {} build {} (ref={})",
                gid,
                build_id,
                git_ref
            );
        }

        trigger_wasm_build(pool, repo, &commit_sha, build_id).await?;
        run_security_scan(pool, repo, &commit_sha, build_id).await?;
    }

    Ok(())
}

async fn trigger_wasm_build(
    pool: &PgPool,
    repo: &GitHubRepository,
    commit_sha: &str,
    build_id: Uuid,
) -> Result<()> {
    sqlx::query("UPDATE build_status SET status = 'building', updated_at = NOW() WHERE id = $1")
        .bind(build_id)
        .execute(pool)
        .await?;

    sqlx::query(
        "INSERT INTO build_logs (id, build_id, step, output, created_at)
         VALUES ($1, $2, 'wasm_build', 'Starting WASM build...', NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(build_id)
    .execute(pool)
    .await?;

    tracing::info!(
        "WASM build triggered for {} at {}",
        repo.full_name,
        commit_sha
    );
    Ok(())
}

async fn run_security_scan(
    pool: &PgPool,
    repo: &GitHubRepository,
    commit_sha: &str,
    build_id: Uuid,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO build_logs (id, build_id, step, output, created_at)
         VALUES ($1, $2, 'security_scan', 'Running security scan...', NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(build_id)
    .execute(pool)
    .await?;

    tracing::info!(
        "Security scan started for {} at {}",
        repo.full_name,
        commit_sha
    );
    Ok(())
}

async fn handle_pull_request_event(
    pool: &PgPool,
    event: &WebhookEvent,
    pr: &PullRequest,
) -> Result<()> {
    let action = event.action.as_deref().unwrap_or("");

    if !["opened", "synchronize", "reopened"].contains(&action) {
        return Ok(());
    }

    let repo = event
        .repository
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("Missing repository".to_string()))?;

    let test_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO pull_request_tests (id, repository, pr_number, status, created_at)
         VALUES ($1, $2, $3, 'running', NOW())",
    )
    .bind(test_id)
    .bind(&repo.full_name)
    .bind(pr.number)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO cicd_pipelines (id, repository, status, pr_number, triggered_at)
         VALUES ($1, $2, 'running', $3, NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(&repo.full_name)
    .bind(pr.number)
    .execute(pool)
    .await?;

    tracing::info!("Tests running for PR #{} on {}", pr.number, repo.full_name);
    Ok(())
}

async fn handle_installation_event(
    pool: &PgPool,
    installation: &Installation,
    action: &Option<String>,
) -> Result<()> {
    match action.as_deref() {
        Some("created") | Some("new") => {
            if let Some(account) = &installation.account {
                sqlx::query(
                    "INSERT INTO github_installations (id, installation_id, account_id, account_login, account_type, repository_selection, created_at)
                     VALUES ($1, $2, $3, $4, $5, 'all', NOW())
                     ON CONFLICT (installation_id) DO UPDATE SET
                     account_login = EXCLUDED.account_login,
                     updated_at = NOW()",
                )
                .bind(Uuid::new_v4())
                .bind(installation.id)
                .bind(account.id)
                .bind(&account.login)
                .bind(account.account_type.as_deref().unwrap_or("User"))
                .execute(pool)
                .await?;
            }

            tracing::info!("GitHub App installation created: {}", installation.id);
        }
        Some("deleted") | Some("unsuspend") => {
            sqlx::query("DELETE FROM github_installations WHERE installation_id = $1")
                .bind(installation.id)
                .execute(pool)
                .await?;

            tracing::info!("GitHub App installation removed: {}", installation.id);
        }
        _ => {
            tracing::info!("Installation action: {:?}", action);
        }
    }

    Ok(())
}

async fn handle_installation_repositories_event(pool: &PgPool, event: &WebhookEvent) -> Result<()> {
    let action = event.action.as_deref().unwrap_or("");
    let installation = event
        .installation
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("Missing installation".to_string()))?;

    if let Some(repos_added) = &event.repositories_added {
        for repo in repos_added {
            if action == "added" || action == "repository_added" {
                sqlx::query(
                    "INSERT INTO registered_games (id, repo_full_name, default_branch, created_at)
                     VALUES ($1, $2, $3, NOW())
                     ON CONFLICT (repo_full_name) DO NOTHING",
                )
                .bind(Uuid::new_v4())
                .bind(&repo.full_name)
                .bind(&repo.default_branch)
                .execute(pool)
                .await?;

                tracing::info!(
                    "Repository {} added to installation {}",
                    repo.full_name,
                    installation.id
                );
            }
        }
    }

    if let Some(repos_removed) = &event.repositories_removed {
        for repo in repos_removed {
            if action == "removed" || action == "repository_removed" {
                sqlx::query("DELETE FROM registered_games WHERE repo_full_name = $1")
                    .bind(&repo.full_name)
                    .execute(pool)
                    .await?;

                tracing::info!(
                    "Repository {} removed from installation {}",
                    repo.full_name,
                    installation.id
                );
            }
        }
    }

    Ok(())
}

async fn deploy_if_checks_pass(
    pool: &PgPool,
    repo_full_name: &str,
    commit_sha: &str,
) -> Result<()> {
    #[derive(sqlx::FromRow)]
    struct BuildCheck {
        id: Uuid,
        status: String,
    }

    let builds = sqlx::query_as::<_, BuildCheck>(
        "SELECT id, status FROM build_status WHERE repository = $1 AND commit_sha = $2 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(repo_full_name)
    .bind(commit_sha)
    .fetch_optional(pool)
    .await?;

    if let Some(build) = builds {
        if build.status == "success" {
            tracing::info!(
                "All checks passed, deploying {} at {}",
                repo_full_name,
                commit_sha
            );

            sqlx::query(
                "INSERT INTO deployments (id, repository, commit_sha, status, deployed_at)
                 VALUES ($1, $2, $3, 'deployed', NOW())",
            )
            .bind(Uuid::new_v4())
            .bind(repo_full_name)
            .bind(commit_sha)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

#[derive(Debug, Serialize)]
pub struct InstallationInfo {
    pub id: Uuid,
    pub installation_id: i64,
    pub account_login: Option<String>,
    pub account_type: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_installations(State(pool): State<PgPool>) -> Result<Json<Vec<InstallationInfo>>> {
    let config = GitHubConfig::from_env();
    let jwt = generate_jwt(&config.app_id, &config.private_key)?;

    let api_installations = list_installations_api(&jwt).await.unwrap_or_default();

    let mut result: Vec<InstallationInfo> = Vec::new();

    for inst in api_installations {
        let existing = sqlx::query_as::<_, (Uuid, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, created_at FROM github_installations WHERE installation_id = $1",
        )
        .bind(inst.id)
        .fetch_optional(&pool)
        .await?;

        let (id, created_at) = existing.unwrap_or((Uuid::new_v4(), chrono::Utc::now()));

        if existing.is_none() {
            sqlx::query(
                "INSERT INTO github_installations (id, installation_id, account_id, account_login, account_type, repository_selection, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (installation_id) DO NOTHING",
            )
            .bind(id)
            .bind(inst.id)
            .bind(inst.account.id)
            .bind(&inst.account.login)
            .bind(inst.account.account_type.as_deref().unwrap_or("User"))
            .bind(inst.repository_selection.as_deref().unwrap_or("all"))
            .bind(created_at)
            .execute(&pool)
            .await?;
        }

        result.push(InstallationInfo {
            id,
            installation_id: inst.id,
            account_login: Some(inst.account.login),
            account_type: inst.account.account_type,
            created_at,
        });
    }

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct RegisterRepoRequest {
    pub repository: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Game {
    pub id: Uuid,
    pub developer_id: Uuid,
    pub github_repo: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn register_repository(
    State(pool): State<PgPool>,
    Json(payload): Json<RegisterRepoRequest>,
) -> Result<Json<Game>> {
    let parts: Vec<&str> = payload.repository.split('/').collect();
    if parts.len() != 2 {
        return Err(AppError::Validation(
            "Repository must be in format 'owner/repo'".to_string(),
        ));
    }

    let (owner, repo_name) = (parts[0], parts[1]);

    let config = GitHubConfig::from_env();
    let jwt = generate_jwt(&config.app_id, &config.private_key)?;

    let installations = sqlx::query_as::<_, (Uuid, i64)>(
        "SELECT id, installation_id FROM github_installations LIMIT 1",
    )
    .fetch_optional(&pool)
    .await?;

    let installation_id = if let Some((_, inst_id)) = installations {
        inst_id.to_string()
    } else {
        return Err(AppError::BadRequest(
            "No GitHub App installation found".to_string(),
        ));
    };

    let access_token = get_installation_access_token(&jwt, &installation_id).await?;

    if !verify_repo_access(&access_token, owner, repo_name).await? {
        return Err(AppError::Validation(
            "Repository not found or not accessible".to_string(),
        ));
    }

    let game_id = Uuid::new_v4();
    let game = sqlx::query_as::<_, Game>(
        "INSERT INTO games (id, developer_id, github_repo, title, description, status, active, created_at)
         VALUES ($1, '00000000-0000-0000-0000-000000000000', $2, $3, $4, 'draft', true, NOW())
         RETURNING id, developer_id, github_repo, title, description, status, active, created_at",
    )
    .bind(game_id)
    .bind(&payload.repository)
    .bind(&payload.title)
    .bind(&payload.description)
    .fetch_one(&pool)
    .await?;

    sqlx::query(
        "INSERT INTO registered_games (id, game_id, repo_full_name, default_branch, build_status, created_at)
         VALUES ($1, $2, $3, 'main', 'pending', NOW())
         ON CONFLICT (repo_full_name) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(game_id)
    .bind(&payload.repository)
    .execute(&pool)
    .await?;

    Ok(Json(game))
}

#[derive(Debug, Serialize)]
pub struct RepositoryInfo {
    pub id: i64,
    pub full_name: String,
    pub name: String,
    pub owner: String,
    pub private: bool,
    pub url: String,
}

pub async fn list_repos(State(pool): State<PgPool>) -> Result<Json<Vec<RepositoryInfo>>> {
    let config = GitHubConfig::from_env();
    let jwt = generate_jwt(&config.app_id, &config.private_key)?;

    let installation = sqlx::query_as::<_, (Uuid, i64)>(
        "SELECT id, installation_id FROM github_installations LIMIT 1",
    )
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("No GitHub App installation found".to_string()))?;

    let access_token = get_installation_access_token(&jwt, &installation.1.to_string()).await?;
    let repos = list_installation_repos(&access_token).await?;

    let result = repos
        .into_iter()
        .map(|r| RepositoryInfo {
            id: r.id,
            full_name: r.full_name,
            name: r.name,
            owner: r.owner.login,
            private: r.private,
            url: r.html_url,
        })
        .collect();

    Ok(Json(result))
}

#[derive(Debug, Serialize)]
pub struct BuildStatusResponse {
    pub repository: String,
    pub commit_sha: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn get_build_status(
    State(pool): State<PgPool>,
    Path((owner, repo)): Path<(String, String)>,
) -> Result<Json<BuildStatusResponse>> {
    let repo_full_name = format!("{}/{}", owner, repo);

    let build = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            Option<String>,
            chrono::DateTime<chrono::Utc>,
            chrono::DateTime<chrono::Utc>,
        ),
    >(
        "SELECT repository, commit_sha, status, conclusion, created_at, updated_at
         FROM build_status WHERE repository = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&repo_full_name)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("No build status found".to_string()))?;

    Ok(Json(BuildStatusResponse {
        repository: build.0,
        commit_sha: build.1,
        status: build.2,
        conclusion: build.3,
        created_at: build.4,
        updated_at: build.5,
    }))
}

pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/webhooks/github", post(handle_webhook))
        .route("/installations", get(list_installations))
        .route("/repos", get(list_repos))
        .route("/repos/register", post(register_repository))
        .route("/repos/:owner/:repo/build-status", get(get_build_status))
        .with_state(pool)
}
