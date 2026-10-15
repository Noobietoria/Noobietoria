/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Copyright (c) 2026 Noobietoria Inc. All Rights Reserved.
// PROPRIETARY AND CONFIDENTIAL. UNAUTHORIZED COPYING IS STRICTLY PROHIBITED.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use nb_http::ModerationClient;
use nb_world::WorldFile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAccount {
    pub user_id: u64,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub display_name: String,
    pub bio: String,
    pub avatar_emoji: String,
    pub body_color_hue: i32,
    pub banner_color: String,
    pub level: i32,
    pub plus_tier: String,
    pub credits: i64,
    pub coins: i64,
    pub streak: i32,
    pub longest_streak: i32,
    pub item_sales: i32,
    pub networth: i64,
    pub status: String,
    pub created_at: String,
    pub last_seen: String,
    pub friends_count: i32,
    pub worlds_count: i32,
    #[serde(default)]
    pub inventory: Vec<String>,
    #[serde(default)]
    pub equipped: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedGame {
    pub place_id: String,
    pub name: String,
    pub description: String,
    pub creator_name: String,
    pub creator_id: u64,
    pub visits: u64,
    pub upvotes: u64,
    pub downvotes: u64,
    pub max_players: u32,
    pub server_port: u16,
    pub created_at: String,
    pub updated_at: String,
    pub world_data: Option<WorldFile>,
}

#[derive(Clone)]
pub struct AppState {
    pub users: Arc<RwLock<HashMap<String, UserAccount>>>,
    pub tokens: Arc<RwLock<HashMap<String, String>>>, // token -> username
    pub games: Arc<RwLock<HashMap<String, PublishedGame>>>,
    pub moderation: Arc<ModerationClient>,
}

impl AppState {
    pub fn new() -> Self {
        let mut users = HashMap::new();
        let mut tokens = HashMap::new();

        // Seed default player account
        let default_user = UserAccount {
            user_id: 1000,
            username: "PlayerOne".to_owned(),
            email: "player@noobietoria.com".to_owned(),
            password_hash: "sha256:password123".to_owned(),
            display_name: "PlayerOne".to_owned(),
            bio: "Welcome to Noobietoria! Building cool adventures.".to_owned(),
            avatar_emoji: "😎".to_owned(),
            body_color_hue: 205,
            banner_color: "#1e293b".to_owned(),
            level: 5,
            plus_tier: "Plus".to_owned(),
            credits: 250,
            coins: 850,
            streak: 7,
            longest_streak: 14,
            item_sales: 3,
            networth: 1100,
            status: "online".to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            last_seen: "Just now".to_owned(),
            friends_count: 12,
            worlds_count: 2,
            inventory: vec!["item_classic_cap".to_owned(), "item_sword".to_owned()],
            equipped: vec!["item_classic_cap".to_owned()],
        };

        tokens.insert("nb_jwt_1000".to_owned(), "PlayerOne".to_owned());
        tokens.insert("nb_token_demo".to_owned(), "PlayerOne".to_owned());
        users.insert("PlayerOne".to_owned(), default_user);

        // Default featured places
        let mut games = HashMap::new();
        let starter_file = WorldFile::default_starter_place();
        games.insert(
            "starter_baseplate".to_owned(),
            PublishedGame {
                place_id: "starter_baseplate".to_owned(),
                name: "Classic Baseplate".to_owned(),
                description: "The classic blank slate where everything begins.".to_owned(),
                creator_name: "Noobietoria".to_owned(),
                creator_id: 1,
                visits: 124500,
                upvotes: 8900,
                downvotes: 120,
                max_players: 50,
                server_port: 53640,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                updated_at: "2026-01-01T00:00:00Z".to_owned(),
                world_data: Some(starter_file),
            },
        );

        games.insert(
            "obby_challenge".to_owned(),
            PublishedGame {
                place_id: "obby_challenge".to_owned(),
                name: "Mega Easy Obby [100+ Stages]".to_owned(),
                description: "Jump, dodge lava bricks, collect speed pads and reach the finish!"
                    .to_owned(),
                creator_name: "ObbyKing2026".to_owned(),
                creator_id: 2,
                visits: 980200,
                upvotes: 45200,
                downvotes: 1100,
                max_players: 30,
                server_port: 53641,
                created_at: "2026-02-14T00:00:00Z".to_owned(),
                updated_at: "2026-03-01T00:00:00Z".to_owned(),
                world_data: None,
            },
        );

        games.insert(
            "natural_survival".to_owned(),
            PublishedGame {
                place_id: "natural_survival".to_owned(),
                name: "Natural Disaster Survival".to_owned(),
                description: "Survive floods, meteors, volcanoes and acid rain on an island!"
                    .to_owned(),
                creator_name: "SurvivalCraft".to_owned(),
                creator_id: 3,
                visits: 2150000,
                upvotes: 120000,
                downvotes: 3400,
                max_players: 40,
                server_port: 53642,
                created_at: "2026-01-10T00:00:00Z".to_owned(),
                updated_at: "2026-02-28T00:00:00Z".to_owned(),
                world_data: None,
            },
        );

        Self {
            users: Arc::new(RwLock::new(users)),
            tokens: Arc::new(RwLock::new(tokens)),
            games: Arc::new(RwLock::new(games)),
            moderation: Arc::new(ModerationClient::from_env()),
        }
    }
}
