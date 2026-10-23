use crate::auth::AuthUser;
use crate::blocks::{self, Block, CreateBlockRequest, ReorderRequest, UpdateBlockRequest};
use crate::error::AppResult;
use crate::extract::JsonBody;
use crate::pages;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::Json;

pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Vec<Block>>> {
    let page = pages::find_by_user_id(&state.pool, &user.id).await?;
    Ok(Json(blocks::list_for_page(&state.pool, &page.id).await?))
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    JsonBody(body): JsonBody<CreateBlockRequest>,
) -> AppResult<(axum::http::StatusCode, Json<Block>)> {
    let page = pages::find_by_user_id(&state.pool, &user.id).await?;
    let block = blocks::create_block(&state.pool, &page.id, body).await?;
    Ok((axum::http::StatusCode::CREATED, Json(block)))
}

pub async fn update(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    JsonBody(body): JsonBody<UpdateBlockRequest>,
) -> AppResult<Json<Block>> {
    Ok(Json(
        blocks::update_block(&state.pool, &user.id, &id, body).await?,
    ))
}

pub async fn delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    blocks::delete_block(&state.pool, &user.id, &id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn duplicate(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> AppResult<Json<Block>> {
    Ok(Json(
        blocks::duplicate_block(&state.pool, &user.id, &id).await?,
    ))
}

pub async fn reorder(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    JsonBody(body): JsonBody<ReorderRequest>,
) -> AppResult<Json<Vec<Block>>> {
    let page = pages::find_by_user_id(&state.pool, &user.id).await?;
    Ok(Json(
        blocks::reorder_blocks(&state.pool, &page.id, &user.id, &body.ids).await?,
    ))
}
