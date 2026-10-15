/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! # nb-store
//!
//! Persistence for a place: `DataStoreService` plus the keyed stores the
//! gameplay services sit on.
//!
//! ## Why the limits live in code
//!
//! The documentation does not describe DataStore as "a dictionary"; it
//! describes a *budgeted* service:
//!
//! * at most **15 datastores** per game,
//! * keys of at most **50 ASCII characters** and values of at most **1 MB**,
//! * total storage of `25MB + 5MB × visits`,
//! * `15 + 10 × CCU` requests per server per minute and at most
//!   **16 concurrent datastore connections** per server,
//! * sessions that drop after **15 seconds** of silence,
//! * values that must be **JSON**.
//!
//! Those numbers are enforced here rather than documented and forgotten, so a
//! script that would be rejected by the real service is rejected identically by
//! the Dedicated server and by the test suite.

pub mod budget;
pub mod datastore;
pub mod kv;

pub use budget::{BudgetError, RequestKind, RequestLedger};
pub use datastore::{
    DataStoreError, DataStoreLimits, DataStoreService, DataStoreValue, OrderedEntry, SessionId,
};
pub use kv::{KeyedStore, StoreError, StoreResult};
