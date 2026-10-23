use crate::auth::AuthUser;
use crate::blocks;
use crate::error::AppResult;
use crate::extract::JsonBody;
use crate::pages::{self, Page, UpdatePageRequest};
use crate::state::AppState;
use axum::extract::State;
use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct PagePayload {
    pub page: Page,
    pub block_count: i64,
    pub public_path: String,
}

#[derive(Serialize)]
pub struct PreviewPayload {
    pub page: Page,
    pub blocks: Vec<blocks::Block>,
}

async fn payload(state: &AppState, user_id: &str) -> AppResult<PagePayload> {
    let page = pages::find_by_user_id(&state.pool, user_id).await?;
    let block_count = blocks::count_for_page(&state.pool, &page.id).await?;
    Ok(PagePayload {
        public_path: format!("/{}", page.slug),
        page,
        block_count,
    })
}

pub async fn get_me(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<PagePayload>> {
    Ok(Json(payload(&state, &user.id).await?))
}

pub async fn patch_me(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    JsonBody(body): JsonBody<UpdatePageRequest>,
) -> AppResult<Json<PagePayload>> {
    pages::update_page(&state.pool, &user.id, body).await?;
    Ok(Json(payload(&state, &user.id).await?))
}

pub async fn publish(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<PagePayload>> {
    pages::set_status(&state.pool, &user.id, "published").await?;
    Ok(Json(payload(&state, &user.id).await?))
}

pub async fn unpublish(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<PagePayload>> {
    pages::set_status(&state.pool, &user.id, "unpublished").await?;
    Ok(Json(payload(&state, &user.id).await?))
}

pub async fn preview(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<PreviewPayload>> {
    let page = pages::find_by_user_id(&state.pool, &user.id).await?;
    let blocks = blocks::list_visible(&state.pool, &page.id).await?;
    Ok(Json(PreviewPayload { page, blocks }))
}
