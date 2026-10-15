/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Noobietoria Client
//!
//! A Roblox-style game client built with eframe/egui.
//!
//! # What it does
//!
//! * Opens a splash / connect screen where the player enters a username,
//!   server host, and port.
//! * Connects to the dedicated server over TCP (newline-delimited JSON).
//! * Receives `snapshot` packets and renders a live list of world instances.
//! * Lets the player fire custom network events and view the server chat log.
//! * Background I/O lives in a Tokio runtime spawned on a separate thread so
//!   the UI thread is never blocked.
//!
//! # CLI flags
//!
//! ```text
//! noobietoria-client [OPTIONS]
//! Options:
//!   --host <HOST>        Server address  [default: 127.0.0.1]
//!   --port <PORT>        Server port     [default: 7777]
//!   --username <NAME>    Auto-fill username
//!   --selftest           Connect → ping → disconnect, then exit 0
//!   -h, --help           Print this help
//! ```

mod net;
mod state;
mod ui;

use eframe::egui;
use state::ClientApp;

fn main() -> eframe::Result<()> {
    // Install a subscriber so log! / tracing! macros work.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();

    // ── --help ────────────────────────────────────────────────────────────────
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Noobietoria Client");
        println!("Usage: noobietoria-client [OPTIONS]");
        println!();
        println!("Options:");
        println!("  --host <HOST>        Server address  [default: 127.0.0.1]");
        println!("  --port <PORT>        Server port     [default: 7777]");
        println!("  --username <NAME>    Auto-fill username");
        println!("  --selftest           Headless ping test then exit");
        println!("  -h, --help           Print this help");
        return Ok(());
    }

    // ── Parse flags ───────────────────────────────────────────────────────────
    let mut host = String::from("127.0.0.1");
    let mut port: u16 = 7777;
    let mut username = String::new();
    let mut selftest = false;

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--host" if i + 1 < args.len() => {
                host = args[i + 1].clone();
                i += 1;
            }
            "--port" if i + 1 < args.len() => {
                if let Ok(p) = args[i + 1].parse() {
                    port = p;
                }
                i += 1;
            }
            "--username" if i + 1 < args.len() => {
                username = args[i + 1].clone();
                i += 1;
            }
            "--selftest" => selftest = true,
            _ => {}
        }
        i += 1;
    }

    // ── Self-test: headless ping ───────────────────────────────────────────────
    if selftest {
        use std::io::{BufRead, Write};
        use std::net::{TcpStream, ToSocketAddrs};
        use std::time::Duration;

        tracing::info!("[selftest] Connecting to {host}:{port}…");
        let addrs: Vec<_> = format!("{host}:{port}")
            .to_socket_addrs()
            .expect("resolve server host:port")
            .collect();
        let addr = addrs.first().expect("no address resolved for host:port");
        let stream = TcpStream::connect_timeout(addr, Duration::from_secs(5))
            .expect("selftest: could not connect to server");
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();

        let mut reader = std::io::BufReader::new(stream.try_clone().expect("clone"));
        let mut writer = stream;

        // Read welcome.
        let mut line = String::new();
        reader.read_line(&mut line).expect("selftest: no welcome");
        tracing::info!("[selftest] ← {}", line.trim());

        // Send ping.
        writeln!(writer, r#"{{"kind":"ping"}}"#).expect("selftest: write ping");

        // Read pong.
        let mut pong = String::new();
        reader.read_line(&mut pong).expect("selftest: no pong");
        tracing::info!("[selftest] ← {}", pong.trim());
        assert!(pong.contains("pong"), "Expected pong, got: {pong}");

        tracing::info!("[selftest] ✓ ping/pong OK");
        return Ok(());
    }

    // ── Launch GUI ────────────────────────────────────────────────────────────
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Noobietoria Client")
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Noobietoria Client",
        native_options,
        Box::new(move |cc| Ok(Box::new(ClientApp::new(cc, host, port, username)))),
    )
}

#[cfg(test)]
mod tests {
    use super::net::*;
    use super::state::*;

    #[test]
    fn test_client_receives_server_messages() {
        let welcome_raw =
            r#"{"kind":"welcome","server_name":"Noobietoria Alpha","max_players":32}"#;
        let welcome: ServerMsg = serde_json::from_str(welcome_raw).expect("deserialize welcome");
        match welcome {
            ServerMsg::Welcome {
                server_name,
                max_players,
            } => {
                assert_eq!(server_name, "Noobietoria Alpha");
                assert_eq!(max_players, 32);
            }
            _ => panic!("expected welcome message"),
        }

        let snap_raw =
            r#"{"kind":"snapshot","instances":[{"name":"Baseplate","kind":"Part"}],"tick":12}"#;
        let snap: ServerMsg = serde_json::from_str(snap_raw).expect("deserialize snapshot");
        match snap {
            ServerMsg::Snapshot { instances, tick } => {
                assert_eq!(tick, 12);
                assert_eq!(instances.len(), 1);
            }
            _ => panic!("expected snapshot message"),
        }

        let kick_raw = r#"{"kind":"kicked","reason":"Server is full (20/20)"}"#;
        let kick: ServerMsg = serde_json::from_str(kick_raw).expect("deserialize kicked");
        match kick {
            ServerMsg::Kicked { reason } => {
                assert_eq!(reason, "Server is full (20/20)");
            }
            _ => panic!("expected kicked message"),
        }
    }

    #[test]
    fn test_client_log_entries() {
        let entry = LogEntry::system("Hello");
        assert_eq!(entry.kind, LogKind::System);
        assert_eq!(entry.text, "Hello");

        let err = LogEntry::error("Failed");
        assert_eq!(err.kind, LogKind::Error);

        let evt = LogEntry::event("Fired");
        assert_eq!(evt.kind, LogKind::Event);
    }
}
