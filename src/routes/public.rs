use crate::blocks;
use crate::error::{AppError, AppResult};
use crate::pages;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct PublicPage {
    pub username: String,
    pub display_name: String,
    pub bio: String,
    pub avatar_url: Option<String>,
    pub cover_url: Option<String>,
    pub theme: String,
    pub accent_color: String,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub published_at: Option<String>,
    pub blocks: Vec<PublicBlock>,
}

#[derive(Serialize)]
pub struct PublicBlock {
    pub id: String,
    #[serde(rename = "type")]
    pub block_type: String,
    pub title: Option<String>,
    pub content: serde_json::Value,
}

pub async fn get_page(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> AppResult<Json<PublicPage>> {
    Ok(Json(load_public(&state, &username).await?))
}

pub async fn load_public(state: &AppState, username: &str) -> AppResult<PublicPage> {
    let page = pages::find_public_by_slug(&state.pool, username)
        .await?
        .ok_or_else(|| AppError::not_found("This page is not available."))?;
    let blocks = blocks::list_visible(&state.pool, &page.id)
        .await?
        .into_iter()
        .map(|b| PublicBlock {
            content: b.content(),
            id: b.id,
            block_type: b.block_type,
            title: b.title,
        })
        .collect();
    Ok(PublicPage {
        username: page.slug,
        display_name: page.display_name,
        bio: page.bio,
        avatar_url: page.avatar_url,
        cover_url: page.cover_url,
        theme: page.theme,
        accent_color: page.accent_color,
        seo_title: page.seo_title,
        seo_description: page.seo_description,
        published_at: page.published_at,
        blocks,
    })
}
