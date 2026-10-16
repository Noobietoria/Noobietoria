/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `ClientApp` – the eframe application state.
//!
//! Holds the current [`Screen`], the two background handles (website API and
//! the game socket), and all data the UI needs to render.
//!
//! State machine:
//!
//! ```text
//! Home ──pick place──► Place ──join server──► Connecting ──welcome──► Lobby
//!                                                        ──error────► Place
//! Lobby ──leave──► Home
//! ```
//!
//! The player never types a host or a port: [`ClientApp`] loads the published
//! places and their joinable servers from the website API and dials whichever
//! server was picked.

use std::time::Instant;

use eframe::{egui, Frame};
use serde_json::Value as JsVal;

use crate::api::{ApiCmd, ApiHandle, ApiMsg, ServerInfo, WorldInfo};
use crate::net::{NetCmd, NetHandle, NetMsg, ServerMsg};

// ── Screens ────────────────────────────────────────────────────────────────────

/// Which top-level screen the client is on.
#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum Screen {
    /// Discover page: published places, styled like the Roblox home grid.
    #[default]
    Home,
    /// A single place: its server list, with one Join button per server.
    Place,
    /// Waiting for the TCP handshake to complete.
    Connecting,
    /// In-game: world view, chat, event console.
    Lobby,
}

// ── Rail sections ─────────────────────────────────────────────────────────────

/// The seven sections of the PC app rail, in mockup order. Only [`Home`] is
/// implemented today; the rest render a placeholder header.
///
/// [`Home`]: NavSection::Home
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum NavSection {
    #[default]
    Home,
    Search,
    Explore,
    Shop,
    Create,
    Chat,
    Profile,
}

impl NavSection {
    /// Label shown in the header and in the placeholder body.
    pub fn label(self) -> &'static str {
        match self {
            NavSection::Home => "Home",
            NavSection::Search => "Search",
            NavSection::Explore => "Explore",
            NavSection::Shop => "Avatar Shop",
            NavSection::Create => "Create",
            NavSection::Chat => "Chat",
            NavSection::Profile => "Profile",
        }
    }

    /// Whether this section is implemented, as opposed to a placeholder.
    pub fn is_implemented(self) -> bool {
        matches!(self, NavSection::Home)
    }
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
        Self {
            kind: LogKind::System,
            text: text.into(),
            at: Instant::now(),
        }
    }
    pub fn event(text: impl Into<String>) -> Self {
        Self {
            kind: LogKind::Event,
            text: text.into(),
            at: Instant::now(),
        }
    }
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            kind: LogKind::Error,
            text: text.into(),
            at: Instant::now(),
        }
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
    // ── PC app shell ──────
    /// Which rail section the app shell is showing.
    pub nav: NavSection,
    /// Brand textures, uploaded lazily on the first frame.
    pub logos: Option<crate::ui::Logos>,

    // ── Website API ───────
    pub api_base: String,
    pub worlds: Vec<WorldInfo>,
    pub worlds_loading: bool,
    pub worlds_error: Option<String>,
    pub search: String,

    // ── Place + servers ───
    pub selected_place: Option<WorldInfo>,
    pub servers: Vec<ServerInfo>,
    pub servers_loading: bool,
    pub servers_error: Option<String>,
    /// Which server the TCP handshake is currently aimed at.
    pub joining_server: Option<ServerInfo>,

    // ── Player ────────────
    pub username: String,

    // ── Screen ────────────
    pub screen: Screen,

    // ── Server metadata ───
    pub server_name: String,
    pub max_players: u32,
    /// Endpoint handed out by the API for the server we joined.
    pub connected_endpoint: String,

    // ── Live data ─────────
    pub snapshot: Snapshot,
    pub log: Vec<LogEntry>,

    // ── Event console ─────
    pub event_name: String,
    pub event_args: String,

    // ── Ping ──────────────
    pub last_ping: Option<Instant>,
    pub ping_ms: Option<u128>,

    // ── Deep link ─────────
    /// `server_id` requested on the command line (`--join`), honoured once the
    /// server list arrives.
    pub pending_join: Option<String>,

    // ── Handles ───────────
    pub api: ApiHandle,
    pub net: NetHandle,
}

impl ClientApp {
    /// A client that is not attached to a live website. Used by the headless
    /// render tests and by the dev-only `ui-shot` tool: it never issues an HTTP
    /// request until something asks it to.
    pub fn new_headless(api_base: impl Into<String>, username: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            nav: NavSection::Home,
            logos: None,
            worlds: Vec::new(),
            worlds_loading: true,
            worlds_error: None,
            search: String::new(),
            selected_place: None,
            servers: Vec::new(),
            servers_loading: false,
            servers_error: None,
            joining_server: None,
            username: username.into(),
            screen: Screen::Home,
            server_name: String::new(),
            max_players: 0,
            connected_endpoint: String::new(),
            snapshot: Snapshot::default(),
            log: vec![LogEntry::system("headless")],
            event_name: String::new(),
            event_args: String::new(),
            last_ping: None,
            ping_ms: None,
            pending_join: None,
            api: ApiHandle::start("http://127.0.0.1:0"),
            net: NetHandle::start(),
        }
    }

    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        api_base: String,
        username: String,
        pending_join: Option<String>,
    ) -> Self {
        let net = NetHandle::start();
        let api = ApiHandle::start(api_base.clone());
        let username = if username.trim().is_empty() {
            let id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| (d.as_millis() % 9000 + 1000) as u64)
                .unwrap_or(1234);
            format!("Player{id}")
        } else {
            username
        };

        let mut app = Self {
            api_base,
            nav: NavSection::Home,
            logos: None,
            worlds: Vec::new(),
            worlds_loading: true,
            worlds_error: None,
            search: String::new(),
            selected_place: None,
            servers: Vec::new(),
            servers_loading: false,
            servers_error: None,
            joining_server: None,
            username,
            screen: Screen::Home,
            server_name: String::new(),
            max_players: 0,
            connected_endpoint: String::new(),
            snapshot: Snapshot::default(),
            log: vec![LogEntry::system("Welcome to Noobietoria.")],
            event_name: String::new(),
            event_args: String::new(),
            last_ping: None,
            ping_ms: None,
            pending_join,
            api,
            net,
        };
        app.refresh_worlds();
        app
    }

    // ── Website API ───────────────────────────────────────────────────────────

    /// Re-loads the published places from the website.
    pub fn refresh_worlds(&mut self) {
        self.worlds_loading = true;
        self.worlds_error = None;
        let _ = self.api.cmd_tx.send(ApiCmd::FetchWorlds);
    }

    /// The worlds matching the search box, most played first.
    pub fn visible_worlds(&self) -> Vec<&WorldInfo> {
        let needle = self.search.trim().to_lowercase();
        self.worlds
            .iter()
            .filter(|w| {
                needle.is_empty()
                    || w.name.to_lowercase().contains(&needle)
                    || w.creator_name.to_lowercase().contains(&needle)
            })
            .collect()
    }

    /// Opens a place: loads its joinable servers from the website.
    pub fn open_place(&mut self, place: WorldInfo) {
        self.servers.clear();
        self.servers_error = None;
        self.servers_loading = true;
        self.selected_place = Some(place.clone());
        self.screen = Screen::Place;
        self.log.push(LogEntry::system(format!(
            "Loading servers for «{}»…",
            place.name
        )));
        let _ = self.api.cmd_tx.send(ApiCmd::FetchServers {
            place_id: place.place_id,
        });
    }

    /// Re-loads the server list of the open place.
    pub fn refresh_servers(&mut self) {
        if let Some(place) = self.selected_place.clone() {
            self.servers_loading = true;
            self.servers_error = None;
            let _ = self.api.cmd_tx.send(ApiCmd::FetchServers {
                place_id: place.place_id,
            });
        }
    }

    /// Goes back to the discover page.
    pub fn go_home(&mut self) {
        self.screen = Screen::Home;
        self.nav = NavSection::Home;
        self.selected_place = None;
        self.servers.clear();
        self.servers_error = None;
        if self.worlds.is_empty() && self.worlds_error.is_none() {
            self.refresh_worlds();
        }
    }

    /// Dials one server from the list. The endpoint comes from the website.
    pub fn join_server(&mut self, server: ServerInfo, username: String) {
        self.joining_server = Some(server.clone());
        self.connected_endpoint = server.endpoint();
        self.screen = Screen::Connecting;
        self.log.push(LogEntry::system(format!(
            "Joining {} ({})…",
            server.server_id,
            server.endpoint()
        )));
        let _ = self.net.cmd_tx.send(NetCmd::Connect {
            host: server.host,
            port: server.port,
            username,
        });
    }

    // ── Draining the two channels ─────────────────────────────────────────────

    /// Drain the website-API channel and update state.
    pub fn poll_api(&mut self) {
        while let Ok(msg) = self.api.msg_rx.try_recv() {
            match msg {
                ApiMsg::Worlds(list) => {
                    self.worlds_loading = false;
                    self.worlds_error = None;
                    self.worlds = list;
                    self.log.push(LogEntry::system(format!(
                        "{} places loaded.",
                        self.worlds.len()
                    )));

                    // Honour `--join <server_id>` once the catalog is in.
                    if let Some(pending) = self.pending_join.clone() {
                        let place_id = pending.split(':').next().unwrap_or("").to_owned();
                        if let Some(place) =
                            self.worlds.iter().find(|w| w.place_id == place_id).cloned()
                        {
                            self.open_place(place);
                        } else {
                            self.pending_join = None;
                            self.log.push(LogEntry::error(format!(
                                "No place matches requested server «{pending}»."
                            )));
                        }
                    }
                }
                ApiMsg::Servers { place_id, servers } => {
                    self.servers_loading = false;
                    self.servers_error = None;
                    // Keep only the servers that really belong to the open place.
                    let servers: Vec<ServerInfo> = servers
                        .into_iter()
                        .filter(|s| s.place_id.is_empty() || s.place_id == place_id)
                        .collect();
                    let still_open = self
                        .selected_place
                        .as_ref()
                        .is_some_and(|p| p.place_id == place_id);
                    if still_open {
                        self.servers = servers.clone();
                    }

                    // Join the requested server, when it is the one we waited for.
                    if let Some(pending) = self.pending_join.clone() {
                        if let Some(server) = servers.iter().find(|s| s.server_id == pending) {
                            self.pending_join = None;
                            let username = self.username.clone();
                            self.join_server(server.clone(), username);
                        } else if still_open {
                            self.pending_join = None;
                            self.log.push(LogEntry::error(format!(
                                "Requested server «{pending}» is not running."
                            )));
                        }
                    }
                }
                ApiMsg::Error(e) => {
                    self.log.push(LogEntry::error(e.clone()));
                    if self.servers_loading {
                        self.servers_loading = false;
                        self.servers_error = Some(e);
                    } else {
                        self.worlds_loading = false;
                        self.worlds_error = Some(e);
                    }
                }
            }
        }
    }

    /// Drain the game-socket channel and update state.
    pub fn poll_net(&mut self) {
        while let Ok(msg) = self.net.msg_rx.try_recv() {
            match msg {
                NetMsg::Connected {
                    server_name,
                    max_players,
                } => {
                    self.server_name = server_name.clone();
                    self.max_players = max_players;
                    self.screen = Screen::Lobby;
                    self.log
                        .push(LogEntry::system(format!("Connected to «{server_name}».")));
                }
                NetMsg::Disconnected { reason } => {
                    self.screen = if self.selected_place.is_some() {
                        Screen::Place
                    } else {
                        Screen::Home
                    };
                    self.joining_server = None;
                    self.log
                        .push(LogEntry::error(format!("Disconnected: {reason}")));
                }
                NetMsg::Error(e) => {
                    if self.screen == Screen::Connecting {
                        self.screen = if self.selected_place.is_some() {
                            Screen::Place
                        } else {
                            Screen::Home
                        };
                        self.joining_server = None;
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

    // ── In-game actions ───────────────────────────────────────────────────────

    pub fn do_disconnect(&mut self) {
        let _ = self.net.cmd_tx.send(NetCmd::Disconnect);
        self.joining_server = None;
        self.screen = if self.selected_place.is_some() {
            Screen::Place
        } else {
            Screen::Home
        };
        self.log.push(LogEntry::system("Left the server."));
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
        let args: Vec<serde_json::Value> =
            serde_json::from_str(&format!("[{}]", self.event_args)).unwrap_or_default();
        self.log.push(LogEntry::event(format!("→ {event}(…)")));
        let _ = self.net.cmd_tx.send(NetCmd::Fire { event, args });
    }
}

impl eframe::App for ClientApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        self.poll_api();
        self.poll_net();
        crate::ui::draw(self, ctx);
        // Keep rendering so we keep polling both channels.
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
}
