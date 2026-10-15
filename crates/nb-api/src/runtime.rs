//! The runtime state every service function reads and writes.
//!
//! A service function is a pure-ish operation over [`ApiContext`]: it sees the
//! world, the stores, the players and the side it is running on, and it returns
//! a [`Value`]. Keeping all mutable state in one place is what lets the Dedicated
//! server be authoritative — the client gets a context whose world is the
//! replicated snapshot, so it physically cannot read server-only state.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use nb_store::{DataStoreService, KeyedStore};
use nb_types::{Rgba, Value, Vector3};
use nb_world::World;
use parking_lot::{Mutex, RwLock};

use crate::error::{ApiError, ApiResult, Side};

/// One connected player, as `PlayerService` reports them.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerRecord {
    /// Display name, the `Username` most services take.
    pub username: String,
    /// Numeric account id.
    pub user_id: u64,
    /// Country code as reported to the game.
    pub country: String,
    /// The player's current world position.
    pub position: Vector3,
    /// The player's current look direction.
    pub look_at: Vector3,
    /// Asset id of the avatar, if the website supplied one.
    pub avatar_id: String,
    /// Whether the player is currently connected.
    pub online: bool,
}

impl PlayerRecord {
    /// A fresh record for a joining player.
    pub fn new(username: impl Into<String>, user_id: u64, country: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            user_id,
            country: country.into(),
            position: Vector3::ZERO,
            look_at: Vector3::new(0.0, 0.0, -1.0),
            avatar_id: String::new(),
            online: true,
        }
    }
}

/// Everyone who is connected, plus the id allocator the website shares.
#[derive(Debug, Default)]
pub struct PlayerRegistry {
    players: BTreeMap<String, PlayerRecord>,
    next_user_id: AtomicU64,
}

impl PlayerRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self {
            players: BTreeMap::new(),
            next_user_id: AtomicU64::new(1),
        }
    }

    /// Allocates the next account id.
    pub fn allocate_user_id(&self) -> u64 {
        self.next_user_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Adds a player, replacing any existing record with the same name.
    pub fn join(&mut self, record: PlayerRecord) {
        let id = record.user_id;
        self.next_user_id.fetch_max(id + 1, Ordering::SeqCst);
        self.players.insert(record.username.clone(), record);
    }

    /// Marks a player as gone.
    ///
    /// The record is kept but flagged offline, because services such as
    /// `BanService.GetBanHistory` and `PurchaseService.GetOrderHistory` still
    /// need to resolve a name after the player has left.
    pub fn leave(&mut self, username: &str) -> Option<PlayerRecord> {
        let record = self.players.get_mut(username)?;
        record.online = false;
        Some(record.clone())
    }

    /// Looks a player up by name.
    pub fn get(&self, username: &str) -> Option<&PlayerRecord> {
        self.players.get(username)
    }

    /// Looks a player up by name, mutably.
    pub fn get_mut(&mut self, username: &str) -> Option<&mut PlayerRecord> {
        self.players.get_mut(username)
    }

    /// Resolves a name from a numeric id.
    pub fn name_of(&self, user_id: u64) -> Option<&str> {
        self.players
            .values()
            .find(|p| p.user_id == user_id)
            .map(|p| p.username.as_str())
    }

    /// Resolves a numeric id from a name.
    pub fn id_of(&self, username: &str) -> Option<u64> {
        self.players.get(username).map(|p| p.user_id)
    }

    /// Every player, online or not.
    pub fn all(&self) -> Vec<PlayerRecord> {
        self.players.values().cloned().collect()
    }

    /// Only connected players.
    pub fn online(&self) -> Vec<PlayerRecord> {
        self.players
            .values()
            .filter(|p| p.online)
            .cloned()
            .collect()
    }

    /// Names of connected players.
    pub fn online_names(&self) -> Vec<String> {
        self.players
            .values()
            .filter(|p| p.online)
            .map(|p| p.username.clone())
            .collect()
    }

    /// Number of connected players, which is the `CCU` the budget scales with.
    pub fn ccu(&self) -> u32 {
        self.players.values().filter(|p| p.online).count() as u32
    }

    /// Whether a name is known at all.
    pub fn contains(&self, username: &str) -> bool {
        self.players.contains_key(username)
    }
}

/// A queued outbound network message.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingEvent {
    /// The `NetworkEvent` name.
    pub event: String,
    /// The recipient, or `"*"` for every client.
    pub to: String,
    /// Payload arguments.
    pub args: Vec<Value>,
    /// Whether this is an invoke expecting a reply.
    pub invoke: bool,
}

/// A registered `NetworkEvent` handler.
#[derive(Clone, Debug, PartialEq)]
pub struct EventHandler {
    /// The event name.
    pub event: String,
    /// Which side registered it.
    pub side: Side,
}

/// A registered `OnServerInvoke` handler.
///
/// Called with the calling player's name and the payload, and must return the
/// value the client receives. A `Fn` rather than a `FnOnce` because an invoke
/// may be called many times before it is disconnected.
#[derive(Clone)]
pub struct InvokeHandler(Arc<dyn Fn(&str, &[Value]) -> ApiResult<Value> + Send + Sync>);

impl InvokeHandler {
    /// Wraps a closure as an invoke handler.
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&str, &[Value]) -> ApiResult<Value> + Send + Sync + 'static,
    {
        Self(Arc::new(f))
    }

    /// Calls the handler.
    pub fn call(&self, caller: &str, payload: &[Value]) -> ApiResult<Value> {
        (self.0)(caller, payload)
    }
}

impl std::fmt::Debug for InvokeHandler {
    /// `Arc<dyn Fn>` is not `Debug`, so the runtime names the handler instead of
    /// dumping a pointer.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InvokeHandler(")?;
        f.write_str("..)")
    }
}

/// A screen notification.
#[derive(Clone, Debug, PartialEq)]
pub struct Notification {
    /// Monotonic id, returned so it can be dismissed.
    pub id: u64,
    /// The recipient.
    pub to: String,
    /// Title line.
    pub title: String,
    /// Body text.
    pub message: String,
    /// `Toast`, `Popup` or `Banner`.
    pub kind: String,
    /// Seconds to display.
    pub duration: f64,
    /// Whether it has been dismissed.
    pub dismissed: bool,
}

/// A camera configuration for one player.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraState {
    /// `Custom`, `Follow`, `Scriptable`, ...
    pub kind: String,
    /// World position.
    pub position: Vector3,
    /// Look direction.
    pub look_at: Vector3,
    /// Field of view in degrees.
    pub fov: f64,
    /// Instance the camera is locked to, if any.
    pub locked_to: Option<String>,
    /// Remaining shake intensity.
    pub shake: f64,
    /// Remaining shake time in seconds.
    pub shake_until: f64,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            kind: "Custom".to_owned(),
            position: Vector3::new(0.0, 5.0, 15.0),
            look_at: Vector3::ZERO,
            fov: 70.0,
            locked_to: None,
            shake: 0.0,
            shake_until: 0.0,
        }
    }
}

/// A UI element created through `UIService`.
///
/// The GUI lives here rather than in the world because `UIService` addresses
/// elements by a global `Name` and a `Parent` name, not by GUID.
#[derive(Clone, Debug, PartialEq)]
pub struct UiElement {
    /// Registered name.
    pub name: String,
    /// `Frame`, `TextLabel`, `TextButton`, `ImageLabel`, `ImageButton`,
    /// `TextBox` or `ScrollingFrame`.
    pub kind: String,
    /// Parent element name, or `"ScreenGui"` for a root.
    pub parent: String,
    /// Current property values.
    pub properties: nb_world::InstanceProperties,
    /// Whether an click/text-changed callback is registered.
    pub has_callback: bool,
}

/// A registered `TweenService` tween.
#[derive(Clone, Debug, PartialEq)]
pub struct Tween {
    /// The instance being moved.
    pub instance: String,
    /// Where it started.
    pub start: Vector3,
    /// Where it is going.
    pub end: Vector3,
    /// Total duration in seconds.
    pub duration: f64,
    /// Seconds elapsed.
    pub elapsed: f64,
    /// Whether it repeats.
    pub looping: bool,
    /// Whether it has been paused.
    pub paused: bool,
    /// Whether it has been stopped or has finished.
    pub finished: bool,
    /// Colour to interpolate to, when supplied.
    pub target_color: Option<Rgba>,
}

/// A sound effect playing in the world.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundEmitter {
    /// The asset id, which is also the handle the API addresses it by.
    pub id: String,
    /// World position.
    pub position: Vector3,
    /// Instance it is attached to, if any.
    pub attached_to: Option<String>,
    /// Volume.
    pub volume: f64,
    /// Distance at which it fades out.
    pub rolloff: f64,
    /// Whether it is still sounding.
    pub playing: bool,
}

/// A visual effect instance.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectInstance {
    /// The effect asset id, used as the handle.
    pub id: String,
    /// World position.
    pub position: Vector3,
    /// Instance it is attached to.
    pub attached_to: Option<String>,
    /// Free-form parameters passed by the caller.
    pub properties: Value,
    /// Whether it is still emitting.
    pub emitting: bool,
}

/// A screen-wide effect applied to one player.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenEffect {
    /// The player.
    pub username: String,
    /// e.g. `Blur`, `ColorCorrection`, `Fade`.
    pub kind: String,
    /// Parameters.
    pub properties: Value,
    /// Total duration.
    pub duration: f64,
    /// Seconds elapsed.
    pub elapsed: f64,
}

/// An NPC pathing agent.
#[derive(Clone, Debug, PartialEq)]
pub struct PathAgent {
    /// The NPC name.
    pub npc: String,
    /// Remaining waypoints.
    pub waypoints: Vec<Vector3>,
    /// Where it is heading.
    pub destination: Vector3,
    /// `Idle`, `Moving`, `Paused` or `Blocked`.
    pub state: String,
    /// Radius used for the path query.
    pub radius: f64,
    /// Height used for the path query.
    pub height: f64,
}

/// A proximity prompt.
#[derive(Clone, Debug, PartialEq)]
pub struct ProximityPrompt {
    /// Registered id.
    pub id: String,
    /// Instance it is attached to.
    pub instance: String,
    /// The label shown.
    pub label: String,
    /// Key that activates it.
    pub key_code: String,
    /// Maximum interaction distance.
    pub max_distance: f64,
    /// Seconds the key must be held.
    pub hold_duration: f64,
    /// Whether it is drawn at all.
    pub visible: bool,
    /// Whether it can be triggered.
    pub enabled: bool,
}

/// A spawn point.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnPoint {
    /// Registered name.
    pub name: String,
    /// World position.
    pub position: Vector3,
    /// Assigned players and NPCs.
    pub assigned: BTreeSet<String>,
}

/// A collision group.
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionGroup {
    /// Group name.
    pub name: String,
    /// Instances assigned to it.
    pub members: BTreeSet<String>,
    /// Groups this one does not collide with.
    pub ignores: BTreeSet<String>,
}

/// A physics body's runtime state.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct BodyState {
    /// Current velocity.
    pub velocity: Vector3,
    /// Active force and the seconds it has left.
    pub force: Option<(Vector3, f64)>,
    /// Whether the body is anchored in place.
    pub anchored: bool,
    /// Mass in kilograms.
    pub mass: f64,
    /// Friction coefficient.
    pub friction: f64,
    /// Bounciness.
    pub elasticity: f64,
}

/// A leaderboard.
#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    /// Registered name.
    pub name: String,
    /// `Ascending` or `Descending`.
    pub order: String,
    /// Username to score.
    pub scores: BTreeMap<String, f64>,
}

/// A matchmaking queue.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchQueue {
    /// Registered name.
    pub name: String,
    /// Maximum players per room.
    pub max_players: usize,
    /// Players waiting, in join order.
    pub waiting: VecDeque<String>,
    /// Rooms created from this queue.
    pub rooms: BTreeSet<String>,
}

/// A matchmaking room.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchRoom {
    /// Room id.
    pub id: String,
    /// The queue it came from.
    pub queue: String,
    /// Players in the room.
    pub players: Vec<String>,
    /// Whether the room has been closed.
    pub open: bool,
}

/// A party.
#[derive(Clone, Debug, PartialEq)]
pub struct Party {
    /// Party id.
    pub id: String,
    /// Current leader.
    pub leader: String,
    /// Members.
    pub members: BTreeSet<String>,
    /// Maximum size.
    pub max_size: usize,
    /// Outstanding invitations.
    pub invites: BTreeSet<String>,
}

/// An in-progress trade.
#[derive(Clone, Debug, PartialEq)]
pub struct Trade {
    /// Trade id.
    pub id: String,
    /// The two participants, in order.
    pub parties: (String, String),
    /// Each side's offer.
    pub offers: BTreeMap<String, Value>,
    /// Who has confirmed.
    pub confirmed: BTreeSet<String>,
    /// `Open`, `Completed` or `Cancelled`.
    pub status: String,
}

/// A mail message.
#[derive(Clone, Debug, PartialEq)]
pub struct Mail {
    /// Mail id.
    pub id: String,
    /// Sender.
    pub from: String,
    /// Recipient.
    pub to: String,
    /// Subject line.
    pub subject: String,
    /// Body text.
    pub body: String,
    /// Attached items or currency.
    pub attachments: Value,
    /// Whether the recipient has read it.
    pub read: bool,
    /// Whether the attachments were claimed.
    pub claimed: bool,
}

/// A registered crafting recipe.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    /// Recipe id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Required ingredients.
    pub ingredients: Value,
    /// Produced item.
    pub output: Value,
}

/// A registered quest.
#[derive(Clone, Debug, PartialEq)]
pub struct Quest {
    /// Quest id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Description text.
    pub description: String,
    /// Step definitions.
    pub steps: Value,
    /// Rewards paid on completion.
    pub rewards: Value,
}

/// A registered achievement.
#[derive(Clone, Debug, PartialEq)]
pub struct Achievement {
    /// Achievement id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Description text.
    pub description: String,
    /// Progress required to unlock.
    pub max_progress: f64,
}

/// A registered badge.
#[derive(Clone, Debug, PartialEq)]
pub struct Badge {
    /// Badge id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Description text.
    pub description: String,
}

/// A registered pass, equivalent to a Roblox gamepass.
#[derive(Clone, Debug, PartialEq)]
pub struct Pass {
    /// Pass id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Description text.
    pub description: String,
    /// Permissions it grants.
    pub permissions: BTreeSet<String>,
    /// Players who own it.
    pub owners: BTreeSet<String>,
}

/// A marketplace listing.
#[derive(Clone, Debug, PartialEq)]
pub struct Listing {
    /// Listing id.
    pub id: String,
    /// Seller username.
    pub seller: String,
    /// Item id.
    pub item_id: String,
    /// Unit price.
    pub price: f64,
    /// Currency the price is in.
    pub currency: String,
    /// Units available.
    pub quantity: f64,
    /// Whether it is still on sale.
    pub active: bool,
}

/// A purchase or trade order.
#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    /// Order id.
    pub id: String,
    /// Buyer.
    pub username: String,
    /// Item bought.
    pub item_id: String,
    /// Units.
    pub amount: f64,
    /// Currency used.
    pub currency: String,
    /// `Pending`, `Confirmed`, `Cancelled` or `Refunded`.
    pub status: String,
    /// A reference to the platform receipt, for platform purchases.
    pub receipt: Option<String>,
}

/// A teleport zone.
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    /// Zone name.
    pub name: String,
    /// The server hosting it.
    pub server_id: String,
    /// Where a player lands.
    pub spawn: Vector3,
}

/// A dialogue script.
#[derive(Clone, Debug, PartialEq)]
pub struct Dialogue {
    /// Dialogue id.
    pub id: String,
    /// The NPC it belongs to.
    pub npc: String,
    /// Node definitions.
    pub nodes: Value,
}

/// Where a player is inside a dialogue.
#[derive(Clone, Debug, PartialEq)]
pub struct DialogueSession {
    /// The player.
    pub username: String,
    /// The dialogue being played.
    pub dialogue: String,
    /// Current node index.
    pub node: usize,
}

/// A code that can be redeemed through `RewardService`.
#[derive(Clone, Debug, PartialEq)]
pub struct RedeemCode {
    /// The code string.
    pub code: String,
    /// What it grants.
    pub rewards: Value,
    /// Maximum redemptions.
    pub max_uses: i64,
    /// Redemptions so far.
    pub uses: i64,
    /// Expiry as a UNIX timestamp, or `None` for never.
    pub expires_at: Option<i64>,
    /// Who has already redeemed it.
    pub redeemed_by: BTreeSet<String>,
}

/// A ban or warning record.
#[derive(Clone, Debug, PartialEq)]
pub struct ModerationRecord {
    /// The player.
    pub username: String,
    /// `Ban` or `Warn`.
    pub kind: String,
    /// Reason text.
    pub reason: String,
    /// Ban duration in seconds; negative means permanent.
    pub duration: f64,
    /// Warnings issued, for `Warn`.
    pub warnings: Vec<String>,
}

/// A prepared outgoing HTTP request.
#[derive(Clone, Debug, PartialEq)]
pub struct HttpRequestSpec {
    /// Numeric handle the API uses.
    pub id: i64,
    /// HTTP method.
    pub method: String,
    /// Target URL.
    pub url: String,
    /// Request headers.
    pub headers: Value,
    /// Request body.
    pub body: String,
    /// Content type.
    pub content_type: String,
    /// Whether the request has already been released.
    pub released: bool,
}

/// An analytics counter.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Counter {
    /// Current value.
    pub value: f64,
}

/// An analytics event.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalyticsEvent {
    /// Event name.
    pub name: String,
    /// The player it relates to, if any.
    pub username: Option<String>,
    /// Attached properties.
    pub properties: Value,
    /// Whether it was an error rather than an event.
    pub is_error: bool,
}

/// The lighting and atmosphere state.
#[derive(Clone, Debug, PartialEq)]
pub struct Environment {
    /// `HH:MM:SS`.
    pub time: String,
    /// `Clear`, `Rain`, `Storm`, `Fog` or `Snow`.
    pub weather: String,
    /// Weather intensity.
    pub weather_intensity: f64,
    /// Ambient colour.
    pub ambient: Rgba,
    /// Whether fog is on.
    pub fog_enabled: bool,
    /// Fog colour.
    pub fog_color: Rgba,
    /// Fog start distance.
    pub fog_start: f64,
    /// Fog end distance.
    pub fog_end: f64,
    /// Named extra properties.
    pub properties: BTreeMap<String, Value>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            time: "14:00:00".to_owned(),
            weather: "Clear".to_owned(),
            weather_intensity: 0.0,
            ambient: Rgba::new(0.4, 0.4, 0.4, 1.0),
            fog_enabled: false,
            fog_color: Rgba::new(0.75, 0.75, 0.75, 1.0),
            fog_start: 0.0,
            fog_end: 500.0,
            properties: BTreeMap::new(),
        }
    }
}

/// A registered input binding.
#[derive(Clone, Debug, PartialEq)]
pub struct InputBinding {
    /// Action name.
    pub action: String,
    /// Keys bound to it.
    pub keys: Vec<String>,
    /// Whether a callback is attached.
    pub has_callback: bool,
}

/// A cooldown's state for one player.
#[derive(Clone, Debug, PartialEq)]
pub struct CooldownEntry {
    /// Seconds remaining.
    pub remaining: f64,
    /// Original duration.
    pub duration: f64,
}

/// A registered `Audio` instance handle, matching `AudioService`.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioHandle {
    /// Handle name.
    pub name: String,
    /// Asset id.
    pub id: String,
    /// Volume.
    pub volume: f64,
    /// Whether it loops.
    pub looping: bool,
    /// `Playing`, `Paused` or `Stopped`.
    pub state: String,
}

/// Where the server is in its minute window and its visit counter.
#[derive(Clone, Debug, Default)]
pub struct ServerClock {
    /// Seconds elapsed since the server started, in 1/100 s ticks.
    pub ticks: u64,
    /// Visits recorded, which scales the storage allowance.
    pub visits: u64,
}

/// All shared state a service function can touch.
///
/// Fields are grouped by the documentation's service groups so a reviewer can
/// find the state behind any function quickly.
#[derive(Clone, Debug)]
pub struct ApiRuntime {
    /// The instance tree.
    pub world: Arc<RwLock<World>>,
    /// Persistent key-value storage.
    pub datastore: DataStoreService,
    /// Connected players.
    pub players: Arc<RwLock<PlayerRegistry>>,
    /// The side this runtime serves.
    pub side: Side,
    /// Monotonic id source for notifications, orders, mails and trades.
    pub ids: Arc<AtomicU64>,

    // --- Networking -----------------------------------------------------
    /// Outbound messages waiting to be delivered.
    pub outbound: Arc<Mutex<Vec<PendingEvent>>>,
    /// Messages received but not yet handled.
    pub inbound: Arc<Mutex<Vec<PendingEvent>>>,
    /// Registered handlers, so `OnServerEvent` twice is not a silent no-op.
    pub handlers: Arc<Mutex<Vec<EventHandler>>>,
    /// Handlers registered with `OnServerInvoke`, which must return a value.
    ///
    /// Kept separately from [`ApiRuntime::handlers`] because only an invoke
    /// carries a reply; the two must not be confused.
    pub invoke_handlers: Arc<Mutex<BTreeMap<String, InvokeHandler>>>,
    /// Broadcast counts per second tick, backing the documented 20-per-second
    /// advice for `FireAllClients`.
    pub broadcasts: Arc<RwLock<BTreeMap<u64, u32>>>,

    // --- Player-facing state -------------------------------------------
    /// Per-player camera state.
    pub cameras: Arc<RwLock<BTreeMap<String, CameraState>>>,
    /// Notification log.
    pub notifications: Arc<Mutex<Vec<Notification>>>,
    /// Chat channels and their filter levels.
    pub chat_channels: Arc<RwLock<BTreeMap<String, String>>>,
    /// Players muted, with seconds remaining.
    pub chat_mutes: Arc<RwLock<BTreeMap<String, f64>>>,
    /// Active chat bubbles.
    pub chat_bubbles: Arc<RwLock<BTreeMap<String, String>>>,
    /// Moderation records per player.
    pub moderation: Arc<RwLock<BTreeMap<String, ModerationRecord>>>,
    /// Full ban/pardon history per player, key `"{Username}:Bans"` -> array of
    /// records. Backs `BanService.GetBanHistory`.
    pub moderation_history: KeyedStore,
    /// Channel membership, so `JoinChannel`/`LeaveChannel`/`GetQueue` mean
    /// something. Without it a channel would be a name with no members.
    pub chat_members: Arc<RwLock<BTreeMap<String, BTreeSet<String>>>>,
    /// Generic registration bookkeeping for every documented `OnXxx` function.
    ///
    /// Key format is `"{Service}.{Method}"`, optionally suffixed with `":{id}"`
    /// when the documentation scopes the callback (per player, per board, per
    /// prompt). One store rather than twenty fields, so every `OnXxx` answers
    /// "was a callback registered?" the same way.
    pub callbacks: KeyedStore,
    /// Chat message log, for `ChatService.OnMessage` and test assertions.
    pub chat_log: Arc<Mutex<Vec<(String, String, String)>>>,

    // --- Economy --------------------------------------------------------
    /// `Username -> CurrencyType -> balance`.
    pub wallets: KeyedStore,
    /// `Username -> ItemId -> quantity`.
    pub inventories: KeyedStore,
    /// `ItemId -> metadata`.
    pub item_metadata: KeyedStore,
    /// Leaderboards by name.
    pub leaderboards: Arc<RwLock<BTreeMap<String, Board>>>,
    /// Orders by id.
    pub orders: Arc<RwLock<BTreeMap<String, Order>>>,
    /// Orders per player, newest last.
    pub order_history: KeyedStore,
    /// Marketplace listings by id.
    pub listings: Arc<RwLock<BTreeMap<String, Listing>>>,
    /// Purchases per player.
    pub purchases: KeyedStore,
    /// Trades by id.
    pub trades: Arc<RwLock<BTreeMap<String, Trade>>>,
    /// Completed trades per player.
    pub trade_history: KeyedStore,
    /// Crafting recipes by id.
    pub recipes: Arc<RwLock<BTreeMap<String, Recipe>>>,
    /// Craft history per player.
    pub craft_history: KeyedStore,
    /// Mailboxes: `Username -> MailId -> Mail`.
    pub mailboxes: Arc<RwLock<BTreeMap<String, BTreeMap<String, Mail>>>>,
    /// Outbox per player, mirroring mailboxes.
    pub outboxes: Arc<RwLock<BTreeMap<String, BTreeMap<String, Mail>>>>,

    // --- Gameplay -------------------------------------------------------
    /// Quests by id.
    pub quests: Arc<RwLock<BTreeMap<String, Quest>>>,
    /// `Username -> QuestId -> progress`.
    pub quest_progress: KeyedStore,
    /// Achievements by id.
    pub achievements: Arc<RwLock<BTreeMap<String, Achievement>>>,
    /// `Username -> AchievementId -> progress`.
    pub achievement_progress: KeyedStore,
    /// `Username -> AchievementId -> unlocked`.
    pub achievement_unlocked: KeyedStore,
    /// Badges by id.
    pub badges: Arc<RwLock<BTreeMap<String, Badge>>>,
    /// `Username -> BadgeId -> owned`.
    pub badge_owners: KeyedStore,
    /// Passes by id.
    pub passes: Arc<RwLock<BTreeMap<String, Pass>>>,
    /// `Username -> PassId -> owned`.
    pub pass_owners: KeyedStore,
    /// Redeem codes.
    pub codes: Arc<RwLock<BTreeMap<String, RedeemCode>>>,
    /// `Username -> day streak`.
    pub daily_streaks: Arc<RwLock<BTreeMap<String, i64>>>,
    /// `Username -> last claim tick`.
    pub daily_claims: Arc<RwLock<BTreeMap<String, u64>>>,
    /// `Username -> CooldownId -> state`.
    pub cooldowns: KeyedStore,
    /// `Username -> metrics`.
    pub analytics: KeyedStore,
    /// Recorded analytics events.
    pub analytics_events: Arc<Mutex<Vec<AnalyticsEvent>>>,
    /// Localisation tables: `Locale -> Key -> text`.
    pub locale_tables: KeyedStore,
    /// `Username -> Locale`.
    pub player_locales: Arc<RwLock<BTreeMap<String, String>>>,
    /// Teleport zones.
    pub zones: Arc<RwLock<BTreeMap<String, Zone>>>,

    // --- Social ---------------------------------------------------------
    /// Parties by id.
    pub parties: Arc<RwLock<BTreeMap<String, Party>>>,
    /// `Username -> PartyId`.
    pub party_of: Arc<RwLock<BTreeMap<String, String>>>,
    /// Matchmaking queues.
    pub queues: Arc<RwLock<BTreeMap<String, MatchQueue>>>,
    /// Matchmaking rooms.
    pub rooms: Arc<RwLock<BTreeMap<String, MatchRoom>>>,

    // --- World and physics ---------------------------------------------
    /// Runtime environment.
    pub environment: Arc<RwLock<Environment>>,
    /// Physics bodies by instance name.
    pub bodies: Arc<RwLock<BTreeMap<String, BodyState>>>,
    /// Collision groups.
    pub collision_groups: Arc<RwLock<BTreeMap<String, CollisionGroup>>>,
    /// `Instance -> group`.
    pub collision_membership: Arc<RwLock<BTreeMap<String, String>>>,
    /// Active tweens by instance name.
    pub tweens: Arc<RwLock<BTreeMap<String, Tween>>>,
    /// Active tweens in creation order, for `Resume`/`Back` addressing.
    pub tween_order: Arc<Mutex<Vec<String>>>,
    /// Sound emitters by id.
    pub sounds: Arc<RwLock<BTreeMap<String, SoundEmitter>>>,
    /// Sound volume adjustments, keyed by id.
    pub sound_volumes: Arc<RwLock<BTreeMap<String, f64>>>,
    /// Audio handles created through `AudioService`.
    pub audio: Arc<RwLock<BTreeMap<String, AudioHandle>>>,
    /// Effects by id.
    pub effects: Arc<RwLock<BTreeMap<String, EffectInstance>>>,
    /// Screen effects per player.
    pub screen_effects: Arc<RwLock<BTreeMap<String, ScreenEffect>>>,
    /// Pathfinding agents by NPC name.
    pub path_agents: Arc<RwLock<BTreeMap<String, PathAgent>>>,
    /// Proximity prompts by id.
    pub prompts: Arc<RwLock<BTreeMap<String, ProximityPrompt>>>,
    /// Spawn points by name.
    pub spawns: Arc<RwLock<BTreeMap<String, SpawnPoint>>>,
    /// In-game accessory assignments: `Target -> ids`.
    pub accessories: Arc<RwLock<BTreeMap<String, Vec<String>>>>,

    // --- UI and input ---------------------------------------------------
    /// UI elements by registered name.
    pub ui: Arc<RwLock<BTreeMap<String, UiElement>>>,
    /// Input bindings by action name.
    pub input_bindings: Arc<RwLock<BTreeMap<String, InputBinding>>>,
    /// Keys currently held down.
    pub keys_down: Arc<RwLock<BTreeSet<String>>>,
    /// `Username -> KeyCode -> callback registered`.
    pub input_handlers: KeyedStore,

    // --- Scripts and world containers ----------------------------------
    /// Prepared HTTP requests by handle.
    pub http_requests: Arc<RwLock<BTreeMap<i64, HttpRequestSpec>>>,
    /// `DataStore` handle cache, so `GetDataStore` is idempotent.
    pub datastore_handles: Arc<RwLock<BTreeMap<String, Value>>>,
    /// Dialogues by id.
    pub dialogues: Arc<RwLock<BTreeMap<String, Dialogue>>>,
    /// Active dialogue sessions.
    pub dialogue_sessions: Arc<RwLock<BTreeMap<String, DialogueSession>>>,
    /// Server clock and visit counter.
    pub clock: Arc<RwLock<ServerClock>>,
    /// Free-form script-visible globals, used for `HttpService` results and
    /// for values a script stores through `AnalyticsService`.
    pub globals: KeyedStore,
}

impl ApiRuntime {
    /// A runtime for the given side with empty state.
    pub fn new(side: Side) -> Self {
        Self {
            world: Arc::new(RwLock::new(World::new())),
            datastore: DataStoreService::new(),
            players: Arc::new(RwLock::new(PlayerRegistry::new())),
            side,
            ids: Arc::new(AtomicU64::new(1)),
            outbound: Arc::new(Mutex::new(Vec::new())),
            inbound: Arc::new(Mutex::new(Vec::new())),
            handlers: Arc::new(Mutex::new(Vec::new())),
            invoke_handlers: Arc::new(Mutex::new(BTreeMap::new())),
            broadcasts: Arc::new(RwLock::new(BTreeMap::new())),
            cameras: Arc::new(RwLock::new(BTreeMap::new())),
            notifications: Arc::new(Mutex::new(Vec::new())),
            chat_channels: Arc::new(RwLock::new(BTreeMap::new())),
            chat_mutes: Arc::new(RwLock::new(BTreeMap::new())),
            chat_bubbles: Arc::new(RwLock::new(BTreeMap::new())),
            moderation: Arc::new(RwLock::new(BTreeMap::new())),
            moderation_history: KeyedStore::new(),
            chat_members: Arc::new(RwLock::new(BTreeMap::new())),
            callbacks: KeyedStore::new(),
            chat_log: Arc::new(Mutex::new(Vec::new())),
            wallets: KeyedStore::new(),
            inventories: KeyedStore::new(),
            item_metadata: KeyedStore::new(),
            leaderboards: Arc::new(RwLock::new(BTreeMap::new())),
            orders: Arc::new(RwLock::new(BTreeMap::new())),
            order_history: KeyedStore::new(),
            listings: Arc::new(RwLock::new(BTreeMap::new())),
            purchases: KeyedStore::new(),
            trades: Arc::new(RwLock::new(BTreeMap::new())),
            trade_history: KeyedStore::new(),
            recipes: Arc::new(RwLock::new(BTreeMap::new())),
            craft_history: KeyedStore::new(),
            mailboxes: Arc::new(RwLock::new(BTreeMap::new())),
            outboxes: Arc::new(RwLock::new(BTreeMap::new())),
            quests: Arc::new(RwLock::new(BTreeMap::new())),
            quest_progress: KeyedStore::new(),
            achievements: Arc::new(RwLock::new(BTreeMap::new())),
            achievement_progress: KeyedStore::new(),
            achievement_unlocked: KeyedStore::new(),
            badges: Arc::new(RwLock::new(BTreeMap::new())),
            badge_owners: KeyedStore::new(),
            passes: Arc::new(RwLock::new(BTreeMap::new())),
            pass_owners: KeyedStore::new(),
            codes: Arc::new(RwLock::new(BTreeMap::new())),
            daily_streaks: Arc::new(RwLock::new(BTreeMap::new())),
            daily_claims: Arc::new(RwLock::new(BTreeMap::new())),
            cooldowns: KeyedStore::new(),
            analytics: KeyedStore::new(),
            analytics_events: Arc::new(Mutex::new(Vec::new())),
            locale_tables: KeyedStore::new(),
            player_locales: Arc::new(RwLock::new(BTreeMap::new())),
            zones: Arc::new(RwLock::new(BTreeMap::new())),
            parties: Arc::new(RwLock::new(BTreeMap::new())),
            party_of: Arc::new(RwLock::new(BTreeMap::new())),
            queues: Arc::new(RwLock::new(BTreeMap::new())),
            rooms: Arc::new(RwLock::new(BTreeMap::new())),
            environment: Arc::new(RwLock::new(Environment::default())),
            bodies: Arc::new(RwLock::new(BTreeMap::new())),
            collision_groups: Arc::new(RwLock::new(BTreeMap::new())),
            collision_membership: Arc::new(RwLock::new(BTreeMap::new())),
            tweens: Arc::new(RwLock::new(BTreeMap::new())),
            tween_order: Arc::new(Mutex::new(Vec::new())),
            sounds: Arc::new(RwLock::new(BTreeMap::new())),
            sound_volumes: Arc::new(RwLock::new(BTreeMap::new())),
            audio: Arc::new(RwLock::new(BTreeMap::new())),
            effects: Arc::new(RwLock::new(BTreeMap::new())),
            screen_effects: Arc::new(RwLock::new(BTreeMap::new())),
            path_agents: Arc::new(RwLock::new(BTreeMap::new())),
            prompts: Arc::new(RwLock::new(BTreeMap::new())),
            spawns: Arc::new(RwLock::new(BTreeMap::new())),
            accessories: Arc::new(RwLock::new(BTreeMap::new())),
            ui: Arc::new(RwLock::new(BTreeMap::new())),
            input_bindings: Arc::new(RwLock::new(BTreeMap::new())),
            keys_down: Arc::new(RwLock::new(BTreeSet::new())),
            input_handlers: KeyedStore::new(),
            http_requests: Arc::new(RwLock::new(BTreeMap::new())),
            datastore_handles: Arc::new(RwLock::new(BTreeMap::new())),
            dialogues: Arc::new(RwLock::new(BTreeMap::new())),
            dialogue_sessions: Arc::new(RwLock::new(BTreeMap::new())),
            clock: Arc::new(RwLock::new(ServerClock::default())),
            globals: KeyedStore::new(),
        }
    }

    /// A fresh monotonic id.
    pub fn next_id(&self) -> u64 {
        self.ids.fetch_add(1, Ordering::SeqCst)
    }

    /// A fresh id rendered as a string, for order/mail/trade ids.
    pub fn next_id_str(&self, prefix: &str) -> String {
        format!("{prefix}_{}", self.next_id())
    }

    /// Current server tick count.
    pub fn ticks(&self) -> u64 {
        self.clock.read().ticks
    }

    /// Advances the clock, which drives cooldowns, tweens and idle timeouts.
    pub fn advance(&self, seconds: f64) {
        let delta = (seconds * 100.0).max(0.0) as u64;
        let mut clock = self.clock.write();
        clock.ticks = clock.ticks.saturating_add(delta);
    }

    /// Records a visit, which raises the storage allowance.
    pub fn record_visit(&self) {
        let visits = {
            let mut clock = self.clock.write();
            clock.visits += 1;
            clock.visits
        };
        self.datastore.set_visits(visits);
    }

    /// Visits recorded so far.
    pub fn visits(&self) -> u64 {
        self.clock.read().visits
    }

    /// Registers a player as connected, refreshing the request budget.
    pub fn player_join(&self, record: PlayerRecord) -> PlayerRecord {
        let username = record.username.clone();
        {
            let mut players = self.players.write();
            players.join(record.clone());
        }
        let ccu = self.players.read().ccu();
        self.datastore.set_ccu(ccu);
        self.ensure_player_defaults(&username);
        record
    }

    /// Marks a player offline.
    pub fn player_leave(&self, username: &str) -> Option<PlayerRecord> {
        let left = self.players.write().leave(username);
        let ccu = self.players.read().ccu();
        self.datastore.set_ccu(ccu);
        left
    }

    /// Ensures per-player buckets exist so reads return zero rather than errors.
    fn ensure_player_defaults(&self, username: &str) {
        let key = format!("{username}:Wallet");
        if !self.wallets.contains(&key) {
            self.wallets
                .set(key, Value::empty_map())
                .expect("non-empty key");
        }
        let inv = format!("{username}:Items");
        if !self.inventories.contains(&inv) {
            self.inventories
                .set(inv, Value::empty_map())
                .expect("non-empty key");
        }
    }

    /// Requires a player to be known, resolving the documented "no such player"
    /// case to a precise error instead of a silent zero.
    pub fn require_player(&self, username: &str) -> ApiResult<PlayerRecord> {
        self.players
            .read()
            .get(username)
            .cloned()
            .ok_or_else(|| ApiError::NotFound {
                kind: "player",
                name: username.to_owned(),
            })
    }

    /// Whether a player is known.
    pub fn has_player(&self, username: &str) -> bool {
        self.players.read().contains(username)
    }

    /// Queues an outbound network message.
    pub fn push_outbound(&self, event: PendingEvent) {
        self.outbound.lock().push(event);
    }

    /// Drains queued outbound messages.
    pub fn take_outbound(&self) -> Vec<PendingEvent> {
        std::mem::take(&mut *self.outbound.lock())
    }

    /// Queues an inbound network message for the script layer to handle.
    pub fn push_inbound(&self, event: PendingEvent) {
        self.inbound.lock().push(event);
    }

    /// Drains queued inbound messages.
    pub fn take_inbound(&self) -> Vec<PendingEvent> {
        std::mem::take(&mut *self.inbound.lock())
    }

    /// Registers an event handler, replacing an existing one for the same name.
    pub fn register_handler(&self, event: &str, side: Side) {
        let mut handlers = self.handlers.lock();
        handlers.retain(|h| h.event != event);
        handlers.push(EventHandler {
            event: event.to_owned(),
            side,
        });
    }

    /// Removes a handler.
    pub fn unregister_handler(&self, event: &str) -> bool {
        let mut handlers = self.handlers.lock();
        let before = handlers.len();
        handlers.retain(|h| h.event != event);
        handlers.len() != before
    }

    /// Whether a handler is registered.
    pub fn has_handler(&self, event: &str) -> bool {
        self.handlers.lock().iter().any(|h| h.event == event)
    }

    /// Registers that a documented `OnXxx` callback has been attached.
    ///
    /// Returns the key it was recorded under, so a test or the script layer can
    /// look it up. `scope` is the documented identifier when the callback is
    /// scoped (a `Username`, a `BoardName`, a `PromptId`), or `None` when the
    /// callback is global to the service.
    pub fn register_callback(&self, service: &str, method: &str, scope: Option<&str>) -> String {
        let key = match scope {
            Some(scope) => format!("{service}.{method}:{scope}"),
            None => format!("{service}.{method}"),
        };
        // The store rejects an empty key, which cannot happen here because both
        // the service and the method are non-empty.
        let _ = self.callbacks.set(key.clone(), Value::Bool(true));
        key
    }

    /// Removes a callback registration, reporting whether one was present.
    pub fn unregister_callback(&self, service: &str, method: &str, scope: Option<&str>) -> bool {
        let key = match scope {
            Some(scope) => format!("{service}.{method}:{scope}"),
            None => format!("{service}.{method}"),
        };
        self.callbacks.remove(&key).is_some()
    }

    /// Whether a callback is registered.
    pub fn has_callback(&self, service: &str, method: &str, scope: Option<&str>) -> bool {
        let key = match scope {
            Some(scope) => format!("{service}.{method}:{scope}"),
            None => format!("{service}.{method}"),
        };
        self.callbacks.contains(&key)
    }

    /// Every registered callback key, sorted.
    pub fn callback_keys(&self) -> Vec<String> {
        self.callbacks.keys()
    }

    /// Names of registered handlers.
    pub fn handler_names(&self) -> Vec<String> {
        self.handlers
            .lock()
            .iter()
            .map(|h| h.event.clone())
            .collect()
    }

    /// Registers a handler that answers `InvokeServer`.
    pub fn register_invoke_handler(&self, event: &str, handler: InvokeHandler) {
        self.invoke_handlers
            .lock()
            .insert(event.to_owned(), handler);
    }

    /// Calls the `OnServerInvoke` handler for an event.
    pub fn invoke(&self, event: &str, caller: &str, payload: &[Value]) -> ApiResult<Value> {
        let handler = self.invoke_handlers.lock().get(event).cloned();
        match handler {
            Some(h) => h.call(caller, payload),
            None => Err(ApiError::Rule(format!(
                "no OnServerInvoke handler is registered for `{event}`"
            ))),
        }
    }

    /// Removes an invoke handler, reporting whether one was there.
    pub fn unregister_invoke_handler(&self, event: &str) -> bool {
        self.invoke_handlers.lock().remove(event).is_some()
    }

    /// Whether an invoke handler is registered.
    pub fn has_invoke_handler(&self, event: &str) -> bool {
        self.invoke_handlers.lock().contains_key(event)
    }

    /// Charges one broadcast against the documented 20-per-second advice.
    ///
    /// The window is the current whole second, derived from the 1/100 s clock,
    /// so a loop that broadcasts every frame is refused rather than allowed to
    /// flood every client.
    pub fn charge_broadcast(&self, now_ticks: u64, limit: u32) -> Result<(), ApiError> {
        let second = now_ticks / 100;
        let mut counts = self.broadcasts.write();
        let entry = counts.entry(second).or_insert(0);
        if *entry >= limit {
            return Err(ApiError::Rule(format!(
                "FireAllClients was called {limit} times in one second; the documented advice is at most {limit} per second"
            )));
        }
        *entry += 1;
        // Keep only the current second, so the map cannot grow without bound.
        counts.retain(|tick, _| *tick == second);
        Ok(())
    }

    /// Removes a handler from both the event and invoke registries.
    pub fn unregister_all(&self, event: &str) {
        self.unregister_handler(event);
        self.unregister_invoke_handler(event);
    }
}

/// A service call's environment: the runtime plus who is calling.
#[derive(Clone, Debug)]
pub struct ApiContext {
    /// Shared state.
    pub runtime: ApiRuntime,
    /// The side the call runs on.
    pub side: Side,
    /// The calling player, when the call came from a client script.
    pub caller: Option<String>,
}

impl ApiContext {
    /// A context for the given side with a fresh runtime.
    pub fn new(side: Side) -> Self {
        Self {
            runtime: ApiRuntime::new(side),
            side,
            caller: None,
        }
    }

    /// A context over an existing runtime.
    pub fn with_runtime(runtime: ApiRuntime) -> Self {
        let side = runtime.side;
        Self {
            runtime,
            side,
            caller: None,
        }
    }

    /// The same context, attributed to a calling player.
    pub fn as_caller(&self, username: &str) -> Self {
        Self {
            runtime: self.runtime.clone(),
            side: self.side,
            caller: Some(username.to_owned()),
        }
    }

    /// The calling player's name, or an error when the call needs one.
    pub fn require_caller(&self) -> ApiResult<String> {
        self.caller.clone().ok_or_else(|| {
            ApiError::Rule("this call must come from a player's client script".to_owned())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Side;

    #[test]
    fn players_join_and_leave() {
        let rt = ApiRuntime::new(Side::Server);
        rt.player_join(PlayerRecord::new("Alice", 1, "VN"));
        rt.player_join(PlayerRecord::new("Bob", 2, "US"));
        assert_eq!(rt.players.read().ccu(), 2);
        assert_eq!(rt.players.read().online_names(), vec!["Alice", "Bob"]);

        rt.player_leave("Alice");
        assert_eq!(rt.players.read().ccu(), 1);
        // The record survives so history lookups keep working.
        assert!(rt.has_player("Alice"));
        assert!(!rt.players.read().get("Alice").unwrap().online);
    }

    #[test]
    fn user_ids_are_allocated_monotonically() {
        let reg = PlayerRegistry::new();
        assert_eq!(reg.allocate_user_id(), 1);
        assert_eq!(reg.allocate_user_id(), 2);
    }

    #[test]
    fn joining_raises_the_request_budget() {
        let rt = ApiRuntime::new(Side::Server);
        assert_eq!(rt.datastore.ledger().allowance(), 15);
        for i in 0..10 {
            rt.player_join(PlayerRecord::new(format!("P{i}"), i + 1, "VN"));
        }
        assert_eq!(
            rt.datastore.ledger().allowance(),
            115,
            "15 + 10 * 10 players"
        );
    }

    #[test]
    fn require_player_reports_a_missing_name() {
        let rt = ApiRuntime::new(Side::Server);
        let err = rt.require_player("Nobody").unwrap_err();
        assert!(matches!(err, ApiError::NotFound { kind: "player", .. }));
    }

    #[test]
    fn outbound_messages_drain_in_order() {
        let rt = ApiRuntime::new(Side::Server);
        rt.push_outbound(PendingEvent {
            event: "Buy".into(),
            to: "Alice".into(),
            args: vec![Value::from("ok")],
            invoke: false,
        });
        rt.push_outbound(PendingEvent {
            event: "Sell".into(),
            to: "*".into(),
            args: vec![],
            invoke: false,
        });
        let drained = rt.take_outbound();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].event, "Buy");
        assert!(rt.take_outbound().is_empty(), "drained");
    }

    #[test]
    fn handlers_are_unique_per_event() {
        let rt = ApiRuntime::new(Side::Server);
        rt.register_handler("Buy", Side::Server);
        rt.register_handler("Buy", Side::Server);
        assert_eq!(rt.handler_names(), vec!["Buy".to_owned()]);
        assert!(rt.has_handler("Buy"));
        assert!(rt.unregister_handler("Buy"));
        assert!(!rt.unregister_handler("Buy"));
    }

    #[test]
    fn clock_advances_and_records_visits() {
        let rt = ApiRuntime::new(Side::Server);
        assert_eq!(rt.ticks(), 0);
        rt.advance(1.5);
        assert_eq!(rt.ticks(), 150, "1/100 s ticks");
        rt.record_visit();
        rt.record_visit();
        assert_eq!(rt.visits(), 2);
        assert_eq!(
            rt.datastore.storage_allowance(),
            25 * 1024 * 1024 + 2 * 5 * 1024 * 1024
        );
    }

    #[test]
    fn joining_seeds_wallet_and_inventory_buckets() {
        let rt = ApiRuntime::new(Side::Server);
        rt.player_join(PlayerRecord::new("Alice", 1, "VN"));
        assert!(rt.wallets.contains("Alice:Wallet"));
        assert!(rt.inventories.contains("Alice:Items"));
    }

    #[test]
    fn caller_attribution_works() {
        let ctx = ApiContext::new(Side::Client);
        assert!(ctx.require_caller().is_err());
        let scoped = ctx.as_caller("Alice");
        assert_eq!(scoped.require_caller().unwrap(), "Alice");
        assert_eq!(scoped.side, Side::Client);
    }

    #[test]
    fn cloned_runtimes_share_state() {
        let rt = ApiRuntime::new(Side::Server);
        let clone = rt.clone();
        rt.player_join(PlayerRecord::new("Alice", 1, "VN"));
        assert!(clone.has_player("Alice"));
    }
}
