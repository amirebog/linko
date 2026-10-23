pub mod admin;
pub mod auth;
pub mod blocks;
pub mod html;
pub mod pages;
pub mod public;

use crate::middleware;
use crate::state::AppState;
use axum::extract::DefaultBodyLimit;
use axum::http::header::{HeaderName, HeaderValue};
use axum::routing::{delete, get, patch, post};
use axum::Router;
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

pub fn app_router(state: AppState) -> Router {
    let static_dir = state.config.asset_root.join("static");
    let api = Router::new()
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        .route("/auth/csrf", get(auth::csrf))
        .route("/auth/change-password", post(auth::change_password))
        .route("/auth/email", patch(auth::change_email))
        .route("/pages/me", get(pages::get_me).patch(pages::patch_me))
        .route("/pages/me/publish", post(pages::publish))
        .route("/pages/me/unpublish", post(pages::unpublish))
        .route("/pages/me/preview", get(pages::preview))
        .route("/pages/me/blocks", get(blocks::list).post(blocks::create))
        .route("/pages/me/blocks/reorder", post(blocks::reorder))
        .route(
            "/pages/me/blocks/{id}",
            patch(blocks::update).delete(blocks::delete),
        )
        .route("/pages/me/blocks/{id}/duplicate", post(blocks::duplicate))
        .route("/public/pages/{username}", get(public::get_page))
        .route("/admin/stats", get(admin::stats))
        .route("/admin/users", get(admin::users))
        .route("/admin/users/{id}/status", patch(admin::set_status))
        .route("/admin/users/{id}", delete(admin::delete_user))
        .route("/admin/audit", get(admin::audit));

    Router::new()
        .route("/healthz", get(health))
        .route("/", get(html::home))
        .route("/login", get(html::login))
        .route("/register", get(html::register))
        .route("/dashboard", get(html::dashboard))
        .route("/editor", get(html::editor))
        .route("/admin", get(html::admin))
        .route("/preview", get(html::preview))
        .route("/robots.txt", get(html::robots))
        .nest("/api", api)
        .nest_service("/static", ServeDir::new(static_dir))
        .route("/{username}", get(html::public_page))
        .fallback(html::not_found)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(DefaultBodyLimit::max(1024 * 1024))
                .layer(axum::middleware::from_fn(middleware::assign_request_id))
                .layer(axum::middleware::from_fn(middleware::security_headers))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    middleware::csrf_protect,
                ))
                .layer(SetResponseHeaderLayer::if_not_present(
                    HeaderName::from_static("cache-control"),
                    HeaderValue::from_static("no-store"),
                )),
        )
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}
