/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Noobietoria Dedicated Server
//!
//! A headless authoritative game host.  On startup it:
//!
//! 1. Parses CLI flags (`--port`, `--world`, `--selftest`, `--help`).
//! 2. Loads a `.nbw` world file (or boots an empty world).
//! 3. Starts a Tokio TCP listener that accepts clients.
//! 4. Runs a 20 Hz game-tick loop that advances the runtime clock and
//!    replicates the world snapshot to every connected client.
//!
//! # Wire protocol (newline-delimited JSON over TCP)
//!
//! Client → Server
//! ```json
//! {"kind":"join","username":"Alice","user_id":1,"country":"VN"}
//! {"kind":"fire","event":"Jump","args":[]}
//! {"kind":"ping"}
//! ```
//!
//! Server → Client
//! ```json
//! {"kind":"welcome","server_name":"…","max_players":20}
//! {"kind":"snapshot","instances":[…],"tick":42}
//! {"kind":"event","event":"Died","args":[]}
//! {"kind":"pong"}
//! {"kind":"kicked","reason":"…"}
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use nb_api::{ApiContext, PendingEvent, PlayerRecord, Side};
use nb_world::WorldFile;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time;
use tracing::{error, info, warn};

// ── Wire types ─────────────────────────────────────────────────────────────────

/// Messages the client sends to the server.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ClientMsg {
    Join {
        username: String,
        #[serde(default)]
        user_id: u64,
        #[serde(default = "default_country")]
        country: String,
    },
    Fire {
        event: String,
        #[serde(default)]
        args: Vec<serde_json::Value>,
    },
    Ping,
}

fn default_country() -> String {
    "UN".to_owned()
}

/// Messages the server sends to one client.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ServerMsg {
    Welcome {
        server_name: String,
        max_players: u32,
    },
    Snapshot {
        instances: Vec<serde_json::Value>,
        tick: u64,
    },
    Event {
        event: String,
        args: Vec<serde_json::Value>,
    },
    Pong,
    Kicked {
        reason: String,
    },
}

// ── Per-connection bookkeeping ─────────────────────────────────────────────────

struct Conn {
    username: Option<String>,
    #[allow(dead_code)]
    addr: SocketAddr,
    tx: mpsc::Sender<ServerMsg>,
}

// ── Shared server state ────────────────────────────────────────────────────────

struct ServerState {
    ctx: ApiContext,
    connections: HashMap<SocketAddr, Arc<RwLock<Conn>>>,
    max_players: u32,
    server_name: String,
}

impl ServerState {
    fn new(ctx: ApiContext, max_players: u32, server_name: String) -> Self {
        Self {
            ctx,
            connections: HashMap::new(),
            max_players,
            server_name,
        }
    }

    fn ccu(&self) -> u32 {
        self.ctx.runtime.players.read().ccu()
    }

    /// A lightweight snapshot of the replicated world.
    fn snapshot(&self) -> Vec<serde_json::Value> {
        let world = self.ctx.runtime.world.read();
        world
            .replicated_snapshot()
            .into_iter()
            .filter_map(|ri| serde_json::to_value(ri).ok())
            .collect()
    }
}

// ── Entry point ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    // ── --help ────────────────────────────────────────────────────────────────
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Noobietoria Dedicated Server");
        println!("Usage: noobietoria-dedicated [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --port <PORT>        TCP port to listen on  [default: 7777]");
        println!("  --world <PATH>       .nbw world file to load");
        println!("  --max-players <N>    Maximum concurrent players  [default: 20]");
        println!("  --name <TEXT>        Server display name");
        println!("  --selftest           Run a smoke test then exit");
        println!("  -h, --help           Print this help");
        return Ok(());
    }

    // ── Parse flags ───────────────────────────────────────────────────────────
    let mut port: u16 = 7777;
    let mut world_path: Option<PathBuf> = None;
    let mut max_players: u32 = 20;
    let mut server_name = String::from("Noobietoria Server");
    let mut selftest = false;

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--port" if i + 1 < args.len() => {
                port = args[i + 1].parse().context("--port must be a number")?;
                i += 1;
            }
            "--world" if i + 1 < args.len() => {
                world_path = Some(PathBuf::from(&args[i + 1]));
                i += 1;
            }
            "--max-players" if i + 1 < args.len() => {
                max_players = args[i + 1]
                    .parse()
                    .context("--max-players must be a number")?;
                i += 1;
            }
            "--name" if i + 1 < args.len() => {
                server_name = args[i + 1].clone();
                i += 1;
            }
            "--selftest" => selftest = true,
            _ => {}
        }
        i += 1;
    }

    // ── Bootstrap ─────────────────────────────────────────────────────────────
    let ctx = ApiContext::new(Side::Server);

    if let Some(ref path) = world_path {
        let file = WorldFile::load_from_file(path)
            .with_context(|| format!("loading world file {}", path.display()))?;
        *ctx.runtime.world.write() = file.to_world();
        info!("Loaded world: {}", path.display());
    } else {
        info!("No world file specified – starting with default starter place.");
        *ctx.runtime.world.write() = WorldFile::default_starter_place().to_world();
    }

    ctx.runtime.record_visit();

    // ── Self-test ─────────────────────────────────────────────────────────────
    if selftest {
        info!("[selftest] Dedicated server smoke test…");

        let rec = PlayerRecord::new("TestBot", 9999, "VN");
        ctx.runtime.player_join(rec);
        assert_eq!(ctx.runtime.players.read().ccu(), 1);
        info!("[selftest] ✓ player_join / CCU");

        ctx.runtime.advance(1.0);
        assert!(ctx.runtime.ticks() > 0);
        info!("[selftest] ✓ clock advance");

        ctx.runtime.player_leave("TestBot");
        assert_eq!(ctx.runtime.players.read().ccu(), 0);
        info!("[selftest] ✓ player_leave");

        info!("[selftest] All checks passed.");
        return Ok(());
    }

    // ── Shared state ──────────────────────────────────────────────────────────
    let state = Arc::new(RwLock::new(ServerState::new(ctx, max_players, server_name)));

    // ── TCP listener ──────────────────────────────────────────────────────────
    let addr: SocketAddr = format!("0.0.0.0:{port}").parse()?;
    let listener = TcpListener::bind(addr).await?;
    info!("Listening on {addr}");

    // ── 20 Hz tick loop ───────────────────────────────────────────────────────
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(50));
            let mut snap_timer: u8 = 0;
            loop {
                interval.tick().await;

                let (outbound_events, snapshot_payload) = {
                    let srv = state.read();
                    srv.ctx.runtime.advance(0.05);
                    snap_timer += 1;

                    let outbound = srv.ctx.runtime.take_outbound();

                    // Full snapshot every 10 ticks ≈ 2 Hz.
                    let snap = if snap_timer >= 10 {
                        snap_timer = 0;
                        Some(ServerMsg::Snapshot {
                            instances: srv.snapshot(),
                            tick: srv.ctx.runtime.ticks(),
                        })
                    } else {
                        None
                    };

                    (outbound, snap)
                };

                // Broadcast snapshot to all joined clients.
                if let Some(snap) = snapshot_payload {
                    let txs: Vec<_> = state
                        .read()
                        .connections
                        .values()
                        .filter(|c| c.read().username.is_some())
                        .map(|c| c.read().tx.clone())
                        .collect();
                    for tx in txs {
                        let _ = tx.try_send(snap.clone());
                    }
                }

                // Dispatch outbound events to target clients.
                for ev in outbound_events {
                    let target = ev.to.clone();
                    let msg = ServerMsg::Event {
                        event: ev.event.clone(),
                        args: ev
                            .args
                            .iter()
                            .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
                            .collect(),
                    };

                    let txs: Vec<_> = {
                        let srv = state.read();
                        srv.connections
                            .values()
                            .filter(|c| {
                                let c = c.read();
                                target == "*" || c.username.as_deref() == Some(target.as_str())
                            })
                            .map(|c| c.read().tx.clone())
                            .collect()
                    };
                    for tx in txs {
                        let _ = tx.try_send(msg.clone());
                    }
                }
            }
        });
    }

    // ── Accept loop ───────────────────────────────────────────────────────────
    loop {
        let (stream, addr) = listener.accept().await?;
        info!("New connection: {addr}");

        let state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(e) = handle_conn(stream, addr, state).await {
                warn!("{addr}: {e:#}");
            }
        });
    }
}

// ── Website registration ───────────────────────────────────────────────────────

/// How often the server re-announces itself to the website. Kept comfortably
/// below the website's registration TTL so a single dropped heartbeat does not
/// make the server disappear from the client's server list.
const REGISTER_INTERVAL: Duration = Duration::from_secs(5);

/// The heartbeat payload, matching `nb_web::ServerRegistration` on the website.
#[derive(Debug, Serialize)]
struct Registration<'a> {
    server_id: &'a str,
    place_id: &'a str,
    host: &'a str,
    port: u16,
    players: u32,
    max_players: u32,
    region: &'a str,
}

/// Starts the heartbeat task that announces this server to the website.
///
/// `api_base` empty disables registration (a purely local server). The task
/// keeps running for the lifetime of the process; if the website is down, the
/// next heartbeat retries and the server keeps serving players regardless.
fn spawn_registration(
    api_base: String,
    server_id: String,
    place_id: String,
    host: String,
    port: u16,
    max_players: u32,
    region: String,
    state: Arc<RwLock<ServerState>>,
) {
    if api_base.trim().is_empty() {
        info!("Website registration disabled (--api \"\").");
        return;
    }

    tokio::spawn(async move {
        let url = format!(
            "{}/api/v1/servers/register",
            api_base.trim_end_matches('/')
        );
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                warn!("registration disabled, HTTP client failed: {e}");
                return;
            }
        };

        let mut ticker = time::interval(REGISTER_INTERVAL);
        let mut announced = false;
        loop {
            ticker.tick().await;

            // CCU as reported by the runtime, so the website shows real numbers.
            let players = state.read().ccu();
            let payload = Registration {
                server_id: &server_id,
                place_id: &place_id,
                host: &host,
                port,
                players,
                max_players,
                region: &region,
            };

            match client.post(&url).json(&payload).send().await {
                Ok(resp) if resp.status().is_success() => {
                    if !announced {
                        announced = true;
                        info!(
                            "Registered with {url} as {server_id} ({players}/{max_players} players)"
                        );
                    }
                }
                Ok(resp) => {
                    warned_once(&mut announced, &format!("{url} returned {}", resp.status()));
                }
                Err(e) => {
                    warned_once(&mut announced, &format!("{url}: {e}"));
                }
            }
        }
    });
}

/// Logs a registration problem once per outage, then stays quiet.
fn warned_once(announced: &mut bool, message: &str) {
    if *announced {
        warn!("Website registration lost: {message}");
        *announced = false;
    } else {
        info!("Website registration retrying: {message}");
    }
}

// ── Per-connection handler ─────────────────────────────────────────────────────

async fn handle_conn(
    stream: TcpStream,
    addr: SocketAddr,
    state: Arc<RwLock<ServerState>>,
) -> Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let reader = BufReader::new(read_half);
    let mut lines = reader.lines();

    let (tx, mut rx) = mpsc::channel::<ServerMsg>(64);

    // Register connection.
    {
        let mut srv = state.write();
        srv.connections.insert(
            addr,
            Arc::new(RwLock::new(Conn {
                username: None,
                addr,
                tx: tx.clone(),
            })),
        );
    }

    // Send welcome – extract needed values before any await to avoid holding
    // the non-Send guard across an await point.
    let welcome_line = {
        let srv = state.read();
        let msg = ServerMsg::Welcome {
            server_name: srv.server_name.clone(),
            max_players: srv.max_players,
        };
        serde_json::to_string(&msg)? + "\n"
    };
    write_half.write_all(welcome_line.as_bytes()).await?;

    // Writer task.
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match serde_json::to_string(&msg) {
                Ok(mut line) => {
                    line.push('\n');
                    if write_half.write_all(line.as_bytes()).await.is_err() {
                        break;
                    }
                }
                Err(e) => error!("serialize error: {e}"),
            }
        }
    });

    let mut joined_username: Option<String> = None;

    while let Ok(Some(raw)) = lines.next_line().await {
        let raw = raw.trim().to_owned();
        if raw.is_empty() {
            continue;
        }
        let msg: ClientMsg = match serde_json::from_str(&raw) {
            Ok(m) => m,
            Err(e) => {
                warn!("{addr}: malformed message: {e}");
                continue;
            }
        };

        match msg {
            ClientMsg::Join {
                username,
                user_id,
                country,
            } => {
                let trimmed_username = username.trim().to_owned();
                if trimmed_username.is_empty() {
                    let _ = tx
                        .send(ServerMsg::Kicked {
                            reason: "Username cannot be empty.".to_owned(),
                        })
                        .await;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    break;
                }

                // Check ban status.
                let is_banned = {
                    let srv = state.read();
                    let b = srv
                        .ctx
                        .runtime
                        .moderation
                        .read()
                        .get(&trimmed_username)
                        .map(|r| r.kind == "Ban")
                        .unwrap_or(false);
                    b
                };
                if is_banned {
                    let _ = tx
                        .send(ServerMsg::Kicked {
                            reason: format!(
                                "Player `{trimmed_username}` is banned from this server."
                            ),
                        })
                        .await;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    break;
                }

                // Check duplicate connection.
                let already_connected = {
                    let srv = state.read();
                    srv.connections
                        .values()
                        .any(|c| c.read().username.as_deref() == Some(trimmed_username.as_str()))
                };
                if already_connected {
                    let _ = tx
                        .send(ServerMsg::Kicked {
                            reason: format!(
                                "Player `{trimmed_username}` is already connected to this server."
                            ),
                        })
                        .await;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    break;
                }

                // Capacity check.
                let (ccu, max) = {
                    let srv = state.read();
                    (srv.ccu(), srv.max_players)
                };
                if ccu >= max {
                    let _ = tx
                        .send(ServerMsg::Kicked {
                            reason: format!("Server is full ({ccu}/{max})"),
                        })
                        .await;
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    break;
                }

                let record = PlayerRecord::new(&trimmed_username, user_id, &country);
                {
                    let srv = state.read();
                    srv.ctx.runtime.player_join(record);
                    srv.ctx.runtime.record_visit();
                }
                joined_username = Some(trimmed_username.clone());

                // Store username in connection.
                if let Some(conn) = state.read().connections.get(&addr).cloned() {
                    conn.write().username = Some(trimmed_username.clone());
                }

                info!(
                    "{addr}: `{trimmed_username}` joined (ccu={})",
                    state.read().ccu()
                );

                // Send initial snapshot.
                let snap = {
                    let srv = state.read();
                    ServerMsg::Snapshot {
                        instances: srv.snapshot(),
                        tick: srv.ctx.runtime.ticks(),
                    }
                };
                let _ = tx.send(snap).await;
            }

            ClientMsg::Fire { event, args } => {
                let Some(username) = &joined_username else {
                    warn!("{addr}: fire before join – ignored");
                    continue;
                };
                info!("{addr}: `{username}` fired `{event}`");

                // Relay multiplayer Move and Chat directly to peers
                if event == "Move" {
                    let mut b_args = vec![serde_json::Value::String(username.clone())];
                    b_args.extend(args.iter().cloned());
                    let b_msg = ServerMsg::Event {
                        event: "Move".to_owned(),
                        args: b_args,
                    };
                    let txs: Vec<_> = state
                        .read()
                        .connections
                        .iter()
                        .filter(|(a, c)| **a != addr && c.read().username.is_some())
                        .map(|(_, c)| c.read().tx.clone())
                        .collect();
                    for peer_tx in txs {
                        let _ = peer_tx.try_send(b_msg.clone());
                    }
                } else if event == "Chat" {
                    let mut b_args = vec![serde_json::Value::String(username.clone())];
                    b_args.extend(args.iter().cloned());
                    let b_msg = ServerMsg::Event {
                        event: "Chat".to_owned(),
                        args: b_args,
                    };
                    let txs: Vec<_> = state
                        .read()
                        .connections
                        .values()
                        .filter(|c| c.read().username.is_some())
                        .map(|c| c.read().tx.clone())
                        .collect();
                    for peer_tx in txs {
                        let _ = peer_tx.try_send(b_msg.clone());
                    }
                }

                let pending = PendingEvent {
                    event: event.clone(),
                    to: "server".to_owned(),
                    args: args
                        .into_iter()
                        .map(|v| {
                            serde_json::from_value::<nb_types::Value>(v)
                                .unwrap_or(nb_types::Value::Nil)
                        })
                        .collect(),
                    invoke: false,
                };
                state.read().ctx.runtime.push_inbound(pending);
            }

            ClientMsg::Ping => {
                let _ = tx.send(ServerMsg::Pong).await;
            }
        }
    }

    // Cleanup.
    info!("{addr}: disconnected");
    if let Some(ref u) = joined_username {
        state.read().ctx.runtime.player_leave(u);
        info!("`{u}` left (ccu={})", state.read().ccu());
    }
    state.write().connections.remove(&addr);
    write_task.abort();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_wire_messages_serialize_and_deserialize() {
        let json = r#"{"kind":"join","username":"Player1","user_id":100,"country":"VN"}"#;
        let msg: ClientMsg = serde_json::from_str(json).expect("deserialize join");
        match msg {
            ClientMsg::Join {
                username,
                user_id,
                country,
            } => {
                assert_eq!(username, "Player1");
                assert_eq!(user_id, 100);
                assert_eq!(country, "VN");
            }
            _ => panic!("expected join message"),
        }

        let ping_json = r#"{"kind":"ping"}"#;
        let ping_msg: ClientMsg = serde_json::from_str(ping_json).expect("deserialize ping");
        assert!(matches!(ping_msg, ClientMsg::Ping));

        let fire_json = r#"{"kind":"fire","event":"Jump","args":[10,"high"]}"#;
        let fire_msg: ClientMsg = serde_json::from_str(fire_json).expect("deserialize fire");
        match fire_msg {
            ClientMsg::Fire { event, args } => {
                assert_eq!(event, "Jump");
                assert_eq!(args.len(), 2);
                assert_eq!(args[0], serde_json::json!(10));
                assert_eq!(args[1], serde_json::json!("high"));
            }
            _ => panic!("expected fire message"),
        }
    }

    #[test]
    fn test_server_wire_messages_serialize_and_deserialize() {
        let welcome = ServerMsg::Welcome {
            server_name: "TestServer".to_owned(),
            max_players: 50,
        };
        let welcome_str = serde_json::to_string(&welcome).expect("serialize welcome");
        assert!(welcome_str.contains(r#""kind":"welcome""#));
        assert!(welcome_str.contains(r#""server_name":"TestServer""#));
        assert!(welcome_str.contains(r#""max_players":50"#));

        let kicked = ServerMsg::Kicked {
            reason: "Server full".to_owned(),
        };
        let kicked_str = serde_json::to_string(&kicked).expect("serialize kicked");
        assert!(kicked_str.contains(r#""kind":"kicked""#));
        assert!(kicked_str.contains(r#""reason":"Server full""#));

        let pong = ServerMsg::Pong;
        let pong_str = serde_json::to_string(&pong).expect("serialize pong");
        assert_eq!(pong_str, r#"{"kind":"pong"}"#);
    }

    #[test]
    fn test_server_state_snapshot_and_ccu() {
        let ctx = ApiContext::new(Side::Server);
        *ctx.runtime.world.write() = WorldFile::default_starter_place().to_world();
        let state = ServerState::new(ctx, 20, "TestServer".to_owned());

        assert_eq!(state.ccu(), 0);
        let snap = state.snapshot();
        assert!(
            !snap.is_empty(),
            "snapshot should contain default starter instances"
        );

        // Validate player join updates CCU
        let rec = PlayerRecord::new("Alice", 1, "VN");
        state.ctx.runtime.player_join(rec);
        assert_eq!(state.ccu(), 1);

        state.ctx.runtime.player_leave("Alice");
        assert_eq!(state.ccu(), 0);
    }
}
