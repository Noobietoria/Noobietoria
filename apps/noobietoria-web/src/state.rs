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

/// One joinable game server of a published place.
///
/// The web backend is the single source of truth for join endpoints: the
/// desktop client never asks the player for a host or a port, it lists the
/// servers of a place here and connects to the one that was picked.
/// One joinable game server of a published place.
///
/// The web backend is the single source of truth for join endpoints: the
/// desktop client never asks the player for a host or a port, it lists the
/// servers of a place here and connects to the one that was picked.
///
/// `allow(dead_code)`: the legacy `noobietoria-web-legacy` binary links this
/// module without the server-directory routes, so these items look unused there.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameServer {
    /// Stable id, also usable as a shareable join code (`<place_id>:<index>`).
    pub server_id: String,
    pub place_id: String,
    pub place_name: String,
    /// Hostname or IP the client should dial.
    pub host: String,
    pub port: u16,
    pub players: u32,
    pub max_players: u32,
    pub region: String,
    pub ping_ms: u32,
    /// `"online"` | `"full"` | `"offline"`.
    pub status: String,
}

/// Hostname handed out to clients when no override is configured.
pub const DEFAULT_SERVER_HOST: &str = "127.0.0.1";

/// Hostname the clients should dial. Overridable with `NOOBIE_SERVER_HOST`
/// (e.g. the public address of the dedicated fleet).
pub fn server_host() -> String {
    std::env::var("NOOBIE_SERVER_HOST")
        .ok()
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| DEFAULT_SERVER_HOST.to_owned())
}

/// Region labels used for the deterministic demo fleet.
#[allow(dead_code)]
const SERVER_REGIONS: [&str; 4] = ["Singapore", "Tokyo", "US East", "Frankfurt"];

#[allow(dead_code)]
impl PublishedGame {
    /// The joinable servers of this place.
    ///
    /// The fleet layout is derived deterministically from the place counters,
    /// so repeated calls (and the self-test) always observe the same list.
    pub fn servers(&self, host: &str) -> Vec<GameServer> {
        let count = (self.visits % 3) + 1;
        let mut servers = Vec::with_capacity(count as usize);

        for i in 0..count {
            let players = ((self.visits + i * 7) % self.max_players.max(1) as u64) as u32;
            let status = if players >= self.max_players {
                "full"
            } else {
                "online"
            };

            servers.push(GameServer {
                server_id: format!("{}:{}", self.place_id, i + 1),
                place_id: self.place_id.clone(),
                place_name: self.name.clone(),
                host: host.to_owned(),
                port: self.server_port + i as u16,
                players,
                max_players: self.max_players,
                region: SERVER_REGIONS[(i as usize) % SERVER_REGIONS.len()].to_owned(),
                ping_ms: 30 + ((self.visits + i * 13) % 90) as u32,
                status: status.to_owned(),
            });
        }

        servers
    }
}

#[derive(Clone)]
pub struct AppState {
    pub users: Arc<RwLock<HashMap<String, UserAccount>>>,
    pub tokens: Arc<RwLock<HashMap<String, String>>>, // token -> username
    pub games: Arc<RwLock<HashMap<String, PublishedGame>>>,
    pub moderation: Arc<ModerationClient>,
    /// Hostname advertised to clients in the server list.
    #[allow(dead_code)]
    pub server_host: Arc<String>,
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
            server_host: Arc::new(server_host()),
        }
    }

    /// Every joinable server of every published place.
    #[allow(dead_code)]
    pub fn all_servers(&self) -> Vec<GameServer> {
        let games = self.games.read();
        let mut list: Vec<GameServer> = games
            .values()
            .flat_map(|g| g.servers(&self.server_host))
            .collect();
        list.sort_by(|a, b| a.server_id.cmp(&b.server_id));
        list
    }

    /// The joinable servers of one place, or `None` when the place is unknown.
    #[allow(dead_code)]
    pub fn servers_of(&self, place_id: &str) -> Option<Vec<GameServer>> {
        let games = self.games.read();
        games.get(place_id).map(|g| g.servers(&self.server_host))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with_place(place_id: &str, visits: u64, max_players: u32, port: u16) -> AppState {
        let state = AppState::new();
        let game = PublishedGame {
            place_id: place_id.to_owned(),
            name: "Test Place".to_owned(),
            description: String::new(),
            creator_name: "Tester".to_owned(),
            creator_id: 1,
            visits,
            upvotes: 0,
            downvotes: 0,
            max_players,
            server_port: port,
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
            world_data: None,
        };
        assert_eq!(
            game.servers(DEFAULT_SERVER_HOST).len(),
            (visits % 3 + 1) as usize
        );
        state.games.write().insert(place_id.to_owned(), game);
        state
    }

    #[test]
    fn servers_are_deterministic_and_use_configured_host() {
        let state = state_with_place("p1", 4, 20, 7000);
        let a = state.servers_of("p1").expect("place exists");
        let b = state.servers_of("p1").expect("place exists");

        // visits = 4 -> 2 servers on consecutive ports.
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].server_id, "p1:1");
        assert_eq!(a[1].server_id, "p1:2");
        assert_eq!(a[0].port, 7000);
        assert_eq!(a[1].port, 7001);
        assert_eq!(a, b, "server list must be stable across calls");

        for s in &a {
            assert_eq!(s.host, DEFAULT_SERVER_HOST);
            assert_eq!(s.place_id, "p1");
            assert!(s.players <= s.max_players);
            assert!(s.status == "online" || s.status == "full");
        }
    }

    #[test]
    fn unknown_place_has_no_servers_and_all_servers_covers_catalog() {
        let state = AppState::new();
        assert!(state.servers_of("does_not_exist").is_none());

        let all = state.all_servers();
        let games = state.games.read();
        let expected: usize = games.values().map(|g| (g.visits % 3 + 1) as usize).sum();
        assert_eq!(all.len(), expected);
        assert!(all.iter().all(|s| games.contains_key(&s.place_id)));
    }
}
