use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use aws_sdk_s3::Client as S3Client;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::env;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupInfo {
    pub id: String,
    pub filename: String,
    pub created_at: DateTime<Utc>,
    pub size_bytes: u64,
}

fn get_backup_dir() -> PathBuf {
    env::var("BACKUP_LOCAL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/lib/magnetite/backups"))
}

fn get_s3_bucket() -> String {
    env::var("BACKUP_S3_BUCKET").expect("BACKUP_S3_BUCKET must be set")
}

fn get_s3_region() -> String {
    env::var("BACKUP_S3_REGION").unwrap_or_else(|_| "us-east-1".to_string())
}

pub async fn create_backup(pool: &PgPool) -> Result<String> {
    let storage_type = env::var("BACKUP_STORAGE_TYPE").unwrap_or_else(|_| "local".to_string());

    let backup_id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("magnetite_backup_{}_{}.sql", timestamp, &backup_id[..8]);
    let temp_path = format!("/tmp/{}", filename);

    let dump_result = tokio::process::Command::new("pg_dump")
        .args(["-Fc", "magnetite"])
        .output()
        .await
        .context("Failed to execute pg_dump");

    match dump_result {
        Ok(output) if output.status.success() => {
            fs::write(&temp_path, output.stdout)
                .await
                .context("Failed to write backup file")?;
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("pg_dump failed: {}", stderr);
        }
        Err(e) => {
            anyhow::bail!("Failed to run pg_dump: {}", e);
        }
    }

    let metadata = fs::metadata(&temp_path).await?;
    let file_size = metadata.len();

    match storage_type.as_str() {
        "s3" => upload_to_s3(&temp_path, &filename).await?,
        "local" => {
            let backup_dir = get_backup_dir();
            fs::create_dir_all(&backup_dir).await?;
            let dest = backup_dir.join(&filename);
            fs::copy(&temp_path, &dest).await?;
        }
        _ => anyhow::bail!("Unknown BACKUP_STORAGE_TYPE: {}", storage_type),
    }

    fs::remove_file(&temp_path).await.ok();

    tracing::info!(
        backup_id = %backup_id,
        filename = %filename,
        size_bytes = file_size,
        "Database backup created"
    );

    Ok(filename)
}

async fn upload_to_s3(local_path: &str, filename: &str) -> Result<()> {
    let region = get_s3_region();
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new(region))
        .load()
        .await;
    let client = S3Client::new(&config);

    let bucket = get_s3_bucket();
    let body = fs::read(local_path).await?;

    client
        .put_object()
        .bucket(&bucket)
        .key(filename)
        .body(body.into())
        .send()
        .await
        .context("Failed to upload backup to S3")?;

    Ok(())
}

pub async fn list_backups() -> Vec<BackupInfo> {
    let storage_type = env::var("BACKUP_STORAGE_TYPE").unwrap_or_else(|_| "local".to_string());

    match storage_type.as_str() {
        "s3" => list_backups_s3().await,
        "local" => list_backups_local().await,
        _ => Vec::new(),
    }
}

async fn list_backups_s3() -> Vec<BackupInfo> {
    let region = get_s3_region();
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new(region))
        .load()
        .await;
    let client = S3Client::new(&config);
    let bucket = get_s3_bucket();

    let output = match client
        .list_objects_v2()
        .bucket(&bucket)
        .prefix("magnetite_backup_")
        .send()
        .await
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    output
        .contents()
        .iter()
        .filter_map(|obj| {
            let key = obj.key()?;
            let filename = key.to_string();
            let (timestamp_part, uuid_part) = parse_backup_filename(&filename)?;

            let created_at = DateTime::parse_from_str(&timestamp_part, "%Y%m%d_%H%M%S")
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Some(BackupInfo {
                id: uuid_part,
                filename,
                created_at,
                size_bytes: obj.size().unwrap_or(0) as u64,
            })
        })
        .collect()
}

async fn list_backups_local() -> Vec<BackupInfo> {
    let backup_dir = get_backup_dir();
    let mut backups = Vec::new();

    let entries = match fs::read_dir(&backup_dir).await {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut dir_stream = tokio_stream::wrappers::ReadDirStream::new(entries);
    use tokio_stream::StreamExt;

    while let Some(entry) = dir_stream.next().await {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if filename.starts_with("magnetite_backup_") && filename.ends_with(".sql") {
                        if let Some((timestamp_part, uuid_part)) = parse_backup_filename(filename) {
                            let metadata = entry.metadata().await.ok();
                            let size_bytes = metadata.map(|m| m.len()).unwrap_or(0);

                            let created_at = DateTime::parse_from_str(&timestamp_part, "%Y%m%d_%H%M%S")
                                .map(|dt| dt.with_timezone(&Utc))
                                .unwrap_or_else(|_| Utc::now());

                            backups.push(BackupInfo {
                                id: uuid_part,
                                filename: filename.to_string(),
                                created_at,
                                size_bytes,
                            });
                        }
                    }
                }
            }
        }
    }

    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    backups
}

fn parse_backup_filename(filename: &str) -> Option<(String, String)> {
    let stripped = filename.strip_prefix("magnetite_backup_")?.strip_suffix(".sql")?;
    let parts: Vec<&str> = stripped.rsplitn(2, '_').collect();
    if parts.len() == 2 {
        let uuid_part = parts[0].to_string();
        let timestamp_part = parts[1].chars().rev().collect::<String>();
        Some((timestamp_part, uuid_part))
    } else {
        None
    }
}

pub async fn restore_from_backup(backup_id: &str, pool: &PgPool) -> Result<()> {
    let storage_type = env::var("BACKUP_STORAGE_TYPE").unwrap_or_else(|_| "local".to_string());

    let filename = find_backup_filename(backup_id).await
        .context("Could not find backup file")?;

    let temp_path = format!("/tmp/restore_{}", filename);

    match storage_type.as_str() {
        "s3" => download_from_s3(&filename, &temp_path).await?,
        "local" => {
            let backup_dir = get_backup_dir();
            let src = backup_dir.join(&filename);
            fs::copy(&src, &temp_path).await?;
        }
        _ => anyhow::bail!("Unknown BACKUP_STORAGE_TYPE: {}", storage_type),
    }

    drop(pool);

    let restore_result = tokio::process::Command::new("pg_restore")
        .args(["--clean", "--if-exists", "-d", "magnetite", &temp_path])
        .output()
        .await;

    fs::remove_file(&temp_path).await.ok();

    match restore_result {
        Ok(output) if output.status.success() => {
            tracing::info!(backup_id = %backup_id, "Database restored from backup");
            Ok(())
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("pg_restore failed: {}", stderr);
        }
        Err(e) => anyhow::bail!("Failed to run pg_restore: {}", e),
    }
}

async fn download_from_s3(filename: &str, dest_path: &str) -> Result<()> {
    let region = get_s3_region();
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new(region))
        .load()
        .await;
    let client = S3Client::new(&config);

    let bucket = get_s3_bucket();

    let output = client
        .get_object()
        .bucket(&bucket)
        .key(filename)
        .send()
        .await
        .context("Failed to download backup from S3")?;

    let body = output
        .body
        .collect()
        .await
        .context("Failed to read S3 object body")?;

    fs::write(dest_path, body.into_bytes())
        .await
        .context("Failed to write downloaded backup to file")?;

    Ok(())
}

async fn find_backup_filename(backup_id: &str) -> Option<String> {
    let backups = list_backups().await;
    backups
        .into_iter()
        .find(|b| b.id == backup_id || b.filename.contains(backup_id))
        .map(|b| b.filename)
}

pub async fn cleanup_old_backups(keep_days: i32) -> Result<u64> {
    let storage_type = env::var("BACKUP_STORAGE_TYPE").unwrap_or_else(|_| "local".to_string());
    let cutoff = Utc::now() - chrono::Duration::days(keep_days as i64);

    let backups = list_backups().await;
    let to_delete: Vec<_> = backups
        .into_iter()
        .filter(|b| b.created_at < cutoff)
        .collect();

    let mut deleted_count = 0u64;

    for backup in to_delete {
        let success = match storage_type.as_str() {
            "s3" => delete_from_s3(&backup.filename).await,
            "local" => delete_local_backup(&backup.filename).await,
            _ => continue,
        };

        if success.is_ok() {
            deleted_count += 1;
            tracing::info!(filename = %backup.filename, "Deleted old backup");
        }
    }

    Ok(deleted_count)
}

async fn delete_from_s3(filename: &str) -> Result<()> {
    let region = get_s3_region();
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new(region))
        .load()
        .await;
    let client = S3Client::new(&config);

    let bucket = get_s3_bucket();

    client
        .delete_object()
        .bucket(&bucket)
        .key(filename)
        .send()
        .await
        .context("Failed to delete backup from S3")?;

    Ok(())
}

async fn delete_local_backup(filename: &str) -> Result<()> {
    let backup_dir = get_backup_dir();
    let path = backup_dir.join(filename);
    fs::remove_file(&path).await?;
    Ok(())
}
