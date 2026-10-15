//! A small keyed store reused by the gameplay services.
//!
//! `CurrencyService`, `InventoryService`, `LeaderboardService`, `BadgeService`,
//! `AchievementService`, `QuestService`, `CooldownService`, `PassService`,
//! `MailService`, `TradingService`, `CraftingService`, `PartyService`,
//! `MatchmakingService`, `AnalyticsService` and the rest all need the same two
//! operations — "read what is under this key" and "write it back" — so they
//! share this type rather than each inventing a map.
//!
//! Everything is stored as [`Value`], so a store can be exported to JSON
//! wholesale for the website, for saving a place, or for `--selftest` output.

use std::collections::BTreeMap;
use std::sync::Arc;

use nb_types::Value;
use parking_lot::RwLock;

/// Errors a keyed store can raise.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum StoreError {
    /// A key was required but empty.
    #[error("key must not be empty")]
    EmptyKey,
    /// A numeric field was expected but the stored value is not a number.
    #[error("`{key}` holds {actual}, expected a number")]
    NotANumber {
        /// The offending key.
        key: String,
        /// The value's actual type.
        actual: &'static str,
    },
    /// A string field was expected but the stored value is not a string.
    #[error("`{key}` holds {actual}, expected a string")]
    NotAString {
        /// The offending key.
        key: String,
        /// The value's actual type.
        actual: &'static str,
    },
    /// The requested entry does not exist.
    #[error("no entry under `{key}`")]
    Missing {
        /// The key that was looked up.
        key: String,
    },
    /// The operation would go below zero.
    #[error("`{key}` has {available} but {requested} was requested")]
    Insufficient {
        /// The key involved.
        key: String,
        /// What is available.
        available: f64,
        /// What was requested.
        requested: f64,
    },
}

/// Shorthand for keyed-store results.
pub type StoreResult<T> = Result<T, StoreError>;

/// A thread-safe map from string keys to [`Value`]s.
#[derive(Clone, Debug)]
pub struct KeyedStore {
    inner: Arc<RwLock<BTreeMap<String, Value>>>,
}

impl Default for KeyedStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyedStore {
    /// An empty store.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Builds a store from an iterator of pairs, for seeding defaults.
    pub fn from_pairs<I, K>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, Value)>,
        K: Into<String>,
    {
        let store = Self::new();
        {
            let mut map = store.inner.write();
            for (k, v) in pairs {
                map.insert(k.into(), v);
            }
        }
        store
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    /// Whether the store holds nothing.
    pub fn is_empty(&self) -> bool {
        self.inner.read().is_empty()
    }

    /// Removes every entry.
    pub fn clear(&self) {
        self.inner.write().clear();
    }

    /// Copies out every entry.
    pub fn snapshot(&self) -> BTreeMap<String, Value> {
        self.inner.read().clone()
    }

    /// Replaces the contents wholesale; used when loading a save.
    pub fn restore(&self, data: BTreeMap<String, Value>) {
        *self.inner.write() = data;
    }

    /// Whether a key is present.
    pub fn contains(&self, key: &str) -> bool {
        self.inner.read().contains_key(key)
    }

    /// Reads a value, or `None`.
    pub fn get(&self, key: &str) -> Option<Value> {
        self.inner.read().get(key).cloned()
    }

    /// Reads a value, raising [`StoreError::Missing`] when absent.
    pub fn require(&self, key: &str) -> StoreResult<Value> {
        self.get(key).ok_or_else(|| StoreError::Missing {
            key: key.to_owned(),
        })
    }

    /// Inserts or replaces a value.
    pub fn set(&self, key: impl Into<String>, value: Value) -> StoreResult<()> {
        let key = key.into();
        if key.is_empty() {
            return Err(StoreError::EmptyKey);
        }
        self.inner.write().insert(key, value);
        Ok(())
    }

    /// Removes a key, returning what was there.
    pub fn remove(&self, key: &str) -> Option<Value> {
        self.inner.write().remove(key)
    }

    /// Reads a number, defaulting to `0.0` when the key is absent.
    ///
    /// An absent currency balance or score is zero, so this deliberately does
    /// *not* raise on a missing key — but it does raise when the key exists and
    /// holds a non-number, because that would silently corrupt arithmetic.
    pub fn number_or_zero(&self, key: &str) -> StoreResult<f64> {
        match self.get(key) {
            None => Ok(0.0),
            Some(Value::Number(n)) => Ok(n),
            Some(other) => Err(StoreError::NotANumber {
                key: key.to_owned(),
                actual: other.type_name(),
            }),
        }
    }

    /// Reads a number, raising when the key is absent or not a number.
    pub fn number(&self, key: &str) -> StoreResult<f64> {
        match self.require(key)? {
            Value::Number(n) => Ok(n),
            other => Err(StoreError::NotANumber {
                key: key.to_owned(),
                actual: other.type_name(),
            }),
        }
    }

    /// Reads a string, defaulting to empty when absent.
    pub fn string_or_empty(&self, key: &str) -> StoreResult<String> {
        match self.get(key) {
            None | Some(Value::Nil) => Ok(String::new()),
            Some(Value::Str(s)) => Ok(s),
            Some(other) => Err(StoreError::NotAString {
                key: key.to_owned(),
                actual: other.type_name(),
            }),
        }
    }

    /// Reads a string, raising when absent or not a string.
    pub fn string(&self, key: &str) -> StoreResult<String> {
        match self.require(key)? {
            Value::Str(s) => Ok(s),
            other => Err(StoreError::NotAString {
                key: key.to_owned(),
                actual: other.type_name(),
            }),
        }
    }

    /// Adds `delta` to a numeric entry, creating it when absent.
    ///
    /// Returns the new total. Negative deltas are allowed; the caller decides
    /// whether an overdraft is legal, which is why the currency service checks
    /// the balance separately.
    pub fn add_number(&self, key: &str, delta: f64) -> StoreResult<f64> {
        let mut map = self.inner.write();
        let current = match map.get(key) {
            None => 0.0,
            Some(Value::Number(n)) => *n,
            Some(other) => {
                return Err(StoreError::NotANumber {
                    key: key.to_owned(),
                    actual: other.type_name(),
                })
            }
        };
        let next = current + delta;
        map.insert(key.to_owned(), Value::from(next));
        Ok(next)
    }

    /// Subtracts `amount` from a numeric entry, refusing to go negative.
    pub fn sub_number_checked(&self, key: &str, amount: f64) -> StoreResult<f64> {
        let mut map = self.inner.write();
        let current = match map.get(key) {
            None => 0.0,
            Some(Value::Number(n)) => *n,
            Some(other) => {
                return Err(StoreError::NotANumber {
                    key: key.to_owned(),
                    actual: other.type_name(),
                })
            }
        };
        if current < amount {
            return Err(StoreError::Insufficient {
                key: key.to_owned(),
                available: current,
                requested: amount,
            });
        }
        let next = current - amount;
        map.insert(key.to_owned(), Value::from(next));
        Ok(next)
    }

    /// Reads an array entry, defaulting to empty when absent.
    pub fn array_or_empty(&self, key: &str) -> StoreResult<Vec<Value>> {
        match self.get(key) {
            None | Some(Value::Nil) => Ok(Vec::new()),
            Some(Value::Array(a)) => Ok(a),
            Some(other) => Err(StoreError::NotANumber {
                key: key.to_owned(),
                actual: other.type_name(),
            }),
        }
    }

    /// Reads a keyed-table entry, defaulting to empty when absent.
    pub fn map_or_empty(&self, key: &str) -> StoreResult<BTreeMap<String, Value>> {
        match self.get(key) {
            None | Some(Value::Nil) => Ok(BTreeMap::new()),
            Some(Value::Map(m)) => Ok(m),
            Some(other) => Err(StoreError::NotANumber {
                key: key.to_owned(),
                actual: other.type_name(),
            }),
        }
    }

    /// Runs a closure with write access, for read-modify-write operations that
    /// must not interleave with another writer.
    pub fn update<R>(&self, key: &str, f: impl FnOnce(Option<Value>) -> (Option<Value>, R)) -> R {
        let mut map = self.inner.write();
        let current = map.get(key).cloned();
        let (next, result) = f(current);
        match next {
            Some(v) => {
                map.insert(key.to_owned(), v);
            }
            None => {
                map.remove(key);
            }
        }
        result
    }

    /// Every key, sorted.
    pub fn keys(&self) -> Vec<String> {
        self.inner.read().keys().cloned().collect()
    }

    /// Every `(key, value)` pair, sorted by key.
    pub fn entries(&self) -> Vec<(String, Value)> {
        self.inner
            .read()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// JSON text of the whole store, for persistence and test output.
    pub fn to_json(&self) -> String {
        let map: BTreeMap<String, Value> = self.inner.read().clone();
        serde_json::to_string(&Value::Map(map)).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Loads a store previously written by [`KeyedStore::to_json`].
    pub fn from_json(json: &str) -> StoreResult<Self> {
        let value: Value = serde_json::from_str(json).map_err(|_| StoreError::EmptyKey)?;
        let store = Self::new();
        match value {
            Value::Map(map) => store.restore(map),
            _ => return Err(StoreError::EmptyKey),
        }
        Ok(store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_remove() {
        let s = KeyedStore::new();
        assert!(s.is_empty());
        s.set("a", Value::from(1i64)).unwrap();
        assert_eq!(s.get("a"), Some(Value::from(1i64)));
        assert!(s.contains("a"));
        assert_eq!(s.len(), 1);
        assert_eq!(s.remove("a"), Some(Value::from(1i64)));
        assert!(s.get("a").is_none());
        assert!(s.is_empty());
    }

    #[test]
    fn empty_keys_are_rejected() {
        let s = KeyedStore::new();
        assert_eq!(s.set("", Value::from(1i64)), Err(StoreError::EmptyKey));
    }

    #[test]
    fn missing_keys_read_as_zero_or_empty() {
        let s = KeyedStore::new();
        assert_eq!(s.number_or_zero("nope").unwrap(), 0.0);
        assert_eq!(s.string_or_empty("nope").unwrap(), "");
        assert!(s.array_or_empty("nope").unwrap().is_empty());
        assert!(s.map_or_empty("nope").unwrap().is_empty());
    }

    #[test]
    fn wrong_types_are_reported_not_coerced() {
        let s = KeyedStore::new();
        s.set("name", Value::from("Alice")).unwrap();
        s.set("coins", Value::from(5i64)).unwrap();

        assert_eq!(
            s.number_or_zero("name"),
            Err(StoreError::NotANumber {
                key: "name".into(),
                actual: "string"
            })
        );
        assert_eq!(
            s.string_or_empty("coins"),
            Err(StoreError::NotAString {
                key: "coins".into(),
                actual: "number"
            })
        );
    }

    #[test]
    fn require_reports_missing() {
        let s = KeyedStore::new();
        assert_eq!(
            s.require("ghost"),
            Err(StoreError::Missing {
                key: "ghost".into()
            })
        );
    }

    #[test]
    fn add_number_accumulates_from_absent() {
        let s = KeyedStore::new();
        assert_eq!(s.add_number("coins", 10.0).unwrap(), 10.0);
        assert_eq!(s.add_number("coins", 5.0).unwrap(), 15.0);
        assert_eq!(s.add_number("coins", -3.0).unwrap(), 12.0);
        assert_eq!(s.number("coins").unwrap(), 12.0);
    }

    #[test]
    fn checked_subtraction_refuses_to_go_negative() {
        let s = KeyedStore::new();
        s.set("coins", Value::from(100i64)).unwrap();
        assert_eq!(s.sub_number_checked("coins", 40.0).unwrap(), 60.0);
        let err = s.sub_number_checked("coins", 100.0).unwrap_err();
        assert_eq!(
            err,
            StoreError::Insufficient {
                key: "coins".into(),
                available: 60.0,
                requested: 100.0
            }
        );
        assert_eq!(
            s.number("coins").unwrap(),
            60.0,
            "balance unchanged on failure"
        );
    }

    #[test]
    fn update_is_atomic_read_modify_write() {
        let s = KeyedStore::new();
        s.update("k", |_| (Some(Value::from(1i64)), ()));
        let seen = s.update("k", |current| {
            let next = match current {
                Some(Value::Number(n)) => n as i64 + 1,
                _ => 0,
            };
            (Some(Value::from(next)), next)
        });
        assert_eq!(seen, 2);
        assert_eq!(s.get("k"), Some(Value::from(2i64)));

        // Returning None removes the entry.
        s.update("k", |_| (None, ()));
        assert!(!s.contains("k"));
    }

    #[test]
    fn keys_and_entries_are_sorted() {
        let s = KeyedStore::new();
        s.set("z", Value::from(1i64)).unwrap();
        s.set("a", Value::from(2i64)).unwrap();
        assert_eq!(s.keys(), vec!["a".to_owned(), "z".to_owned()]);
        let entries = s.entries();
        assert_eq!(entries[0].0, "a");
        assert_eq!(entries[1].0, "z");
    }

    #[test]
    fn json_round_trip() {
        let s = KeyedStore::new();
        s.set("a", Value::from(1i64)).unwrap();
        s.set("b", Value::from("text")).unwrap();
        let json = s.to_json();
        let back = KeyedStore::from_json(&json).unwrap();
        assert_eq!(back.get("a"), Some(Value::from(1i64)));
        assert_eq!(back.get("b"), Some(Value::from("text")));
    }

    #[test]
    fn clones_share_the_same_storage() {
        let s = KeyedStore::new();
        let clone = s.clone();
        s.set("shared", Value::from(7i64)).unwrap();
        assert_eq!(clone.get("shared"), Some(Value::from(7i64)));
    }

    #[test]
    fn snapshot_and_restore() {
        let s = KeyedStore::new();
        s.set("a", Value::from(1i64)).unwrap();
        let snap = s.snapshot();
        s.clear();
        assert!(s.is_empty());
        s.restore(snap);
        assert_eq!(s.get("a"), Some(Value::from(1i64)));
    }
}
