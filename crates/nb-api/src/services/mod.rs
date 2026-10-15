//! Service implementations, grouped the way the documentation groups them.
//!
//! [`dispatch`] is the single place a `Service.Method` pair turns into work.
//! Each group module owns one documentation group and answers
//! [`ApiError::UnknownFunction`] for anything outside it, so the routing table
//! and the implementations cannot disagree with each other.
//!
//! The groups mirror the "Điều hướng nhanh" table in the documentation:
//!
//! | Group | Module | Services |
//! |---|---|---|
//! | Người chơi | [`player`] | PlayerService, BanService, CameraService, NotificationService, ChatService, BadgeService, PassService |
//! | Vật phẩm & Kinh tế | [`economy`] | CurrencyService, InventoryService, PurchaseService, MarketplaceService, TradingService, CraftingService |
//! | Giao diện | [`ui`] | UIService, InputService, TextService |
//! | Thế giới | [`world`] | InstanceService, CollisionService, LightingService, PhysicsService, TweenService, SpawnService, ProximityService, PathfindingService, AccessoryService |
//! | Gameplay | [`gameplay`] | QuestService, AchievementService, LeaderboardService, RewardService, CooldownService, PartyService, MatchmakingService, DialogueService, MailService |
//! | Hệ thống | [`system`] | DataStoreService, DataStore, HttpService, AnalyticsService, LocalizationService, TeleportService |
//! | Mạng & Script | [`network`] | NetworkService, NetworkEventService |
//! | Âm thanh & Hiệu ứng | [`world`] | AudioService, SoundEffectService, EffectService |

pub mod economy;
pub mod gameplay;
pub mod network;
pub mod player;
pub mod shared;
pub mod system;
pub mod ui;
pub mod world;

use crate::args::Args;
use crate::error::{ApiError, ApiResult};
use crate::runtime::ApiContext;

/// The error a group module answers for a method it does not own.
///
/// Exposed so every group reports an unwired method identically, which is what
/// the registry's coverage probe relies on.
pub(crate) fn unknown(service: &str, method: &str) -> ApiError {
    ApiError::UnknownFunction {
        service: service.to_owned(),
        method: method.to_owned(),
    }
}

/// Routes a documented call to its implementation.
///
/// Returns [`ApiError::UnknownFunction`] for a service this build does not
/// implement, and each group module does the same for a method it does not own.
pub fn dispatch(
    ctx: &ApiContext,
    service: &str,
    method: &str,
    args: &Args,
) -> ApiResult<nb_types::Value> {
    match service {
        // --- Người chơi ------------------------------------------------
        "PlayerService"
        | "BanService"
        | "CameraService"
        | "NotificationService"
        | "ChatService"
        | "BadgeService"
        | "PassService" => player::dispatch(ctx, service, method, args),

        // --- Vật phẩm & Kinh tế ---------------------------------------
        "CurrencyService" | "InventoryService" | "PurchaseService" | "MarketplaceService"
        | "TradingService" | "CraftingService" => economy::dispatch(ctx, service, method, args),

        // --- Giao diện -------------------------------------------------
        "UIService" | "InputService" | "TextService" => ui::dispatch(ctx, service, method, args),

        // --- Thế giới, âm thanh & hiệu ứng -----------------------------
        "InstanceService" | "CollisionService" | "LightingService" | "PhysicsService"
        | "TweenService" | "SpawnService" | "ProximityService" | "PathfindingService"
        | "AccessoryService" | "AudioService" | "SoundEffectService" | "EffectService" => {
            world::dispatch(ctx, service, method, args)
        }

        // --- Gameplay --------------------------------------------------
        "QuestService" | "AchievementService" | "LeaderboardService" | "RewardService"
        | "CooldownService" | "PartyService" | "MatchmakingService" | "DialogueService"
        | "MailService" => gameplay::dispatch(ctx, service, method, args),

        // --- Hệ thống --------------------------------------------------
        "DataStoreService"
        | "DataStore"
        | "HttpService"
        | "AnalyticsService"
        | "LocalizationService"
        | "TeleportService" => system::dispatch(ctx, service, method, args),

        // --- Mạng & Script ---------------------------------------------
        "NetworkService" | "NetworkEventService" => network::dispatch(ctx, service, method, args),

        _ => Err(unknown(service, method)),
    }
}

/// The services this build routes, used by documentation and diagnostics.
pub const ROUTED_SERVICES: [&str; 45] = [
    "AccessoryService",
    "AchievementService",
    "AnalyticsService",
    "AudioService",
    "BadgeService",
    "BanService",
    "CameraService",
    "ChatService",
    "CollisionService",
    "CooldownService",
    "CraftingService",
    "CurrencyService",
    "DataStore",
    "DataStoreService",
    "DialogueService",
    "EffectService",
    "HttpService",
    "InputService",
    "InstanceService",
    "InventoryService",
    "LeaderboardService",
    "LightingService",
    "LocalizationService",
    "MailService",
    "MarketplaceService",
    "MatchmakingService",
    "NetworkEventService",
    "NetworkService",
    "NotificationService",
    "PartyService",
    "PassService",
    "PathfindingService",
    "PhysicsService",
    "PlayerService",
    "ProximityService",
    "PurchaseService",
    "QuestService",
    "RewardService",
    "SoundEffectService",
    "SpawnService",
    "TeleportService",
    "TextService",
    "TradingService",
    "TweenService",
    "UIService",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_documented_service_is_routed() {
        let mut documented: Vec<String> = crate::spec::services().to_vec();
        documented.sort();
        let mut routed: Vec<String> = ROUTED_SERVICES.iter().map(|s| (*s).to_owned()).collect();
        routed.sort();
        assert_eq!(
            routed, documented,
            "the routing table must cover every service"
        );
    }

    #[test]
    fn unknown_services_are_rejected() {
        let ctx = ApiContext::new(crate::error::Side::Server);
        let args = Args::new("Nope", "Nope", vec![]);
        assert!(matches!(
            dispatch(&ctx, "NopeService", "Nope", &args),
            Err(ApiError::UnknownFunction { .. })
        ));
    }
}
