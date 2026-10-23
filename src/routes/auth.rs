use crate::auth::password::{hash_password, verify_password};
use crate::auth::session::{self, rotate_session};
use crate::auth::{csrf_from_jar, AuthUser, CSRF_COOKIE, SESSION_COOKIE};
use crate::error::{AppError, AppResult};
use crate::extract::JsonBody;
use crate::state::AppState;
use crate::users::{self, User};
use crate::util::{
    new_id, normalize_email, normalize_username, now_rfc3339, random_token, validate_password,
};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use time::Duration as TimeDuration;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub password_confirmation: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub identifier: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
    pub new_password_confirmation: String,
}

#[derive(Deserialize)]
pub struct ChangeEmailRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct UserView {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub account_status: String,
    pub created_at: String,
}

impl From<User> for UserView {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            role: user.role,
            account_status: user.account_status,
            created_at: user.created_at,
        }
    }
}

pub async fn csrf(jar: CookieJar, State(state): State<AppState>) -> impl IntoResponse {
    let token = csrf_from_jar(&jar).unwrap_or_else(random_token);
    let jar = set_csrf_cookie(jar, &token, state.config.cookie_secure());
    (jar, Json(serde_json::json!({ "csrf_token": token })))
}

pub async fn register(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    JsonBody(body): JsonBody<RegisterRequest>,
) -> AppResult<impl IntoResponse> {
    check_rate_limit(&state, "register", &headers)?;

    let username = normalize_username(&body.username)?;
    let email = normalize_email(&body.email)?;
    if body.password != body.password_confirmation {
        return Err(AppError::validation(
            "Password confirmation does not match.",
        ));
    }
    validate_password(&body.password)?;
    let password_hash = hash_password(&body.password)?;

    let user_id = new_id();
    let page_id = new_id();
    let now = now_rfc3339();

    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, role, account_status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'user', 'active', ?5, ?5)",
    )
    .bind(&user_id)
    .bind(&username)
    .bind(&email)
    .bind(&password_hash)
    .bind(&now)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO pages (id, user_id, slug, display_name, bio, avatar_url, cover_url, theme,
            accent_color, status, seo_title, seo_description, created_at, updated_at, published_at)
         VALUES (?1, ?2, ?3, ?4, '', NULL, NULL, 'minimal', '#3EC6FF', 'draft', ?4, '', ?5, ?5, NULL)",
    )
    .bind(&page_id)
    .bind(&user_id)
    .bind(&username)
    .bind(&username)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    finish_login(&state, jar, &user_id, username).await
}

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    JsonBody(body): JsonBody<LoginRequest>,
) -> AppResult<impl IntoResponse> {
    check_rate_limit(&state, "login", &headers)?;

    let invalid = || AppError::unauthorized("Invalid username, email, or password.");
    let user = users::find_by_username_or_email(&state.pool, &body.identifier)
        .await?
        .ok_or_else(invalid)?;
    if !verify_password(&body.password, &user.password_hash)? {
        return Err(invalid());
    }
    if !user.is_active() {
        return Err(AppError::forbidden("This account is disabled."));
    }
    finish_login(&state, jar, &user.id, user.username).await
}

async fn finish_login(
    state: &AppState,
    jar: CookieJar,
    user_id: &str,
    username: String,
) -> AppResult<impl IntoResponse> {
    let old = jar.get(SESSION_COOKIE).map(|c| c.value().to_string());
    let token = rotate_session(
        &state.pool,
        old.as_deref(),
        user_id,
        state.config.session_ttl,
    )
    .await?;
    let jar = set_session_cookie(jar, &token, state);
    let csrf = csrf_from_jar(&jar).unwrap_or_else(random_token);
    let jar = set_csrf_cookie(jar, &csrf, state.config.cookie_secure());
    let user = users::find_by_id(&state.pool, user_id)
        .await?
        .ok_or_else(AppError::internal)?;
    Ok((
        jar,
        Json(serde_json::json!({
            "user": UserView::from(user),
            "username": username,
            "csrf_token": csrf
        })),
    ))
}

pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> AppResult<impl IntoResponse> {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        session::delete_by_raw(&state.pool, cookie.value()).await?;
    }
    let mut expired = Cookie::from(SESSION_COOKIE);
    expired.set_path("/");
    let jar = jar.remove(expired);
    Ok((jar, Json(serde_json::json!({ "ok": true }))))
}

pub async fn me(AuthUser(user): AuthUser) -> Json<UserView> {
    Json(UserView::from(user))
}

pub async fn change_password(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    JsonBody(body): JsonBody<ChangePasswordRequest>,
) -> AppResult<Json<serde_json::Value>> {
    if !verify_password(&body.current_password, &user.password_hash)? {
        return Err(AppError::unauthorized("Current password is incorrect."));
    }
    if body.new_password != body.new_password_confirmation {
        return Err(AppError::validation(
            "Password confirmation does not match.",
        ));
    }
    validate_password(&body.new_password)?;
    let hash = hash_password(&body.new_password)?;
    users::update_password(&state.pool, &user.id, &hash).await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = ?1")
        .bind(&user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn change_email(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    JsonBody(body): JsonBody<ChangeEmailRequest>,
) -> AppResult<Json<UserView>> {
    if !verify_password(&body.password, &user.password_hash)? {
        return Err(AppError::unauthorized("Password is incorrect."));
    }
    let email = normalize_email(&body.email)?;
    users::update_email(&state.pool, &user.id, &email).await?;
    let user = users::find_by_id(&state.pool, &user.id)
        .await?
        .ok_or_else(AppError::internal)?;
    Ok(Json(UserView::from(user)))
}

fn set_session_cookie(jar: CookieJar, token: &str, state: &AppState) -> CookieJar {
    let mut cookie = Cookie::new(SESSION_COOKIE, token.to_string());
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(TimeDuration::seconds(
        i64::try_from(state.config.session_ttl.as_secs()).unwrap_or(i64::MAX),
    ));
    if state.config.cookie_secure() {
        cookie.set_secure(true);
    }
    jar.add(cookie)
}

fn set_csrf_cookie(jar: CookieJar, token: &str, secure: bool) -> CookieJar {
    let mut cookie = Cookie::new(CSRF_COOKIE, token.to_string());
    cookie.set_http_only(false);
    cookie.set_path("/");
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(TimeDuration::hours(24));
    if secure {
        cookie.set_secure(true);
    }
    jar.add(cookie)
}

fn check_rate_limit(state: &AppState, kind: &str, headers: &HeaderMap) -> AppResult<()> {
    if state.config.skip_rate_limit {
        return Ok(());
    }
    state.rate_limiter.check(
        &format!("{kind}:{}", client_key(headers)),
        10,
        Duration::from_secs(900),
    )
}

fn client_key(headers: &HeaderMap) -> String {
    headers
        .get("x-real-ip")
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("local")
        .to_string()
}
