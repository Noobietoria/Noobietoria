/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Background TCP networking for the client.
//!
//! A single Tokio runtime is started on a dedicated OS thread.  The UI thread
//! communicates with it through two `std::sync::mpsc` channels:
//!
//! - `cmd_tx`: UI → net, to send [`NetCmd`]s (connect, fire, disconnect).
//! - `msg_rx`: net → UI, to receive [`ServerMsg`]s (snapshot, event, pong…).

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tracing::{info, warn};

// ── Wire types (mirrored from dedicated server) ────────────────────────────────

/// Commands the UI sends to the networking task.
#[derive(Debug)]
pub enum NetCmd {
    Connect {
        host: String,
        port: u16,
        username: String,
    },
    Fire {
        event: String,
        args: Vec<serde_json::Value>,
    },
    Ping,
    Disconnect,
}

/// Messages received from the server (subset we care about in the client).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServerMsg {
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
    #[serde(other)]
    Unknown,
}

/// Messages posted from the net thread to the UI.
#[derive(Debug)]
pub enum NetMsg {
    Connected {
        server_name: String,
        max_players: u32,
    },
    Disconnected {
        reason: String,
    },
    Error(String),
    Server(ServerMsg),
}

// ── Wire message the client sends ─────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ClientMsg {
    Join {
        username: String,
        user_id: u64,
        country: String,
    },
    Fire {
        event: String,
        args: Vec<serde_json::Value>,
    },
    Ping,
}

// ── Net handle ─────────────────────────────────────────────────────────────────

/// A handle to the background network thread.
pub struct NetHandle {
    pub cmd_tx: Sender<NetCmd>,
    pub msg_rx: Receiver<NetMsg>,
}

impl NetHandle {
    /// Starts the background Tokio runtime and returns a handle.
    pub fn start() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<NetCmd>();
        let (msg_tx, msg_rx) = mpsc::channel::<NetMsg>();

        thread::Builder::new()
            .name("nb-net".to_owned())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime");
                rt.block_on(net_actor(cmd_rx, msg_tx));
            })
            .expect("spawn net thread");

        Self { cmd_tx, msg_rx }
    }
}

// ── Background actor ──────────────────────────────────────────────────────────

async fn net_actor(cmd_rx: mpsc::Receiver<NetCmd>, msg_tx: Sender<NetMsg>) {
    // The actor runs an outer loop: it waits for a Connect command, then
    // manages one TCP session at a time.
    loop {
        // Wait for a Connect.
        let (host, port, username) = loop {
            match cmd_rx.try_recv() {
                Ok(NetCmd::Connect { host, port, username }) => break (host, port, username),
                Ok(_) => continue, // stale commands before connect
                Err(mpsc::TryRecvError::Disconnected) => return,
                Err(mpsc::TryRecvError::Empty) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            }
        };

        let addr = format!("{host}:{port}");
        info!("Connecting to {addr}…");

        let stream = match TcpStream::connect(&addr).await {
            Ok(s) => s,
            Err(e) => {
                let _ = msg_tx.send(NetMsg::Error(format!("Connect failed: {e}")));
                continue;
            }
        };

        let (read_half, mut write_half) = stream.into_split();
        let reader = BufReader::new(read_half);
        let mut lines = reader.lines();

        // A one-shot to signal the writer to stop.
        let (stop_tx, mut stop_rx) = oneshot::channel::<()>();

        // Outbound channel for the writer task.
        let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<String>(64);

        // Writer task.
        let writer_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(line) = out_rx.recv() => {
                        if write_half.write_all(line.as_bytes()).await.is_err() {
                            break;
                        }
                        if write_half.write_all(b"\n").await.is_err() {
                            break;
                        }
                    }
                    _ = &mut stop_rx => break,
                }
            }
        });

        // Send join immediately.
        let join_msg = serde_json::to_string(&ClientMsg::Join {
            username: username.clone(),
            user_id: 0,
            country: "UN".to_owned(),
        })
        .unwrap_or_default();
        let _ = out_tx.send(join_msg).await;

        // Reader / command pump loop.
        let mut disconnected = false;
        loop {
            // Non-blocking command poll.
            loop {
                match cmd_rx.try_recv() {
                    Ok(NetCmd::Fire { event, args }) => {
                        if let Ok(s) = serde_json::to_string(&ClientMsg::Fire { event, args }) {
                            let _ = out_tx.try_send(s);
                        }
                    }
                    Ok(NetCmd::Ping) => {
                        if let Ok(s) = serde_json::to_string(&ClientMsg::Ping) {
                            let _ = out_tx.try_send(s);
                        }
                    }
                    Ok(NetCmd::Disconnect) | Ok(NetCmd::Connect { .. }) => {
                        disconnected = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
            if disconnected {
                break;
            }

            // Receive one line from the server (with 50 ms timeout so we can
            // poll commands again).
            let result = tokio::time::timeout(
                Duration::from_millis(50),
                lines.next_line(),
            )
            .await;

            match result {
                Ok(Ok(Some(line))) => {
                    match serde_json::from_str::<ServerMsg>(&line) {
                        Ok(ServerMsg::Welcome { server_name, max_players }) => {
                            let _ = msg_tx.send(NetMsg::Connected {
                                server_name,
                                max_players,
                            });
                        }
                        Ok(ServerMsg::Kicked { reason }) => {
                            let _ = msg_tx.send(NetMsg::Disconnected { reason: reason.clone() });
                            disconnected = true;
                        }
                        Ok(msg) => {
                            let _ = msg_tx.send(NetMsg::Server(msg));
                        }
                        Err(e) => warn!("Parse error: {e} — {line}"),
                    }
                }
                Ok(Ok(None)) => {
                    // Server closed connection.
                    let _ = msg_tx.send(NetMsg::Disconnected {
                        reason: "Server closed the connection.".to_owned(),
                    });
                    break;
                }
                Ok(Err(e)) => {
                    let _ = msg_tx.send(NetMsg::Error(format!("Read error: {e}")));
                    break;
                }
                Err(_timeout) => {} // normal, loop back
            }

            if disconnected {
                break;
            }
        }

        // Signal writer to stop.
        let _ = stop_tx.send(());
        writer_task.abort();

        if disconnected {
            let _ = msg_tx.send(NetMsg::Disconnected {
                reason: "Disconnected by user.".to_owned(),
            });
        }
    }
}
