/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! # nb-http
//!
//! Shared HTTP client used by HttpService, the website's internal calls,
//! and the content moderation client.

pub mod moderation;

pub use moderation::{
    ModerationCache, ModerationClient, ModerationError, ModerationResult, DEFAULT_MODERATION_URL,
};
