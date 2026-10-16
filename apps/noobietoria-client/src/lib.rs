/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Noobietoria Client library.
//!
//! Holds everything except process startup (CLI parsing and the eframe
//! bootstrap, which live in the `noobietoria-client` binary):
//!
//! * [`api`] – website API access (place catalog + server directory).
//! * [`net`] – the game socket.
//! * [`state`] – the `ClientApp` state machine.
//! * [`ui`] – the egui PC app shell and in-game HUD.
//!
//! Exposing the UI as a library also lets the dev-only `ui-shot` binary render
//! the real screens to PNG for visual checking.

pub mod api;
pub mod net;
pub mod state;
pub mod ui;

#[cfg(test)]
mod tests {
    use crate::api::{ServerInfo, WorldInfo};
    use crate::net::*;
    use crate::state::*;
    use crate::ui;
    use eframe::egui;

    // ── Wire protocol ─────────────────────────────────────────────────────────

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

    /// The client's whole browsing state machine starts on Discover, with no
    /// host/port to type, and its screens follow the documented transitions.
    #[test]
    fn test_screens_start_on_home_and_have_no_endpoint_input() {
        assert_eq!(Screen::default(), Screen::Home);

        // Joining only ever moves forward through these states.
        assert_ne!(Screen::Home, Screen::Place);
        assert_ne!(Screen::Place, Screen::Connecting);
        assert_ne!(Screen::Connecting, Screen::Lobby);
    }

    // ── Rail sections ─────────────────────────────────────────────────────────

    /// The embedded brand artwork must decode, and keep the proportions the
    /// header layout assumes.
    #[test]
    fn embedded_brand_artwork_decodes() {
        for (name, bytes) in ui::BRAND_TEXTURES {
            let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                .unwrap_or_else(|e| panic!("{name} must decode: {e}"))
                .to_rgba8();
            assert!(image.width() > 0 && image.height() > 0, "{name} is empty");
            // Every brand asset is landscape-or-square and never tiny.
            assert!(image.height() >= 64, "{name} is too small");
            assert!(
                (image.width() as f32) / (image.height() as f32) >= 0.8,
                "{name} has an unexpected aspect ratio"
            );
        }
    }

    /// The rail mirrors the mockup: Home first, and only Home is built.
    #[test]
    fn rail_sections_match_the_pc_app_shell() {
        assert_eq!(NavSection::default(), NavSection::Home);
        let all = [
            NavSection::Home,
            NavSection::Search,
            NavSection::Explore,
            NavSection::Shop,
            NavSection::Create,
            NavSection::Chat,
            NavSection::Profile,
        ];
        assert_eq!(all.len(), 7, "the mockup rail has seven icons");
        for section in all {
            assert!(!section.label().is_empty());
        }
        assert!(NavSection::Home.is_implemented());
        for section in &all[1..] {
            assert!(
                !section.is_implemented(),
                "{} is not built yet",
                section.label()
            );
        }
    }

    // ── Render tests ──────────────────────────────────────────────────────────

    fn world(place_id: &str, name: &str) -> WorldInfo {
        serde_json::from_value(serde_json::json!({
            "place_id": place_id,
            "name": name,
            "description": "A test place",
            "creator_name": "Tester",
            "visits": 4000,
            "upvotes": 90,
            "downvotes": 10,
            "max_players": 50,
        }))
        .expect("build world")
    }

    fn server(id: &str) -> ServerInfo {
        serde_json::from_value(serde_json::json!({
            "server_id": id,
            "place_id": "p1",
            "place_name": "Test Place",
            "host": "127.0.0.1",
            "port": 53640,
            "players": 12,
            "max_players": 50,
            "region": "Singapore",
            "ping_ms": 42,
            "status": "online",
        }))
        .expect("build server")
    }

    /// Paints one real egui frame of the client UI. Panicking here means the
    /// screen is broken, not merely unverified.
    fn paint(app: &mut ClientApp) {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ui::draw(app, ctx);
        });
    }

    /// Every screen of the PC app shell must actually render: the tile grid,
    /// each rail section including the placeholders, the server list and the
    /// in-game HUD.
    #[test]
    fn paints_every_screen_without_panicking() {
        let mut app = ClientApp::new_headless("http://127.0.0.1:3000", "Tester");

        // Home while loading.
        assert_eq!(app.screen, Screen::Home);
        assert_eq!(app.nav, NavSection::Home);
        paint(&mut app);

        // Home with a loaded catalog.
        app.worlds_loading = false;
        app.worlds = vec![world("p1", "Mega Easy Obby"), world("p2", "Rainy Day Cafe")];
        paint(&mut app);

        // Every rail section renders.
        for section in [
            NavSection::Home,
            NavSection::Search,
            NavSection::Explore,
            NavSection::Shop,
            NavSection::Create,
            NavSection::Chat,
            NavSection::Profile,
        ] {
            app.nav = section;
            paint(&mut app);
        }
        app.nav = NavSection::Home;

        // The Discover grid filters by the search box.
        app.search = "obby".to_owned();
        assert_eq!(app.visible_worlds().len(), 1);
        assert_eq!(app.visible_worlds()[0].place_id, "p1");
        app.search = "tester".to_owned();
        assert_eq!(
            app.visible_worlds().len(),
            2,
            "creator name is searchable too"
        );
        app.search = "nothing-matches".to_owned();
        assert!(app.visible_worlds().is_empty());
        paint(&mut app);
        app.search.clear();

        // Home with an API error.
        app.worlds_error = Some("API unreachable".to_owned());
        paint(&mut app);
        app.worlds_error = None;

        // Place screen: loading, empty, error, then populated.
        app.open_place(world("p1", "Mega Easy Obby"));
        assert_eq!(app.screen, Screen::Place);
        paint(&mut app);

        app.servers_loading = false;
        paint(&mut app);

        app.servers_error = Some("No route to API".to_owned());
        paint(&mut app);
        app.servers_error = None;

        app.servers = vec![server("p1:1"), server("p1:2")];
        paint(&mut app);

        // A full server renders too, with its Join button disabled.
        app.servers[1].status = "full".to_owned();
        assert!(!app.servers[1].is_joinable());
        paint(&mut app);

        // Connecting screen, driven by a server picked from the list.
        app.join_server(server("p1:1"), "Tester".to_owned());
        assert_eq!(app.screen, Screen::Connecting);
        assert_eq!(app.connected_endpoint, "127.0.0.1:53640");
        paint(&mut app);

        // Lobby with a live snapshot.
        app.screen = Screen::Lobby;
        app.server_name = "Test Server".to_owned();
        app.max_players = 50;
        app.snapshot = Snapshot {
            tick: 42,
            instances: vec![serde_json::json!({
                "name": "Baseplate",
                "kind": "Part",
                "container": "Workspace",
            })],
        };
        paint(&mut app);
    }

    /// Joining always takes the endpoint out of the server list, never from input.
    #[test]
    fn join_uses_the_server_endpoint_from_the_api() {
        let mut app = ClientApp::new_headless("http://127.0.0.1:3000", "Tester");
        let mut picked = server("p1:3");
        picked.host = "game.noobietoria.example".to_owned();
        picked.port = 53699;

        app.join_server(picked, "Tester".to_owned());
        assert_eq!(app.screen, Screen::Connecting);
        assert_eq!(
            app.connected_endpoint, "game.noobietoria.example:53699",
            "the dialled endpoint must come from the API payload"
        );
    }

    /// Leaving a server returns to the place it was joined from; going home
    /// clears the place and returns the rail to Home.
    #[test]
    fn leaving_returns_to_the_place_screen() {
        let mut app = ClientApp::new_headless("http://127.0.0.1:3000", "Tester");
        app.nav = NavSection::Shop;
        app.open_place(world("p1", "Mega Easy Obby"));
        app.join_server(server("p1:1"), "Tester".to_owned());
        assert_eq!(app.screen, Screen::Connecting);

        app.do_disconnect();
        assert_eq!(app.screen, Screen::Place);
        assert!(app.joining_server.is_none());

        app.go_home();
        assert_eq!(app.screen, Screen::Home);
        assert_eq!(
            app.nav,
            NavSection::Home,
            "back returns to the Home section"
        );
        assert!(app.selected_place.is_none());
        assert!(app.servers.is_empty());
    }

    /// Leaving when no place was ever opened lands on Discover, not on an empty
    /// place screen.
    #[test]
    fn leaving_without_a_place_lands_on_home() {
        let mut app = ClientApp::new_headless("http://127.0.0.1:3000", "Tester");
        app.screen = Screen::Lobby;
        assert!(app.selected_place.is_none());

        app.do_disconnect();
        assert_eq!(app.screen, Screen::Home);
        assert!(app.joining_server.is_none());
    }
}
