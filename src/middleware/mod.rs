use crate::auth::{CSRF_COOKIE, CSRF_HEADER};
use crate::error::AppError;
use crate::state::AppState;
use crate::util::random_token;
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, Method};
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use time::Duration as TimeDuration;
use tokio::task_local;

task_local! {
    static REQUEST_ID: String;
}

#[derive(Clone, Default)]
pub struct RateLimiter {
    inner: Arc<DashMap<String, Window>>,
}

#[derive(Clone)]
struct Window {
    started: Instant,
    count: u32,
}

impl RateLimiter {
    pub fn check(&self, key: &str, limit: u32, window: Duration) -> Result<(), AppError> {
        let now = Instant::now();
        let mut entry = self.inner.entry(key.to_string()).or_insert(Window {
            started: now,
            count: 0,
        });
        if now.duration_since(entry.started) > window {
            entry.started = now;
            entry.count = 0;
        }
        if entry.count >= limit {
            return Err(AppError::too_many(
                "Too many attempts. Please wait a few minutes and try again.",
            ));
        }
        entry.count += 1;
        Ok(())
    }
}

pub fn current_request_id() -> String {
    REQUEST_ID
        .try_with(|id| id.clone())
        .unwrap_or_else(|_| "unknown".into())
}

pub async fn assign_request_id(req: Request, next: Next) -> Response {
    let id = random_token()[..16].to_string();
    REQUEST_ID
        .scope(id.clone(), async move {
            let mut res = next.run(req).await;
            if let Ok(value) = HeaderValue::from_str(&id) {
                res.headers_mut().insert("x-request-id", value);
            }
            res
        })
        .await
}

pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; img-src 'self' https: data:; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; script-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'",
        ),
    );
    res
}

pub async fn csrf_protect(
    State(state): State<AppState>,
    jar: CookieJar,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let method = req.method().clone();
    let unsafe_method = matches!(
        method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );

    if unsafe_method {
        if let Some(origin) = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
        {
            if let Ok(url) = url::Url::parse(origin) {
                let host_ok = req
                    .headers()
                    .get(header::HOST)
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|host| url.host_str().is_some_and(|h| host.starts_with(h)));
                if !host_ok {
                    return Err(AppError::forbidden("Invalid request origin."));
                }
            }
        }

        let cookie_token = jar.get(CSRF_COOKIE).map(|c| c.value().to_string());
        let header_token = req
            .headers()
            .get(CSRF_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        match (cookie_token, header_token) {
            (Some(a), Some(b)) if !a.is_empty() && a == b => {}
            _ => return Err(AppError::forbidden("CSRF token missing or invalid.")),
        }
    }

    let mut res = next.run(req).await;
    if jar.get(CSRF_COOKIE).is_none() {
        let token = random_token();
        let mut cookie = Cookie::new(CSRF_COOKIE, token);
        cookie.set_path("/");
        cookie.set_same_site(SameSite::Lax);
        cookie.set_http_only(false);
        cookie.set_max_age(TimeDuration::hours(24));
        if state.config.cookie_secure() {
            cookie.set_secure(true);
        }
        if let Ok(value) = HeaderValue::from_str(&cookie.to_string()) {
            res.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    Ok(res)
}
