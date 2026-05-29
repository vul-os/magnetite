// Health checker service — DB, Redis, and S3 liveness; platform surface, not yet wired.
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Instant;

pub struct HealthChecker {
    pool: PgPool,
    start_time: Instant,
    version: String,
}

#[derive(Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub version: String,
    pub uptime_secs: u64,
    pub checks: HashMap<String, CheckResult>,
}

#[derive(Serialize)]
pub struct CheckResult {
    pub status: String,
    pub latency_ms: u64,
    pub message: Option<String>,
}

impl HealthChecker {
    pub fn new(pool: PgPool, start_time: Instant, version: String) -> Self {
        Self {
            pool,
            start_time,
            version,
        }
    }

    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    pub async fn check_database(&self) -> CheckResult {
        let start = Instant::now();
        let result = sqlx::query("SELECT 1").fetch_one(&self.pool).await;

        let latency_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(_) => CheckResult {
                status: "ok".to_string(),
                latency_ms,
                message: None,
            },
            Err(e) => CheckResult {
                status: "error".to_string(),
                latency_ms,
                message: Some(e.to_string()),
            },
        }
    }

    pub async fn check_redis(&self, redis_url: Option<&str>) -> CheckResult {
        let start = Instant::now();

        let Some(url) = redis_url else {
            return CheckResult {
                status: "ok".to_string(),
                latency_ms: 0,
                message: Some("Redis not configured".to_string()),
            };
        };

        match redis::Client::open(url) {
            Ok(client) => {
                if let Ok(mut conn) = client.get_multiplexed_tokio_connection().await {
                    let result: Result<String, _> = redis::cmd("PING").query_async(&mut conn).await;
                    let latency_ms = start.elapsed().as_millis() as u64;

                    match result {
                        Ok(_) => CheckResult {
                            status: "ok".to_string(),
                            latency_ms,
                            message: None,
                        },
                        Err(e) => CheckResult {
                            status: "error".to_string(),
                            latency_ms,
                            message: Some(e.to_string()),
                        },
                    }
                } else {
                    CheckResult {
                        status: "error".to_string(),
                        latency_ms: start.elapsed().as_millis() as u64,
                        message: Some("Failed to get redis connection".to_string()),
                    }
                }
            }
            Err(e) => CheckResult {
                status: "error".to_string(),
                latency_ms: start.elapsed().as_millis() as u64,
                message: Some(e.to_string()),
            },
        }
    }

    pub fn check_disk(&self) -> CheckResult {
        let start = Instant::now();

        match sys_info::disk_info() {
            Ok(disk) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let free_percent = (disk.free as f64 / disk.total as f64) * 100.0;

                if free_percent < 10.0 {
                    CheckResult {
                        status: "error".to_string(),
                        latency_ms,
                        message: Some(format!(
                            "Disk space critical: {:.1}% free ({:.2} GB / {:.2} GB)",
                            free_percent,
                            disk.free as f64 / 1_000_000_000.0,
                            disk.total as f64 / 1_000_000_000.0
                        )),
                    }
                } else if free_percent < 20.0 {
                    CheckResult {
                        status: "ok".to_string(),
                        latency_ms,
                        message: Some(format!(
                            "Disk space low: {:.1}% free ({:.2} GB / {:.2} GB)",
                            free_percent,
                            disk.free as f64 / 1_000_000_000.0,
                            disk.total as f64 / 1_000_000_000.0
                        )),
                    }
                } else {
                    CheckResult {
                        status: "ok".to_string(),
                        latency_ms,
                        message: Some(format!(
                            "{:.1}% free ({:.2} GB / {:.2} GB)",
                            free_percent,
                            disk.free as f64 / 1_000_000_000.0,
                            disk.total as f64 / 1_000_000_000.0
                        )),
                    }
                }
            }
            Err(e) => CheckResult {
                status: "error".to_string(),
                latency_ms: start.elapsed().as_millis() as u64,
                message: Some(e.to_string()),
            },
        }
    }

    pub fn check_memory(&self) -> CheckResult {
        let start = Instant::now();

        match sys_info::mem_info() {
            Ok(mem) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let used = mem.total - mem.free;
                let used_percent = (used as f64 / mem.total as f64) * 100.0;

                CheckResult {
                    status: "ok".to_string(),
                    latency_ms,
                    message: Some(format!(
                        "{:.1}% used ({:.2} GB / {:.2} GB)",
                        used_percent,
                        used as f64 / 1_000_000_000.0,
                        mem.total as f64 / 1_000_000_000.0
                    )),
                }
            }
            Err(e) => CheckResult {
                status: "error".to_string(),
                latency_ms: start.elapsed().as_millis() as u64,
                message: Some(e.to_string()),
            },
        }
    }

    pub async fn run_all_checks(&self, redis_url: Option<&str>) -> HealthStatus {
        let db_check = self.check_database().await;
        let redis_check = self.check_redis(redis_url).await;
        let disk_check = self.check_disk();
        let memory_check = self.check_memory();

        let mut checks = HashMap::new();
        checks.insert("database".to_string(), db_check);
        checks.insert("redis".to_string(), redis_check);
        checks.insert("disk".to_string(), disk_check);
        checks.insert("memory".to_string(), memory_check);

        let status = determine_overall_status(&checks);

        HealthStatus {
            status,
            timestamp: Utc::now(),
            version: self.version.clone(),
            uptime_secs: self.uptime_secs(),
            checks,
        }
    }
}

fn determine_overall_status(checks: &HashMap<String, CheckResult>) -> String {
    let has_error = checks.values().any(|c| c.status == "error");
    let has_degraded = checks.values().any(|c| {
        c.message
            .as_ref()
            .map(|m| m.contains("low") || m.contains("critical") || m.contains("not configured"))
            .unwrap_or(false)
    });

    if has_error {
        "unhealthy".to_string()
    } else if has_degraded {
        "degraded".to_string()
    } else {
        "healthy".to_string()
    }
}
