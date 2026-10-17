/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! SQLite-backed [`SqliteAuthService`] — durable Argon2 auth with SQL persistence.

use crate::session::UserRecord;
use crate::AuthError;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{DateTime, Duration, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::str::FromStr;

const SESSION_TTL_HOURS: i64 = 24;

/// SQLite-persistent auth service.
#[derive(Clone)]
pub struct SqliteAuthService {
    pool: SqlitePool,
}

impl SqliteAuthService {
    /// Open (or create) a SQLite database and run migrations.
    pub async fn connect(database_url: &str) -> Result<Self, AuthError> {
        let opts = SqliteConnectOptions::from_str(database_url)
            .map_err(|e| AuthError::Database(e.to_string()))?
            .create_if_missing(true);
        // `:memory:` databases are per-connection: a pool > 1 would split
        // reads and writes across isolated databases. Pin to one connection.
        let max_connections = if database_url.contains(":memory:") {
            1
        } else {
            5
        };
        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .connect_with(opts)
            .await
            .map_err(|e| AuthError::Database(e.to_string()))?;
        let svc = Self { pool };
        svc.migrate().await?;
        Ok(svc)
    }

    /// In-memory SQLite for tests.
    #[cfg(test)]
    pub async fn in_memory() -> Result<Self, AuthError> {
        Self::connect("sqlite::memory:").await
    }

    async fn migrate(&self) -> Result<(), AuthError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS nb_users (
                user_id       INTEGER PRIMARY KEY AUTOINCREMENT,
                username      TEXT NOT NULL UNIQUE,
                email         TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                display_name  TEXT NOT NULL DEFAULT '',
                level         INTEGER NOT NULL DEFAULT 1,
                credits       INTEGER NOT NULL DEFAULT 50,
                coins         INTEGER NOT NULL DEFAULT 100,
                created_at    TEXT NOT NULL,
                last_seen     TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::Database(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS nb_sessions (
                token      TEXT PRIMARY KEY,
                user_id    INTEGER NOT NULL,
                username   TEXT NOT NULL,
                issued_at  TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES nb_users(user_id)
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::Database(e.to_string()))?;
        Ok(())
    }

    /// Register a new user with Argon2-hashed password.
    pub async fn register(
        &self,
        username: &str,
        email: &str,
        password: &str,
    ) -> Result<UserRecord, AuthError> {
        let dup: Option<String> = sqlx::query("SELECT username FROM nb_users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AuthError::Database(e.to_string()))?
            .map(|r| r.get("username"));
        if dup.is_some() {
            return Err(AuthError::UsernameTaken(username.to_string()));
        }
        let dup: Option<String> = sqlx::query("SELECT email FROM nb_users WHERE email = ?")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AuthError::Database(e.to_string()))?
            .map(|r| r.get("email"));
        if dup.is_some() {
            return Err(AuthError::EmailTaken(email.to_string()));
        }

        let hash = hash_password(password)?;
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "INSERT INTO nb_users (username, email, password_hash, display_name, created_at, last_seen)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(username)
        .bind(email)
        .bind(&hash)
        .bind(username)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::Database(e.to_string()))?;

        Ok(UserRecord {
            user_id: result.last_insert_rowid(),
            username: username.to_string(),
            email: email.to_string(),
            password_hash: hash,
            display_name: username.to_string(),
            level: 1,
            credits: 50,
            coins: 100,
            created_at: Utc::now(),
            last_seen: Utc::now(),
        })
    }

    /// Verify credentials and mint a session token.
    pub async fn login(&self, username: &str, password: &str) -> Result<String, AuthError> {
        let row = sqlx::query("SELECT user_id, password_hash FROM nb_users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AuthError::Database(e.to_string()))?
            .ok_or_else(|| AuthError::UserNotFound(username.to_string()))?;

        let user_id: i64 = row.get("user_id");
        let stored_hash: String = row.get("password_hash");
        if !verify_password(password, &stored_hash) {
            return Err(AuthError::InvalidCredentials);
        }

        let token = format!("nb_{}", uuid::Uuid::new_v4());
        let now = Utc::now();
        let expires = now + Duration::hours(SESSION_TTL_HOURS);
        sqlx::query(
            "INSERT INTO nb_sessions (token, user_id, username, issued_at, expires_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&token)
        .bind(user_id)
        .bind(username)
        .bind(now.to_rfc3339())
        .bind(expires.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::Database(e.to_string()))?;
        Ok(token)
    }

    /// Look up the user for a session token, refreshing `last_seen`.
    pub async fn validate(&self, token: &str) -> Result<UserRecord, AuthError> {
        let row = sqlx::query(
            "SELECT s.user_id, s.username, s.expires_at,
                    u.email, u.password_hash, u.display_name,
                    u.level, u.credits, u.coins, u.created_at
             FROM nb_sessions s
             JOIN nb_users u ON u.user_id = s.user_id
             WHERE s.token = ?",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Database(e.to_string()))?
        .ok_or(AuthError::InvalidToken)?;

        let expires_at = parse_ts(&row.get::<String, _>("expires_at"));
        if expires_at < Utc::now() {
            return Err(AuthError::SessionExpired);
        }

        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE nb_users SET last_seen = ? WHERE user_id = ?")
            .bind(&now)
            .bind(row.get::<i64, _>("user_id"))
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::Database(e.to_string()))?;

        Ok(UserRecord {
            user_id: row.get("user_id"),
            username: row.get("username"),
            email: row.get("email"),
            password_hash: row.get("password_hash"),
            display_name: row.get("display_name"),
            level: row.get("level"),
            credits: row.get("credits"),
            coins: row.get("coins"),
            created_at: parse_ts(&row.get::<String, _>("created_at")),
            last_seen: Utc::now(),
        })
    }

    /// Revoke a session token (logout).
    pub async fn logout(&self, token: &str) -> Result<(), AuthError> {
        sqlx::query("DELETE FROM nb_sessions WHERE token = ?")
            .bind(token)
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::Database(e.to_string()))?;
        Ok(())
    }

    /// Drop expired sessions; returns how many were removed.
    pub async fn cleanup_expired(&self) -> Result<u64, AuthError> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query("DELETE FROM nb_sessions WHERE expires_at < ?")
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::Database(e.to_string()))?;
        Ok(result.rows_affected())
    }
}

impl crate::backend::AuthBackend for SqliteAuthService {
    async fn register(
        &self,
        username: &str,
        email: &str,
        password: &str,
    ) -> Result<UserRecord, AuthError> {
        SqliteAuthService::register(self, username, email, password).await
    }
    async fn login(&self, username: &str, password: &str) -> Result<String, AuthError> {
        SqliteAuthService::login(self, username, password).await
    }
    async fn validate(&self, token: &str) -> Result<UserRecord, AuthError> {
        SqliteAuthService::validate(self, token).await
    }
    async fn logout(&self, token: &str) -> Result<(), AuthError> {
        SqliteAuthService::logout(self, token).await
    }
    async fn cleanup_expired(&self) -> Result<u64, AuthError> {
        SqliteAuthService::cleanup_expired(self).await
    }
}

fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt_bytes: [u8; 16] = rand::random();
    let salt =
        SaltString::encode_b64(&salt_bytes).map_err(|e| AuthError::Database(e.to_string()))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AuthError::Database(e.to_string()))
}

fn verify_password(password: &str, stored: &str) -> bool {
    PasswordHash::new(stored)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

fn parse_ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_and_login() {
        let auth = SqliteAuthService::in_memory().await.unwrap();
        let user = auth
            .register("alice", "a@test.com", "pw12345")
            .await
            .unwrap();
        assert_eq!(user.username, "alice");
        let token = auth.login("alice", "pw12345").await.unwrap();
        let got = auth.validate(&token).await.unwrap();
        assert_eq!(got.user_id, user.user_id);
    }

    #[tokio::test]
    async fn duplicate_username_rejected() {
        let auth = SqliteAuthService::in_memory().await.unwrap();
        auth.register("bob", "b1@test.com", "x").await.unwrap();
        let err = auth.register("bob", "b2@test.com", "y").await.unwrap_err();
        assert!(matches!(err, AuthError::UsernameTaken(_)));
    }

    #[tokio::test]
    async fn wrong_password_rejected() {
        let auth = SqliteAuthService::in_memory().await.unwrap();
        auth.register("carol", "c@test.com", "right").await.unwrap();
        let err = auth.login("carol", "wrong").await.unwrap_err();
        assert!(matches!(err, AuthError::InvalidCredentials));
    }

    #[tokio::test]
    async fn logout_revokes_token() {
        let auth = SqliteAuthService::in_memory().await.unwrap();
        auth.register("dave", "d@test.com", "pw").await.unwrap();
        let token = auth.login("dave", "pw").await.unwrap();
        assert!(auth.validate(&token).await.is_ok());
        auth.logout(&token).await.unwrap();
        assert!(matches!(
            auth.validate(&token).await,
            Err(AuthError::InvalidToken)
        ));
    }

    #[tokio::test]
    async fn persistence_survives_reconnect() {
        let dir = std::env::temp_dir().join(format!("nb_auth_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.join("test.db").display());

        {
            let auth = SqliteAuthService::connect(&url).await.unwrap();
            auth.register("eve", "e@test.com", "secret").await.unwrap();
        } // pool dropped

        let auth = SqliteAuthService::connect(&url).await.unwrap();
        let token = auth.login("eve", "secret").await.unwrap();
        let user = auth.validate(&token).await.unwrap();
        assert_eq!(user.username, "eve");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn cleanup_removes_expired() {
        let auth = SqliteAuthService::in_memory().await.unwrap();
        auth.register("frank", "f@test.com", "pw").await.unwrap();
        let token = auth.login("frank", "pw").await.unwrap();
        sqlx::query("UPDATE nb_sessions SET expires_at='2000-01-01T00:00:00Z' WHERE token=?")
            .bind(&token)
            .execute(&auth.pool)
            .await
            .unwrap();
        assert_eq!(auth.cleanup_expired().await.unwrap(), 1);
        assert!(matches!(
            auth.validate(&token).await,
            Err(AuthError::InvalidToken)
        ));
    }
}
