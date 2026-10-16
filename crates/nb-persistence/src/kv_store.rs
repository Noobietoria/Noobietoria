/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `KeyValueStore`: durable key → JSON value storage.
//!
//! This is the abstraction [`crate::SqlKvStore`] and
//! [`crate::MemoryKvStore`] implement.  `DataStoreService` in `nb-store` works
//! with any `KeyValueStore`, so swapping from in-memory to a real database is
//! just one constructor call.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use nb_types::Value;
#[allow(unused_imports)]
use serde_json::to_string as to_json_string;
use tokio::sync::RwLock as TokioRwLock;

/// Result type for key-value store operations.
pub type KvResult<T> = Result<T, KvError>;

/// Errors a key-value store can raise.
#[derive(Debug, Clone, thiserror::Error)]
pub enum KvError {
    #[error("key must not be empty")]
    EmptyKey,
    #[error("no entry under `{0}`")]
    Missing(String),
    #[error("backend error: {0}")]
    Backend(String),
    #[error("serialisation error: {0}")]
    Serialisation(String),
}

/// Configuration for [`SqlKvStore`].
#[derive(Debug, Clone)]
pub struct SqlKvStoreConfig {
    /// Database URL, e.g. `sqlite://data/noobietoria.db`,
    /// `postgres://user:pass@host:5432/db`,
    /// `mysql://user:pass@host:3306/db`,
    /// `cockroachdb://user@host:26257/db?sslmode=disable`.
    pub database_url: String,
    /// Whether to run `sqlx` migrations on startup (creates the `kv_entries`
    /// table if missing).
    pub run_migrations: bool,
}

/// Trait for async key → JSON value stores.
#[async_trait]
pub trait KeyValueStore: Send + Sync {
    /// Returns the value under `key`, or `None` if absent.
    async fn get(&self, key: &str) -> KvResult<Option<Value>>;

    /// Stores `value` under `key`, overwriting any existing entry.
    async fn set(&self, key: &str, value: &Value) -> KvResult<()>;

    /// Removes `key` if present.  Returns `true` if something was deleted.
    async fn delete(&self, key: &str) -> KvResult<bool>;

    /// Returns all keys currently stored (may be expensive on large backends).
    async fn list_keys(&self) -> KvResult<Vec<String>>;

    /// Number of entries.
    async fn len(&self) -> KvResult<usize>;

    /// Whether the store holds nothing.
    async fn is_empty(&self) -> KvResult<bool>;

    /// Removes every entry.
    async fn clear(&self) -> KvResult<()>;
}

// ════════════════════════════════════════════════════════════════════════════
// In-memory implementation (zero config, for tests and local dev)
// ════════════════════════════════════════════════════════════════════════════

/// An in-memory `KeyValueStore`.  Nothing survives a process restart.
#[derive(Clone, Debug, Default)]
pub struct MemoryKvStore {
    inner: Arc<TokioRwLock<BTreeMap<String, Value>>>,
}

impl MemoryKvStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl KeyValueStore for MemoryKvStore {
    async fn get(&self, key: &str) -> KvResult<Option<Value>> {
        if key.is_empty() {
            return Err(KvError::EmptyKey);
        }
        let map = self.inner.read().await;
        Ok(map.get(key).cloned())
    }

    async fn set(&self, key: &str, value: &Value) -> KvResult<()> {
        if key.is_empty() {
            return Err(KvError::EmptyKey);
        }
        let mut map = self.inner.write().await;
        map.insert(key.to_owned(), value.clone());
        Ok(())
    }

    async fn delete(&self, key: &str) -> KvResult<bool> {
        if key.is_empty() {
            return Err(KvError::EmptyKey);
        }
        let mut map = self.inner.write().await;
        Ok(map.remove(key).is_some())
    }

    async fn list_keys(&self) -> KvResult<Vec<String>> {
        let map = self.inner.read().await;
        Ok(map.keys().cloned().collect())
    }

    async fn len(&self) -> KvResult<usize> {
        let map = self.inner.read().await;
        Ok(map.len())
    }

    async fn is_empty(&self) -> KvResult<bool> {
        let map = self.inner.read().await;
        Ok(map.is_empty())
    }

    async fn clear(&self) -> KvResult<()> {
        let mut map = self.inner.write().await;
        map.clear();
        Ok(())
    }
}

impl MemoryKvStore {
    /// Atomic read-modify-write on a single key (inherent, not part of the
    /// object-safe `KeyValueStore` trait).
    pub async fn update<F, R>(
        &self,
        key: &str,
        f: F,
    ) -> KvResult<R>
    where
        F: Fn(Option<&Value>) -> KvResult<(Option<Value>, R)> + Send + Sync,
        R: Send,
    {
        if key.is_empty() {
            return Err(KvError::EmptyKey);
        }
        let mut map = self.inner.write().await;
        let current = map.get(key);
        let (next, ret) = f(current)?;
        match next {
            Some(v) => {
                map.insert(key.to_owned(), v);
            }
            None => {
                map.remove(key);
            }
        }
        Ok(ret)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// SQL implementation (sqlite / postgres / mysql / cockroachdb)
// ════════════════════════════════════════════════════════════════════════════

#[cfg(any(feature = "sqlite", feature = "any-db"))]
/// SQL-backed `KeyValueStore`.  One table `kv_entries(key TEXT PRIMARY KEY,
/// value JSON)` holds all entries.
pub struct SqlKvStore {
    pool: sqlx::Pool<sqlx::Any>,
}

#[cfg(any(feature = "sqlite", feature = "any-db"))]
impl std::fmt::Debug for SqlKvStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqlKvStore").finish()
    }
}

#[cfg(any(feature = "sqlite", feature = "any-db"))]
impl SqlKvStore {
    /// Connect to `database_url` and ensure the table exists.
    pub async fn connect(config: SqlKvStoreConfig) -> KvResult<Self> {
        let pool = sqlx::any::AnyPoolOptions::new()
            .max_connections(5)
            .connect(&config.database_url)
            .await
            .map_err(|e| KvError::Backend(e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS kv_entries (
                key   TEXT PRIMARY KEY,
                value JSON NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|e| KvError::Backend(e.to_string()))?;

        Ok(Self { pool })
    }

    /// Connect to a SQLite file at `path`.
    pub async fn sqlite(path: impl AsRef<str>) -> KvResult<Self> {
        let url = format!("sqlite://{}", path.as_ref());
        Self::connect(SqlKvStoreConfig {
            database_url: url,
            run_migrations: true,
        })
        .await
    }
}


#[cfg(any(feature = "sqlite", feature = "any-db"))]
#[async_trait]
impl KeyValueStore for SqlKvStore {
    async fn get(&self, key: &str) -> KvResult<Option<Value>> {
        if key.is_empty() {
            return Err(KvError::EmptyKey);
        }
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM kv_entries WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| KvError::Backend(e.to_string()))?;
        match row {
            Some((value,)) => serde_json::from_str(&value)
                .map(Some)
                .map_err(|e| KvError::Serialisation(e.to_string())),
            None => Ok(None),
        }
    }

    async fn set(&self, key: &str, value: &Value) -> KvResult<()> {
        if key.is_empty() {
            return Err(KvError::EmptyKey);
        }
        let json = to_json_string(value).map_err(|e| KvError::Serialisation(e.to_string()))?;
        sqlx::query(
            r#"
            INSERT INTO kv_entries (key, value) VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            "#,
        )
        .bind(key)
        .bind(json)
        .execute(&self.pool)
        .await
        .map_err(|e| KvError::Backend(e.to_string()))?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> KvResult<bool> {
        if key.is_empty() {
            return Err(KvError::EmptyKey);
        }
        let result = sqlx::query("DELETE FROM kv_entries WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(|e| KvError::Backend(e.to_string()))?;
        Ok(result.rows_affected() > 0)
    }

    async fn list_keys(&self) -> KvResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT key FROM kv_entries ORDER BY key")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| KvError::Backend(e.to_string()))?;
        Ok(rows.into_iter().map(|(k,)| k).collect())
    }

    async fn len(&self) -> KvResult<usize> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM kv_entries")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| KvError::Backend(e.to_string()))?;
        Ok(row.0 as usize)
    }

    async fn is_empty(&self) -> KvResult<bool> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM kv_entries")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| KvError::Backend(e.to_string()))?;
        Ok(row.0 == 0)
    }

    async fn clear(&self) -> KvResult<()> {
        sqlx::query("DELETE FROM kv_entries")
            .execute(&self.pool)
            .await
            .map_err(|e| KvError::Backend(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_store_round_trip() {
        let store = MemoryKvStore::new();
        store.set("a", &Value::from(1i64)).await.unwrap();
        assert_eq!(store.get("a").await.unwrap(), Some(Value::from(1i64)));
        assert!(store.delete("a").await.unwrap());
        assert_eq!(store.get("a").await.unwrap(), None);
    }

    #[tokio::test]
    async fn memory_store_update_is_atomic() {
        let store = MemoryKvStore::new();
        store.set("counter", &Value::from(0i64)).await.unwrap();
        let _ = store
            .update("counter", |current| {
                let next = match current {
                    Some(Value::Number(n)) => Value::from((*n as i64) + 1),
                    _ => Value::from(1i64),
                };
                Ok((Some(next), ()))
            })
            .await
            .unwrap();
        assert_eq!(store.get("counter").await.unwrap(), Some(Value::from(1i64)));
    }
}

