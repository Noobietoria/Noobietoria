//! # nb-http
//!
//! Shared HTTP client used by HttpService, the website's internal calls,
//! and the content moderation client.

pub mod moderation;

pub use moderation::{
    ModerationCache, ModerationClient, ModerationError, ModerationResult, DEFAULT_MODERATION_URL,
};
