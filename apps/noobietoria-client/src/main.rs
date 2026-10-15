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
        use std::net::TcpStream;
        use std::time::Duration;

        tracing::info!("[selftest] Connecting to {host}:{port}…");
        let stream = TcpStream::connect_timeout(
            &format!("{host}:{port}").parse().expect("valid addr"),
            Duration::from_secs(5),
        )
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
    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = egui::ViewportBuilder::default()
        .with_title("Noobietoria Client")
        .with_inner_size([1100.0, 700.0])
        .with_min_inner_size([640.0, 400.0]);

    eframe::run_native(
        "Noobietoria Client",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(ClientApp::new(cc, host, port, username)))
        }),
    )
}
