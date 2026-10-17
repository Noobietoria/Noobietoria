/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! # nb-auth
//! Authentication and user session management for Noobietoria.

pub mod auth;
pub mod backend;
pub mod session;
#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(feature = "postgres")]
pub mod postgres;

pub use auth::AuthService;
pub use backend::AuthBackend;
pub use session::{Session, UserRecord};
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteAuthService;
#[cfg(feature = "postgres")]
pub use postgres::PostgresAuthService;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("user not found: {0}")]
    UserNotFound(String),
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("username already taken: {0}")]
    UsernameTaken(String),
    #[error("email already taken: {0}")]
    EmailTaken(String),
    #[error("session expired")]
    SessionExpired,
    #[error("invalid token")]
    InvalidToken,
    #[error("database error: {0}")]
    Database(String),
}