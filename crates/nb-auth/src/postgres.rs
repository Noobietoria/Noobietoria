/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! PostgreSQL-backed [`PostgresAuthService`] — production-grade Argon2 auth.

use crate::session::UserRecord;
use crate::AuthError;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{DateTime, Duration, Utc};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;

const SESSION_TTL_HOURS: i64 = 24;

/// PostgreSQL-persistent auth service.
#[derive(Clone)]
pub struct PostgresAuthService {
    pool: PgPool,
}

impl PostgresAuthService {
    /// Connect to a PostgreSQL database and run migrations.
    ///
    /// Accepts any standard `postgres://` or `postgresql://` URL.
    pub async fn connect(database_url: &str) -> Result<Self, AuthError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|e| AuthError::Database(e.to_string()))?;
        let svc = Self { pool };
        svc.migrate().await?;
        Ok(svc)
    }

    async fn migrate(&self) -> Result<(), AuthError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS nb_users (
                user_id       BIGSERIAL PRIMARY KEY,
                username      TEXT NOT NULL UNIQUE,
                email         TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                display_name  TEXT NOT NULL DEFAULT '',
                level         INTEGER NOT NULL DEFAULT 1,
                credits       BIGINT NOT NULL DEFAULT 50,
                coins         BIGINT NOT NULL DEFAULT 100,
                created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
                last_seen     TIMESTAMPTZ NOT NULL DEFAULT now()
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::Database(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS nb_sessions (
                token      TEXT PRIMARY KEY,
                user_id    BIGINT NOT NULL REFERENCES nb_users(user_id) ON DELETE CASCADE,
                username   TEXT NOT NULL,
                issued_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
                expires_at TIMESTAMPTZ NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AuthError::Database(e.to_string()))?;

        // Index for fast cleanup_expired scans.
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_nb_sessions_expires ON nb_sessions(expires_at)")
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
        let dup: Option<String> =
            sqlx::query("SELECT username FROM nb_users WHERE username = $1")
                .bind(username)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| AuthError::Database(e.to_string()))?
                .map(|r| r.get("username"));
        if dup.is_some() {
            return Err(AuthError::UsernameTaken(username.to_string()));
        }
        let dup: Option<String> = sqlx::query("SELECT email FROM nb_users WHERE email = $1")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AuthError::Database(e.to_string()))?
            .map(|r| r.get("email"));
        if dup.is_some() {
            return Err(AuthError::EmailTaken(email.to_string()));
        }

        let hash = hash_password(password)?;
        let row = sqlx::query(
            "INSERT INTO nb_users (username, email, password_hash, display_name)
             VALUES ($1, $2, $3, $4)
             RETURNING user_id, created_at, last_seen, level, credits, coins",
        )
        .bind(username)
        .bind(email)
        .bind(&hash)
        .bind(username)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AuthError::Database(e.to_string()))?;

        Ok(UserRecord {
            user_id: row.get("user_id"),
            username: username.to_string(),
            email: email.to_string(),
            password_hash: hash,
            display_name: username.to_string(),
            level: row.get("level"),
            credits: row.get("credits"),
            coins: row.get("coins"),
            created_at: row.get::<DateTime<Utc>, _>("created_at"),
            last_seen: row.get::<DateTime<Utc>, _>("last_seen"),
        })
    }

    /// Verify credentials and mint a session token.
    pub async fn login(&self, username: &str, password: &str) -> Result<String, AuthError> {
        let row = sqlx::query("SELECT user_id, password_hash FROM nb_users WHERE username = $1")
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
        let expires = Utc::now() + Duration::hours(SESSION_TTL_HOURS);
        sqlx::query(
            "INSERT INTO nb_sessions (token, user_id, username, expires_at)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(&token)
        .bind(user_id)
        .bind(username)
        .bind(expires)
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
             WHERE s.token = $1",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AuthError::Database(e.to_string()))?
        .ok_or(AuthError::InvalidToken)?;

        let expires_at: DateTime<Utc> = row.get("expires_at");
        if expires_at < Utc::now() {
            return Err(AuthError::SessionExpired);
        }

        sqlx::query("UPDATE nb_users SET last_seen = now() WHERE user_id = $1")
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
            created_at: row.get("created_at"),
            last_seen: Utc::now(),
        })
    }

    /// Revoke a session token (logout).
    pub async fn logout(&self, token: &str) -> Result<(), AuthError> {
        sqlx::query("DELETE FROM nb_sessions WHERE token = $1")
            .bind(token)
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::Database(e.to_string()))?;
        Ok(())
    }

    /// Drop expired sessions; returns how many were removed.
    pub async fn cleanup_expired(&self) -> Result<u64, AuthError> {
        let result = sqlx::query("DELETE FROM nb_sessions WHERE expires_at < now()")
            .execute(&self.pool)
            .await
            .map_err(|e| AuthError::Database(e.to_string()))?;
        Ok(result.rows_affected())
    }
}

impl crate::backend::AuthBackend for PostgresAuthService {
    async fn register(
        &self,
        username: &str,
        email: &str,
        password: &str,
    ) -> Result<UserRecord, AuthError> {
        PostgresAuthService::register(self, username, email, password).await
    }
    async fn login(&self, username: &str, password: &str) -> Result<String, AuthError> {
        PostgresAuthService::login(self, username, password).await
    }
    async fn validate(&self, token: &str) -> Result<UserRecord, AuthError> {
        PostgresAuthService::validate(self, token).await
    }
    async fn logout(&self, token: &str) -> Result<(), AuthError> {
        PostgresAuthService::logout(self, token).await
    }
    async fn cleanup_expired(&self) -> Result<u64, AuthError> {
        PostgresAuthService::cleanup_expired(self).await
    }
}

fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt_bytes: [u8; 16] = rand::random();
    let salt = SaltString::encode_b64(&salt_bytes)
        .map_err(|e| AuthError::Database(e.to_string()))?;
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