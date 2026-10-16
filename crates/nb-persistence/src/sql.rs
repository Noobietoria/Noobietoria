/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shared SQL helpers for persistence backends.
//!
//! Migrations live in `crates/nb-persistence/migrations/` and are applied by
//! `SqlKvStore` when `run_migrations` is true.

/// DDL for the key-value table, used when auto-creating the schema.
pub const KV_ENTRIES_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS kv_entries (
    key   TEXT PRIMARY KEY,
    value JSON NOT NULL
)
"#;

/// DDL for an object-store table (alternative to file/S3 backends).
pub const OBJECT_ENTRIES_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS object_entries (
    key   TEXT PRIMARY KEY,
    data  BLOB NOT NULL
)
"#;
