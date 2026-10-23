use crate::error::{AppError, AppResult};
use crate::util::{
    now_rfc3339, optional_http_url, optional_text, sanitize_text, validate_hex_color,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

pub const THEMES: &[&str] = &["minimal", "developer", "creator"];

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Page {
    pub id: String,
    pub user_id: String,
    pub slug: String,
    pub display_name: String,
    pub bio: String,
    pub avatar_url: Option<String>,
    pub cover_url: Option<String>,
    pub theme: String,
    pub accent_color: String,
    pub status: String,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
}

impl Page {
    pub fn is_published(&self) -> bool {
        self.status == "published"
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdatePageRequest {
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub cover_url: Option<String>,
    pub theme: Option<String>,
    pub accent_color: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
}

pub async fn find_by_user_id(pool: &SqlitePool, user_id: &str) -> AppResult<Page> {
    sqlx::query_as::<_, Page>(
        "SELECT id, user_id, slug, display_name, bio, avatar_url, cover_url, theme, accent_color,
                status, seo_title, seo_description, created_at, updated_at, published_at
         FROM pages WHERE user_id = ?1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::not_found("Page not found."))
}

pub async fn find_public_by_slug(pool: &SqlitePool, slug: &str) -> AppResult<Option<Page>> {
    Ok(sqlx::query_as::<_, Page>(
        "SELECT p.id, p.user_id, p.slug, p.display_name, p.bio, p.avatar_url, p.cover_url, p.theme,
                p.accent_color, p.status, p.seo_title, p.seo_description, p.created_at, p.updated_at,
                p.published_at
         FROM pages p
         JOIN users u ON u.id = p.user_id
         WHERE p.slug = ?1 AND p.status = 'published' AND u.account_status = 'active'",
    )
    .bind(slug.to_ascii_lowercase())
    .fetch_optional(pool)
    .await?)
}

pub async fn update_page(
    pool: &SqlitePool,
    user_id: &str,
    req: UpdatePageRequest,
) -> AppResult<Page> {
    let mut page = find_by_user_id(pool, user_id).await?;
    if let Some(name) = req.display_name {
        let name = sanitize_text(&name, 80)?;
        if name.is_empty() {
            return Err(AppError::validation("Display name is required."));
        }
        page.display_name = name;
    }
    if let Some(bio) = req.bio {
        page.bio = sanitize_text(&bio, 500)?;
    }
    if req.avatar_url.is_some() {
        page.avatar_url = optional_http_url(req.avatar_url.as_deref())?;
    }
    if req.cover_url.is_some() {
        page.cover_url = optional_http_url(req.cover_url.as_deref())?;
    }
    if let Some(theme) = req.theme {
        let theme = theme.trim().to_ascii_lowercase();
        if !THEMES.contains(&theme.as_str()) {
            return Err(AppError::validation("Unknown theme."));
        }
        page.theme = theme;
    }
    if let Some(color) = req.accent_color {
        page.accent_color = validate_hex_color(&color)?;
    }
    if req.seo_title.is_some() {
        page.seo_title = optional_text(req.seo_title.as_deref(), 80)?;
    }
    if req.seo_description.is_some() {
        page.seo_description = optional_text(req.seo_description.as_deref(), 200)?;
    }
    page.updated_at = now_rfc3339();

    sqlx::query(
        "UPDATE pages SET display_name = ?1, bio = ?2, avatar_url = ?3, cover_url = ?4, theme = ?5,
                accent_color = ?6, seo_title = ?7, seo_description = ?8, updated_at = ?9
         WHERE id = ?10 AND user_id = ?11",
    )
    .bind(&page.display_name)
    .bind(&page.bio)
    .bind(&page.avatar_url)
    .bind(&page.cover_url)
    .bind(&page.theme)
    .bind(&page.accent_color)
    .bind(&page.seo_title)
    .bind(&page.seo_description)
    .bind(&page.updated_at)
    .bind(&page.id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(page)
}

pub async fn set_status(pool: &SqlitePool, user_id: &str, status: &str) -> AppResult<Page> {
    let mut page = find_by_user_id(pool, user_id).await?;
    let now = now_rfc3339();
    page.status = status.to_string();
    page.updated_at = now.clone();
    if status == "published" {
        page.published_at = Some(now.clone());
    }
    sqlx::query(
        "UPDATE pages SET status = ?1, updated_at = ?2, published_at = CASE
            WHEN ?1 = 'published' THEN ?2 ELSE published_at END
         WHERE id = ?3 AND user_id = ?4",
    )
    .bind(status)
    .bind(&now)
    .bind(&page.id)
    .bind(user_id)
    .execute(pool)
    .await?;
    find_by_user_id(pool, user_id).await
}
