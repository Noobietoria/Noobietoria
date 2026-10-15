/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Copyright (c) 2026 Noobietoria Inc. All Rights Reserved.
// PROPRIETARY AND CONFIDENTIAL. UNAUTHORIZED COPYING IS STRICTLY PROHIBITED.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::state::{AppState, PublishedGame, UserAccount};
use nb_world::WorldFile;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub success: bool,
    pub user_id: Option<u64>,
    pub username: Option<String>,
    pub token: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct PublishPlaceRequest {
    pub name: String,
    pub description: String,
    pub creator_name: String,
    pub world_file_json: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModerationCheckRequest {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct ModerationCheckResponse {
    pub allowed: bool,
    pub confidence_score: f64,
    pub cached: bool,
    pub message: String,
}

// --- Handler: Register ---
pub async fn register_handler(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> impl IntoResponse {
    let mut users = state.users.write();
    if users.contains_key(&payload.username) {
        return (
            StatusCode::CONFLICT,
            Json(AuthResponse {
                success: false,
                user_id: None,
                username: None,
                token: None,
                message: "Username already taken".to_owned(),
            }),
        );
    }

    let user_id = users.len() as u64 + 1000;
    let token = format!("nb_jwt_{user_id}");
    let user = UserAccount {
        user_id,
        username: payload.username.clone(),
        email: format!("{}@noobietoria.com", payload.username),
        password_hash: format!("sha256:{}", payload.password),
        display_name: payload
            .display_name
            .unwrap_or_else(|| payload.username.clone()),
        bio: "Playing Noobietoria!".to_owned(),
        avatar_emoji: "😀".to_owned(),
        body_color_hue: 205,
        banner_color: "#1e293b".to_owned(),
        level: 1,
        plus_tier: "Free".to_owned(),
        credits: 50,
        coins: 100, // Starter currency
        streak: 1,
        longest_streak: 1,
        item_sales: 0,
        networth: 150,
        status: "online".to_owned(),
        created_at: "2026-10-15T00:00:00Z".to_owned(),
        last_seen: "Just now".to_owned(),
        friends_count: 0,
        worlds_count: 0,
        inventory: vec![],
        equipped: vec![],
    };

    users.insert(payload.username.clone(), user);
    drop(users);
    state
        .tokens
        .write()
        .insert(token.clone(), payload.username.clone());

    (
        StatusCode::CREATED,
        Json(AuthResponse {
            success: true,
            user_id: Some(user_id),
            username: Some(payload.username),
            token: Some(token),
            message: "Account created successfully".to_owned(),
        }),
    )
}

// --- Handler: Login ---
pub async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    let users = state.users.read();
    if let Some(user) = users.get(&payload.username) {
        if user.password_hash == format!("sha256:{}", payload.password) {
            let user_id = user.user_id;
            let token = format!("nb_jwt_{}", user_id);
            drop(users);
            state
                .tokens
                .write()
                .insert(token.clone(), payload.username.clone());

            return (
                StatusCode::OK,
                Json(AuthResponse {
                    success: true,
                    user_id: Some(user_id),
                    username: Some(payload.username),
                    token: Some(token),
                    message: "Login successful".to_owned(),
                }),
            );
        }
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(AuthResponse {
            success: false,
            user_id: None,
            username: None,
            token: None,
            message: "Invalid username or password".to_owned(),
        }),
    )
}

// --- Handler: Auth Providers ---
pub async fn providers_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "google": false,
        "discord": false,
        "roblox": false,
    }))
}

// --- Handler: List Games ---
pub async fn list_games_handler(State(state): State<AppState>) -> impl IntoResponse {
    let games = state.games.read();
    let list: Vec<PublishedGame> = games.values().cloned().collect();
    Json(list)
}

// --- Handler: Game Details ---
pub async fn game_details_handler(
    State(state): State<AppState>,
    Path(place_id): Path<String>,
) -> impl IntoResponse {
    let games = state.games.read();
    if let Some(game) = games.get(&place_id) {
        (StatusCode::OK, Json(Some(game.clone())))
    } else {
        (StatusCode::NOT_FOUND, Json(None))
    }
}

// --- Handler: Publish Place ---
pub async fn publish_place_handler(
    State(state): State<AppState>,
    Json(payload): Json<PublishPlaceRequest>,
) -> impl IntoResponse {
    // 1. Content moderation check on Game Title & Description
    let title_check = state
        .moderation
        .check(&payload.name)
        .await
        .unwrap_or_else(|_| nb_http::ModerationResult {
            is_safe: true,
            score: 0.0,
            flagged: false,
            label: None,
            cached: false,
            raw: serde_json::Value::Null,
        });
    let desc_check = state
        .moderation
        .check(&payload.description)
        .await
        .unwrap_or_else(|_| nb_http::ModerationResult {
            is_safe: true,
            score: 0.0,
            flagged: false,
            label: None,
            cached: false,
            raw: serde_json::Value::Null,
        });

    if !title_check.is_safe || !desc_check.is_safe {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "message": "Game title or description violated community safety guidelines."
            })),
        );
    }

    // 2. Parse world data if provided
    let world_data = payload
        .world_file_json
        .and_then(|j| WorldFile::from_json(&j).ok());

    let place_id = format!("place_{}", uuid::Uuid::new_v4().simple());
    let published = PublishedGame {
        place_id: place_id.clone(),
        name: payload.name,
        description: payload.description,
        creator_name: payload.creator_name,
        creator_id: 1,
        visits: 0,
        upvotes: 0,
        downvotes: 0,
        max_players: 50,
        server_port: 53650,
        created_at: "2026-10-15T00:00:00Z".to_owned(),
        updated_at: "2026-10-15T00:00:00Z".to_owned(),
        world_data,
    };

    state.games.write().insert(place_id.clone(), published);
    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "place_id": place_id,
            "message": "Place published successfully to Noobietoria Web!"
        })),
    )
}

// --- Handler: Content Moderation Check ---
pub async fn moderation_check_handler(
    State(state): State<AppState>,
    Json(payload): Json<ModerationCheckRequest>,
) -> impl IntoResponse {
    let result = state
        .moderation
        .check(&payload.text)
        .await
        .unwrap_or_else(|_| nb_http::ModerationResult {
            is_safe: true,
            score: 0.0,
            flagged: false,
            label: None,
            cached: false,
            raw: serde_json::Value::Null,
        });

    Json(ModerationCheckResponse {
        allowed: result.is_safe,
        confidence_score: result.score,
        cached: result.cached,
        message: if result.is_safe {
            "Content passed moderation.".to_owned()
        } else {
            "Content flagged as inappropriate.".to_owned()
        },
    })
}

// --- Handler: Front-end Homepage (Roblox Dark Theme) ---
#[allow(dead_code)]
pub async fn index_page_handler(State(state): State<AppState>) -> Html<String> {
    let games = state.games.read();
    let mut games_html = String::new();

    for game in games.values() {
        games_html.push_str(&format!(
            "<div class=\"game-card\"><div class=\"game-thumb\">🎮</div><div class=\"game-info\"><h3 class=\"game-title\">{}</h3><p class=\"game-creator\">By {}</p><div class=\"game-stats\"><span>👍 {}</span><span>👁 {}</span></div><button class=\"play-btn\" onclick=\"launchGame('{}')\">▶ Play</button></div></div>",
            game.name,
            game.creator_name,
            game.upvotes,
            game.visits,
            game.place_id,
        ));
    }

    let template = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Noobietoria - Discover & Play</title>
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; }
        body { background-color: #111418; color: #ffffff; min-height: 100vh; }
        header { background: #161b22; border-bottom: 1px solid #30363d; padding: 12px 24px; display: flex; align-items: center; justify-content: space-between; }
        .logo { font-size: 22px; font-weight: 800; color: #00a2ff; letter-spacing: -0.5px; display: flex; align-items: center; gap: 8px; }
        .nav-links { display: flex; gap: 20px; }
        .nav-links a { color: #8b949e; text-decoration: none; font-size: 14px; font-weight: 600; }
        .nav-links a.active { color: #ffffff; }
        .user-panel { display: flex; align-items: center; gap: 12px; }
        .coins-badge { background: #21262d; padding: 6px 12px; border-radius: 16px; font-size: 13px; font-weight: 700; color: #f1c40f; border: 1px solid #30363d; }
        .main-container { max-width: 1200px; margin: 0 auto; padding: 32px 20px; }
        .hero { background: linear-gradient(135deg, #1f2937, #111827); border-radius: 12px; padding: 36px; margin-bottom: 36px; border: 1px solid #374151; }
        .hero h1 { font-size: 32px; margin-bottom: 10px; font-weight: 800; }
        .hero p { color: #8b949e; font-size: 16px; max-width: 600px; line-height: 1.5; }
        .section-title { font-size: 20px; font-weight: 700; margin-bottom: 20px; }
        .games-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 20px; }
        .game-card { background: #1c2128; border: 1px solid #30363d; border-radius: 10px; overflow: hidden; display: flex; flex-direction: column; }
        .game-card:hover { transform: translateY(-4px); border-color: #00a2ff; }
        .game-thumb { height: 130px; background: #2d333b; display: flex; align-items: center; justify-content: center; font-size: 48px; }
        .game-info { padding: 16px; display: flex; flex-direction: column; flex-grow: 1; }
        .game-title { font-size: 15px; font-weight: 700; margin-bottom: 4px; }
        .game-creator { font-size: 12px; color: #8b949e; margin-bottom: 12px; }
        .game-stats { display: flex; justify-content: space-between; font-size: 12px; color: #8b949e; margin-bottom: 16px; font-weight: 600; }
        .play-btn { background: #2ea043; color: white; border: none; border-radius: 6px; padding: 10px; font-weight: 700; cursor: pointer; }
        .play-btn:hover { background: #238636; }
        footer { margin-top: 60px; text-align: center; padding: 24px; color: #8b949e; font-size: 13px; border-top: 1px solid #21262d; }
    </style>
</head>
<body>
    <header>
        <div class="logo">🟥 Noobietoria</div>
        <div class="nav-links">
            <a href="#" class="active">Discover</a>
            <a href="#">Avatar</a>
            <a href="#">Create</a>
            <a href="#">Robux</a>
        </div>
        <div class="user-panel">
            <span class="coins-badge">🪙 500 Coins</span>
            <span style="font-size:14px;font-weight:600;">PlayerOne</span>
        </div>
    </header>

    <div class="main-container">
        <div class="hero">
            <h1>Create Anything, Play Everything</h1>
            <p>Welcome to Noobietoria. Discover millions of player-created worlds or build your own with Noobietoria Studio.</p>
        </div>

        <h2 class="section-title">🔥 Featured Experiences</h2>
        <div class="games-grid">
            <!--GAMES-->
        </div>
    </div>

    <footer>
        <p>© 2026 Noobietoria Inc. All Rights Reserved. Closed-Source Proprietary System.</p>
    </footer>

    <script>
        function launchGame(placeId) {
            alert("Connecting Noobietoria Client to Place: " + placeId);
        }
    </script>
</body>
</html>"##;

    Html(template.replace("<!--GAMES-->", &games_html))
}
