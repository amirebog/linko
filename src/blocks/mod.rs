use crate::error::{AppError, AppResult};
use crate::util::{new_id, now_rfc3339, sanitize_text, validate_http_url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, Sqlite, SqlitePool, Transaction};

pub const BLOCK_TYPES: &[&str] = &[
    "heading",
    "paragraph",
    "link",
    "social_links",
    "divider",
    "image",
    "cta",
    "project_card",
    "skills",
    "contact",
];

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Block {
    pub id: String,
    pub page_id: String,
    #[sqlx(rename = "type")]
    pub block_type: String,
    pub title: Option<String>,
    pub content_json: String,
    pub position: i64,
    pub is_visible: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl Block {
    pub fn content(&self) -> Value {
        serde_json::from_str(&self.content_json).unwrap_or_else(|_| json!({}))
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateBlockRequest {
    #[serde(rename = "type")]
    pub block_type: String,
    pub title: Option<String>,
    pub content: Option<Value>,
    pub is_visible: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBlockRequest {
    pub title: Option<String>,
    pub content: Option<Value>,
    pub is_visible: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderRequest {
    pub ids: Vec<String>,
}

pub fn validate_block_payload(block_type: &str, content: &Value) -> AppResult<Value> {
    let kind = block_type.trim().to_ascii_lowercase();
    if !BLOCK_TYPES.contains(&kind.as_str()) {
        return Err(AppError::validation("Unknown block type."));
    }
    let obj = content
        .as_object()
        .ok_or_else(|| AppError::validation("Block content must be a JSON object."))?;

    let validated = match kind.as_str() {
        "heading" => {
            let text = required_string(obj, "text", 120)?;
            let level = obj
                .get("level")
                .and_then(Value::as_u64)
                .unwrap_or(2)
                .clamp(1, 3);
            json!({ "text": text, "level": level })
        }
        "paragraph" => json!({ "text": required_string(obj, "text", 4000)? }),
        "link" => json!({
            "label": required_string(obj, "label", 80)?,
            "url": validate_http_url(&required_string(obj, "url", 2048)?)?
        }),
        "social_links" => {
            let items = obj
                .get("items")
                .and_then(Value::as_array)
                .ok_or_else(|| AppError::validation("Social links need an items array."))?;
            if items.len() > 12 {
                return Err(AppError::validation(
                    "A maximum of 12 social links is allowed.",
                ));
            }
            let mut out = Vec::new();
            for item in items {
                let map = item
                    .as_object()
                    .ok_or_else(|| AppError::validation("Each social link must be an object."))?;
                out.push(json!({
                    "network": required_string(map, "network", 32)?,
                    "label": required_string(map, "label", 40)?,
                    "url": validate_http_url(&required_string(map, "url", 2048)?)?
                }));
            }
            json!({ "items": out })
        }
        "divider" => json!({}),
        "image" => json!({
            "url": validate_http_url(&required_string(obj, "url", 2048)?)?,
            "alt": optional_string(obj, "alt", 120)?.unwrap_or_default()
        }),
        "cta" => json!({
            "label": required_string(obj, "label", 48)?,
            "url": validate_http_url(&required_string(obj, "url", 2048)?)?
        }),
        "project_card" => json!({
            "title": required_string(obj, "title", 80)?,
            "description": optional_string(obj, "description", 400)?.unwrap_or_default(),
            "url": match optional_string(obj, "url", 2048)? {
                Some(url) => Some(validate_http_url(&url)?),
                None => None,
            },
            "image_url": match optional_string(obj, "image_url", 2048)? {
                Some(url) => Some(validate_http_url(&url)?),
                None => None,
            }
        }),
        "skills" => {
            let items = obj
                .get("items")
                .and_then(Value::as_array)
                .ok_or_else(|| AppError::validation("Skills need an items array."))?;
            if items.len() > 30 {
                return Err(AppError::validation("A maximum of 30 skills is allowed."));
            }
            let mut out = Vec::new();
            for item in items {
                let text = item
                    .as_str()
                    .ok_or_else(|| AppError::validation("Each skill must be text."))?;
                let text = sanitize_text(text, 40)?;
                if !text.is_empty() {
                    out.push(Value::String(text));
                }
            }
            json!({ "items": out })
        }
        "contact" => json!({
            "email": optional_string(obj, "email", 254)?.unwrap_or_default(),
            "phone": optional_string(obj, "phone", 40)?.unwrap_or_default(),
            "location": optional_string(obj, "location", 80)?.unwrap_or_default()
        }),
        _ => unreachable!(),
    };
    Ok(validated)
}

fn required_string(
    obj: &serde_json::Map<String, Value>,
    key: &str,
    max: usize,
) -> AppResult<String> {
    let value = obj
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::validation(format!("Field `{key}` is required.")))?;
    let value = sanitize_text(value, max)?;
    if value.is_empty() {
        return Err(AppError::validation(format!("Field `{key}` is required.")));
    }
    Ok(value)
}

fn optional_string(
    obj: &serde_json::Map<String, Value>,
    key: &str,
    max: usize,
) -> AppResult<Option<String>> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(sanitize_text(s, max)?)),
        Some(_) => Err(AppError::validation(format!("Field `{key}` must be text."))),
    }
}

pub async fn list_for_page(pool: &SqlitePool, page_id: &str) -> AppResult<Vec<Block>> {
    Ok(sqlx::query_as::<_, Block>(
        r#"SELECT id, page_id, type, title, content_json, position, is_visible, created_at, updated_at
           FROM blocks WHERE page_id = ?1 ORDER BY position ASC"#,
    )
    .bind(page_id)
    .fetch_all(pool)
    .await?)
}

pub async fn list_visible(pool: &SqlitePool, page_id: &str) -> AppResult<Vec<Block>> {
    Ok(sqlx::query_as::<_, Block>(
        r#"SELECT id, page_id, type, title, content_json, position, is_visible, created_at, updated_at
           FROM blocks WHERE page_id = ?1 AND is_visible = 1 ORDER BY position ASC"#,
    )
    .bind(page_id)
    .fetch_all(pool)
    .await?)
}

pub async fn count_for_page(pool: &SqlitePool, page_id: &str) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM blocks WHERE page_id = ?1")
            .bind(page_id)
            .fetch_one(pool)
            .await?,
    )
}

async fn owned_block(pool: &SqlitePool, user_id: &str, block_id: &str) -> AppResult<Block> {
    sqlx::query_as::<_, Block>(
        r#"SELECT b.id, b.page_id, b.type, b.title, b.content_json, b.position, b.is_visible,
                  b.created_at, b.updated_at
           FROM blocks b
           JOIN pages p ON p.id = b.page_id
           WHERE b.id = ?1 AND p.user_id = ?2"#,
    )
    .bind(block_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("Block not found."))
}

pub async fn create_block(
    pool: &SqlitePool,
    page_id: &str,
    req: CreateBlockRequest,
) -> AppResult<Block> {
    let kind = req.block_type.trim().to_ascii_lowercase();
    let content = validate_block_payload(&kind, req.content.as_ref().unwrap_or(&json!({})))?;
    let title = match req.title {
        Some(t) => {
            let t = sanitize_text(&t, 80)?;
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        }
        None => None,
    };
    let now = now_rfc3339();
    let id = new_id();
    let next: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(position), -1) + 1 FROM blocks WHERE page_id = ?1")
            .bind(page_id)
            .fetch_one(pool)
            .await?;
    let visible = if req.is_visible.unwrap_or(true) { 1 } else { 0 };
    sqlx::query(
        r#"INSERT INTO blocks (id, page_id, type, title, content_json, position, is_visible, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)"#,
    )
    .bind(&id)
    .bind(page_id)
    .bind(&kind)
    .bind(&title)
    .bind(content.to_string())
    .bind(next)
    .bind(visible)
    .bind(&now)
    .execute(pool)
    .await?;

    owned_block_by_id(pool, &id).await
}

async fn owned_block_by_id(pool: &SqlitePool, id: &str) -> AppResult<Block> {
    sqlx::query_as::<_, Block>(
        r#"SELECT id, page_id, type, title, content_json, position, is_visible, created_at, updated_at
           FROM blocks WHERE id = ?1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(AppError::internal)
}

pub async fn update_block(
    pool: &SqlitePool,
    user_id: &str,
    block_id: &str,
    req: UpdateBlockRequest,
) -> AppResult<Block> {
    let block = owned_block(pool, user_id, block_id).await?;
    let mut title = block.title.clone();
    if let Some(t) = req.title {
        let t = sanitize_text(&t, 80)?;
        title = if t.is_empty() { None } else { Some(t) };
    }
    let content = if let Some(c) = req.content {
        validate_block_payload(&block.block_type, &c)?
    } else {
        block.content()
    };
    let visible = req
        .is_visible
        .map(|v| if v { 1 } else { 0 })
        .unwrap_or(block.is_visible);
    let now = now_rfc3339();
    sqlx::query(
        "UPDATE blocks SET title = ?1, content_json = ?2, is_visible = ?3, updated_at = ?4 WHERE id = ?5",
    )
    .bind(&title)
    .bind(content.to_string())
    .bind(visible)
    .bind(&now)
    .bind(block_id)
    .execute(pool)
    .await?;
    owned_block(pool, user_id, block_id).await
}

pub async fn delete_block(pool: &SqlitePool, user_id: &str, block_id: &str) -> AppResult<()> {
    let block = owned_block(pool, user_id, block_id).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM blocks WHERE id = ?1")
        .bind(&block.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE blocks SET position = position - 1 WHERE page_id = ?1 AND position > ?2")
        .bind(&block.page_id)
        .bind(block.position)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn duplicate_block(pool: &SqlitePool, user_id: &str, block_id: &str) -> AppResult<Block> {
    let block = owned_block(pool, user_id, block_id).await?;
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE blocks SET position = position + 1 WHERE page_id = ?1 AND position > ?2")
        .bind(&block.page_id)
        .bind(block.position)
        .execute(&mut *tx)
        .await?;
    let id = new_id();
    let now = now_rfc3339();
    sqlx::query(
        r#"INSERT INTO blocks (id, page_id, type, title, content_json, position, is_visible, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)"#,
    )
    .bind(&id)
    .bind(&block.page_id)
    .bind(&block.block_type)
    .bind(&block.title)
    .bind(&block.content_json)
    .bind(block.position + 1)
    .bind(block.is_visible)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    owned_block(pool, user_id, &id).await
}

pub async fn reorder_blocks(
    pool: &SqlitePool,
    page_id: &str,
    _user_id: &str,
    ids: &[String],
) -> AppResult<Vec<Block>> {
    let existing = list_for_page(pool, page_id).await?;
    if existing.len() != ids.len() {
        return Err(AppError::validation(
            "Reorder payload must include every block.",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for id in ids {
        if !existing.iter().any(|b| b.id == *id) || !seen.insert(id) {
            return Err(AppError::validation("Invalid block order."));
        }
    }
    let mut tx: Transaction<'_, Sqlite> = pool.begin().await?;
    for (index, id) in ids.iter().enumerate() {
        sqlx::query(
            "UPDATE blocks SET position = ?1, updated_at = ?2 WHERE id = ?3 AND page_id = ?4",
        )
        .bind(index as i64)
        .bind(now_rfc3339())
        .bind(id)
        .bind(page_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    list_for_page(pool, page_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_type() {
        let err = validate_block_payload("script", &json!({"text": "x"})).unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
    }

    #[test]
    fn rejects_javascript_url() {
        let err =
            validate_block_payload("link", &json!({"label": "x", "url": "javascript:alert(1)"}))
                .unwrap_err();
        assert_eq!(err.code, "VALIDATION_ERROR");
    }
}
