pub mod admin;
pub mod auth;
pub mod blocks;
pub mod config;
pub mod db;
pub mod error;
pub mod extract;
pub mod middleware;
pub mod pages;
pub mod routes;
pub mod state;
pub mod users;
pub mod util;

use crate::config::Config;
use crate::state::AppState;
use axum::Router;
use sqlx::SqlitePool;

pub async fn build_state(config: Config) -> Result<AppState, String> {
    let pool = db::init_pool(&config.database_path).await?;
    db::bootstrap_admin(&pool, &config).await?;
    Ok(AppState::new(pool, config))
}

pub fn router(state: AppState) -> Router {
    routes::app_router(state)
}

pub async fn router_with_pool(pool: SqlitePool, config: Config) -> Router {
    router(AppState::new(pool, config))
}
