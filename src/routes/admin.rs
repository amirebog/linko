use crate::admin;
use crate::auth::AdminUser;
use crate::error::{AppError, AppResult};
use crate::extract::JsonBody;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct UserQuery {
    pub q: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Deserialize)]
pub struct StatusBody {
    pub status: String,
}

pub async fn stats(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
) -> AppResult<Json<admin::AdminStats>> {
    Ok(Json(admin::stats(&state.pool).await?))
}

pub async fn users(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Query(query): Query<UserQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);
    let (users, total) = admin::list_users(&state.pool, query.q.as_deref(), page, per_page).await?;
    Ok(Json(serde_json::json!({
        "users": users,
        "total": total,
        "page": page,
        "per_page": per_page
    })))
}

pub async fn set_status(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(id): Path<String>,
    JsonBody(body): JsonBody<StatusBody>,
) -> AppResult<Json<serde_json::Value>> {
    admin::set_user_status(&state.pool, &admin.id, &id, &body.status).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn delete_user(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    if id == admin.id {
        return Err(AppError::forbidden(
            "You cannot delete your own account from admin.",
        ));
    }
    admin::delete_user(&state.pool, &admin.id, &id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn audit(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
) -> AppResult<Json<Vec<admin::AuditLog>>> {
    Ok(Json(admin::recent_audit(&state.pool, 50).await?))
}
