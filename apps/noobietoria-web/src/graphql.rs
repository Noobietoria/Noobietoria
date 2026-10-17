/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Copyright (c) 2026 Noobietoria Inc. All Rights Reserved.
// PROPRIETARY AND CONFIDENTIAL. UNAUTHORIZED COPYING IS STRICTLY PROHIBITED.

//! GraphQL API Dispatcher for Noobietoria Web.
//! Implements all schema operations required by the web frontend:
//! accounts, authentication, worlds, catalog, inventory, guilds, friends.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::state::{AppState, PublishedGame, UserAccount};

#[derive(Debug, Deserialize)]
pub struct GraphQLRequest {
    pub query: String,
    #[serde(default)]
    pub variables: Value,
}

#[derive(Debug, Serialize)]
pub struct GraphQLResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Serialize)]
pub struct GraphQLError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Value>,
}

pub async fn graphql_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<GraphQLRequest>,
) -> Json<GraphQLResponse> {
    let auth_token = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| {
            s.strip_prefix("Bearer ")
                .or_else(|| s.strip_prefix("bearer "))
        })
        .map(|s| s.trim().to_owned());

    let current_user: Option<UserAccount> = if let Some(tok) = &auth_token {
        // SQLite-first (durable sessions), in-memory fallback.
        match state.auth_validate(tok).await {
            Ok(record) => {
                // Mirror the auth record into the rich UserAccount store.
                let mut users = state.users.write();
                let account = users
                    .entry(record.username.clone())
                    .or_insert_with(|| UserAccount {
                        user_id: record.user_id as u64,
                        username: record.username.clone(),
                        email: record.email.clone(),
                        password_hash: String::new(),
                        display_name: record.display_name.clone(),
                        bio: "Playing Noobietoria!".to_owned(),
                        avatar_emoji: "😀".to_owned(),
                        body_color_hue: 205,
                        banner_color: "#1e293b".to_owned(),
                        level: record.level,
                        plus_tier: "Free".to_owned(),
                        credits: record.credits,
                        coins: record.coins,
                        streak: 1,
                        longest_streak: 1,
                        item_sales: 0,
                        networth: 150,
                        status: "online".to_owned(),
                        created_at: record.created_at.to_rfc3339(),
                        last_seen: "Just now".to_owned(),
                        friends_count: 0,
                        worlds_count: 0,
                        inventory: vec![],
                        equipped: vec![],
                        studs: 25,
                        last_studs_claim: None,
                        avatar: Some(nb_types::AvatarConfig::classic_noob()),
                    });
                Some(account.clone())
            }
            Err(_) => {
                // Fallback: legacy token map (demo user etc.)
                let tokens = state.tokens.read();
                let username = tokens.get(tok).cloned();
                drop(tokens);
                if let Some(u) = username {
                    state.users.read().get(&u).cloned()
                } else {
                    None
                }
            }
        }
    } else {
        None
    };

    let q = payload.query.trim();
    let vars = &payload.variables;

    let res = dispatch(
        &state,
        q,
        vars,
        current_user.as_ref(),
        auth_token.as_deref(),
    )
    .await;

    match res {
        Ok(data) => Json(GraphQLResponse {
            data: Some(data),
            errors: None,
        }),
        Err(err) => Json(GraphQLResponse {
            data: None,
            errors: Some(vec![GraphQLError {
                message: err,
                extensions: None,
            }]),
        }),
    }
}

async fn dispatch(
    state: &AppState,
    query: &str,
    vars: &Value,
    current_user: Option<&UserAccount>,
    auth_token: Option<&str>,
) -> Result<Value, String> {
    let mut data = serde_json::Map::new();

    // ── Mutation: login ───────────────────────────────────────────────────────
    if query.contains("login(") || query.contains("login ") {
        let u = get_str_var(vars, query, &["u", "usernameOrEmail", "username"])
            .ok_or_else(|| "Missing username or email".to_owned())?;
        let p = get_str_var(vars, query, &["p", "password"])
            .ok_or_else(|| "Missing password".to_owned())?;

        // SQLite-first (Argon2 verification, durable sessions).
        match state.auth_login(&u, &p).await {
            Ok(token) => {
                state.tokens.write().insert(token.clone(), u.clone());
                data.insert("login".to_owned(), json!({ "token": token }));
                return Ok(Value::Object(data));
            }
            Err(_) => {
                // Fallback: legacy in-memory check for the demo user.
                let users = state.users.read();
                let matched = users
                    .values()
                    .find(|acc| acc.username == u || acc.email == u)
                    .cloned();
                drop(users);

                if let Some(user) = matched {
                    if user.password_hash == p {
                        let token = format!("nb_tok_{}", uuid::Uuid::new_v4().simple());
                        state.tokens.write().insert(token.clone(), user.username);
                        data.insert("login".to_owned(), json!({ "token": token }));
                        return Ok(Value::Object(data));
                    }
                }
                return Err("Invalid username or password".to_owned());
            }
        }
    }

    // ── Mutation: register ────────────────────────────────────────────────────
    if query.contains("register(") || query.contains("register ") {
        let u = get_str_var(vars, query, &["u", "username"])
            .ok_or_else(|| "Missing username".to_owned())?;
        let e = get_str_var(vars, query, &["e", "email"])
            .unwrap_or_else(|| format!("{}@noobietoria.com", u));
        let p = get_str_var(vars, query, &["p", "password"])
            .ok_or_else(|| "Missing password".to_owned())?;

        // 1. Check content moderation on username
        let mod_check =
            state
                .moderation
                .check(&u)
                .await
                .unwrap_or_else(|_| nb_http::ModerationResult {
                    is_safe: true,
                    score: 0.0,
                    flagged: false,
                    label: None,
                    cached: false,
                    raw: Value::Null,
                });

        if !mod_check.is_safe {
            return Err(
                "Username contains inappropriate language or violates community guidelines."
                    .to_owned(),
            );
        }

        // Persist credentials to the durable auth backend first (SQLite when
        // configured). Best-effort: if the user already exists there from a
        // previous run, continue with the mirrored in-memory profile.
        //
        // NOTE: no lock may be held across `.await` — parking_lot guards are
        // not Send, which breaks axum's `Handler` bound.
        if state.users.read().contains_key(&u) {
            return Err("That username is already taken. Please choose another.".to_owned());
        }
        match state.auth_register(&u, &e, &p).await {
            Ok(_) => {}
            Err(nb_auth::AuthError::UsernameTaken(_)) | Err(nb_auth::AuthError::EmailTaken(_)) => {
                return Err("That username is already taken. Please choose another.".to_owned());
            }
            Err(e) => return Err(format!("Registration failed: {e}")),
        }

        let user_count = state.users.read().len();
        let new_id = (user_count as u64 + 1001).to_string();
        let user = UserAccount {
            user_id: new_id.parse().unwrap_or(1001),
            username: u.clone(),
            email: e,
            password_hash: format!("sha256:{}", p),
            display_name: u.clone(),
            bio: "Building cool worlds on Noobietoria!".to_owned(),
            avatar_emoji: "😀".to_owned(),
            body_color_hue: 205,
            banner_color: "#1e293b".to_owned(),
            level: 1,
            plus_tier: "Free".to_owned(),
            credits: 50,
            coins: 200,
            streak: 1,
            longest_streak: 1,
            item_sales: 0,
            networth: 250,
            status: "online".to_owned(),
            created_at: "2026-10-15T00:00:00Z".to_owned(),
            last_seen: "Just now".to_owned(),
            friends_count: 0,
            worlds_count: 0,
            inventory: vec!["item_classic_cap".to_owned()],
            equipped: vec!["item_classic_cap".to_owned()],
            studs: 25,
            last_studs_claim: None,
            avatar: Some(nb_types::AvatarConfig::classic_noob()),
        };

        state.users.write().insert(u.clone(), user);

        // Mint a durable session token so the new account survives restarts.
        let token = state
            .auth_login(&u, &p)
            .await
            .unwrap_or_else(|_| format!("nb_tok_{}", uuid::Uuid::new_v4().simple()));
        state.tokens.write().insert(token.clone(), u);
        data.insert("register".to_owned(), json!({ "token": token }));
        return Ok(Value::Object(data));
    }

    // ── Mutation: logout ──────────────────────────────────────────────────────
    if query.contains("logout") {
        if let Some(tok) = auth_token {
            state.auth_logout(tok).await;
            state.tokens.write().remove(tok);
        }
        data.insert("logout".to_owned(), json!(true));
        return Ok(Value::Object(data));
    }

    // ── Mutation: createWorld ─────────────────────────────────────────────────
    if query.contains("createWorld") {
        let name =
            get_str_var(vars, query, &["n", "name"]).unwrap_or_else(|| "My Place".to_owned());
        let desc = get_str_var(vars, query, &["d", "description"]).unwrap_or_default();
        let _genre =
            get_str_var(vars, query, &["g", "genre"]).unwrap_or_else(|| "Adventure".to_owned());
        let _vis =
            get_str_var(vars, query, &["v", "visibility"]).unwrap_or_else(|| "public".to_owned());

        let mod_check =
            state
                .moderation
                .check(&name)
                .await
                .unwrap_or_else(|_| nb_http::ModerationResult {
                    is_safe: true,
                    score: 0.0,
                    flagged: false,
                    label: None,
                    cached: false,
                    raw: Value::Null,
                });

        if !mod_check.is_safe {
            return Err("World title violates safety guidelines.".to_owned());
        }

        let place_id = format!("place_{}", uuid::Uuid::new_v4().simple());
        let creator_name = current_user
            .map(|u| u.username.clone())
            .unwrap_or_else(|| "Player".to_owned());

        let new_game = PublishedGame {
            place_id: place_id.clone(),
            name,
            description: desc,
            creator_name,
            creator_id: current_user.map(|u| u.user_id).unwrap_or(1),
            visits: 0,
            upvotes: 0,
            downvotes: 0,
            max_players: 50,
            server_port: 53645,
            created_at: "2026-10-15T00:00:00Z".to_owned(),
            updated_at: "2026-10-15T00:00:00Z".to_owned(),
            world_data: Some(nb_world::WorldFile::default_starter_place()),
        };

        state.games.write().insert(place_id.clone(), new_game);
        data.insert("createWorld".to_owned(), json!({ "id": place_id }));
        return Ok(Value::Object(data));
    }

    // ── Mutation: deleteWorld ─────────────────────────────────────────────────
    if query.contains("deleteWorld") {
        let id = get_str_var(vars, query, &["id"]).unwrap_or_default();
        state.games.write().remove(&id);
        data.insert("deleteWorld".to_owned(), json!(true));
        return Ok(Value::Object(data));
    }

    // ── Mutation: setBodyColor ────────────────────────────────────────────────
    if query.contains("setBodyColor") {
        let hue = vars.get("h").and_then(|v| v.as_i64()).unwrap_or(205) as i32;
        if let Some(u) = current_user {
            let mut users = state.users.write();
            if let Some(acc) = users.get_mut(&u.username) {
                acc.body_color_hue = hue;
            }
        }
        data.insert("setBodyColor".to_owned(), json!({ "bodyColorHue": hue }));
        return Ok(Value::Object(data));
    }

    // ── Mutation: buyItem ─────────────────────────────────────────────────────
    if query.contains("buyItem") {
        let item_id = get_str_var(vars, query, &["id", "itemId"]).unwrap_or_default();
        if let Some(u) = current_user {
            let mut users = state.users.write();
            if let Some(acc) = users.get_mut(&u.username) {
                if !acc.inventory.contains(&item_id) {
                    acc.inventory.push(item_id.clone());
                }
            }
        }
        data.insert("buyItem".to_owned(), json!({ "item": { "name": item_id } }));
        return Ok(Value::Object(data));
    }

    // ── Query: me ─────────────────────────────────────────────────────────────
    if query.contains(" me ")
        || query.contains(" me{")
        || query.starts_with("query { me")
        || query.starts_with("query{me")
    {
        if let Some(u) = current_user {
            data.insert("me".to_owned(), user_to_gql(u));
        } else {
            // Default guest user for anonymous browsing
            let guest = UserAccount {
                user_id: 1000,
                username: "Guest".to_owned(),
                email: "guest@noobietoria.com".to_owned(),
                password_hash: "".to_owned(),
                display_name: "Guest Player".to_owned(),
                bio: "Welcome to Noobietoria!".to_owned(),
                avatar_emoji: "🙂".to_owned(),
                body_color_hue: 200,
                banner_color: "#1e293b".to_owned(),
                level: 1,
                plus_tier: "Free".to_owned(),
                credits: 50,
                coins: 100,
                streak: 1,
                longest_streak: 1,
                item_sales: 0,
                networth: 150,
                status: "online".to_owned(),
                created_at: "2026-10-15T00:00:00Z".to_owned(),
                last_seen: "Now".to_owned(),
                friends_count: 0,
                worlds_count: 0,
                inventory: vec![],
                equipped: vec![],
                studs: 25,
                last_studs_claim: None,
                avatar: Some(nb_types::AvatarConfig::classic_noob()),
            };
            data.insert("me".to_owned(), user_to_gql(&guest));
        }
    }

    // ── Query: worlds / featuredWorlds / myWorlds ─────────────────────────────
    if query.contains("worlds") || query.contains("featuredWorlds") || query.contains("myWorlds") {
        let games = state.games.read();
        let list: Vec<Value> = games
            .values()
            .map(|g| {
                let playing = (g.visits % 350) + 12;
                let likes_pct = if g.upvotes + g.downvotes > 0 {
                    (g.upvotes as f64 / (g.upvotes + g.downvotes) as f64 * 100.0).round() as i64
                } else {
                    95
                };
                let emoji = match g.place_id.as_str() {
                    "starter_baseplate" => "🟩",
                    "obby_challenge" => "🏃",
                    "natural_survival" => "🌋",
                    _ => "🎮",
                };
                let thumb_color = match g.place_id.as_str() {
                    "starter_baseplate" => "#1e3a1e",
                    "obby_challenge" => "#3b1e3b",
                    "natural_survival" => "#3b2a1e",
                    _ => "#1e293b",
                };

                json!({
                    "id": g.place_id,
                    "name": g.name,
                    "description": g.description,
                    "creator": {
                        "username": g.creator_name,
                    },
                    "genre": "Adventure",
                    "visibility": "public",
                    "playing": playing,
                    "likesPct": likes_pct,
                    "thumbColor": thumb_color,
                    "emoji": emoji,
                })
            })
            .collect();

        if query.contains("featuredWorlds") {
            data.insert("featuredWorlds".to_owned(), json!(list));
        }
        if query.contains("myWorlds") {
            data.insert("myWorlds".to_owned(), json!(list));
        }
        if query.contains("worlds") {
            data.insert("worlds".to_owned(), json!(list));
        }
    }

    // ── Query: catalog ────────────────────────────────────────────────────────
    if query.contains("catalog") {
        let catalog_items = json!({
            "total": 6,
            "items": [
                { "id": "cap_classic", "name": "Classic Cap", "emoji": "🧢", "color": "#00a2ff", "category": "HATS", "slot": "hat", "price": 50, "currency": "COINS", "owned": true },
                { "id": "shades_cool", "name": "Speed Shades", "emoji": "🕶️", "color": "#111827", "category": "FACES", "slot": "face", "price": 100, "currency": "COINS", "owned": false },
                { "id": "hoodie_neon", "name": "Cyber Hoodie", "emoji": "🧥", "color": "#7c3aed", "category": "SHIRTS", "slot": "shirt", "price": 150, "currency": "COINS", "owned": false },
                { "id": "sword_blade", "name": "Spark Sword", "emoji": "⚔️", "color": "#eab308", "category": "GEAR", "slot": "gear", "price": 300, "currency": "COINS", "owned": false },
                { "id": "wings_dragon", "name": "Dragon Wings", "emoji": "🪽", "color": "#ef4444", "category": "BACK", "slot": "back", "price": 500, "currency": "CREDITS", "owned": false },
                { "id": "halo_gold", "name": "Golden Halo", "emoji": "✨", "color": "#f59e0b", "category": "HATS", "slot": "hat", "price": 1000, "currency": "CREDITS", "owned": false }
            ]
        });
        data.insert("catalog".to_owned(), catalog_items);
    }

    // ── Query: inventory ──────────────────────────────────────────────────────
    if query.contains("inventory") {
        let items = json!({
            "total": 2,
            "entries": [
                { "item": { "id": "cap_classic", "name": "Classic Cap", "emoji": "🧢", "color": "#00a2ff", "category": "HATS", "creator": { "username": "Noobietoria" } } },
                { "item": { "id": "sword_starter", "name": "Wooden Sword", "emoji": "🗡️", "color": "#8b5a2b", "category": "GEAR", "creator": { "username": "Noobietoria" } } }
            ]
        });
        data.insert("inventory".to_owned(), items);
    }

    // ── Query: guilds / myGuilds ──────────────────────────────────────────────
    if query.contains("guilds") || query.contains("myGuilds") {
        let guild_list = json!([
            { "id": "g1", "name": "Builders Guild", "description": "Master architects of Studio places.", "emoji": "🔨", "bannerColors": ["#2563eb", "#1d4ed8"], "verified": true, "memberCount": 1420, "joined": true },
            { "id": "g2", "name": "Obby Masters", "description": "World record speedrunners and parkour creators.", "emoji": "🏆", "bannerColors": ["#d97706", "#b45309"], "verified": true, "memberCount": 980, "joined": false },
            { "id": "g3", "name": "Luau Coders", "description": "Scripting community, shared modules and tutorials.", "emoji": "💻", "bannerColors": ["#059669", "#047857"], "verified": false, "memberCount": 650, "joined": false }
        ]);
        if query.contains("myGuilds") {
            data.insert("myGuilds".to_owned(), guild_list.clone());
        }
        if query.contains("guilds") {
            data.insert("guilds".to_owned(), guild_list);
        }
    }

    // ── Query: friends / friendRequests / users ───────────────────────────────
    if query.contains("friends") || query.contains("friendRequests") || query.contains("users(") {
        let sample_friends = json!([
            { "id": "2", "username": "ObbyKing2026", "bio": "Jump champion", "avatarEmoji": "😎", "status": "online", "level": 12, "plusTier": "Plus" },
            { "id": "3", "username": "SurvivalCraft", "bio": "Disaster survival master", "avatarEmoji": "🤠", "status": "ingame", "level": 8, "plusTier": "Free" }
        ]);
        if query.contains("friendRequests") {
            data.insert("friendRequests".to_owned(), json!([]));
        }
        if query.contains("friends") {
            data.insert("friends".to_owned(), sample_friends.clone());
        }
        if query.contains("users(") {
            data.insert("users".to_owned(), sample_friends);
        }
    }

    // ── Query: news ───────────────────────────────────────────────────────────
    if query.contains("news") {
        data.insert("news".to_owned(), json!([
            { "id": "n1", "title": "Noobietoria Studio v0.1.0 Released!", "emoji": "🚀", "thumbColor": "#1e293b", "publishedAt": "2026-10-15T00:00:00Z" },
            { "id": "n2", "title": "Halloween Place Building Contest", "emoji": "🎃", "thumbColor": "#3b1e2b", "publishedAt": "2026-10-10T00:00:00Z" }
        ]));
    }

    // ── Query: user(username: ...) ────────────────────────────────────────────
    if query.contains("user(") || query.contains("profile:") {
        let uname = get_str_var(vars, query, &["username"]).unwrap_or_else(|| "Player".to_owned());
        let users = state.users.read();
        let target_user = users.get(&uname).cloned().unwrap_or_else(|| UserAccount {
            user_id: 1002,
            username: uname.clone(),
            email: format!("{}@noobietoria.com", uname),
            password_hash: "".to_owned(),
            display_name: uname.clone(),
            bio: "Exploring the metaverse.".to_owned(),
            avatar_emoji: "🙂".to_owned(),
            body_color_hue: 200,
            banner_color: "#1e293b".to_owned(),
            level: 3,
            plus_tier: "Free".to_owned(),
            credits: 20,
            coins: 150,
            streak: 3,
            longest_streak: 5,
            item_sales: 0,
            networth: 170,
            status: "online".to_owned(),
            created_at: "2026-03-01T00:00:00Z".to_owned(),
            last_seen: "Today".to_owned(),
            friends_count: 5,
            worlds_count: 1,
            inventory: vec![],
            equipped: vec![],
            studs: 25,
            last_studs_claim: None,
            avatar: Some(nb_types::AvatarConfig::classic_noob()),
        });

        data.insert("profile".to_owned(), user_to_gql(&target_user));
        data.insert("user".to_owned(), user_to_gql(&target_user));
    }

    Ok(Value::Object(data))
}

fn user_to_gql(u: &UserAccount) -> Value {
    json!({
        "id": u.user_id.to_string(),
        "username": u.username,
        "email": u.email,
        "bio": u.bio,
        "avatarEmoji": u.avatar_emoji,
        "bodyColorHue": u.body_color_hue,
        "bannerColor": u.banner_color,
        "level": u.level,
        "plusTier": u.plus_tier,
        "credits": u.credits,
        "coins": u.coins,
        "studs": u.studs,
        "streak": u.streak,
        "longestStreak": u.longest_streak,
        "itemSales": u.item_sales,
        "networth": u.networth,
        "status": u.status,
        "createdAt": u.created_at,
        "lastSeen": u.last_seen,
        "friendsCount": u.friends_count,
        "worldsCount": u.worlds_count,
        "avatar": {
            "equipped": u.equipped.iter().map(|id| json!({ "item": { "id": id } })).collect::<Vec<_>>()
        }
    })
}

fn get_str_var(vars: &Value, query: &str, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(v) = vars.get(*k).and_then(|v| v.as_str()) {
            return Some(v.to_owned());
        }
    }

    // Fallback: extract string from query if inlined: key: "value" or key:"value"
    for k in keys {
        let pattern = format!("{}:\"", k);
        if let Some(pos) = query.find(&pattern) {
            let start = pos + pattern.len();
            if let Some(end) = query[start..].find('"') {
                return Some(query[start..start + end].to_owned());
            }
        }
        let pattern_space = format!("{}: \"", k);
        if let Some(pos) = query.find(&pattern_space) {
            let start = pos + pattern_space.len();
            if let Some(end) = query[start..].find('"') {
                return Some(query[start..start + end].to_owned());
            }
        }
    }

    None
}
