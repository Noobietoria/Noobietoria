/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! AuthService: register / login / validate backed by an in-memory store.

use crate::session::{Session, UserRecord};
use crate::AuthError;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

const SESSION_TTL_HOURS: i64 = 24;

#[derive(Clone)]
pub struct AuthService {
    inner: Arc<Inner>,
}

struct Inner {
    users: RwLock<HashMap<String, UserRecord>>,
    sessions: RwLock<HashMap<String, Session>>,
}

impl AuthService {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                users: RwLock::new(HashMap::new()),
                sessions: RwLock::new(HashMap::new()),
            }),
        }
    }

    /// Register a new user; returns the created record on success.
    pub fn register(
        &self,
        username: &str,
        email: &str,
        password: &str,
    ) -> Result<UserRecord, AuthError> {
        let mut users = self.inner.users.write();
        if users.contains_key(username) {
            return Err(AuthError::UsernameTaken(username.to_string()));
        }
        if users.values().any(|u| u.email == email) {
            return Err(AuthError::EmailTaken(email.to_string()));
        }
        let password_hash = hash_password(password)?;
        let now = Utc::now();
        let user = UserRecord {
            user_id: users.len() as i64 + 1,
            username: username.to_string(),
            email: email.to_string(),
            password_hash,
            display_name: username.to_string(),
            level: 1,
            credits: 50,
            coins: 100,
            created_at: now,
            last_seen: now,
        };
        users.insert(username.to_string(), user.clone());
        Ok(user)
    }

    /// Verify credentials and mint a session token.
    pub fn login(&self, username: &str, password: &str) -> Result<String, AuthError> {
        let user = {
            let users = self.inner.users.read();
            users.get(username).cloned().ok_or_else(|| {
                AuthError::UserNotFound(username.to_string())
            })?
        };
        if !verify_password(password, &user.password_hash) {
            return Err(AuthError::InvalidCredentials);
        }
        let now = Utc::now();
        let token = format!("nb_{}", uuid::Uuid::new_v4());
        let session = Session {
            token: token.clone(),
            user_id: user.user_id,
            username: user.username.clone(),
            issued_at: now,
            expires_at: now + Duration::hours(SESSION_TTL_HOURS),
        };
        self.inner.sessions.write().insert(token.clone(), session);
        Ok(token)
    }

    /// Look up the user for a session token, refreshing `last_seen`.
    pub fn validate(&self, token: &str) -> Result<UserRecord, AuthError> {
        let username = {
            let sessions = self.inner.sessions.read();
            let session = sessions.get(token).ok_or(AuthError::InvalidToken)?;
            if session.expires_at < Utc::now() {
                return Err(AuthError::SessionExpired);
            }
            session.username.clone()
        };
        let mut users = self.inner.users.write();
        let user = users
            .get_mut(&username)
            .ok_or_else(|| AuthError::UserNotFound(username.clone()))?;
        user.last_seen = Utc::now();
        Ok(user.clone())
    }

    /// Revoke a session token (logout).
    pub fn logout(&self, token: &str) {
        self.inner.sessions.write().remove(token);
    }

    /// Drop expired sessions; returns how many were removed.
    pub fn cleanup_expired(&self) -> usize {
        let now = Utc::now();
        let mut sessions = self.inner.sessions.write();
        let before = sessions.len();
        sessions.retain(|_, s| s.expires_at >= now);
        before - sessions.len()
    }
}

impl Default for AuthService {
    fn default() -> Self {
        Self::new()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_login() {
        let auth = AuthService::new();
        let user = auth.register("alice", "a@example.com", "pw12345").unwrap();
        assert_eq!(user.username, "alice");
        let token = auth.login("alice", "pw12345").unwrap();
        let got = auth.validate(&token).unwrap();
        assert_eq!(got.user_id, user.user_id);
    }

    #[test]
    fn duplicate_username_rejected() {
        let auth = AuthService::new();
        auth.register("bob", "b1@example.com", "x").unwrap();
        assert!(matches!(
            auth.register("bob", "b2@example.com", "y"),
            Err(AuthError::UsernameTaken(_))
        ));
    }

    #[test]
    fn wrong_password_rejected() {
        let auth = AuthService::new();
        auth.register("carol", "c@example.com", "right").unwrap();
        assert!(matches!(
            auth.login("carol", "wrong"),
            Err(AuthError::InvalidCredentials)
        ));
    }
}