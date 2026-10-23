pub mod password;
pub mod session;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::users::{User, UserRole};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::CookieJar;

pub const SESSION_COOKIE: &str = "linko_session";
pub const CSRF_COOKIE: &str = "linko_csrf";
pub const CSRF_HEADER: &str = "x-csrf-token";

#[derive(Clone, Debug)]
pub struct AuthUser(pub User);

#[derive(Clone, Debug)]
pub struct AdminUser(pub User);

#[derive(Clone, Debug)]
pub struct OptionalUser(pub Option<User>);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match load_user_from_parts(parts, state).await? {
            Some(user) => Ok(AuthUser(user)),
            None => Err(AppError::unauthorized("Please sign in to continue.")),
        }
    }
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(user) = AuthUser::from_request_parts(parts, state).await?;
        if user.role != UserRole::Admin {
            return Err(AppError::forbidden("Administrator access is required."));
        }
        Ok(AdminUser(user))
    }
}

impl FromRequestParts<AppState> for OptionalUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(OptionalUser(load_user_from_parts(parts, state).await?))
    }
}

async fn load_user_from_parts(parts: &mut Parts, state: &AppState) -> AppResult<Option<User>> {
    let jar = CookieJar::from_headers(&parts.headers);
    let Some(cookie) = jar.get(SESSION_COOKIE) else {
        return Ok(None);
    };
    session::user_from_token(&state.pool, cookie.value()).await
}

pub fn csrf_from_jar(jar: &CookieJar) -> Option<String> {
    jar.get(CSRF_COOKIE).map(|c| c.value().to_string())
}
