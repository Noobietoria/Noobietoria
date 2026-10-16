/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Noobietoria Client — the PC app.
//!
//! A Roblox-style game client built with eframe/egui:
//!
//! * Opens on the Home shell: an icon rail, the "Noobietoria." lockup and a
//!   grid of place tiles.
//! * Opening a place lists the servers currently running it, with region,
//!   occupancy and ping, and one Join button per server.
//! * Joining dials the endpoint that the website handed out — the player never
//!   types an IP address or a port.
//! * Receives `snapshot` packets and renders a live list of world instances,
//!   and lets the player fire custom network events and read the server log.
//!
//! The UI, state machine and I/O live in the `noobietoria_client` library so the
//! dev-only `ui-shot` binary can render the same screens to PNG.
//!
//! # CLI flags
//!
//! ```text
//! noobietoria-client [OPTIONS]
//! Options:
//!   --api <URL>          Website API base  [default: http://127.0.0.1:3000]
//!   --username <NAME>    Auto-fill username
//!   --join <SERVER_ID>   Open a place and join <place_id>:<n> on startup
//!   --selftest           Ping/pong smoke test, then exit 0
//!   --server <HOST:PORT> Endpoint used by --selftest  [default: 127.0.0.1:7777]
//!   -h, --help           Print this help
//! ```

use eframe::egui;
use noobietoria_client::state::ClientApp;

const DEFAULT_API_BASE: &str = "http://127.0.0.1:3000";
const DEFAULT_SELFTEST_SERVER: &str = "127.0.0.1:7777";

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
        println!("  --api <URL>          Website API base  [default: {DEFAULT_API_BASE}]");
        println!("  --username <NAME>    Auto-fill username");
        println!("  --join <SERVER_ID>   Open a place and join <place_id>:<n> on startup");
        println!("  --selftest           Ping/pong smoke test, then exit");
        println!(
            "  --server <HOST:PORT> Endpoint used by --selftest  [default: {DEFAULT_SELFTEST_SERVER}]"
        );
        println!("  -h, --help           Print this help");
        return Ok(());
    }

    // ── Parse flags ───────────────────────────────────────────────────────────
    let mut api_base = String::from(DEFAULT_API_BASE);
    let mut username = String::new();
    let mut join_target: Option<String> = None;
    let mut selftest_server = String::from(DEFAULT_SELFTEST_SERVER);
    let mut selftest = false;

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--api" if i + 1 < args.len() => {
                api_base = args[i + 1].clone();
                i += 1;
            }
            "--username" if i + 1 < args.len() => {
                username = args[i + 1].clone();
                i += 1;
            }
            "--join" if i + 1 < args.len() => {
                join_target = Some(args[i + 1].clone());
                i += 1;
            }
            "--server" if i + 1 < args.len() => {
                selftest_server = args[i + 1].clone();
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

        tracing::info!("[selftest] Connecting to {selftest_server}…");
        let addrs: Vec<_> = selftest_server
            .to_socket_addrs()
            .expect("resolve selftest server host:port")
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
            .with_title("Noobietoria")
            .with_inner_size([1176.0, 675.0])
            .with_min_inner_size([720.0, 460.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Noobietoria",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(ClientApp::new(
                cc,
                api_base,
                username,
                join_target,
            )))
        }),
    )
}
