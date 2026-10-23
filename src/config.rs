use std::env;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: IpAddr,
    pub port: u16,
    pub database_path: PathBuf,
    pub session_ttl: Duration,
    pub secure_cookies: bool,
    pub bootstrap_admin: Option<String>,
    pub asset_root: PathBuf,
    pub skip_rate_limit: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        load_dotenv();

        let host = parse_host(env_or("LINKO_HOST", "127.0.0.1"))?;
        let port = parse_port(env_or("LINKO_PORT", "3000"))?;
        let database_path = PathBuf::from(env_or("LINKO_DATABASE_PATH", "data/linko.db"));
        let hours: u64 = env_or("LINKO_SESSION_HOURS", "168")
            .parse()
            .map_err(|_| "LINKO_SESSION_HOURS must be a positive integer".to_string())?;
        if hours == 0 {
            return Err("LINKO_SESSION_HOURS must be greater than 0".into());
        }
        let secure_cookies = parse_bool(&env_or("LINKO_SECURE_COOKIES", "false"))?;
        let bootstrap_admin = env::var("LINKO_BOOTSTRAP_ADMIN")
            .ok()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty());
        let asset_root = env::var("LINKO_ASSET_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")));

        Ok(Self {
            host,
            port,
            database_path,
            session_ttl: Duration::from_secs(hours.saturating_mul(3600)),
            secure_cookies,
            bootstrap_admin,
            asset_root,
            skip_rate_limit: false,
        })
    }

    pub fn for_tests(database_path: PathBuf, session_ttl: Duration) -> Self {
        Self {
            host: "127.0.0.1".parse().expect("loopback"),
            port: 0,
            database_path,
            session_ttl,
            secure_cookies: false,
            bootstrap_admin: None,
            asset_root: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            skip_rate_limit: true,
        }
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn cookie_secure(&self) -> bool {
        self.secure_cookies
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn parse_host(value: String) -> Result<IpAddr, String> {
    value
        .parse()
        .map_err(|_| format!("LINKO_HOST is not a valid IP address: {value}"))
}

fn parse_port(value: String) -> Result<u16, String> {
    value
        .parse()
        .map_err(|_| format!("LINKO_PORT is not a valid port: {value}"))
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!("invalid boolean value: {value}")),
    }
}

fn load_dotenv() {
    let candidates = [
        PathBuf::from(".env"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".env"),
    ];
    for path in candidates {
        if path.is_file() {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                apply_dotenv(&contents);
            }
        }
    }
}

fn apply_dotenv(contents: &str) {
    for raw in contents.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || env::var_os(key).is_some() {
            continue;
        }
        let value = value.trim().trim_matches('"').trim_matches('\'');
        // Safety: process-local configuration before workers start.
        unsafe { env::set_var(key, value) };
    }
}
