/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! User and session data types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRecord {
    pub user_id: i64,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub display_name: String,
    pub level: i32,
    pub credits: i64,
    pub coins: i64,
    pub created_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub token: String,
    pub user_id: i64,
    pub username: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}