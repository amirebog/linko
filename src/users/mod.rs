use crate::error::AppResult;
use crate::util::now_rfc3339;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    User,
    Admin,
}

impl UserRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Admin => "admin",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AccountStatus {
    Active,
    Disabled,
}

impl AccountStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub account_status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl User {
    pub fn role_enum(&self) -> UserRole {
        if self.role == "admin" {
            UserRole::Admin
        } else {
            UserRole::User
        }
    }

    pub fn is_admin(&self) -> bool {
        self.role_enum() == UserRole::Admin
    }

    pub fn is_active(&self) -> bool {
        self.account_status == AccountStatus::Active.as_str()
    }
}

impl PartialEq<UserRole> for String {
    fn eq(&self, other: &UserRole) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<AccountStatus> for String {
    fn eq(&self, other: &AccountStatus) -> bool {
        self == other.as_str()
    }
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> AppResult<Option<User>> {
    Ok(sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, role, account_status, created_at, updated_at
         FROM users WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn find_by_username_or_email(
    pool: &SqlitePool,
    identifier: &str,
) -> AppResult<Option<User>> {
    let ident = identifier.trim().to_ascii_lowercase();
    Ok(sqlx::query_as::<_, User>(
        "SELECT id, username, email, password_hash, role, account_status, created_at, updated_at
         FROM users WHERE username = ?1 OR email = ?1",
    )
    .bind(ident)
    .fetch_optional(pool)
    .await?)
}

pub async fn update_password(
    pool: &SqlitePool,
    user_id: &str,
    password_hash: &str,
) -> AppResult<()> {
    sqlx::query("UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(password_hash)
        .bind(now_rfc3339())
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_email(pool: &SqlitePool, user_id: &str, email: &str) -> AppResult<()> {
    sqlx::query("UPDATE users SET email = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(email)
        .bind(now_rfc3339())
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
