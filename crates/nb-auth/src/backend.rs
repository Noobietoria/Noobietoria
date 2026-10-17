/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! Common [`AuthBackend`] trait implemented by SQLite and PostgreSQL services.

use crate::session::UserRecord;
use crate::AuthError;

/// A durable authentication backend.
///
/// Implemented by [`crate::sqlite::SqliteAuthService`] and
/// [`crate::postgres::PostgresAuthService`].  Both share the same
/// Argon2 hashing, UUID session tokens and 24-hour TTL.
pub trait AuthBackend: Send + Sync {
    /// Register a new user with an Argon2-hashed password.
    fn register(
        &self,
        username: &str,
        email: &str,
        password: &str,
    ) -> impl std::future::Future<Output = Result<UserRecord, AuthError>> + Send;

    /// Verify credentials and mint a session token.
    fn login(
        &self,
        username: &str,
        password: &str,
    ) -> impl std::future::Future<Output = Result<String, AuthError>> + Send;

    /// Look up the user for a session token, refreshing `last_seen`.
    fn validate(
        &self,
        token: &str,
    ) -> impl std::future::Future<Output = Result<UserRecord, AuthError>> + Send;

    /// Revoke a session token (logout).
    fn logout(
        &self,
        token: &str,
    ) -> impl std::future::Future<Output = Result<(), AuthError>> + Send;

    /// Drop expired sessions; returns how many were removed.
    fn cleanup_expired(&self) -> impl std::future::Future<Output = Result<u64, AuthError>> + Send;
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::sqlite::SqliteAuthService;

    // Compile-time check: SqliteAuthService implements AuthBackend.
    fn _assert_impl<T: AuthBackend>() {}
    #[test]
    fn sqlite_implements_trait() {
        _assert_impl::<SqliteAuthService>();
    }
}
