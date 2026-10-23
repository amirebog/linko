use crate::error::{AppError, AppResult};
use crate::users::{AccountStatus, User};
use crate::util::{new_id, now_rfc3339, rfc3339_plus, sha256_hex};
use sqlx::SqlitePool;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SessionRecord {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub expires_at: String,
}

pub async fn create_session(
    pool: &SqlitePool,
    user_id: &str,
    ttl: Duration,
) -> AppResult<(String, SessionRecord)> {
    let raw = crate::util::random_token();
    let token_hash = sha256_hex(&raw);
    let record = SessionRecord {
        id: new_id(),
        user_id: user_id.to_string(),
        token_hash,
        expires_at: rfc3339_plus(ttl),
    };
    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, expires_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(&record.id)
    .bind(&record.user_id)
    .bind(&record.token_hash)
    .bind(&record.expires_at)
    .bind(now_rfc3339())
    .execute(pool)
    .await?;
    Ok((raw, record))
}

pub async fn rotate_session(
    pool: &SqlitePool,
    old_raw: Option<&str>,
    user_id: &str,
    ttl: Duration,
) -> AppResult<String> {
    if let Some(old) = old_raw {
        let _ = delete_by_raw(pool, old).await;
    }
    let (raw, _) = create_session(pool, user_id, ttl).await?;
    Ok(raw)
}

pub async fn delete_by_raw(pool: &SqlitePool, raw: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?1")
        .bind(sha256_hex(raw))
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn user_from_token(pool: &SqlitePool, raw: &str) -> AppResult<Option<User>> {
    if raw.len() != 64 || !raw.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(None);
    }
    let now = now_rfc3339();
    let row = sqlx::query_as::<_, User>(
        "SELECT u.id, u.username, u.email, u.password_hash, u.role, u.account_status, u.created_at, u.updated_at
         FROM sessions s
         JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = ?1 AND s.expires_at > ?2",
    )
    .bind(sha256_hex(raw))
    .bind(&now)
    .fetch_optional(pool)
    .await?;

    let Some(user) = row else {
        return Ok(None);
    };
    if user.account_status != AccountStatus::Active {
        return Err(AppError::forbidden("This account is disabled."));
    }
    Ok(Some(user))
}

pub async fn create_expired_session_for_tests(
    pool: &SqlitePool,
    user_id: &str,
    raw: &str,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, expires_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(new_id())
    .bind(user_id)
    .bind(sha256_hex(raw))
    .bind("2000-01-01T00:00:00+00:00")
    .bind(now_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}
