/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0. */

//! Pluggable key-value backends for [`KeyedStore`] and [`DataStoreService`].

use std::collections::BTreeMap;
use std::sync::Arc;

use nb_types::Value;
use crate::kv::StoreError;
#[cfg(feature = "sqlx")]
pub mod sqlx;
#[cfg(feature = "sqlx")]
pub use self::sqlx::SqlxBackend;

/// A persistent key-value store holding JSON-serialisable [`Value`]s.
pub trait KvBackend: Send + Sync + std::fmt::Debug {
    fn get(&self, key: &str) -> Result<Option<Value>, StoreError>;
    fn set(&self, key: &str, value: &Value) -> Result<(), StoreError>;
    fn remove(&self, key: &str) -> Result<Option<Value>, StoreError>;
    fn contains(&self, key: &str) -> Result<bool, StoreError>;
    fn keys(&self) -> Result<Vec<String>, StoreError>;
    fn clear(&self) -> Result<(), StoreError>;
    fn snapshot(&self) -> Result<BTreeMap<String, Value>, StoreError>;
    fn restore(&self, map: BTreeMap<String, Value>) -> Result<(), StoreError>;
}

/// A serialisation error from the backend.
#[derive(Debug, Clone, thiserror::Error)]
#[error("storage backend error: {0}")]
pub struct BackendError(pub String);

/// In-memory key-value store. Data is lost when process exits.
#[derive(Clone, Debug, Default)]
pub struct MemoryBackend {
    inner: Arc<parking_lot::RwLock<BTreeMap<String, Value>>>,
}

impl MemoryBackend {
    pub fn new() -> Self { Self::default() }
}

impl KvBackend for MemoryBackend {
    fn get(&self, key: &str) -> Result<Option<Value>, StoreError> {
        Ok(self.inner.read().get(key).cloned())
    }
    fn set(&self, key: &str, value: &Value) -> Result<(), StoreError> {
        self.inner.write().insert(key.to_owned(), value.clone());
        Ok(())
    }
    fn remove(&self, key: &str) -> Result<Option<Value>, StoreError> {
        Ok(self.inner.write().remove(key))
    }
    fn contains(&self, key: &str) -> Result<bool, StoreError> {
        Ok(self.inner.read().contains_key(key))
    }
    fn keys(&self) -> Result<Vec<String>, StoreError> {
        Ok(self.inner.read().keys().cloned().collect())
    }
    fn clear(&self) -> Result<(), StoreError> {
        self.inner.write().clear();
        Ok(())
    }
    fn snapshot(&self) -> Result<BTreeMap<String, Value>, StoreError> {
        Ok(self.inner.read().clone())
    }
    fn restore(&self, map: BTreeMap<String, Value>) -> Result<(), StoreError> {
        *self.inner.write() = map;
        Ok(())
    }
}

