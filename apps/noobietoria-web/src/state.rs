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
    #[serde(default = "default_studs")]
    pub studs: i64,
    #[serde(default)]
    pub last_studs_claim: Option<String>,
    #[serde(default)]
    pub avatar: Option<nb_types::AvatarConfig>,
}

fn default_studs() -> i64 {
    25
}

impl UserAccount {
    /// Attempts to claim the daily 5 studs reward.
    pub fn claim_daily_studs(&mut self) -> Result<i64, String> {
        let now_days = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() / 86400)
            .unwrap_or(0);
        let today_token = format!("day_{now_days}");
        if let Some(ref last) = self.last_studs_claim {
            if last == &today_token {
                return Err("Already claimed today's 5 studs. Come back tomorrow!".to_owned());
            }
        }
        self.studs += 5;
        self.last_studs_claim = Some(today_token);
        Ok(self.studs)
    }
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
    /// `true` when a dedicated server registered this endpoint through a
    /// heartbeat; `false` for the built-in placeholder fleet used when nothing
    /// is running, so clients can label it honestly.
    #[allow(dead_code)]
    pub live: bool,
}

/// A dedicated server announcing itself to the website.
///
/// Every running dedicated server posts this on a heartbeat; entries that stop
/// beating are dropped once they are older than [`REGISTRATION_TTL`].
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerRegistration {
    /// Unique per running server process.
    pub server_id: String,
    pub place_id: String,
    /// Advertised host. Falls back to the platform-wide host when omitted.
    #[serde(default)]
    pub host: Option<String>,
    pub port: u16,
    #[serde(default)]
    pub players: u32,
    pub max_players: u32,
    #[serde(default)]
    pub region: String,
}

/// How long a registration stays valid without a fresh heartbeat.
///
/// `allow(dead_code)`: the legacy `noobietoria-web-legacy` binary links this
/// module without the server-directory routes, so registry items look unused
/// there.
#[allow(dead_code)]
pub const REGISTRATION_TTL: std::time::Duration = std::time::Duration::from_secs(15);

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
    /// The placeholder servers of this place, used when no dedicated server has
    /// registered for it. The layout is derived deterministically from the place
    /// counters, so repeated calls always observe the same list.
    pub fn placeholder_servers(&self, host: &str) -> Vec<GameServer> {
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
                live: false,
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
    /// Live dedicated servers, keyed by `server_id`, with their last heartbeat.
    #[allow(dead_code)]
    pub registry: Arc<RwLock<HashMap<String, RegisteredServer>>>,
    /// Universal blob storage for places and assets (Local filesystem, S3, R2, MinIO).
    pub storage: Arc<nb_store::StorageManager>,
    /// Real SQL database backend (PostgreSQL, MySQL, CockroachDB, SQLite).
    pub db: Option<Arc<nb_store::SqlxBackend>>,
    /// Authentication service (Argon2 hashing, session tokens).
    pub auth: nb_auth::AuthService,
    /// SQLite-backed auth for production persistence.
    pub sql_auth: Option<nb_auth::SqliteAuthService>,
    /// PostgreSQL-backed auth for production persistence.
    pub pg_auth: Option<nb_auth::PostgresAuthService>,
}

/// A dedicated server that announced itself, plus when it last checked in.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RegisteredServer {
    pub registration: ServerRegistration,
    pub last_seen: std::time::Instant,
}

#[allow(dead_code)]
impl RegisteredServer {
    /// Whether this registration is still fresh enough to advertise.
    pub fn is_live(&self) -> bool {
        self.last_seen.elapsed() < REGISTRATION_TTL
    }
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
            studs: 50,
            last_studs_claim: None,
            avatar: Some(nb_types::AvatarConfig::classic_noob()),
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

        let storage = Arc::new(nb_store::StorageManager::from_env());
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://data/noobietoria_web.db?mode=rwc".to_owned());
        let _ = std::fs::create_dir_all("data");
        let db = nb_store::SqlxBackend::connect(&db_url, "nb_web_store").ok().map(Arc::new);

        // Restore stored users and games from DB if available
        if let Some(ref d) = db {
            use nb_store::KvBackend;
            if let Ok(keys) = d.keys() {
                for key in keys {
                    if let Some(username) = key.strip_prefix("user:") {
                        if let Ok(Some(val)) = d.get(&key) {
                            if let Ok(user) = serde_json::from_value::<UserAccount>(serde_json::to_value(&val).unwrap_or_default()) {
                                tokens.insert(format!("nb_jwt_{}", user.user_id), username.to_owned());
                                users.insert(username.to_owned(), user);
                            }
                        }
                    } else if let Some(place_id) = key.strip_prefix("game:") {
                        if let Ok(Some(val)) = d.get(&key) {
                            if let Ok(game) = serde_json::from_value::<PublishedGame>(serde_json::to_value(&val).unwrap_or_default()) {
                                games.insert(place_id.to_owned(), game);
                            }
                        }
                    }
                }
            }
        }

        Self {
            users: Arc::new(RwLock::new(users)),
            tokens: Arc::new(RwLock::new(tokens)),
            games: Arc::new(RwLock::new(games)),
            moderation: Arc::new(ModerationClient::from_env()),
            server_host: Arc::new(server_host()),
            registry: Arc::new(RwLock::new(HashMap::new())),
            storage,
            db,
            auth: nb_auth::AuthService::new(),
            sql_auth: None,
            pg_auth: None,
        }
    }

    /// Build a new AppState with the auth backend selected by the environment.
    ///
    /// `NOOBIE_AUTH_SCHEME` selects the backend: `sqlite` (default) or
    /// `postgres`. `NOOBIE_AUTH_DB` supplies the connection URL.
    #[allow(dead_code)]
    pub async fn new_with_auth() -> Self {
        let scheme = std::env::var("NOOBIE_AUTH_SCHEME")
            .unwrap_or_else(|_| "sqlite".to_owned())
            .to_lowercase();
        let db_url = std::env::var("NOOBIE_AUTH_DB")
            .unwrap_or_else(|_| "sqlite://data/noobietoria_auth.db?mode=rwc".to_owned());

        if scheme == "postgres" || scheme == "postgresql" {
            Self::new().with_pg_auth(&db_url).await
        } else {
            Self::new().with_sql_auth(&db_url).await
        }
    }

    /// Attach a SQLite-backed auth service for durable production persistence.

    /// Attach a SQLite-backed auth service for durable production persistence.
    ///
    /// Call this after [`AppState::new()`] with the URL of a SQLite database
    /// (e.g. `sqlite://data/noobietoria_auth.db?mode=rwc`).
    pub async fn with_sql_auth(mut self, database_url: &str) -> Self {
        match nb_auth::SqliteAuthService::connect(database_url).await {
            Ok(svc) => {
                tracing::info!("SQLite auth service connected: {database_url}");
                // Seed the demo account so logins that worked in-memory keep
                // working against the durable backend (idempotent: ignored
                // when the user already exists from a previous run).
                let _ = svc
                    .register("PlayerOne", "player@noobietoria.com", "password123")
                    .await;
                self.sql_auth = Some(svc);
            }
            Err(e) => {
                tracing::warn!("SQLite auth unavailable ({e}); falling back to in-memory");
            }
        }
        self
    }

    /// Attach a PostgreSQL-backed auth service for durable production persistence.
    ///
    /// Call this after [`AppState::new()`] with the connection string of a
    /// PostgreSQL database (e.g. `postgres://user:pass@localhost/noobietoria`).
    /// The service must already contain the required tables (run the
    /// migrations from `nb-auth` separately if needed).
    pub async fn with_pg_auth(mut self, database_url: &str) -> Self {
        match nb_auth::PostgresAuthService::connect(database_url).await {
            Ok(svc) => {
                tracing::info!("PostgreSQL auth service connected: {database_url}");
                let _ = svc
                    .register("PlayerOne", "player@noobietoria.com", "password123")
                    .await;
                self.pg_auth = Some(svc);
            }
            Err(e) => {
                tracing::warn!(
                    "PostgreSQL auth unavailable ({e}); falling back to in-memory"
                );
            }
        }
        self
    }

    /// Register via SQLite or PostgreSQL when available, falling back to
    /// in-memory. Priority: SQLite > PostgreSQL > in-memory.
    pub async fn auth_register(
        &self,
        username: &str,
        email: &str,
        password: &str,
    ) -> Result<nb_auth::UserRecord, nb_auth::AuthError> {
        if let Some(ref sql) = self.sql_auth {
            sql.register(username, email, password).await
        } else if let Some(ref pg) = self.pg_auth {
            pg.register(username, email, password).await
        } else {
            self.auth.register(username, email, password)
        }
    }

    /// Login via SQLite or PostgreSQL when available, falling back to
    /// in-memory. Priority: SQLite > PostgreSQL > in-memory.
    pub async fn auth_login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<String, nb_auth::AuthError> {
        if let Some(ref sql) = self.sql_auth {
            sql.login(username, password).await
        } else if let Some(ref pg) = self.pg_auth {
            pg.login(username, password).await
        } else {
            self.auth.login(username, password)
        }
    }

    /// Validate a session token via the first available durable backend,
    /// falling back to in-memory. Priority: SQLite > PostgreSQL > in-memory.
    pub async fn auth_validate(
        &self,
        token: &str,
    ) -> Result<nb_auth::UserRecord, nb_auth::AuthError> {
        if let Some(ref sql) = self.sql_auth {
            sql.validate(token).await
        } else if let Some(ref pg) = self.pg_auth {
            pg.validate(token).await
        } else {
            self.auth.validate(token)
        }
    }

    /// Revoke a session token via the first available durable backend,
    /// falling back to in-memory. Priority: SQLite > PostgreSQL > in-memory.
    pub async fn auth_logout(&self, token: &str) {
        if let Some(ref sql) = self.sql_auth {
            let _ = sql.logout(token).await;
        } else if let Some(ref pg) = self.pg_auth {
            let _ = pg.logout(token).await;
        } else {
            self.auth.logout(token);
        }
    }

    /// Persists a user account to the real database backend if configured.
    pub fn persist_user(&self, user: &UserAccount) {
        if let Some(ref db) = self.db {
            use nb_store::KvBackend;
            if let Ok(val) = serde_json::to_value(user) {
                if let Ok(nb_val) = serde_json::from_value::<nb_types::Value>(val) {
                    let _ = db.set(&format!("user:{}", user.username), &nb_val);
                }
            }
        }
    }

    /// Persists a published game to the real database backend if configured.
    pub fn persist_game(&self, game: &PublishedGame) {
        if let Some(ref db) = self.db {
            use nb_store::KvBackend;
            if let Ok(val) = serde_json::to_value(game) {
                if let Ok(nb_val) = serde_json::from_value::<nb_types::Value>(val) {
                    let _ = db.set(&format!("game:{}", game.place_id), &nb_val);
                }
            }
        }
    }

    // ── Dedicated server registry ────────────────────────────────────────────

    /// Records (or refreshes) a dedicated server's heartbeat and returns the
    /// advertised endpoint.
    #[allow(dead_code)]
    pub fn register_server(&self, registration: ServerRegistration) -> GameServer {
        let game = self.games.read().get(&registration.place_id).cloned();
        let host = registration
            .host
            .clone()
            .filter(|h| !h.trim().is_empty())
            .unwrap_or_else(|| self.server_host.to_string());
        let players = registration.players.min(registration.max_players);

        let server = GameServer {
            server_id: registration.server_id.clone(),
            place_id: registration.place_id.clone(),
            place_name: game.as_ref().map(|g| g.name.clone()).unwrap_or_default(),
            host,
            port: registration.port,
            players,
            max_players: registration.max_players,
            region: if registration.region.trim().is_empty() {
                "Unknown region".to_owned()
            } else {
                registration.region.clone()
            },
            // A real heartbeat carries no latency figure; the client shows "—".
            ping_ms: 0,
            status: if players >= registration.max_players {
                "full".to_owned()
            } else {
                "online".to_owned()
            },
            live: true,
        };

        self.registry.write().insert(
            registration.server_id.clone(),
            RegisteredServer {
                registration,
                last_seen: std::time::Instant::now(),
            },
        );
        server
    }

    /// Drops registrations whose last heartbeat is too old to trust.
    #[allow(dead_code)]
    fn expire_registrations(&self) {
        self.registry.write().retain(|_, s| s.is_live());
    }

    /// The live servers of one place, newest registration first.
    #[allow(dead_code)]
    fn live_servers_of(&self, place_id: &str) -> Vec<GameServer> {
        let mut live: Vec<GameServer> = self
            .registry
            .read()
            .values()
            .filter(|s| s.is_live() && s.registration.place_id == place_id)
            .map(|s| {
                let r = &s.registration;
                let players = r.players.min(r.max_players);
                GameServer {
                    server_id: r.server_id.clone(),
                    place_id: r.place_id.clone(),
                    place_name: String::new(),
                    host: r
                        .host
                        .clone()
                        .filter(|h| !h.trim().is_empty())
                        .unwrap_or_else(|| self.server_host.to_string()),
                    port: r.port,
                    players,
                    max_players: r.max_players,
                    region: if r.region.trim().is_empty() {
                        "Unknown region".to_owned()
                    } else {
                        r.region.clone()
                    },
                    ping_ms: 0,
                    status: if players >= r.max_players {
                        "full".to_owned()
                    } else {
                        "online".to_owned()
                    },
                    live: true,
                }
            })
            .collect();
        live.sort_by(|a, b| a.server_id.cmp(&b.server_id));
        live
    }

    /// Every joinable server of every published place.
    ///
    /// Live dedicated servers take precedence: a place with at least one fresh
    /// registration advertises only those. Places with no live server fall back
    /// to the deterministic placeholder fleet so the UI still has content.
    #[allow(dead_code)]
    pub fn all_servers(&self) -> Vec<GameServer> {
        self.expire_registrations();
        let place_ids: Vec<String> = self.games.read().keys().cloned().collect();

        let mut list = Vec::new();
        for place_id in place_ids {
            list.extend(self.servers_of(&place_id).unwrap_or_default());
        }
        list.sort_by(|a, b| a.server_id.cmp(&b.server_id));
        list
    }

    /// The joinable servers of one place, or `None` when the place is unknown.
    #[allow(dead_code)]
    pub fn servers_of(&self, place_id: &str) -> Option<Vec<GameServer>> {
        let game = self.games.read().get(place_id).cloned()?;

        let live = self.live_servers_of(place_id);
        if !live.is_empty() {
            return Some(
                live.into_iter()
                    .map(|mut s| {
                        s.place_name = game.name.clone();
                        s
                    })
                    .collect(),
            );
        }

        Some(game.placeholder_servers(&self.server_host))
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
            game.placeholder_servers(DEFAULT_SERVER_HOST).len(),
            (visits % 3 + 1) as usize
        );
        state.games.write().insert(place_id.to_owned(), game);
        state
    }

    fn registration(
        server_id: &str,
        place_id: &str,
        port: u16,
        players: u32,
    ) -> ServerRegistration {
        ServerRegistration {
            server_id: server_id.to_owned(),
            place_id: place_id.to_owned(),
            host: None,
            port,
            players,
            max_players: 20,
            region: "Singapore".to_owned(),
        }
    }

    #[test]
    fn placeholder_fleet_is_deterministic_and_uses_configured_host() {
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
            assert!(!s.live, "placeholder servers are not live");
        }
    }

    /// A real heartbeat must replace the placeholder fleet for that place, and
    /// carry the endpoint the dedicated server actually listens on.
    #[test]
    fn live_registration_overrides_the_placeholder_fleet() {
        let state = state_with_place("p1", 4, 20, 7000);
        assert_eq!(
            state.servers_of("p1").unwrap().len(),
            2,
            "placeholder first"
        );

        let advertised = state.register_server(ServerRegistration {
            server_id: "p1:live-1".to_owned(),
            place_id: "p1".to_owned(),
            host: Some("game.example.test".to_owned()),
            port: 9100,
            players: 7,
            max_players: 20,
            region: "Tokyo".to_owned(),
        });
        assert!(advertised.live);
        assert_eq!(advertised.host, "game.example.test");
        assert_eq!(advertised.port, 9100);
        assert_eq!(advertised.place_name, "Test Place");

        let live = state.servers_of("p1").expect("place exists");
        assert_eq!(live.len(), 1, "only the registered server is advertised");
        assert_eq!(live[0].server_id, "p1:live-1");
        assert_eq!(live[0].host, "game.example.test");
        assert_eq!(live[0].port, 9100);
        assert_eq!(live[0].region, "Tokyo");
        assert_eq!(live[0].status, "online");
        assert!(live[0].live);
    }

    /// A full server is advertised as full so the client disables Join.
    #[test]
    fn registration_marks_full_and_clamps_occupancy() {
        let state = state_with_place("p1", 1, 20, 7000);
        // players above capacity is clamped rather than trusted.
        state.register_server(registration("p1:over", "p1", 9101, 99));

        let servers = state.servers_of("p1").unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].players, 20);
        assert_eq!(servers[0].status, "full");
    }

    /// Unknown places still have a placeholder fleet, and unregistered places
    /// keep theirs even when another place has a live server.
    #[test]
    fn unknown_place_has_no_servers_and_live_servers_are_scoped() {
        let state = state_with_place("p1", 1, 20, 7000);
        state.register_server(registration("p1:live", "p1", 9100, 0));

        // A second place, with no live server of its own.
        state.games.write().insert(
            "p2".to_owned(),
            PublishedGame {
                place_id: "p2".to_owned(),
                name: "Other Place".to_owned(),
                description: String::new(),
                creator_name: "Tester".to_owned(),
                creator_id: 1,
                visits: 1,
                upvotes: 0,
                downvotes: 0,
                max_players: 20,
                server_port: 7100,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                updated_at: "2026-01-01T00:00:00Z".to_owned(),
                world_data: None,
            },
        );

        assert!(state.servers_of("does_not_exist").is_none());

        let p2 = state.servers_of("p2").unwrap();
        assert!(
            p2.iter().all(|s| !s.live),
            "p2 must not inherit p1's live server"
        );
        assert!(p2.iter().all(|s| s.place_id == "p2"));

        let all = state.all_servers();
        assert!(all.iter().any(|s| s.server_id == "p1:live"));
        assert!(all.iter().any(|s| s.place_id == "p2"));
    }

    /// Re-registering the same server id refreshes it instead of duplicating it.
    #[test]
    fn heartbeat_refreshes_instead_of_duplicating() {
        let state = state_with_place("p1", 1, 20, 7000);
        state.register_server(registration("p1:live", "p1", 9100, 3));
        state.register_server(registration("p1:live", "p1", 9100, 9));

        let servers = state.servers_of("p1").unwrap();
        assert_eq!(servers.len(), 1, "the heartbeat must not duplicate the row");
        assert_eq!(servers[0].players, 9, "the latest heartbeat wins");
        assert_eq!(state.registry.read().len(), 1);
    }

    /// With no live servers at all, the catalog still has a placeholder fleet.
    #[test]
    fn catalog_without_live_servers_still_has_placeholders() {
        let state = AppState::new();
        let all = state.all_servers();
        let games = state.games.read();
        let expected: usize = games.values().map(|g| (g.visits % 3 + 1) as usize).sum();
        assert_eq!(all.len(), expected);
        assert!(all.iter().all(|s| games.contains_key(&s.place_id)));
        assert!(all.iter().all(|s| !s.live));
    }
}
