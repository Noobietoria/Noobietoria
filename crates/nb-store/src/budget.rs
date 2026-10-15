//! Request budgeting and concurrent-connection limits for `DataStoreService`.
//!
//! The documentation specifies a per-server request allowance of
//! `15 + (10 × N)` requests per minute (where `N` is the concurrent player
//! count) and a ceiling of 16 simultaneous datastore connections for the whole
//! server. This module turns both into a small, testable ledger.

use std::collections::HashMap;

/// The kind of datastore request, matching
/// `DataStoreService:GetRequestBudgetForRequestType(requestType)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RequestKind {
    /// `Get`
    Get,
    /// `Set` / `Insert`
    Set,
    /// `Update`
    Update,
    /// `Remove` / `Delete`
    Remove,
    /// `GetKeys` / `ListKeys`
    ListKeys,
    /// Any request type a script names that this build does not model.
    Other,
}

impl RequestKind {
    /// Parses the request-type string the documented API takes.
    pub fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "get" | "getasync" => RequestKind::Get,
            "set" | "setasync" | "insert" => RequestKind::Set,
            "update" | "updateasync" => RequestKind::Update,
            "remove" | "removeasync" | "delete" => RequestKind::Remove,
            "getkeys" | "listkeys" | "listkeysasync" => RequestKind::ListKeys,
            _ => RequestKind::Other,
        }
    }

    /// The canonical name, as reported back to scripts.
    pub fn name(self) -> &'static str {
        match self {
            RequestKind::Get => "Get",
            RequestKind::Set => "Set",
            RequestKind::Update => "Update",
            RequestKind::Remove => "Remove",
            RequestKind::ListKeys => "GetKeys",
            RequestKind::Other => "Other",
        }
    }
}

/// Why a request could not be admitted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BudgetError {
    /// The per-minute allowance for this server is exhausted.
    #[error(
        "datastore request budget exhausted for {kind:?}: used {used} of {allowance} this minute"
    )]
    Exhausted {
        /// The request type that was refused.
        kind: RequestKind,
        /// Requests already made in the current window.
        used: u32,
        /// The allowance for the window.
        allowance: u32,
    },
    /// All 16 datastore connections are already open.
    #[error("datastore connection limit reached: {open} of {limit} connections open")]
    TooManyConnections {
        /// Connections currently open.
        open: usize,
        /// The documented ceiling.
        limit: usize,
    },
    /// The connection being closed was never opened.
    #[error("no open datastore connection for session `{session}`")]
    UnknownConnection {
        /// The session id that was not found.
        session: String,
    },
}

/// The documented per-server ceilings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum simultaneous datastore connections for the whole server.
    pub max_connections: usize,
    /// Base requests per minute before the per-player allowance.
    pub base_requests_per_minute: u32,
    /// Additional requests per minute per concurrent player.
    pub requests_per_player_per_minute: u32,
    /// Seconds of silence after which a session is dropped.
    pub idle_timeout_secs: u64,
}

impl Default for Limits {
    /// The numbers from the documentation: 16 connections, `15 + 10 × CCU`
    /// requests per minute, a 15-second idle timeout.
    fn default() -> Self {
        Self {
            max_connections: 16,
            base_requests_per_minute: 15,
            requests_per_player_per_minute: 10,
            idle_timeout_secs: 15,
        }
    }
}

/// Tracks requests and open connections for one server.
#[derive(Debug, Clone)]
pub struct RequestLedger {
    limits: Limits,
    /// Requests spent in the current one-minute window, per kind.
    spent: HashMap<RequestKind, u32>,
    /// Requests spent in the current window overall.
    spent_total: u32,
    /// The current concurrent-player count, which scales the allowance.
    ccu: u32,
    /// Open connections, keyed by session id, with the last activity tick.
    connections: HashMap<String, u64>,
}

impl RequestLedger {
    /// A fresh ledger with the documented limits.
    pub fn new() -> Self {
        Self::with_limits(Limits::default())
    }

    /// A ledger with explicitly supplied limits, used by tests.
    pub fn with_limits(limits: Limits) -> Self {
        Self {
            limits,
            spent: HashMap::new(),
            spent_total: 0,
            ccu: 0,
            connections: HashMap::new(),
        }
    }

    /// The limits in force.
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// Updates the concurrent-player count, which changes the allowance.
    pub fn set_ccu(&mut self, ccu: u32) {
        self.ccu = ccu;
    }

    /// The current concurrent-player count.
    pub fn ccu(&self) -> u32 {
        self.ccu
    }

    /// The total minutes allowance: `15 + 10 × CCU`.
    pub fn allowance(&self) -> u32 {
        self.limits.base_requests_per_minute.saturating_add(
            self.limits
                .requests_per_player_per_minute
                .saturating_mul(self.ccu),
        )
    }

    /// Requests still available this window.
    ///
    /// Backs `DataStoreService:GetRequestBudgetForRequestType`.
    pub fn remaining(&self, kind: RequestKind) -> u32 {
        let allowance = self.allowance();
        let used_by_kind = self.spent.get(&kind).copied().unwrap_or(0);
        // The allowance is a server-wide pool, so the honest answer is the
        // smaller of "what this kind has left" and "what the server has left".
        allowance
            .saturating_sub(self.spent_total)
            .min(allowance.saturating_sub(used_by_kind))
    }

    /// Requests already spent in this window.
    pub fn used(&self) -> u32 {
        self.spent_total
    }

    /// Begins a new one-minute window, clearing the spend counters.
    pub fn roll_window(&mut self) {
        self.spent.clear();
        self.spent_total = 0;
    }

    /// Charges one request, or fails when the allowance is gone.
    pub fn charge(&mut self, kind: RequestKind) -> Result<(), BudgetError> {
        let allowance = self.allowance();
        if self.spent_total >= allowance {
            return Err(BudgetError::Exhausted {
                kind,
                used: self.spent_total,
                allowance,
            });
        }
        *self.spent.entry(kind).or_insert(0) += 1;
        self.spent_total += 1;
        Ok(())
    }

    /// Opens a connection, honouring the 16-connection ceiling.
    pub fn open_connection(&mut self, session: &str, now_tick: u64) -> Result<(), BudgetError> {
        // Re-opening an existing session simply refreshes it.
        if let Some(last) = self.connections.get_mut(session) {
            *last = now_tick;
            return Ok(());
        }
        if self.connections.len() >= self.limits.max_connections {
            return Err(BudgetError::TooManyConnections {
                open: self.connections.len(),
                limit: self.limits.max_connections,
            });
        }
        self.connections.insert(session.to_owned(), now_tick);
        Ok(())
    }

    /// Records activity on a session so it is not reaped.
    pub fn touch(&mut self, session: &str, now_tick: u64) -> Result<(), BudgetError> {
        match self.connections.get_mut(session) {
            Some(last) => {
                *last = now_tick;
                Ok(())
            }
            None => Err(BudgetError::UnknownConnection {
                session: session.to_owned(),
            }),
        }
    }

    /// Closes a connection.
    pub fn close_connection(&mut self, session: &str) -> Result<(), BudgetError> {
        match self.connections.remove(session) {
            Some(_) => Ok(()),
            None => Err(BudgetError::UnknownConnection {
                session: session.to_owned(),
            }),
        }
    }

    /// Number of connections currently open.
    pub fn open_connections(&self) -> usize {
        self.connections.len()
    }

    /// Whether a session is open.
    pub fn is_open(&self, session: &str) -> bool {
        self.connections.contains_key(session)
    }

    /// Session ids that have been silent for longer than the documented
    /// 15-second timeout and should therefore be dropped.
    ///
    /// `ticks_per_second` is supplied by the caller so tests can drive time
    /// directly instead of sleeping.
    pub fn timed_out(&self, now_tick: u64, ticks_per_second: u64) -> Vec<String> {
        let timeout_ticks = self
            .limits
            .idle_timeout_secs
            .saturating_mul(ticks_per_second);
        let mut timed_out: Vec<String> = self
            .connections
            .iter()
            .filter(|(_, last)| now_tick.saturating_sub(**last) >= timeout_ticks)
            .map(|(session, _)| session.clone())
            .collect();
        timed_out.sort();
        timed_out
    }

    /// Drops every timed-out session, returning the ones that were closed.
    pub fn reap_timed_out(&mut self, now_tick: u64, ticks_per_second: u64) -> Vec<String> {
        let expired = self.timed_out(now_tick, ticks_per_second);
        for session in &expired {
            self.connections.remove(session);
        }
        expired
    }
}

impl Default for RequestLedger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowance_follows_the_documented_formula() {
        let mut l = RequestLedger::new();
        assert_eq!(l.allowance(), 15, "no players yet");
        l.set_ccu(10);
        assert_eq!(l.allowance(), 115, "15 + 10*10");
        l.set_ccu(0);
        assert_eq!(l.allowance(), 15);
    }

    #[test]
    fn charging_stops_at_the_allowance() {
        let mut l = RequestLedger::with_limits(Limits {
            base_requests_per_minute: 3,
            requests_per_player_per_minute: 0,
            ..Limits::default()
        });
        assert!(l.charge(RequestKind::Get).is_ok());
        assert!(l.charge(RequestKind::Get).is_ok());
        assert!(l.charge(RequestKind::Set).is_ok());
        let err = l.charge(RequestKind::Get).unwrap_err();
        assert!(matches!(err, BudgetError::Exhausted { allowance: 3, .. }));
        assert_eq!(l.used(), 3);
        assert_eq!(l.remaining(RequestKind::Get), 0);
    }

    #[test]
    fn rolling_the_window_restores_the_budget() {
        let mut l = RequestLedger::with_limits(Limits {
            base_requests_per_minute: 1,
            requests_per_player_per_minute: 0,
            ..Limits::default()
        });
        assert!(l.charge(RequestKind::Get).is_ok());
        assert!(l.charge(RequestKind::Get).is_err());
        l.roll_window();
        assert_eq!(l.used(), 0);
        assert!(l.charge(RequestKind::Get).is_ok());
    }

    #[test]
    fn request_kinds_parse_the_documented_names() {
        assert_eq!(RequestKind::from_name("Get"), RequestKind::Get);
        assert_eq!(RequestKind::from_name("getasync"), RequestKind::Get);
        assert_eq!(RequestKind::from_name("SetAsync"), RequestKind::Set);
        assert_eq!(RequestKind::from_name("UpdateAsync"), RequestKind::Update);
        assert_eq!(RequestKind::from_name("RemoveAsync"), RequestKind::Remove);
        assert_eq!(RequestKind::from_name("GetKeys"), RequestKind::ListKeys);
        assert_eq!(RequestKind::from_name("Something"), RequestKind::Other);
    }

    #[test]
    fn connection_ceiling_is_sixteen() {
        let mut l = RequestLedger::new();
        for i in 0..16 {
            l.open_connection(&format!("s{i}"), 0)
                .expect("under the limit");
        }
        assert_eq!(l.open_connections(), 16);
        let err = l.open_connection("s16", 0).unwrap_err();
        assert!(matches!(
            err,
            BudgetError::TooManyConnections {
                open: 16,
                limit: 16
            }
        ));
    }

    #[test]
    fn reopening_a_session_refreshes_rather_than_duplicates() {
        let mut l = RequestLedger::new();
        l.open_connection("a", 0).unwrap();
        l.open_connection("a", 5).unwrap();
        assert_eq!(l.open_connections(), 1);
        assert!(l.is_open("a"));
    }

    #[test]
    fn closing_an_unknown_session_errors() {
        let mut l = RequestLedger::new();
        assert!(matches!(
            l.close_connection("nope"),
            Err(BudgetError::UnknownConnection { .. })
        ));
        l.open_connection("a", 0).unwrap();
        assert!(l.close_connection("a").is_ok());
        assert!(!l.is_open("a"));
    }

    #[test]
    fn sessions_time_out_after_fifteen_seconds_of_silence() {
        // 100 ticks per second for easy arithmetic.
        let mut l = RequestLedger::new();
        l.open_connection("fresh", 0).unwrap();
        l.open_connection("stale", 0).unwrap();
        // Keep `fresh` alive at tick 500 (5 seconds).
        l.touch("fresh", 500).unwrap();

        // At 1400 ticks (14 s) nothing has expired: `stale` is at 14s, under 15.
        assert!(
            l.timed_out(1400, 100).is_empty(),
            "14s is still within the timeout"
        );

        // At 1500 ticks (15 s) `stale` is exactly at the timeout and is reaped.
        let expired = l.reap_timed_out(1500, 100);
        assert_eq!(expired, vec!["stale".to_owned()]);
        assert!(l.is_open("fresh"));
        assert!(!l.is_open("stale"));

        // `fresh` expires 15s after its last activity at tick 500, i.e. tick 2000.
        assert!(l.timed_out(1999, 100).is_empty());
        assert_eq!(l.reap_timed_out(2000, 100), vec!["fresh".to_owned()]);
    }

    #[test]
    fn touching_an_unknown_session_errors() {
        let mut l = RequestLedger::new();
        assert!(matches!(
            l.touch("ghost", 1),
            Err(BudgetError::UnknownConnection { .. })
        ));
    }

    #[test]
    fn remaining_never_underflows() {
        let mut l = RequestLedger::with_limits(Limits {
            base_requests_per_minute: 2,
            requests_per_player_per_minute: 0,
            ..Limits::default()
        });
        l.charge(RequestKind::Get).unwrap();
        l.charge(RequestKind::Get).unwrap();
        assert_eq!(l.remaining(RequestKind::Get), 0);
        assert_eq!(l.remaining(RequestKind::Set), 0, "shared pool is empty");
    }
}
