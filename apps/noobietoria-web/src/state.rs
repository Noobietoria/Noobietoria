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
    pub password_hash: String,
    pub display_name: String,
    pub coins: u64,
    pub created_at: String,
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
    pub games: Arc<RwLock<HashMap<String, PublishedGame>>>,
    pub moderation: Arc<ModerationClient>,
}

impl AppState {
    pub fn new() -> Self {
        let mut games = HashMap::new();

        // Default featured Roblox-style places
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
            users: Arc::new(RwLock::new(HashMap::new())),
            games: Arc::new(RwLock::new(games)),
            moderation: Arc::new(ModerationClient::from_env()),
        }
    }
}
