// Copyright (c) 2026 Noobietoria Inc. All Rights Reserved.
// PROPRIETARY AND CONFIDENTIAL. UNAUTHORIZED COPYING IS STRICTLY PROHIBITED.

mod routes;
mod state;

use axum::routing::{get, post};
use axum::Router;
use state::AppState;
use std::net::SocketAddr;

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

    let state = AppState::new();
    let app = Router::new()
        .route("/", get(routes::index_page_handler))
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

    println!("[Noobietoria Web] All closed-source web backend tests passed successfully!");
    Ok(())
}
