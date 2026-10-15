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
        info!("No world file – starting with an empty world.");
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

                // Broadcast snapshot to all clients.
                if let Some(snap) = snapshot_payload {
                    let txs: Vec<_> = state
                        .read()
                        .connections
                        .values()
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
                            .map(|v| serde_json::Value::String(format!("{v:?}")))
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
                    break;
                }

                let record = PlayerRecord::new(&username, user_id, &country);
                {
                    let srv = state.read();
                    srv.ctx.runtime.player_join(record);
                    srv.ctx.runtime.record_visit();
                }
                joined_username = Some(username.clone());

                // Store username in connection.
                if let Some(conn) = state.read().connections.get(&addr).cloned() {
                    conn.write().username = Some(username.clone());
                }

                info!("{addr}: `{username}` joined (ccu={})", state.read().ccu());

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

                let pending = PendingEvent {
                    event: event.clone(),
                    to: "server".to_owned(),
                    args: args
                        .into_iter()
                        .map(|v| nb_types::Value::from(v.to_string()))
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
