/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! # nb-events
//! Broadcast event bus for cross-service notifications.

pub mod notification;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub topic: String,
    pub payload: serde_json::Value,
}

/// A fan-out event bus. Publishers send on a topic; every subscriber
/// receives a clone of every event on topics they are subscribed to.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
    subscribers: Arc<RwLock<HashMap<String, broadcast::Sender<Event>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            tx,
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Publish an event to all subscribers of `topic`.
    pub fn publish(&self, topic: &str, payload: serde_json::Value) {
        let event = Event {
            topic: topic.to_string(),
            payload: payload.clone(),
        };
        let _ = self.tx.send(event.clone());
        if let Some(sub) = self.subscribers.read().get(topic) {
            let _ = sub.send(event);
        }
        tracing::debug!("published event on topic={topic}");
    }

    /// Subscribe to a topic; returns a receiver of events on that topic.
    pub fn subscribe(&self, topic: &str) -> broadcast::Receiver<Event> {
        let mut subs = self.subscribers.write();
        subs.entry(topic.to_string())
            .or_insert_with(|| broadcast::channel(256).0)
            .subscribe()
    }

    /// Global receiver that sees every published event.
    pub fn global(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
