/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `ClientApp` – the eframe application state.
//!
//! Holds the current [`Screen`], the network handle, and all data the UI needs
//! to render.  Drives a state machine:
//!
//! ```text
//! Connect ──connect──► Connecting ──welcome──► Lobby
//!                                 ──error───► Connect (with message)
//! Lobby ──disconnect──► Connect
//! ```

use std::time::Instant;

use eframe::{egui, Frame};
use serde_json::Value as JsVal;

use crate::net::{NetCmd, NetHandle, NetMsg, ServerMsg};

// ── Screens ────────────────────────────────────────────────────────────────────

/// Which top-level screen the client is on.
#[derive(Debug, Default, PartialEq)]
pub enum Screen {
    /// The splash / connect form.
    #[default]
    Connect,
    /// Waiting for the TCP handshake to complete.
    Connecting,
    /// In-game: world view, chat, event console.
    Lobby,
}

// ── Chat / log entry ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub kind: LogKind,
    pub text: String,
    #[allow(dead_code)]
    pub at: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogKind {
    System,
    Event,
    Error,
}

impl LogEntry {
    pub fn system(text: impl Into<String>) -> Self {
        Self { kind: LogKind::System, text: text.into(), at: Instant::now() }
    }
    pub fn event(text: impl Into<String>) -> Self {
        Self { kind: LogKind::Event, text: text.into(), at: Instant::now() }
    }
    pub fn error(text: impl Into<String>) -> Self {
        Self { kind: LogKind::Error, text: text.into(), at: Instant::now() }
    }
}

// ── Snapshot ──────────────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct Snapshot {
    pub tick: u64,
    pub instances: Vec<JsVal>,
}

// ── Application state ─────────────────────────────────────────────────────────

pub struct ClientApp {
    // ── Connection form ───
    pub host: String,
    pub port_str: String,
    pub username: String,
    pub connect_error: Option<String>,

    // ── Screen ────────────
    pub screen: Screen,

    // ── Server metadata ───
    pub server_name: String,
    pub max_players: u32,

    // ── Live data ─────────
    pub snapshot: Snapshot,
    pub log: Vec<LogEntry>,

    // ── Event console ─────
    pub event_name: String,
    pub event_args: String,

    // ── Ping ──────────────
    pub last_ping: Option<Instant>,
    pub ping_ms: Option<u128>,

    // ── Net handle ────────
    pub net: NetHandle,
}

impl ClientApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        host: String,
        port: u16,
        username: String,
    ) -> Self {
        let net = NetHandle::start();
        Self {
            host,
            port_str: port.to_string(),
            username,
            connect_error: None,
            screen: Screen::Connect,
            server_name: String::new(),
            max_players: 0,
            snapshot: Snapshot::default(),
            log: vec![LogEntry::system("Welcome to Noobietoria Client.")],
            event_name: String::new(),
            event_args: String::new(),
            last_ping: None,
            ping_ms: None,
            net,
        }
    }

    /// Drain the net channel and update state.
    pub fn poll_net(&mut self) {
        while let Ok(msg) = self.net.msg_rx.try_recv() {
            match msg {
                NetMsg::Connected { server_name, max_players } => {
                    self.server_name = server_name.clone();
                    self.max_players = max_players;
                    self.screen = Screen::Lobby;
                    self.log
                        .push(LogEntry::system(format!("Connected to «{server_name}».")));
                }
                NetMsg::Disconnected { reason } => {
                    self.screen = Screen::Connect;
                    self.connect_error = Some(reason.clone());
                    self.log.push(LogEntry::error(format!("Disconnected: {reason}")));
                }
                NetMsg::Error(e) => {
                    if self.screen == Screen::Connecting {
                        self.screen = Screen::Connect;
                        self.connect_error = Some(e.clone());
                    }
                    self.log.push(LogEntry::error(e));
                }
                NetMsg::Server(ServerMsg::Snapshot { instances, tick }) => {
                    self.snapshot = Snapshot { tick, instances };
                }
                NetMsg::Server(ServerMsg::Event { event, args }) => {
                    self.log.push(LogEntry::event(format!(
                        "← {event}({})",
                        args.iter()
                            .map(|a| a.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )));
                }
                NetMsg::Server(ServerMsg::Pong) => {
                    if let Some(sent) = self.last_ping {
                        self.ping_ms = Some(sent.elapsed().as_millis());
                    }
                }
                NetMsg::Server(_) => {}
            }
        }
    }

    pub fn do_connect(&mut self) {
        let port: u16 = self.port_str.parse().unwrap_or(7777);
        self.screen = Screen::Connecting;
        self.connect_error = None;
        let _ = self.net.cmd_tx.send(NetCmd::Connect {
            host: self.host.clone(),
            port,
            username: self.username.clone(),
        });
    }

    pub fn do_disconnect(&mut self) {
        let _ = self.net.cmd_tx.send(NetCmd::Disconnect);
        self.screen = Screen::Connect;
        self.log.push(LogEntry::system("Disconnected."));
    }

    pub fn do_ping(&mut self) {
        self.last_ping = Some(Instant::now());
        let _ = self.net.cmd_tx.send(NetCmd::Ping);
    }

    pub fn do_fire(&mut self) {
        let event = self.event_name.trim().to_owned();
        if event.is_empty() {
            return;
        }
        let args: Vec<serde_json::Value> = serde_json::from_str(&format!("[{}]", self.event_args))
            .unwrap_or_default();
        self.log.push(LogEntry::event(format!("→ {event}(…)")));
        let _ = self.net.cmd_tx.send(NetCmd::Fire { event, args });
    }
}

impl eframe::App for ClientApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        self.poll_net();
        crate::ui::draw(self, ctx);
        // Keep rendering so we keep polling the net channel.
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
}
