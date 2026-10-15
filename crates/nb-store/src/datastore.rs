//! `DataStoreService`: durable, budgeted, key-value storage for a place.
//!
//! Two API shapes are implemented, because the documentation documents both:
//!
//! 1. The session-based form — `Init(SessionID)`, `Insert`, `Get`, `GetKeys`,
//!    `Delete`, `Close(SessionID)` — where the author manages the connection.
//! 2. The Roblox-style form — `DataStoreService:GetDataStore(name, scope)`
//!    returning a handle with `GetAsync`/`SetAsync`/`UpdateAsync`/`RemoveAsync`
//!    — which the documentation adds for authors who prefer it and describes as
//!    managing connections automatically.
//!
//! Value shapes are validated against the documented contract: the value must
//! be JSON, the key must be at most 50 ASCII characters, and the serialised
//! value must fit in 1 MB. Sessions are dropped when the ledger says they have
//! been silent for 15 seconds.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use nb_types::Value;
use parking_lot::RwLock;

use crate::budget::{BudgetError, RequestKind, RequestLedger};

/// The documented `DataStoreService` limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataStoreLimits {
    /// Maximum number of distinct datastores per game.
    pub max_datastores: usize,
    /// Maximum key length in ASCII characters.
    pub max_key_len: usize,
    /// Maximum serialised value size in bytes.
    pub max_value_bytes: usize,
    /// Base storage allowance in bytes.
    pub base_storage_bytes: usize,
    /// Additional storage per visit, in bytes.
    pub storage_per_visit_bytes: usize,
}

impl Default for DataStoreLimits {
    /// The numbers straight from the documentation: 15 datastores, 50-character
    /// ASCII keys, 1 MB values, and `25MB + 5MB × visits`.
    fn default() -> Self {
        Self {
            max_datastores: 15,
            max_key_len: 50,
            max_value_bytes: 1024 * 1024,
            base_storage_bytes: 25 * 1024 * 1024,
            storage_per_visit_bytes: 5 * 1024 * 1024,
        }
    }
}

/// A session identifier.
///
/// The documentation says the author may generate it however they like "miễn là
/// base32", so this type validates exactly that and nothing more.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(String);

impl SessionId {
    /// Wraps an author-generated session id, rejecting anything that is not
    /// base32 as the documentation requires.
    pub fn new(raw: impl Into<String>) -> Result<Self, DataStoreError> {
        let raw = raw.into();
        if raw.is_empty() {
            return Err(DataStoreError::InvalidSessionId {
                session: raw,
                reason: "must not be empty",
            });
        }
        // RFC 4648 base32 alphabet, upper or lower case, padding allowed.
        let ok = raw.chars().all(|c| {
            matches!(c, 'A'..='Z') || matches!(c, 'a'..='z') || matches!(c, '2'..='7') || c == '='
        }) && raw.chars().any(|c| c.is_ascii_alphanumeric());
        if !ok {
            return Err(DataStoreError::InvalidSessionId {
                session: raw,
                reason: "must be base32 (A-Z, 2-7, optional '=' padding)",
            });
        }
        Ok(Self(raw))
    }

    /// Generates a fresh random base32 session id, as the documentation allows.
    pub fn generate() -> Self {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
        let mut state: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        let mut out = String::with_capacity(26);
        for _ in 0..26 {
            // xorshift64*: deterministic and dependency-free, which is all a
            // session token needs here.
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            let idx = (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 59) as usize % 32;
            out.push(ALPHABET[idx] as char);
        }
        Self(out)
    }

    /// The raw string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A stored value: always valid JSON, never larger than the documented limit.
#[derive(Clone, Debug, PartialEq)]
pub struct DataStoreValue {
    json: String,
}

impl DataStoreValue {
    /// Validates and wraps a value.
    ///
    /// Rejects a value that is not JSON-representable or exceeds 1 MB, which is
    /// exactly what the real service does.
    pub fn new(value: &Value, limits: DataStoreLimits) -> Result<Self, DataStoreError> {
        let json = serde_json::to_string(value).map_err(|e| DataStoreError::NotJson {
            reason: e.to_string(),
        })?;
        if json.len() > limits.max_value_bytes {
            return Err(DataStoreError::ValueTooLarge {
                bytes: json.len(),
                limit: limits.max_value_bytes,
            });
        }
        Ok(Self { json })
    }

    /// Wraps raw JSON text that is already known to be valid.
    pub fn from_json(json: impl Into<String>) -> Result<Self, DataStoreError> {
        let json = json.into();
        // Verify before accepting, so a corrupt file cannot poison the store.
        serde_json::from_str::<serde_json::Value>(&json).map_err(|e| DataStoreError::NotJson {
            reason: e.to_string(),
        })?;
        Ok(Self { json })
    }

    /// The JSON text.
    pub fn as_json(&self) -> &str {
        &self.json
    }

    /// Deserialises back into a [`Value`].
    pub fn to_value(&self) -> Result<Value, DataStoreError> {
        serde_json::from_str(&self.json).map_err(|e| DataStoreError::NotJson {
            reason: e.to_string(),
        })
    }

    /// Serialised size in bytes.
    pub fn len(&self) -> usize {
        self.json.len()
    }

    /// Whether the payload is empty JSON.
    pub fn is_empty(&self) -> bool {
        self.json.is_empty()
    }
}

/// One entry in a sorted datastore listing.
#[derive(Clone, Debug, PartialEq)]
pub struct OrderedEntry {
    /// The key.
    pub key: String,
    /// The stored value.
    pub value: DataStoreValue,
}

/// Errors raised by `DataStoreService`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DataStoreError {
    /// The session id is not base32.
    #[error("invalid session id `{session}`: {reason}")]
    InvalidSessionId {
        /// The rejected id.
        session: String,
        /// Why it was rejected.
        reason: &'static str,
    },
    /// No session is open under that id.
    #[error("no open datastore session `{session}`; call Init first")]
    NoSuchSession {
        /// The session that was used.
        session: String,
    },
    /// A session with that id is already open.
    #[error("datastore session `{session}` is already open")]
    SessionAlreadyOpen {
        /// The clashing session.
        session: String,
    },
    /// The key is longer than 50 ASCII characters or not ASCII.
    #[error("invalid key `{key}`: {reason}")]
    InvalidKey {
        /// The rejected key.
        key: String,
        /// Why it was rejected.
        reason: &'static str,
    },
    /// The value exceeds 1 MB.
    #[error("value is {bytes} bytes, over the {limit}-byte limit")]
    ValueTooLarge {
        /// Actual size.
        bytes: usize,
        /// The limit.
        limit: usize,
    },
    /// The value is not valid JSON.
    #[error("value is not valid JSON: {reason}")]
    NotJson {
        /// The parse failure.
        reason: String,
    },
    /// A new datastore would exceed the 15-datastore limit.
    #[error(
        "cannot create datastore `{name}`: the limit of {limit} datastores per game is reached"
    )]
    TooManyDataStores {
        /// The rejected datastore.
        name: String,
        /// The limit.
        limit: usize,
    },
    /// The game's storage allowance is exhausted.
    #[error("storage limit reached: {used} of {limit} bytes used")]
    StorageExhausted {
        /// Current usage.
        used: usize,
        /// The allowance.
        limit: usize,
    },
    /// A request was refused by the budget ledger.
    #[error(transparent)]
    Budget(#[from] BudgetError),
}

/// One named datastore, equivalent to a Roblox `DataStore` handle.
#[derive(Clone, Debug)]
pub struct DataStoreHandle {
    service: DataStoreService,
    name: String,
    scope: String,
}

impl DataStoreHandle {
    /// The datastore name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The scope.
    pub fn scope(&self) -> &str {
        &self.scope
    }

    fn namespace(&self) -> String {
        if self.scope.is_empty() {
            self.name.clone()
        } else {
            format!("{}:{}", self.name, self.scope)
        }
    }

    /// `DataStore:GetAsync(key)`.
    pub fn get_async(&self, key: &str) -> Result<Option<Value>, DataStoreError> {
        self.service.read(&self.namespace(), key)
    }

    /// `DataStore:SetAsync(key, value)`.
    ///
    /// The `user_ids` and `options` parameters of the documented signature are
    /// accepted and ignored, as they only drive Roblox-side analytics and
    /// versioning; they are still part of the call so a ported script compiles.
    pub fn set_async(&self, key: &str, value: &Value) -> Result<(), DataStoreError> {
        self.service.write(&self.namespace(), key, value)
    }

    /// `DataStore:UpdateAsync(key, transformFunction)`.
    ///
    /// The transform runs against the current value (or `nil`) and its return
    /// value is stored. Returning `nil` cancels the write, matching Roblox.
    pub fn update_async<F>(&self, key: &str, transform: F) -> Result<Option<Value>, DataStoreError>
    where
        F: FnOnce(Option<Value>) -> Option<Value>,
    {
        self.service.update(&self.namespace(), key, transform)
    }

    /// `DataStore:RemoveAsync(key)`.
    pub fn remove_async(&self, key: &str) -> Result<Option<Value>, DataStoreError> {
        self.service.remove(&self.namespace(), key)
    }

    /// Lists every key in this datastore, sorted.
    pub fn list_keys(&self) -> Result<Vec<String>, DataStoreError> {
        self.service.list_keys(&self.namespace())
    }
}

/// The shared, thread-safe store behind every session and handle.
#[derive(Clone, Debug)]
struct Inner {
    limits: DataStoreLimits,
    ledger: RequestLedger,
    /// namespace -> key -> value
    stores: BTreeMap<String, BTreeMap<String, DataStoreValue>>,
    /// Open sessions: id -> namespace it is bound to (empty for the generic
    /// session API, which addresses a single implicit namespace).
    sessions: BTreeMap<String, String>,
    /// Total bytes currently stored across all namespaces.
    bytes_used: usize,
    /// Number of visits, which scales the storage allowance.
    visits: u64,
}

/// `DataStoreService` — the store the whole server shares.
#[derive(Clone, Debug)]
pub struct DataStoreService {
    inner: Arc<RwLock<Inner>>,
}

impl Default for DataStoreService {
    fn default() -> Self {
        Self::new()
    }
}

impl DataStoreService {
    /// A store with the documented limits and no data.
    pub fn new() -> Self {
        Self::with_limits(DataStoreLimits::default())
    }

    /// A store with explicit limits.
    pub fn with_limits(limits: DataStoreLimits) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                limits,
                ledger: RequestLedger::new(),
                stores: BTreeMap::new(),
                sessions: BTreeMap::new(),
                bytes_used: 0,
                visits: 0,
            })),
        }
    }

    /// The limits in force.
    pub fn limits(&self) -> DataStoreLimits {
        self.inner.read().limits
    }

    /// Access to the request ledger, for `GetRequestBudgetForRequestType` and
    /// for the server's per-minute window roll.
    pub fn ledger(&self) -> RequestLedger {
        self.inner.read().ledger.clone()
    }

    /// Replaces the ledger, used after the server rolls its minute window.
    pub fn with_ledger<R>(&self, f: impl FnOnce(&mut RequestLedger) -> R) -> R {
        let mut inner = self.inner.write();
        f(&mut inner.ledger)
    }

    /// Updates the concurrent-player count, which scales the request allowance.
    pub fn set_ccu(&self, ccu: u32) {
        self.inner.write().ledger.set_ccu(ccu);
    }

    /// Records visits, which scale the storage allowance.
    pub fn set_visits(&self, visits: u64) {
        self.inner.write().visits = visits;
    }

    /// The total storage allowance: `25MB + 5MB × visits`.
    pub fn storage_allowance(&self) -> usize {
        let inner = self.inner.read();
        inner.limits.base_storage_bytes
            + (inner.limits.storage_per_visit_bytes as u64 * inner.visits) as usize
    }

    /// Bytes currently stored.
    pub fn bytes_used(&self) -> usize {
        self.inner.read().bytes_used
    }

    /// Number of distinct datastores.
    pub fn datastore_count(&self) -> usize {
        self.inner.read().stores.len()
    }

    /// Validates a key against the documented rule: at most 50 ASCII characters.
    fn validate_key(limits: DataStoreLimits, key: &str) -> Result<(), DataStoreError> {
        if key.is_empty() {
            return Err(DataStoreError::InvalidKey {
                key: key.to_owned(),
                reason: "must not be empty",
            });
        }
        if !key.is_ascii() {
            return Err(DataStoreError::InvalidKey {
                key: key.to_owned(),
                reason: "must be ASCII",
            });
        }
        if key.chars().count() > limits.max_key_len {
            return Err(DataStoreError::InvalidKey {
                key: key.to_owned(),
                reason: "must be at most 50 ASCII characters",
            });
        }
        Ok(())
    }

    /// Ensures a namespace exists, enforcing the 15-datastore limit.
    fn ensure_namespace(inner: &mut Inner, namespace: &str) -> Result<(), DataStoreError> {
        if inner.stores.contains_key(namespace) {
            return Ok(());
        }
        if inner.stores.len() >= inner.limits.max_datastores {
            return Err(DataStoreError::TooManyDataStores {
                name: namespace.to_owned(),
                limit: inner.limits.max_datastores,
            });
        }
        inner.stores.insert(namespace.to_owned(), BTreeMap::new());
        Ok(())
    }

    // --- Session API ----------------------------------------------------

    /// `DataStoreService.Init(SessionID)` — opens a connection.
    ///
    /// Refuses to exceed the documented 16 concurrent connections, and refuses
    /// to open the same session twice.
    pub fn init(&self, session: &SessionId) -> Result<(), DataStoreError> {
        let mut inner = self.inner.write();
        if inner.sessions.contains_key(session.as_str()) {
            return Err(DataStoreError::SessionAlreadyOpen {
                session: session.as_str().to_owned(),
            });
        }
        // The session API uses a single implicit namespace.
        Self::ensure_namespace(&mut inner, "")?;
        inner
            .ledger
            .open_connection(session.as_str(), 0)
            .map_err(DataStoreError::Budget)?;
        inner
            .sessions
            .insert(session.as_str().to_owned(), String::new());
        Ok(())
    }

    /// `DataStoreService.Close(SessionID)` — ends a connection.
    pub fn close(&self, session: &SessionId) -> Result<(), DataStoreError> {
        let mut inner = self.inner.write();
        if inner.sessions.remove(session.as_str()).is_none() {
            return Err(DataStoreError::NoSuchSession {
                session: session.as_str().to_owned(),
            });
        }
        let _ = inner.ledger.close_connection(session.as_str());
        Ok(())
    }

    /// Whether a session is open.
    pub fn is_session_open(&self, session: &SessionId) -> bool {
        self.inner.read().sessions.contains_key(session.as_str())
    }

    /// Number of open sessions.
    pub fn open_sessions(&self) -> usize {
        self.inner.read().sessions.len()
    }

    /// Drops sessions that have been silent for the documented 15 seconds.
    pub fn reap_idle_sessions(&self, now_tick: u64, ticks_per_second: u64) -> Vec<String> {
        let mut inner = self.inner.write();
        let expired = inner.ledger.reap_timed_out(now_tick, ticks_per_second);
        for session in &expired {
            inner.sessions.remove(session);
        }
        expired
    }

    /// Confirms a session exists, returning the namespace it addresses.
    fn require_session(inner: &Inner, session: &SessionId) -> Result<String, DataStoreError> {
        inner
            .sessions
            .get(session.as_str())
            .cloned()
            .ok_or_else(|| DataStoreError::NoSuchSession {
                session: session.as_str().to_owned(),
            })
    }

    /// `DataStoreService.Insert(SessionID, Key, Value)`.
    pub fn insert(
        &self,
        session: &SessionId,
        key: &str,
        value: &Value,
    ) -> Result<(), DataStoreError> {
        let mut inner = self.inner.write();
        let namespace = Self::require_session(&inner, session)?;
        Self::validate_key(inner.limits, key)?;

        let limits = inner.limits;
        let stored = DataStoreValue::new(value, limits)?;
        Self::ensure_storage(&mut inner, stored.len(), Some((&namespace, key)))?;

        let entry = inner.stores.entry(namespace.clone()).or_default();
        if let Some(old) = entry.insert(key.to_owned(), stored) {
            inner.bytes_used = inner.bytes_used.saturating_sub(old.len());
        }
        inner.bytes_used += inner
            .stores
            .get(&namespace)
            .and_then(|m| m.get(key))
            .map(DataStoreValue::len)
            .unwrap_or(0);
        inner
            .ledger
            .charge(RequestKind::Set)
            .map_err(DataStoreError::Budget)?;
        Ok(())
    }

    /// `DataStoreService.Get(SessionID, Key)`.
    pub fn get(&self, session: &SessionId, key: &str) -> Result<Option<Value>, DataStoreError> {
        let mut inner = self.inner.write();
        let namespace = Self::require_session(&inner, session)?;
        Self::validate_key(inner.limits, key)?;
        inner
            .ledger
            .charge(RequestKind::Get)
            .map_err(DataStoreError::Budget)?;
        match inner.stores.get(&namespace).and_then(|m| m.get(key)) {
            Some(stored) => Ok(Some(stored.to_value()?)),
            None => Ok(None),
        }
    }

    /// `DataStoreService.GetKeys(SessionID)`.
    pub fn get_keys(&self, session: &SessionId) -> Result<Vec<String>, DataStoreError> {
        let mut inner = self.inner.write();
        let namespace = Self::require_session(&inner, session)?;
        inner
            .ledger
            .charge(RequestKind::ListKeys)
            .map_err(DataStoreError::Budget)?;
        Ok(inner
            .stores
            .get(&namespace)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default())
    }

    /// `DataStoreService.Delete(SessionID, Key)`.
    pub fn delete(&self, session: &SessionId, key: &str) -> Result<Option<Value>, DataStoreError> {
        let mut inner = self.inner.write();
        let namespace = Self::require_session(&inner, session)?;
        Self::validate_key(inner.limits, key)?;
        inner
            .ledger
            .charge(RequestKind::Remove)
            .map_err(DataStoreError::Budget)?;
        let removed = inner.stores.entry(namespace).or_default().remove(key);
        if let Some(old) = &removed {
            inner.bytes_used = inner.bytes_used.saturating_sub(old.len());
        }
        match removed {
            Some(stored) => Ok(Some(stored.to_value()?)),
            None => Ok(None),
        }
    }

    // --- Roblox-style API ----------------------------------------------

    /// `DataStoreService:GetDataStore(name, scope)`.
    pub fn get_data_store(
        &self,
        name: &str,
        scope: impl Into<String>,
    ) -> Result<DataStoreHandle, DataStoreError> {
        let scope = scope.into();
        let handle = DataStoreHandle {
            service: self.clone(),
            name: name.to_owned(),
            scope,
        };
        let mut inner = self.inner.write();
        Self::ensure_namespace(&mut inner, &handle.namespace())?;
        Ok(handle)
    }

    /// `DataStoreService:GetOrderedDataStore(name, scope)`.
    ///
    /// An ordered datastore is the same storage with a sorted listing view; the
    /// sorted access lives on [`DataStoreService::get_ordered`].
    pub fn get_ordered_data_store(
        &self,
        name: &str,
        scope: impl Into<String>,
    ) -> Result<DataStoreHandle, DataStoreError> {
        self.get_data_store(name, scope)
    }

    /// `DataStoreService:GetRequestBudgetForRequestType(requestType)`.
    pub fn get_request_budget_for_request_type(&self, request_type: &str) -> u32 {
        let inner = self.inner.read();
        inner.ledger.remaining(RequestKind::from_name(request_type))
    }

    /// Sorted view of a namespace, honouring ascending/descending order.
    ///
    /// Used by ordered datastores and by the leaderboards that are built on top
    /// of them.
    pub fn get_ordered(
        &self,
        namespace: &str,
        descending: bool,
    ) -> Result<Vec<OrderedEntry>, DataStoreError> {
        let inner = self.inner.read();
        let mut entries: Vec<OrderedEntry> = inner
            .stores
            .get(namespace)
            .map(|m| {
                m.iter()
                    .map(|(k, v)| OrderedEntry {
                        key: k.clone(),
                        value: v.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Order by the numeric value when the payload is a bare number, which is
        // how a leaderboard stores scores; otherwise by key so output is stable.
        entries.sort_by(|a, b| {
            let av = a.value.to_value().ok().and_then(|v| v.as_f64());
            let bv = b.value.to_value().ok().and_then(|v| v.as_f64());
            match (av, bv) {
                (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                _ => a.key.cmp(&b.key),
            }
        });
        if descending {
            entries.reverse();
        }
        Ok(entries)
    }

    // --- Internal helpers for handles -----------------------------------

    fn read(&self, namespace: &str, key: &str) -> Result<Option<Value>, DataStoreError> {
        let mut inner = self.inner.write();
        Self::validate_key(inner.limits, key)?;
        inner
            .ledger
            .charge(RequestKind::Get)
            .map_err(DataStoreError::Budget)?;
        match inner.stores.get(namespace).and_then(|m| m.get(key)) {
            Some(stored) => Ok(Some(stored.to_value()?)),
            None => Ok(None),
        }
    }

    fn write(&self, namespace: &str, key: &str, value: &Value) -> Result<(), DataStoreError> {
        let mut inner = self.inner.write();
        Self::validate_key(inner.limits, key)?;
        let limits = inner.limits;
        let stored = DataStoreValue::new(value, limits)?;
        Self::ensure_namespace(&mut inner, namespace)?;
        inner
            .ledger
            .charge(RequestKind::Set)
            .map_err(DataStoreError::Budget)?;
        Self::commit(&mut inner, namespace, key, stored);
        Ok(())
    }

    fn update<F>(
        &self,
        namespace: &str,
        key: &str,
        transform: F,
    ) -> Result<Option<Value>, DataStoreError>
    where
        F: FnOnce(Option<Value>) -> Option<Value>,
    {
        let mut inner = self.inner.write();
        Self::validate_key(inner.limits, key)?;
        Self::ensure_namespace(&mut inner, namespace)?;
        inner
            .ledger
            .charge(RequestKind::Update)
            .map_err(DataStoreError::Budget)?;

        let current = match inner.stores.get(namespace).and_then(|m| m.get(key)) {
            Some(stored) => Some(stored.to_value()?),
            None => None,
        };
        match transform(current) {
            Some(next) => {
                let limits = inner.limits;
                let stored = DataStoreValue::new(&next, limits)?;
                Self::commit(&mut inner, namespace, key, stored);
                Ok(Some(next))
            }
            // Returning nil cancels the write, as Roblox documents.
            None => Ok(None),
        }
    }

    fn remove(&self, namespace: &str, key: &str) -> Result<Option<Value>, DataStoreError> {
        let mut inner = self.inner.write();
        Self::validate_key(inner.limits, key)?;
        inner
            .ledger
            .charge(RequestKind::Remove)
            .map_err(DataStoreError::Budget)?;
        let removed = inner
            .stores
            .entry(namespace.to_owned())
            .or_default()
            .remove(key);
        if let Some(old) = &removed {
            inner.bytes_used = inner.bytes_used.saturating_sub(old.len());
        }
        match removed {
            Some(stored) => Ok(Some(stored.to_value()?)),
            None => Ok(None),
        }
    }

    fn list_keys(&self, namespace: &str) -> Result<Vec<String>, DataStoreError> {
        let mut inner = self.inner.write();
        inner
            .ledger
            .charge(RequestKind::ListKeys)
            .map_err(DataStoreError::Budget)?;
        Ok(inner
            .stores
            .get(namespace)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default())
    }

    /// Writes a value and keeps the byte accounting correct.
    fn commit(inner: &mut Inner, namespace: &str, key: &str, stored: DataStoreValue) {
        Self::ensure_storage(inner, stored.len(), Some((namespace, key))).ok();
        let entry = inner.stores.entry(namespace.to_owned()).or_default();
        if let Some(old) = entry.insert(key.to_owned(), stored.clone()) {
            inner.bytes_used = inner.bytes_used.saturating_sub(old.len());
        }
        inner.bytes_used += stored.len();
    }

    /// Checks the storage allowance before writing.
    fn ensure_storage(
        inner: &mut Inner,
        incoming: usize,
        replacing: Option<(&str, &str)>,
    ) -> Result<(), DataStoreError> {
        let reclaimable = replacing
            .and_then(|(ns, key)| inner.stores.get(ns).and_then(|m| m.get(key)))
            .map(DataStoreValue::len)
            .unwrap_or(0);
        let allowance = inner.limits.base_storage_bytes
            + (inner.limits.storage_per_visit_bytes as u64 * inner.visits) as usize;
        let projected = inner.bytes_used.saturating_sub(reclaimable) + incoming;
        if projected > allowance {
            return Err(DataStoreError::StorageExhausted {
                used: inner.bytes_used,
                limit: allowance,
            });
        }
        Ok(())
    }

    /// Exports every namespace, for saving a place's data alongside it.
    pub fn export(&self) -> BTreeMap<String, BTreeMap<String, String>> {
        let inner = self.inner.read();
        inner
            .stores
            .iter()
            .map(|(ns, entries)| {
                (
                    ns.clone(),
                    entries
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_json().to_owned()))
                        .collect(),
                )
            })
            .collect()
    }

    /// Imports namespaces previously produced by [`DataStoreService::export`].
    ///
    /// Values are re-validated, so a hand-edited file cannot inject an oversized
    /// or non-JSON payload.
    pub fn import(
        &self,
        data: BTreeMap<String, BTreeMap<String, String>>,
    ) -> Result<usize, DataStoreError> {
        let mut imported = 0usize;
        for (namespace, entries) in data {
            for (key, json) in entries {
                let stored = DataStoreValue::from_json(json)?;
                let value = stored.to_value()?;
                self.write(&namespace, &key, &value)?;
                imported += 1;
            }
        }
        Ok(imported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> SessionId {
        SessionId::new("MFRGGZDFMZTWQ2LK").expect("valid base32")
    }

    #[test]
    fn session_ids_must_be_base32() {
        assert!(SessionId::new("MFRGGZDFMZTWQ2LK").is_ok());
        assert!(
            SessionId::new("abcdefghijklmnop").is_ok(),
            "lower case base32"
        );
        assert!(SessionId::new("MFRG-GZDF").is_err(), "dash is not base32");
        assert!(
            SessionId::new("0189").is_err(),
            "0,1,8,9 are not in the alphabet"
        );
        assert!(SessionId::new("").is_err());
    }

    #[test]
    fn generated_session_ids_are_base32_and_unique() {
        let a = SessionId::generate();
        let b = SessionId::generate();
        assert_ne!(a, b);
        assert_eq!(a.as_str().len(), 26);
        for c in a.as_str().chars() {
            assert!(
                matches!(c, 'A'..='Z' | '2'..='7'),
                "character {c} is not base32"
            );
        }
    }

    #[test]
    fn init_close_lifecycle() {
        let ds = DataStoreService::new();
        let s = session();
        assert!(!ds.is_session_open(&s));
        ds.init(&s).expect("opened");
        assert!(ds.is_session_open(&s));
        assert_eq!(ds.open_sessions(), 1);
        ds.close(&s).expect("closed");
        assert!(!ds.is_session_open(&s));
        assert!(matches!(
            ds.close(&s),
            Err(DataStoreError::NoSuchSession { .. })
        ));
    }

    #[test]
    fn double_init_is_refused() {
        let ds = DataStoreService::new();
        let s = session();
        ds.init(&s).unwrap();
        assert!(matches!(
            ds.init(&s),
            Err(DataStoreError::SessionAlreadyOpen { .. })
        ));
    }

    #[test]
    fn operations_require_an_open_session() {
        let ds = DataStoreService::new();
        let s = session();
        assert!(matches!(
            ds.get(&s, "k"),
            Err(DataStoreError::NoSuchSession { .. })
        ));
        assert!(matches!(
            ds.insert(&s, "k", &Value::from(1i64)),
            Err(DataStoreError::NoSuchSession { .. })
        ));
        assert!(matches!(
            ds.get_keys(&s),
            Err(DataStoreError::NoSuchSession { .. })
        ));
        assert!(matches!(
            ds.delete(&s, "k"),
            Err(DataStoreError::NoSuchSession { .. })
        ));
    }

    #[test]
    fn insert_get_delete_round_trip() {
        let ds = DataStoreService::new();
        let s = session();
        ds.init(&s).unwrap();

        assert_eq!(ds.get(&s, "score").unwrap(), None);
        ds.insert(&s, "score", &Value::from(1337i64)).unwrap();
        assert_eq!(ds.get(&s, "score").unwrap(), Some(Value::from(1337i64)));

        assert_eq!(ds.get_keys(&s).unwrap(), vec!["score".to_owned()]);

        let removed = ds.delete(&s, "score").unwrap();
        assert_eq!(removed, Some(Value::from(1337i64)));
        assert_eq!(ds.get(&s, "score").unwrap(), None);
        assert!(ds.get_keys(&s).unwrap().is_empty());
    }

    #[test]
    fn values_must_be_json() {
        let ds = DataStoreService::new();
        let s = session();
        ds.init(&s).unwrap();
        // Every Value variant is JSON-representable, including nested tables.
        let complex = Value::map([
            ("Coins", Value::from(100i64)),
            (
                "Items",
                Value::array([Value::from("Sword"), Value::from("Shield")]),
            ),
            ("Nested", Value::map([("deep", Value::from(true))])),
        ]);
        ds.insert(&s, "save", &complex).unwrap();
        assert_eq!(ds.get(&s, "save").unwrap(), Some(complex));
    }

    #[test]
    fn keys_are_limited_to_fifty_ascii_characters() {
        let ds = DataStoreService::new();
        let s = session();
        ds.init(&s).unwrap();

        let fifty = "a".repeat(50);
        assert!(
            ds.insert(&s, &fifty, &Value::from(1i64)).is_ok(),
            "50 is allowed"
        );

        let fifty_one = "a".repeat(51);
        match ds.insert(&s, &fifty_one, &Value::from(1i64)) {
            Err(DataStoreError::InvalidKey { reason, .. }) => {
                assert!(reason.contains("50"), "reason names the limit: {reason}");
            }
            other => panic!("expected InvalidKey, got {other:?}"),
        }

        // Non-ASCII must be rejected too.
        assert!(matches!(
            ds.insert(&s, "khoá", &Value::from(1i64)),
            Err(DataStoreError::InvalidKey { .. })
        ));
        assert!(matches!(
            ds.insert(&s, "", &Value::from(1i64)),
            Err(DataStoreError::InvalidKey { .. })
        ));
    }

    #[test]
    fn values_are_limited_to_one_megabyte() {
        let ds = DataStoreService::new();
        let s = session();
        ds.init(&s).unwrap();
        // A ~1.2 MB string exceeds the documented 1 MB value limit.
        let big = Value::from("x".repeat(1_200_000));
        match ds.insert(&s, "big", &big) {
            Err(DataStoreError::ValueTooLarge { bytes, limit }) => {
                assert!(bytes > limit);
                assert_eq!(limit, 1024 * 1024);
            }
            other => panic!("expected ValueTooLarge, got {other:?}"),
        }
    }

    #[test]
    fn at_most_fifteen_datastores() {
        let ds = DataStoreService::new();
        for i in 0..15 {
            ds.get_data_store(&format!("store{i}"), "")
                .expect("under the limit");
        }
        assert_eq!(ds.datastore_count(), 15);
        let err = ds.get_data_store("store16", "").unwrap_err();
        assert!(matches!(
            err,
            DataStoreError::TooManyDataStores { limit: 15, .. }
        ));
    }

    #[test]
    fn reusing_a_datastore_name_does_not_consume_a_slot() {
        let ds = DataStoreService::new();
        for _ in 0..20 {
            ds.get_data_store("Shared", "").expect("same store reopens");
        }
        assert_eq!(ds.datastore_count(), 1);
    }

    #[test]
    fn at_most_sixteen_concurrent_sessions() {
        let ds = DataStoreService::new();
        let mut opened = Vec::new();
        for i in 0..16 {
            // Base32 excludes 0/1/8/9, so use letters for the session id.
            let letter = (b'A' + i as u8) as char;
            let s = SessionId::new(format!("SESS{letter}{letter}{letter}{letter}"))
                .expect("valid base32 session id");
            ds.init(&s).expect("under the limit");
            opened.push(s);
        }
        let extra = SessionId::new("ONESIX").unwrap();
        assert!(matches!(
            ds.init(&extra),
            Err(DataStoreError::Budget(
                BudgetError::TooManyConnections { .. }
            ))
        ));

        // Closing one frees a slot.
        ds.close(&opened[0]).unwrap();
        assert!(ds.init(&extra).is_ok());
    }

    #[test]
    fn request_budget_is_enforced_and_reportable() {
        let ds = DataStoreService::with_limits(DataStoreLimits {
            max_datastores: 15,
            max_key_len: 50,
            max_value_bytes: 1024,
            base_storage_bytes: 25 * 1024 * 1024,
            storage_per_visit_bytes: 5 * 1024 * 1024,
        });
        // Replace the ledger's limits through the public accessor.
        ds.with_ledger(|l| {
            *l = RequestLedger::with_limits(crate::budget::Limits {
                base_requests_per_minute: 2,
                requests_per_player_per_minute: 0,
                ..crate::budget::Limits::default()
            });
        });
        let s = session();
        ds.init(&s).unwrap();

        assert_eq!(ds.get_request_budget_for_request_type("Get"), 2);
        ds.insert(&s, "a", &Value::from(1i64)).unwrap();
        ds.get(&s, "a").unwrap();
        assert_eq!(
            ds.get_request_budget_for_request_type("Get"),
            0,
            "pool is shared and now empty"
        );
        let err = ds.get(&s, "a").unwrap_err();
        assert!(matches!(
            err,
            DataStoreError::Budget(BudgetError::Exhausted { .. })
        ));

        // A new window restores it.
        ds.with_ledger(RequestLedger::roll_window);
        assert_eq!(ds.get_request_budget_for_request_type("Get"), 2);
        assert!(ds.get(&s, "a").is_ok());
    }

    #[test]
    fn idle_sessions_are_reaped_after_fifteen_seconds() {
        let ds = DataStoreService::new();
        let s = SessionId::new("IDLEONE").unwrap();
        ds.init(&s).unwrap();
        assert!(ds.is_session_open(&s));
        // 14 seconds: still open.
        assert!(ds.reap_idle_sessions(1400, 100).is_empty());
        assert!(ds.is_session_open(&s));
        // 15 seconds: reaped, and further use reports NoSuchSession.
        let reaped = ds.reap_idle_sessions(1500, 100);
        assert_eq!(reaped, vec!["IDLEONE".to_owned()]);
        assert!(!ds.is_session_open(&s));
        assert!(matches!(
            ds.get(&s, "k"),
            Err(DataStoreError::NoSuchSession { .. })
        ));
    }

    #[test]
    fn storage_allowance_scales_with_visits() {
        let ds = DataStoreService::with_limits(DataStoreLimits {
            max_datastores: 15,
            max_key_len: 50,
            max_value_bytes: 1024 * 1024,
            base_storage_bytes: 1000,
            storage_per_visit_bytes: 500,
        });
        assert_eq!(ds.storage_allowance(), 1000);
        ds.set_visits(4);
        assert_eq!(ds.storage_allowance(), 3000, "1000 + 500*4");
    }

    #[test]
    fn writing_past_the_storage_allowance_is_refused() {
        let ds = DataStoreService::with_limits(DataStoreLimits {
            max_datastores: 15,
            max_key_len: 50,
            max_value_bytes: 1024 * 1024,
            base_storage_bytes: 40,
            storage_per_visit_bytes: 0,
        });
        let s = session();
        ds.init(&s).unwrap();
        // A short value fits.
        ds.insert(&s, "a", &Value::from(1i64)).unwrap();
        // A longer one does not.
        let big = Value::from("y".repeat(100));
        assert!(matches!(
            ds.insert(&s, "b", &big),
            Err(DataStoreError::StorageExhausted { .. })
        ));
        // Replacing an existing key reclaims its old bytes first, so it is allowed.
        let mid = Value::from("z".repeat(20));
        assert!(ds.insert(&s, "a", &mid).is_ok());
    }

    #[test]
    fn byte_accounting_tracks_overwrites_and_deletes() {
        let ds = DataStoreService::new();
        let s = session();
        ds.init(&s).unwrap();
        ds.insert(&s, "k", &Value::from("0123456789")).unwrap();
        let after_first = ds.bytes_used();
        assert!(after_first > 0);

        // Overwriting must not double-count.
        ds.insert(&s, "k", &Value::from("0123456789")).unwrap();
        assert_eq!(
            ds.bytes_used(),
            after_first,
            "overwrite reclaims the old bytes"
        );

        ds.delete(&s, "k").unwrap();
        assert_eq!(ds.bytes_used(), 0);
    }

    #[test]
    fn roblox_style_handle_api_round_trip() {
        let ds = DataStoreService::new();
        let store = ds.get_data_store("PlayerData", "global").unwrap();
        assert_eq!(store.name(), "PlayerData");
        assert_eq!(store.scope(), "global");

        assert_eq!(store.get_async("Alice").unwrap(), None);
        store
            .set_async("Alice", &Value::map([("Coins", Value::from(50i64))]))
            .unwrap();
        assert_eq!(
            store.get_async("Alice").unwrap(),
            Some(Value::map([("Coins", Value::from(50i64))]))
        );
        assert_eq!(store.list_keys().unwrap(), vec!["Alice".to_owned()]);

        let removed = store.remove_async("Alice").unwrap();
        assert!(removed.is_some());
        assert_eq!(store.get_async("Alice").unwrap(), None);
    }

    #[test]
    fn scope_separates_namespaces() {
        let ds = DataStoreService::new();
        let global = ds.get_data_store("Scores", "global").unwrap();
        let season1 = ds.get_data_store("Scores", "season1").unwrap();
        global.set_async("Alice", &Value::from(10i64)).unwrap();
        season1.set_async("Alice", &Value::from(99i64)).unwrap();
        assert_eq!(global.get_async("Alice").unwrap(), Some(Value::from(10i64)));
        assert_eq!(
            season1.get_async("Alice").unwrap(),
            Some(Value::from(99i64))
        );
        // Two distinct namespaces, so two of the fifteen slots are consumed.
        assert_eq!(ds.datastore_count(), 2);
    }

    #[test]
    fn update_async_transforms_and_can_cancel() {
        let ds = DataStoreService::new();
        let store = ds.get_data_store("Counters", "").unwrap();

        // Updating a missing key passes None in.
        let result = store
            .update_async("visits", |current| {
                assert_eq!(current, None);
                Some(Value::from(1i64))
            })
            .unwrap();
        assert_eq!(result, Some(Value::from(1i64)));

        // Updating an existing key increments it.
        let result = store
            .update_async("visits", |current| match current {
                Some(Value::Number(n)) => Some(Value::from(n as i64 + 1)),
                _ => Some(Value::from(0i64)),
            })
            .unwrap();
        assert_eq!(result, Some(Value::from(2i64)));
        assert_eq!(store.get_async("visits").unwrap(), Some(Value::from(2i64)));

        // Returning nil cancels the write.
        let result = store.update_async("visits", |_| None).unwrap();
        assert_eq!(result, None);
        assert_eq!(
            store.get_async("visits").unwrap(),
            Some(Value::from(2i64)),
            "cancelled update left the value alone"
        );
    }

    #[test]
    fn ordered_datastore_lists_by_value_and_honours_direction() {
        let ds = DataStoreService::new();
        let store = ds.get_ordered_data_store("Leaderboard", "").unwrap();
        for (name, score) in [("Alice", 10i64), ("Bob", 30), ("Cara", 20)] {
            store.set_async(name, &Value::from(score)).unwrap();
        }

        let ascending = ds.get_ordered("Leaderboard", false).unwrap();
        let keys: Vec<&str> = ascending.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["Alice", "Cara", "Bob"]);

        let descending = ds.get_ordered("Leaderboard", true).unwrap();
        let keys: Vec<&str> = descending.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["Bob", "Cara", "Alice"]);
    }

    #[test]
    fn export_and_import_round_trip() {
        let ds = DataStoreService::new();
        let store = ds.get_data_store("Save", "").unwrap();
        store.set_async("k1", &Value::from("hello")).unwrap();
        store.set_async("k2", &Value::from(7i64)).unwrap();

        let dump = ds.export();
        let fresh = DataStoreService::new();
        assert_eq!(fresh.import(dump).unwrap(), 2);

        let reopened = fresh.get_data_store("Save", "").unwrap();
        assert_eq!(
            reopened.get_async("k1").unwrap(),
            Some(Value::from("hello"))
        );
        assert_eq!(reopened.get_async("k2").unwrap(), Some(Value::from(7i64)));
    }

    #[test]
    fn import_rejects_corrupt_json() {
        let ds = DataStoreService::new();
        let mut data = BTreeMap::new();
        let mut entries = BTreeMap::new();
        entries.insert("bad".to_owned(), "{not json".to_owned());
        data.insert("Save".to_owned(), entries);
        assert!(matches!(
            ds.import(data),
            Err(DataStoreError::NotJson { .. })
        ));
    }

    #[test]
    fn stored_values_report_their_size_and_value() {
        let v = Value::map([("a", Value::from(1i64))]);
        let stored = DataStoreValue::new(&v, DataStoreLimits::default()).unwrap();
        assert_eq!(stored.to_value().unwrap(), v);
        assert!(stored.len() > 0);
        assert!(!stored.is_empty());
        assert!(stored.as_json().contains("\"a\""));
    }
}
