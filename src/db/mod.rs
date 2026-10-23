use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::util::now_rfc3339;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::path::Path;
use std::time::Duration;

pub async fn init_pool(database_path: &Path) -> Result<SqlitePool, String> {
    if let Some(parent) = database_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!("Failed to create data directory {}: {e}", parent.display())
            })?;
        }
    }

    let options = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(8))
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(options)
        .await
        .map_err(|e| format!("Failed to open SQLite database: {e}"))?;

    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(&pool)
        .await
        .map_err(|e| format!("Failed to enable foreign keys: {e}"))?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| format!("Database migration failed: {e}"))?;

    Ok(pool)
}

pub async fn bootstrap_admin(pool: &SqlitePool, config: &Config) -> Result<(), String> {
    let Some(username) = config.bootstrap_admin.as_deref() else {
        return Ok(());
    };

    let admin_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE role = 'admin' AND account_status = 'active'",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to count administrators: {e}"))?;

    if admin_count > 0 {
        tracing::info!("Administrator already exists; bootstrap skipped");
        return Ok(());
    }

    let now = now_rfc3339();
    let result = sqlx::query(
        "UPDATE users SET role = 'admin', updated_at = ?1 WHERE username = ?2 AND account_status = 'active'",
    )
    .bind(&now)
    .bind(username)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to bootstrap administrator: {e}"))?;

    if result.rows_affected() == 0 {
        return Err(format!(
            "LINKO_BOOTSTRAP_ADMIN={username} did not match an active user. Register first, then restart."
        ));
    }

    tracing::info!(username, "Promoted bootstrap administrator");
    Ok(())
}

pub async fn count_active_admins(pool: &SqlitePool) -> AppResult<i64> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE role = 'admin' AND account_status = 'active'",
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}
