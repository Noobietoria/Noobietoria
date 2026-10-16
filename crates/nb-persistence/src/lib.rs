/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! # nb-persistence
//!
//! Pluggable, async backends for the two kinds of durable storage Noobietoria
//! needs:
//!
//! * [`KeyValueStore`] — key → JSON value, used by `DataStoreService` and the
//!   gameplay keyed stores (wallets, inventories, leaderboards, …).
//! * [`ObjectStore`] — named binary objects, used for `.nbw` world files, place
//!   assets, screenshots and user uploads.
//!
//! Each trait has an in-memory implementation (zero config, good for tests and
//! local development) and one or more production backends:
//!
//! * `KeyValueStore`: `SqlKvStore` (SQLite via `sqlx`, and any
//!   `sqlx::any` driver — Postgres, MySQL, CockroachDB) when the `sqlite` /
//!   `any-db` feature is on.
//! * `ObjectStore`: `LocalFileObjectStore` (on-disk directory) and
//!   `S3ObjectStore` (S3 / Cloudflare R2 / MinIO / any S3-compatible endpoint)
//!   when the `s3` feature is on.
//!
//! ## Choosing a backend
//!
//! ```no_run
//! use nb_persistence::{KeyValueStore, MemoryKvStore};
//!
//! // In-memory (default): nothing is persisted to disk.
//! let store: Box<dyn KeyValueStore + Send + Sync> = Box::new(MemoryKvStore::new());
//!
//! // SQLite file-backed (needs the `sqlite` feature):
//! // let store: Box<dyn KeyValueStore + Send + Sync> =
//! //     Box::new(SqlKvStore::connect(SqlKvStoreConfig {
//! //         database_url: "sqlite://data/noobietoria.db".into(),
//! //         run_migrations: true,
//! //     })
//! //     .await
//! //     .unwrap());
//! # anyhow::Ok(())
//! ```
//!
//! For object storage:
//!
//! ```no_run
//! use nb_persistence::{ObjectStore, LocalFileObjectStore};
//! let store: Box<dyn ObjectStore + Send + Sync> =
//!     Box::new(LocalFileObjectStore::new("data/objects").unwrap());
//! # anyhow::Ok(())
//! ```

pub mod kv_store;
pub mod object_store;
pub mod sql;

pub use kv_store::{KeyValueStore, MemoryKvStore, SqlKvStoreConfig};

#[cfg(any(feature = "sqlite", feature = "any-db"))]
pub use kv_store::SqlKvStore;

pub use object_store::{LocalFileObjectStore, MemoryObjectStore, ObjectStore};

#[cfg(feature = "s3")]
pub use object_store::{S3ObjectStore, S3ObjectStoreConfig};
