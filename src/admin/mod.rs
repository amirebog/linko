use crate::error::{AppError, AppResult};
use crate::util::{new_id, now_rfc3339};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct AdminStats {
    pub users: i64,
    pub pages: i64,
    pub published_pages: i64,
    pub unpublished_pages: i64,
    pub draft_pages: i64,
}

#[derive(Debug, FromRow, Serialize)]
pub struct AdminUserRow {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub account_status: String,
    pub created_at: String,
    pub page_status: Option<String>,
    pub page_slug: Option<String>,
}

#[derive(Debug, FromRow, Serialize)]
pub struct AuditLog {
    pub id: String,
    pub actor_user_id: Option<String>,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

pub async fn stats(pool: &SqlitePool) -> AppResult<AdminStats> {
    let users = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    let pages = sqlx::query_scalar("SELECT COUNT(*) FROM pages")
        .fetch_one(pool)
        .await?;
    let published_pages =
        sqlx::query_scalar("SELECT COUNT(*) FROM pages WHERE status = 'published'")
            .fetch_one(pool)
            .await?;
    let unpublished_pages =
        sqlx::query_scalar("SELECT COUNT(*) FROM pages WHERE status = 'unpublished'")
            .fetch_one(pool)
            .await?;
    let draft_pages = sqlx::query_scalar("SELECT COUNT(*) FROM pages WHERE status = 'draft'")
        .fetch_one(pool)
        .await?;
    Ok(AdminStats {
        users,
        pages,
        published_pages,
        unpublished_pages,
        draft_pages,
    })
}

pub async fn list_users(
    pool: &SqlitePool,
    query: Option<&str>,
    page: i64,
    per_page: i64,
) -> AppResult<(Vec<AdminUserRow>, i64)> {
    let offset = (page - 1).max(0) * per_page;
    let like = query
        .map(|q| format!("%{}%", q.trim().to_ascii_lowercase()))
        .filter(|q| q.len() > 2);
    let rows = if let Some(like) = like.clone() {
        sqlx::query_as::<_, AdminUserRow>(
            "SELECT u.id, u.username, u.email, u.role, u.account_status, u.created_at,
                    p.status as page_status, p.slug as page_slug
             FROM users u
             LEFT JOIN pages p ON p.user_id = u.id
             WHERE u.username LIKE ?1 OR u.email LIKE ?1
             ORDER BY u.created_at DESC
             LIMIT ?2 OFFSET ?3",
        )
        .bind(like)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, AdminUserRow>(
            "SELECT u.id, u.username, u.email, u.role, u.account_status, u.created_at,
                    p.status as page_status, p.slug as page_slug
             FROM users u
             LEFT JOIN pages p ON p.user_id = u.id
             ORDER BY u.created_at DESC
             LIMIT ?1 OFFSET ?2",
        )
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await?
    };

    let total: i64 = if let Some(like) = like {
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE username LIKE ?1 OR email LIKE ?1")
            .bind(like)
            .fetch_one(pool)
            .await?
    } else {
        sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(pool)
            .await?
    };

    Ok((rows, total))
}

pub async fn record_audit(
    pool: &SqlitePool,
    actor_user_id: &str,
    action: &str,
    target_type: &str,
    target_id: Option<&str>,
    metadata: HashMap<&str, String>,
) -> AppResult<()> {
    let metadata_json = serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".into());
    sqlx::query(
        "INSERT INTO audit_logs (id, actor_user_id, action, target_type, target_id, metadata_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(new_id())
    .bind(actor_user_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(metadata_json)
    .bind(now_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn recent_audit(pool: &SqlitePool, limit: i64) -> AppResult<Vec<AuditLog>> {
    Ok(sqlx::query_as::<_, AuditLog>(
        "SELECT id, actor_user_id, action, target_type, target_id, metadata_json, created_at
         FROM audit_logs ORDER BY created_at DESC LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

pub async fn set_user_status(
    pool: &SqlitePool,
    actor_id: &str,
    target_id: &str,
    status: &str,
) -> AppResult<()> {
    if status != "active" && status != "disabled" {
        return Err(AppError::validation("Invalid account status."));
    }
    let user = crate::users::find_by_id(pool, target_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found."))?;
    if user.is_admin() && status == "disabled" {
        let admins = crate::db::count_active_admins(pool).await?;
        if admins <= 1 {
            return Err(AppError::forbidden(
                "The last active administrator cannot be disabled.",
            ));
        }
    }
    sqlx::query("UPDATE users SET account_status = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(status)
        .bind(now_rfc3339())
        .bind(target_id)
        .execute(pool)
        .await?;
    if status == "disabled" {
        sqlx::query("DELETE FROM sessions WHERE user_id = ?1")
            .bind(target_id)
            .execute(pool)
            .await?;
    }
    let mut meta = HashMap::new();
    meta.insert("status", status.to_string());
    record_audit(pool, actor_id, "user.status", "user", Some(target_id), meta).await
}

pub async fn delete_user(pool: &SqlitePool, actor_id: &str, target_id: &str) -> AppResult<()> {
    let user = crate::users::find_by_id(pool, target_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found."))?;
    if user.is_admin() {
        let admins = crate::db::count_active_admins(pool).await?;
        if admins <= 1 {
            return Err(AppError::forbidden(
                "The last active administrator cannot be deleted.",
            ));
        }
    }
    if actor_id == target_id {
        return Err(AppError::forbidden(
            "You cannot delete your own account from admin.",
        ));
    }
    sqlx::query("DELETE FROM users WHERE id = ?1")
        .bind(target_id)
        .execute(pool)
        .await?;
    let mut meta = HashMap::new();
    meta.insert("username", user.username);
    record_audit(pool, actor_id, "user.delete", "user", Some(target_id), meta).await
}
