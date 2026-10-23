use crate::error::{AppError, AppResult};
use chrono::{Duration, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const RESERVED_USERNAMES: &[&str] = &[
    "admin",
    "api",
    "static",
    "login",
    "register",
    "dashboard",
    "editor",
    "preview",
    "health",
    "healthz",
    "assets",
    "settings",
    "account",
    "css",
    "js",
    "robots",
    "favicon",
    "well-known",
    "linko",
    "support",
    "help",
    "null",
    "undefined",
];

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

pub fn rfc3339_plus(duration: std::time::Duration) -> String {
    let secs = i64::try_from(duration.as_secs()).unwrap_or(i64::MAX);
    (Utc::now() + Duration::seconds(secs)).to_rfc3339()
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

pub fn normalize_username(raw: &str) -> AppResult<String> {
    let username = raw.trim().to_ascii_lowercase();
    if username.len() < 3 || username.len() > 24 {
        return Err(AppError::validation(
            "Username must be between 3 and 24 characters.",
        ));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(AppError::validation(
            "Username may contain only letters, numbers, and underscores.",
        ));
    }
    if !username.chars().any(|c| c.is_ascii_alphabetic()) {
        return Err(AppError::validation(
            "Username must include at least one letter.",
        ));
    }
    if RESERVED_USERNAMES.contains(&username.as_str()) {
        return Err(AppError::validation("This username is reserved."));
    }
    Ok(username)
}

pub fn normalize_email(raw: &str) -> AppResult<String> {
    let email = raw.trim().to_ascii_lowercase();
    if email.len() > 254 {
        return Err(AppError::validation("Email is too long."));
    }
    let Some((local, domain)) = email.split_once('@') else {
        return Err(AppError::validation("Enter a valid email address."));
    };
    if local.is_empty() || domain.is_empty() || !domain.contains('.') {
        return Err(AppError::validation("Enter a valid email address."));
    }
    if !email
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '.' | '_' | '+' | '-' | '%'))
    {
        return Err(AppError::validation("Enter a valid email address."));
    }
    Ok(email)
}

pub fn validate_password(password: &str) -> AppResult<()> {
    if password.len() < 10 {
        return Err(AppError::validation(
            "Password must be at least 10 characters.",
        ));
    }
    if password.len() > 128 {
        return Err(AppError::validation("Password is too long."));
    }
    Ok(())
}

pub fn validate_hex_color(raw: &str) -> AppResult<String> {
    let value = raw.trim();
    let rest = value
        .strip_prefix('#')
        .ok_or_else(|| AppError::validation("Accent color must be a hex value such as #3EC6FF."))?;
    if rest.len() != 3 && rest.len() != 6 {
        return Err(AppError::validation(
            "Accent color must be #RGB or #RRGGBB.",
        ));
    }
    if !rest.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::validation(
            "Accent color contains invalid characters.",
        ));
    }
    Ok(format!("#{}", rest.to_ascii_uppercase()))
}

pub fn sanitize_text(raw: &str, max: usize) -> AppResult<String> {
    let text: String = raw
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();
    let text = text.trim().to_string();
    if text.len() > max {
        return Err(AppError::validation(format!(
            "Text exceeds the maximum length of {max} characters."
        )));
    }
    Ok(text)
}

pub fn optional_text(raw: Option<&str>, max: usize) -> AppResult<Option<String>> {
    match raw {
        None => Ok(None),
        Some(value) if value.trim().is_empty() => Ok(None),
        Some(value) => Ok(Some(sanitize_text(value, max)?)),
    }
}

pub fn validate_http_url(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.len() > 2048 {
        return Err(AppError::validation("URL is too long."));
    }
    let parsed =
        url::Url::parse(trimmed).map_err(|_| AppError::validation("Enter a valid http(s) URL."))?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => {
            return Err(AppError::validation(
                "Only http and https URLs are allowed.",
            ))
        }
    }
    if parsed.host_str().is_none() {
        return Err(AppError::validation("Enter a valid http(s) URL."));
    }
    Ok(trimmed.to_string())
}

pub fn optional_http_url(raw: Option<&str>) -> AppResult<Option<String>> {
    match raw {
        None => Ok(None),
        Some(value) if value.trim().is_empty() => Ok(None),
        Some(value) => Ok(Some(validate_http_url(value)?)),
    }
}

pub fn escape(value: &str) -> String {
    html_escape::encode_safe(value).to_string()
}
