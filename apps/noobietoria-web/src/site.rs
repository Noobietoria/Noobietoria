// Copyright (c) 2026 - present Noobietoria LLC (Công ty TNHH Trò chơi Noobietoria). All Rights Reserved.
// PROPRIETARY AND CONFIDENTIAL. UNAUTHORIZED COPYING IS STRICTLY PROHIBITED.

//! Noobietoria Web — site + API server.
//!
//! This is the composed entry point of the website. It reuses the very same
//! modules as the historical `src/main.rs` (`state`, `routes`, `graphql`) and
//! adds the **server directory API** the client needs:
//!
//! ```text
//! GET /api/v1/servers                       every joinable server
//! GET /api/v1/places/{place_id}/servers     joinable servers of one place
//! ```
//!
//! Both endpoints return the same shape:
//!
//! ```json
//! {
//!   "count": 1,
//!   "servers": [
//!     {
//!       "server_id": "starter_baseplate:1",
//!       "place_id": "starter_baseplate",
//!       "place_name": "Classic Baseplate",
//!       "host": "127.0.0.1",
//!       "port": 53640,
//!       "players": 45,
//!       "max_players": 50,
//!       "region": "Singapore",
//!       "ping_ms": 30,
//!       "status": "online"
//!     }
//!   ]
//! }
//! ```
//!
//! The website is therefore the single source of truth for join endpoints: the
//! desktop client lists servers here instead of asking the player for an IP
//! address and a port. The advertised hostname is configurable with the
//! `NOOBIE_SERVER_HOST` environment variable and defaults to `127.0.0.1`.
//!
//! Usage:
//!
//! ```text
//! noobietoria-web [OPTIONS]
//!   -p, --port <PORT>    Listening port  [default: 3000]
//!       --selftest       Run backend + moderation + server-api self-tests
//!   -h, --help           Print this help
//! ```

#[path = "graphql.rs"]
mod graphql;
#[path = "routes.rs"]
mod routes;
#[path = "state.rs"]
mod state;

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use axum::routing::{get, post};
use axum::Router;
use state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::services::ServeDir;

const DEFAULT_PORT: u16 = 3000;

fn find_static_dir() -> PathBuf {
    let candidates = [
        PathBuf::from("apps/noobietoria-web/static"),
        PathBuf::from("static"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static"),
    ];
    for c in candidates {
        if c.exists() {
            return c;
        }
    }
    PathBuf::from("static")
}

fn print_help() {
    println!("Noobietoria Web - Closed-Source Proprietary Gaming Platform");
    println!("Copyright (c) 2026 - present Noobietoria LLC (Công ty TNHH Trò chơi Noobietoria). All Rights Reserved.");
    println!();
    println!("Usage: noobietoria-web [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -p, --port <PORT>    Server listening port (default: {DEFAULT_PORT})");
    println!("  --selftest           Run automated backend and moderation self-tests");
    println!("  -h, --help           Print help information");
    println!();
    println!("Environment:");
    println!("  NOOBIE_SERVER_HOST   Hostname advertised in /api/v1/servers (default: 127.0.0.1)");
}

/// `GET /api/v1/servers` — every joinable server of the platform.
async fn list_servers_handler(State(state): State<AppState>) -> impl IntoResponse {
    let servers = state.all_servers();
    Json(serde_json::json!({
        "count": servers.len(),
        "servers": servers,
    }))
}

/// `GET /api/v1/places/{place_id}/servers` — servers of one place.
async fn place_servers_handler(
    State(state): State<AppState>,
    AxumPath(place_id): AxumPath<String>,
) -> impl IntoResponse {
    match state.servers_of(&place_id) {
        Some(servers) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "count": servers.len(),
                "servers": servers,
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "count": 0,
                "servers": [],
                "message": "Unknown place id",
            })),
        ),
    }
}

/// `POST /api/v1/servers/register` — a dedicated server announces itself.
///
/// Dedicated servers post a heartbeat every few seconds; the registration
/// expires on its own if the heartbeats stop, so a crashed host disappears from
/// the client's server list without any explicit deregistration.
async fn register_server_handler(
    State(state): State<AppState>,
    Json(registration): Json<state::ServerRegistration>,
) -> impl IntoResponse {
    if registration.server_id.trim().is_empty() || registration.place_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "message": "server_id and place_id are required",
            })),
        );
    }
    if registration.port == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "message": "port must be non-zero",
            })),
        );
    }

    let known = state.games.read().contains_key(&registration.place_id);
    let advertised = state.register_server(registration);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            // `known_place: false` means the server hosts a place the website
            // does not publish; it is still registered, but clients browsing
            // the catalog will not surface it.
            "success": true,
            "known_place": known,
            "server": advertised,
        })),
    )
}

fn build_router(state: AppState, static_dir: &PathBuf) -> Router {
    let serve_dir = ServeDir::new(static_dir).append_index_html_on_directories(true);

    Router::new()
        .route("/graphql", post(graphql::graphql_handler))
        .route("/auth/providers", get(routes::providers_handler))
        .route("/api/v1/auth/register", post(routes::register_handler))
        .route("/api/v1/auth/login", post(routes::login_handler))
        .route("/api/v1/games", get(routes::list_games_handler))
        .route(
            "/api/v1/games/{place_id}",
            get(routes::game_details_handler),
        )
        .route(
            "/api/v1/places/publish",
            post(routes::publish_place_handler),
        )
        .route(
            "/api/v1/moderation/check",
            post(routes::moderation_check_handler),
        )
        // ── Server directory (join endpoints for clients) ─────────────────────
        .route("/api/v1/servers", get(list_servers_handler))
        .route("/api/v1/servers/register", post(register_server_handler))
        .route(
            "/api/v1/places/{place_id}/servers",
            get(place_servers_handler),
        )
        .fallback_service(serve_dir)
        .with_state(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return Ok(());
    }

    if args.iter().any(|a| a == "--selftest") {
        run_selftest().await?;
        return Ok(());
    }

    // ── --port ────────────────────────────────────────────────────────────────
    let mut port = DEFAULT_PORT;
    let mut i = 1usize;
    while i < args.len() {
        if (args[i] == "--port" || args[i] == "-p") && i + 1 < args.len() {
            port = args[i + 1].parse()?;
            i += 1;
        }
        i += 1;
    }

    let static_dir = find_static_dir();
    println!(
        "[Noobietoria Web] Serving static assets from: {}",
        static_dir.display()
    );

    let state = AppState::new();
    println!(
        "[Noobietoria Web] Advertised game host: {}",
        state.server_host
    );
    let app = build_router(state, &static_dir);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!(
        "[Noobietoria Web] Proprietary Server listening on http://{}",
        addr
    );
    println!("[Noobietoria Web] Server directory: http://{addr}/api/v1/servers");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn run_selftest() -> Result<(), Box<dyn std::error::Error>> {
    println!("[Noobietoria Web] Running automated backend and moderation self-tests...");

    let state = AppState::new();

    // 1. Verify default published games
    let games = state.games.read();
    assert!(
        games.contains_key("starter_baseplate"),
        "Starter Baseplate must exist"
    );
    assert!(games.contains_key("obby_challenge"), "Obby game must exist");
    println!(
        "  [✓] Game catalog database initialized ({} featured games)",
        games.len()
    );
    drop(games);

    // 2. Content moderation client with caching
    let clean_text = "Welcome to my cool adventure game!";
    let res1 = state.moderation.check(clean_text).await?;
    assert!(res1.is_safe, "Clean game description must pass moderation");
    println!(
        "  [✓] Moderation API evaluated text safely (score: {})",
        res1.score
    );

    let res2 = state.moderation.check(clean_text).await?;
    assert!(res2.cached, "Subsequent check must be served from cache");
    println!("  [✓] Content cache verified: anti-spam cache prevented redundant API cost");

    // 3. Place publishing workflow (Studio -> Web)
    let starter = nb_world::WorldFile::default_starter_place();
    let starter_json = starter.to_json()?;
    let payload = routes::PublishPlaceRequest {
        name: "My Awesome Parkour".to_owned(),
        description: "A fast-paced jump challenge built in Studio.".to_owned(),
        creator_name: "ProBuilder".to_owned(),
        world_file_json: Some(starter_json),
    };
    let title_check = state.moderation.check(&payload.name).await?;
    assert!(title_check.is_safe);

    let place_id = format!("place_{}", uuid::Uuid::new_v4().simple());
    state.games.write().insert(
        place_id.clone(),
        state::PublishedGame {
            place_id: place_id.clone(),
            name: payload.name,
            description: payload.description,
            creator_name: payload.creator_name,
            creator_id: 1001,
            visits: 6,
            upvotes: 0,
            downvotes: 0,
            max_players: 50,
            server_port: 53645,
            created_at: "2026-10-15T00:00:00Z".to_owned(),
            updated_at: "2026-10-15T00:00:00Z".to_owned(),
            world_data: Some(starter),
        },
    );
    assert!(state.games.read().contains_key(&place_id));
    println!("  [✓] Studio to Web place publishing workflow verified");

    // 4. Server directory API (join endpoints for the desktop client)
    let all = state.all_servers();
    assert!(
        !all.is_empty(),
        "the platform must advertise at least one joinable server"
    );
    let expected: usize = state
        .games
        .read()
        .values()
        .map(|g| (g.visits % 3 + 1) as usize)
        .sum();
    assert_eq!(all.len(), expected, "every place contributes its servers");
    assert!(
        all.iter().all(|s| s.port >= 1024),
        "advertised ports must be unprivileged"
    );
    assert!(
        all.iter()
            .all(|s| s.status == "online" || s.status == "full"),
        "unknown server status would make clients refuse to join"
    );

    let published = state
        .servers_of(&place_id)
        .expect("published place must expose its servers");
    // visits = 6 -> 6 % 3 + 1 = 1 server, on the place's own port.
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].port, 53645);
    assert_eq!(published[0].server_id, format!("{place_id}:1"));
    assert_eq!(published[0].place_name, "My Awesome Parkour");
    assert_eq!(published[0].max_players, 50);
    assert!(state.servers_of("no_such_place").is_none());
    println!(
        "  [✓] Server directory API verified ({}) e.g. {} at {}:{}",
        all.len(),
        published[0].server_id,
        published[0].host,
        published[0].port
    );

    // 5. GraphQL API compatibility
    println!("  [✓] Testing GraphQL API operations (/graphql)...");
    let req = graphql::GraphQLRequest {
        query: "query { me { username coins credits } }".to_owned(),
        variables: serde_json::Value::Null,
    };
    let headers = axum::http::HeaderMap::new();
    let resp = graphql::graphql_handler(
        axum::extract::State(state.clone()),
        headers,
        axum::Json(req),
    )
    .await;
    let val: serde_json::Value = serde_json::to_value(&resp.0)?;
    assert!(
        val.get("data").is_some(),
        "GraphQL response must contain data"
    );
    println!("  [✓] GraphQL me query verified compatible");

    // 6. Static assets
    let static_dir = find_static_dir();
    assert!(static_dir.exists(), "Static directory must exist");
    assert!(
        static_dir.join("landing.html").exists(),
        "landing.html must exist"
    );
    assert!(
        static_dir.join("js/api.js").exists(),
        "js/api.js must exist"
    );
    println!(
        "  [✓] Static website files located at: {}",
        static_dir.display()
    );

    println!(
        "[Noobietoria Web] All closed-source web backend & server-directory tests passed successfully!"
    );
    Ok(())
}
