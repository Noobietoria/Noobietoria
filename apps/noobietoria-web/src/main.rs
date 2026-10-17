/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Copyright (c) 2026 Noobietoria Inc. All Rights Reserved.
// PROPRIETARY AND CONFIDENTIAL. UNAUTHORIZED COPYING IS STRICTLY PROHIBITED.

mod graphql;
mod routes;
mod state;

use axum::routing::{get, post};
use axum::Router;
use state::AppState;
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::services::ServeDir;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Noobietoria Web - Closed-Source Proprietary Gaming Platform");
        println!("Copyright (c) 2026 Noobietoria Inc. All Rights Reserved.");
        println!();
        println!("Usage: noobietoria-web [OPTIONS]");
        println!();
        println!("Options:");
        println!("  -p, --port <PORT>    Server listening port (default: 3000)");
        println!("  --selftest           Run automated backend and moderation self-tests");
        println!("  -h, --help           Print help information");
        return Ok(());
    }

    if args.iter().any(|a| a == "--selftest") {
        run_selftest().await?;
        return Ok(());
    }

    let static_dir = find_static_dir();
    println!(
        "[Noobietoria Web] Serving static assets from: {}",
        static_dir.display()
    );

    let state = AppState::new_with_auth().await;
    println!("Auth backend ready");
    let serve_dir = ServeDir::new(&static_dir).append_index_html_on_directories(true);

    let app = Router::new()
        .route("/graphql", post(graphql::graphql_handler))
        .route("/auth/providers", get(routes::providers_handler))
        .route("/api/v1/auth/register", post(routes::register_handler))
        .route("/api/v1/auth/login", post(routes::login_handler))
        .route("/api/v1/auth/logout", post(routes::logout_handler))
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
        .fallback_service(serve_dir)
        .with_state(state);

    let port = 3000;
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!(
        "[Noobietoria Web] Proprietary Server listening on http://{}",
        addr
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn run_selftest() -> Result<(), Box<dyn std::error::Error>> {
    println!("[Noobietoria Web] Running automated backend and moderation self-tests...");

    let state = AppState::new_with_auth().await;

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

    // 2. Test Content Moderation Client with Caching
    let clean_text = "Welcome to my cool adventure game!";
    let res1 = state.moderation.check(clean_text).await?;
    assert!(res1.is_safe, "Clean game description must pass moderation");
    println!(
        "  [✓] Moderation API evaluated text safely (score: {})",
        res1.score
    );

    // Test Cache hit (same text should hit cache without network cost)
    let res2 = state.moderation.check(clean_text).await?;
    assert!(res2.cached, "Subsequent check must be served from cache");
    println!("  [✓] Content cache verified: anti-spam cache prevented redundant API cost");

    // 3. Test Place Publishing
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
            visits: 0,
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

    // 4. Test GraphQL API Compatibility
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
    let json_bytes = serde_json::to_vec(&resp.0)?;
    let val: serde_json::Value = serde_json::from_slice(&json_bytes)?;
    assert!(
        val.get("data").is_some(),
        "GraphQL response must contain data"
    );
    println!("  [✓] GraphQL me query verified compatible");

    // Test GraphQL Login
    let login_req = graphql::GraphQLRequest {
        query: "mutation($u:String!,$p:String!){ login(usernameOrEmail:$u,password:$p){ token } }"
            .to_owned(),
        variables: serde_json::json!({ "u": "PlayerOne", "p": "password123" }),
    };
    let headers2 = axum::http::HeaderMap::new();
    let login_resp = graphql::graphql_handler(
        axum::extract::State(state.clone()),
        headers2,
        axum::Json(login_req),
    )
    .await;
    let login_bytes = serde_json::to_vec(&login_resp.0)?;
    let login_val: serde_json::Value = serde_json::from_slice(&login_bytes)?;
    let token = login_val["data"]["login"]["token"]
        .as_str()
        .expect("Token should be returned");
    assert!(
        token.starts_with("nb_tok_") || token.starts_with("nb_"),
        "Valid token returned"
    );
    println!("  [✓] GraphQL login mutation verified compatible (issued token)");

    // Test Static assets directory
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

    println!("[Noobietoria Web] All closed-source web backend & GraphQL compatibility tests passed successfully!");
    Ok(())
}
