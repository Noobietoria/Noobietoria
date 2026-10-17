/* Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! Notification system

use tokio::sync::mpsc;

pub type NotificationSender = mpsc::UnboundedSender<String>;
pub type NotificationReceiver = mpsc::UnboundedReceiver<String>;

pub struct NotificationService {
    tx: NotificationSender,
}

impl NotificationService {
    pub fn new() -> (Self, NotificationReceiver) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx }, rx)
    }

    pub fn send(&self, message: &str) {
        self.tx.send(message.to_string()).ok();
    }

    pub fn notify_user(&self, user_id: u64, message: &str) {
        let msg = format!("[User{user_id}] {}", message);
        self.send(&msg);
    }

    pub fn notify_server(&self, server_id: &str, message: &str) {
        let msg = format!("[Server{server_id}] {}", message);
        self.send(&msg);
    }
}
