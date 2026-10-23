use crate::config::Config;
use crate::middleware::RateLimiter;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<Config>,
    pub rate_limiter: RateLimiter,
}

impl AppState {
    pub fn new(pool: SqlitePool, config: Config) -> Self {
        Self {
            pool,
            config: Arc::new(config),
            rate_limiter: RateLimiter::default(),
        }
    }
}
