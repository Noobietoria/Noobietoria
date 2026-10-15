//! Người chơi: `PlayerService`, `BanService`, `CameraService`,
//! `NotificationService`, `ChatService`, `BadgeService`, `PassService`.
//!
//! 60 documented functions. Three themes run through this group, and each is
//! enforced rather than described:
//!
//! ## 1. A player who is not there is an error, not a zero
//!
//! "Send a notification to Bob" when Bob does not exist is a bug in the calling
//! script, so every function here that names a `Username` resolves it first and
//! answers [`ApiError::NotFound`]. Historical lookups are the deliberate
//! exception: `GetBanHistory` and `GetWarnings` must keep working after a player
//! disconnects, so they accept any name the registry has ever seen.
//!
//! ## 2. Moderation is stateful and time-aware
//!
//! The documentation gives `Ban(Username, Reason, Duration)` a duration and asks
//! `IsBanned` about the present, so a ban is stored with the tick it was issued
//! at and compared against the server clock. A negative duration means permanent,
//! which is the only reading that keeps `Unban` meaningful. Warnings accumulate
//! and are cleared explicitly, so `GetWarnings` returns a list, not a count.
//!
//! ## 3. The client/server split is a security boundary
//!
//! `CameraService` and `ChatService.ShowBubble` are client-only, and the registry
//! refuses to dispatch them on the server before reaching this module. Everything
//! else here is server-authoritative, because a client that could grant itself a
//! badge or clear its own ban would make every economy in the game meaningless.

use std::collections::BTreeSet;

use nb_types::{Rgba, Value, Vector3};

use crate::args::Args;
use crate::error::{ApiError, ApiResult};
use crate::runtime::{ApiContext, CameraState, ModerationRecord, Notification, PlayerRecord};
use crate::services::shared::{enum_arg, require_player as require_player_exists};
use crate::services::unknown;

/// Routes a player-group call.
pub fn dispatch(ctx: &ApiContext, service: &str, method: &str, args: &Args) -> ApiResult<Value> {
    match (service, method) {
        // --- PlayerService --------------------------------------------
        ("PlayerService", "GetPlayers") => get_players(ctx),
        ("PlayerService", "GetPlayer") => get_player(ctx, args),
        ("PlayerService", "GetPlayerName") => get_player_name(ctx, args),
        ("PlayerService", "GetPlayerCountry") => get_player_country(ctx, args),
        ("PlayerService", "GetPlayerAvatar") => get_player_avatar(ctx, args),
        ("PlayerService", "GetPlayerAvatarUrl") => get_player_avatar_url(ctx, args),
        ("PlayerService", "GetPlayerThumbnail") => get_player_thumbnail(ctx, args),
        ("PlayerService", "GetPlayerThumbnailUrl") => get_player_thumbnail_url(ctx, args),
        ("PlayerService", "GetPlayerCharacter") => get_player_character(ctx, args),
        ("PlayerService", "GetPlayerCharacterUrl") => get_player_character_url(ctx, args),
        ("PlayerService", "GetCoors") => get_coors(ctx, args),
        ("PlayerService", "Teleport") => teleport(ctx, args),

        // --- BanService -----------------------------------------------
        ("BanService", "Kick") => kick(ctx, args),
        ("BanService", "Ban") => ban(ctx, args),
        ("BanService", "Unban") => unban(ctx, args),
        ("BanService", "IsBanned") => is_banned(ctx, args),
        ("BanService", "GetBanInfo") => get_ban_info(ctx, args),
        ("BanService", "GetBanHistory") => get_ban_history(ctx, args),
        ("BanService", "Warn") => warn(ctx, args),
        ("BanService", "GetWarnings") => get_warnings(ctx, args),
        ("BanService", "ClearWarnings") => clear_warnings(ctx, args),

        // --- CameraService --------------------------------------------
        ("CameraService", "SetType") => set_camera_type(ctx, args),
        ("CameraService", "SetPosition") => set_camera_position(ctx, args),
        ("CameraService", "SetFOV") => set_camera_fov(ctx, args),
        ("CameraService", "LockTo") => lock_camera(ctx, args),
        ("CameraService", "Unlock") => unlock_camera(ctx, args),
        ("CameraService", "Shake") => shake_camera(ctx, args),
        ("CameraService", "Reset") => reset_camera(ctx, args),

        // --- NotificationService --------------------------------------
        ("NotificationService", "Send") => send_notification(ctx, args, false),
        ("NotificationService", "SendAll") => send_notification(ctx, args, true),
        ("NotificationService", "Dismiss") => dismiss_notification(ctx, args),
        ("NotificationService", "OnDismiss") => on_dismiss(ctx, args),

        // --- ChatService ----------------------------------------------
        ("ChatService", "CreateChannel") => create_channel(ctx, args),
        ("ChatService", "DeleteChannel") => delete_channel(ctx, args),
        ("ChatService", "JoinChannel") => join_channel(ctx, args),
        ("ChatService", "LeaveChannel") => leave_channel(ctx, args),
        ("ChatService", "SendMessage") => send_message(ctx, args),
        ("ChatService", "MutePlayer") => mute_player(ctx, args),
        ("ChatService", "UnmutePlayer") => unmute_player(ctx, args),
        ("ChatService", "SetFilter") => set_filter(ctx, args),
        ("ChatService", "ShowBubble") => show_bubble(ctx, args),
        ("ChatService", "HideBubble") => hide_bubble(ctx, args),
        ("ChatService", "OnMessage") => on_message(ctx, args),

        // --- BadgeService ---------------------------------------------
        ("BadgeService", "Register") => register_badge(ctx, args),
        ("BadgeService", "Unregister") => unregister_badge(ctx, args),
        ("BadgeService", "Award") => award_badge(ctx, args),
        ("BadgeService", "Revoke") => revoke_badge(ctx, args),
        ("BadgeService", "HasBadge") => has_badge(ctx, args),
        ("BadgeService", "GetBadges") => get_badges(ctx, args),
        ("BadgeService", "GetAll") => get_all_badges(ctx),
        ("BadgeService", "OnAward") => on_award(ctx, args),

        // --- PassService ----------------------------------------------
        ("PassService", "Register") => register_pass(ctx, args),
        ("PassService", "Unregister") => unregister_pass(ctx, args),
        ("PassService", "Grant") => grant_pass(ctx, args),
        ("PassService", "Revoke") => revoke_pass(ctx, args),
        ("PassService", "HasPass") => has_pass(ctx, args),
        ("PassService", "GetPasses") => get_passes(ctx, args),
        ("PassService", "GetAll") => get_all_passes(ctx),
        ("PassService", "CheckPermission") => check_permission(ctx, args),
        ("PassService", "OnGrant") => on_grant(ctx, args),

        _ => Err(unknown(service, method)),
    }
}

// --- PlayerService --------------------------------------------------------

/// `PlayerService.GetPlayers()` — every connected player's name.
fn get_players(ctx: &ApiContext) -> ApiResult<Value> {
    let names = ctx.runtime.players.read().online_names();
    Ok(Value::array(names.into_iter().map(Value::from)))
}

/// `PlayerService.GetPlayer("Username")` — one player record, or `nil`.
///
/// An unknown name returns `nil` rather than an error, because this is the
/// documented way to *test* whether a player exists.
fn get_player(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    match ctx.runtime.players.read().get(&username) {
        Some(record) => Ok(player_to_value(record)),
        None => Ok(Value::Nil),
    }
}

/// The documented table shape for a player record.
fn player_to_value(record: &PlayerRecord) -> Value {
    Value::map([
        ("Username", Value::from(record.username.clone())),
        ("UserId", Value::from(record.user_id)),
        ("Country", Value::from(record.country.clone())),
        ("Position", record.position.to_value()),
        ("LookAt", record.look_at.to_value()),
        ("Avatar", Value::from(record.avatar_id.clone())),
        ("Online", Value::from(record.online)),
    ])
}

/// Resolves a numeric `UserId` to a player record, or fails.
fn player_by_user_id(ctx: &ApiContext, user_id: u64) -> ApiResult<PlayerRecord> {
    let name = ctx
        .runtime
        .players
        .read()
        .name_of(user_id)
        .map(str::to_owned)
        .ok_or_else(|| ApiError::NotFound {
            kind: "player",
            name: user_id.to_string(),
        })?;
    ctx.runtime.require_player(&name)
}

/// `PlayerService.GetPlayerName(UserId)`.
fn get_player_name(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let user_id = args.integer(0, "UserId")? as u64;
    Ok(Value::from(player_by_user_id(ctx, user_id)?.username))
}

/// `PlayerService.GetPlayerCountry(UserId)`.
fn get_player_country(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let user_id = args.integer(0, "UserId")? as u64;
    Ok(Value::from(player_by_user_id(ctx, user_id)?.country))
}

/// `PlayerService.GetPlayerAvatar(UserId)`.
fn get_player_avatar(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let user_id = args.integer(0, "UserId")? as u64;
    Ok(Value::from(player_by_user_id(ctx, user_id)?.avatar_id))
}

/// `PlayerService.GetPlayerAvatarUrl(UserId)`.
fn get_player_avatar_url(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let user_id = args.integer(0, "UserId")? as u64;
    let record = player_by_user_id(ctx, user_id)?;
    Ok(Value::from(asset_url("avatar", &record.avatar_id)))
}

/// `PlayerService.GetPlayerThumbnail(UserId)`.
///
/// A thumbnail is the same avatar rendered small, so the asset id is returned.
fn get_player_thumbnail(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let user_id = args.integer(0, "UserId")? as u64;
    Ok(Value::from(player_by_user_id(ctx, user_id)?.avatar_id))
}

/// `PlayerService.GetPlayerThumbnailUrl(UserId)`.
fn get_player_thumbnail_url(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let user_id = args.integer(0, "UserId")? as u64;
    let record = player_by_user_id(ctx, user_id)?;
    Ok(Value::from(asset_url("thumbnail", &record.avatar_id)))
}

/// `PlayerService.GetPlayerCharacter(UserId)`.
fn get_player_character(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let user_id = args.integer(0, "UserId")? as u64;
    let record = player_by_user_id(ctx, user_id)?;
    Ok(Value::from(format!("{}:Character", record.username)))
}

/// `PlayerService.GetPlayerCharacterUrl(UserId)`.
fn get_player_character_url(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let user_id = args.integer(0, "UserId")? as u64;
    let record = player_by_user_id(ctx, user_id)?;
    Ok(Value::from(asset_url(
        "character",
        &format!("{}:Character", record.username),
    )))
}

/// Builds an asset URL for the documented `*Url` accessors.
///
/// An empty id yields an empty string rather than a URL pointing at nothing, so
/// a client can check for "no avatar set" by looking for emptiness.
fn asset_url(kind: &str, id: &str) -> String {
    if id.is_empty() {
        String::new()
    } else {
        format!("https://assets.noobietoria.local/{kind}/{id}")
    }
}

/// `PlayerService.GetCoors(Username)` — the player's current position.
fn get_coors(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let record = ctx.runtime.require_player(&username)?;
    Ok(record.position.to_value())
}

/// `PlayerService.Teleport(Username, Coors)` — moves a player.
fn teleport(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let destination = args.position(1, "Coors")?;
    require_player_exists(&ctx.runtime, &username)?;
    if let Some(record) = ctx.runtime.players.write().get_mut(&username) {
        record.position = destination;
    }
    Ok(destination.to_value())
}

// --- BanService -----------------------------------------------------------

/// The tick a ban expires at, or `None` for a permanent ban.
///
/// A negative duration is the documented way to say "permanent"; zero is treated
/// as permanent too, because a ban that lapses instantly is never intended.
fn ban_expiry(now_ticks: u64, duration_secs: f64) -> Option<u64> {
    if duration_secs <= 0.0 {
        None
    } else {
        Some(now_ticks.saturating_add((duration_secs * 100.0) as u64))
    }
}

/// `BanService.Kick(Username, Reason)` — removes a player without banning them.
fn kick(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let reason = args.string_or(1, "Reason", "")?;
    ctx.runtime.require_player(&username)?;
    ctx.runtime.player_leave(&username);
    Ok(Value::map([
        ("Username", Value::from(username)),
        ("Kicked", Value::Bool(true)),
        ("Reason", Value::from(reason)),
    ]))
}

/// `BanService.Ban(Username, Reason, Duration)`.
///
/// Records the ban in `moderation` (current state) and appends to
/// `moderation_history` (audit trail), then disconnects the player. Keeping both
/// is what lets `IsBanned` be cheap while `GetBanHistory` stays complete.
fn ban(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let reason = args.string_or(1, "Reason", "")?;
    let duration = args.number(2, "Duration")?;
    ctx.runtime.require_player(&username)?;

    let now = ctx.runtime.ticks();
    let expires_at = ban_expiry(now, duration);
    let record = ModerationRecord {
        username: username.clone(),
        kind: "Ban".to_owned(),
        reason: reason.clone(),
        duration,
        warnings: Vec::new(),
    };

    // Preserve any warnings already on file: banning must not erase them.
    {
        let mut moderation = ctx.runtime.moderation.write();
        let warnings = moderation
            .get(&username)
            .map(|r| r.warnings.clone())
            .unwrap_or_default();
        let mut stored = record.clone();
        stored.warnings = warnings;
        moderation.insert(username.clone(), stored);
    }

    append_moderation_history(ctx, &record)?;
    ctx.runtime.player_leave(&username);

    Ok(Value::map([
        ("Username", Value::from(username)),
        ("Banned", Value::Bool(true)),
        ("Reason", Value::from(reason)),
        ("Permanent", Value::Bool(expires_at.is_none())),
        (
            "ExpiresAt",
            expires_at.map(Value::from).unwrap_or(Value::Nil),
        ),
    ]))
}

/// Appends a moderation action to the player's audit trail.
fn append_moderation_history(ctx: &ApiContext, record: &ModerationRecord) -> ApiResult<()> {
    let key = format!("{}:Bans", record.username);
    let mut history = ctx.runtime.moderation_history.array_or_empty(&key)?;
    history.push(Value::map([
        ("Kind", Value::from(record.kind.clone())),
        ("Reason", Value::from(record.reason.clone())),
        ("Duration", Value::from(record.duration)),
        ("At", Value::from(ctx.runtime.ticks())),
    ]));
    ctx.runtime
        .moderation_history
        .set(key, Value::Array(history))
        .map_err(ApiError::Store)
}

/// `BanService.Unban(Username)`.
///
/// Clears the record entirely, because `IsBanned` reads its presence: a leftover
/// record with no ban would be indistinguishable from an active one.
fn unban(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    ctx.runtime.require_player(&username)?;

    if ctx.runtime.moderation.write().remove(&username).is_none() {
        return Err(ApiError::NotFound {
            kind: "ban",
            name: username,
        });
    }
    let record = ModerationRecord {
        username: username.clone(),
        kind: "Unban".to_owned(),
        reason: String::new(),
        duration: 0.0,
        warnings: Vec::new(),
    };
    append_moderation_history(ctx, &record)?;
    Ok(Value::map([
        ("Username", Value::from(username)),
        ("Unbanned", Value::Bool(true)),
    ]))
}

/// The tick a player was most recently banned at, from the audit trail.
fn last_ban_tick(ctx: &ApiContext, username: &str) -> Option<u64> {
    let history = ctx
        .runtime
        .moderation_history
        .array_or_empty(&format!("{username}:Bans"))
        .ok()?;
    history
        .iter()
        .rev()
        .find(|entry| entry.get("Kind").and_then(Value::as_str) == Some("Ban"))
        .and_then(|entry| entry.get("At"))
        .and_then(Value::as_f64)
        .map(|t| t as u64)
}

/// The active ban for a player, if one exists and has not lapsed.
fn active_ban(ctx: &ApiContext, username: &str) -> Option<ModerationRecord> {
    let record = ctx.runtime.moderation.read().get(username).cloned()?;
    if record.kind != "Ban" {
        return None;
    }
    match ban_expiry(0, record.duration) {
        // Permanent.
        None => Some(record),
        Some(window) => {
            let issued = last_ban_tick(ctx, username).unwrap_or(0);
            if ctx.runtime.ticks() < issued.saturating_add(window) {
                Some(record)
            } else {
                // The ban lapsed; drop it so the state reflects the clock.
                ctx.runtime.moderation.write().remove(username);
                None
            }
        }
    }
}

/// `BanService.IsBanned(Username)`.
fn is_banned(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    Ok(Value::Bool(active_ban(ctx, &username).is_some()))
}

/// `BanService.GetBanInfo(Username)` — details, or `nil` when not banned.
fn get_ban_info(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    match active_ban(ctx, &username) {
        Some(record) => Ok(Value::map([
            ("Username", Value::from(record.username)),
            ("Reason", Value::from(record.reason)),
            ("Duration", Value::from(record.duration)),
            ("Permanent", Value::Bool(record.duration <= 0.0)),
        ])),
        None => Ok(Value::Nil),
    }
}

/// `BanService.GetBanHistory(Username, Limit)` — newest first.
///
/// Works for a player who has disconnected, which is the point of an audit log.
fn get_ban_history(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let limit = args.integer_or(1, "Limit", 50)?.max(0) as usize;
    let mut history = ctx
        .runtime
        .moderation_history
        .array_or_empty(&format!("{username}:Bans"))?;
    history.reverse();
    history.truncate(limit);
    Ok(Value::Array(history))
}

/// `BanService.Warn(Username, Reason)` — records a warning, returns the count.
fn warn(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let reason = args.string_or(1, "Reason", "")?;
    ctx.runtime.require_player(&username)?;

    let count = {
        let mut moderation = ctx.runtime.moderation.write();
        let entry = moderation
            .entry(username.clone())
            .or_insert_with(|| ModerationRecord {
                username: username.clone(),
                kind: "Warn".to_owned(),
                reason: String::new(),
                duration: 0.0,
                warnings: Vec::new(),
            });
        entry.warnings.push(reason.clone());
        entry.warnings.len()
    };

    append_moderation_history(
        ctx,
        &ModerationRecord {
            username: username.clone(),
            kind: "Warn".to_owned(),
            reason,
            duration: 0.0,
            warnings: Vec::new(),
        },
    )?;

    Ok(Value::from(count as i64))
}

/// `BanService.GetWarnings(Username)`.
fn get_warnings(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let warnings = ctx
        .runtime
        .moderation
        .read()
        .get(&username)
        .map(|r| r.warnings.clone())
        .unwrap_or_default();
    Ok(Value::array(warnings.into_iter().map(Value::from)))
}

/// `BanService.ClearWarnings(Username)`.
fn clear_warnings(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    ctx.runtime.require_player(&username)?;
    let mut moderation = ctx.runtime.moderation.write();
    match moderation.get_mut(&username) {
        Some(record) if !record.warnings.is_empty() => {
            record.warnings.clear();
            Ok(Value::Bool(true))
        }
        _ => Err(ApiError::NotFound {
            kind: "warning",
            name: username,
        }),
    }
}

// --- CameraService --------------------------------------------------------

/// Fetches a player's camera state, creating the documented default if absent.
///
/// `CameraService` controls a camera that "đã tồn tại", so the default is only a
/// starting point and every accessor works immediately.
fn camera_for(ctx: &ApiContext, username: &str) -> ApiResult<CameraState> {
    require_player_exists(&ctx.runtime, username)?;
    let mut cameras = ctx.runtime.cameras.write();
    Ok(cameras
        .entry(username.to_owned())
        .or_insert_with(CameraState::default)
        .clone())
}

/// Stores a camera state back.
fn store_camera(ctx: &ApiContext, username: &str, camera: &CameraState) {
    ctx.runtime
        .cameras
        .write()
        .insert(username.to_owned(), camera.clone());
}

/// `CameraService.SetType(Username, CameraType)`.
fn set_camera_type(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let kind = args.string(1, "CameraType")?;
    let mut camera = camera_for(ctx, &username)?;
    camera.kind = kind.clone();
    store_camera(ctx, &username, &camera);
    Ok(Value::from(kind))
}

/// `CameraService.SetPosition(Username, Position, LookAt)`.
fn set_camera_position(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let position = args.position(1, "Position")?;
    let look_at = args.position(2, "LookAt")?;
    let mut camera = camera_for(ctx, &username)?;
    camera.position = position;
    camera.look_at = look_at;
    // An explicit position supersedes a previous lock, otherwise the two would
    // fight over where the camera is.
    camera.locked_to = None;
    store_camera(ctx, &username, &camera);
    Ok(Value::map([
        ("Position", position.to_value()),
        ("LookAt", look_at.to_value()),
    ]))
}

/// `CameraService.SetFOV(Username, FieldOfView)`.
///
/// A field of view outside what a projection can represent is refused, because a
/// zero or negative FOV would make the renderer divide by zero.
fn set_camera_fov(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let fov = args.number(1, "FieldOfView")?;
    if !(1.0..=179.0).contains(&fov) {
        return Err(args.rule(format!(
            "FieldOfView must be between 1 and 179 degrees, got {fov}"
        )));
    }
    let mut camera = camera_for(ctx, &username)?;
    camera.fov = fov;
    store_camera(ctx, &username, &camera);
    Ok(Value::from(fov))
}

/// `CameraService.LockTo(Username, Instance)`.
fn lock_camera(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let instance = args.string(1, "Instance")?;
    if instance.is_empty() {
        return Err(args.rule("Instance must name something to lock the camera to"));
    }
    let mut camera = camera_for(ctx, &username)?;
    camera.locked_to = Some(instance.clone());
    store_camera(ctx, &username, &camera);
    Ok(Value::from(instance))
}

/// `CameraService.Unlock(Username)`.
fn unlock_camera(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let mut camera = camera_for(ctx, &username)?;
    if camera.locked_to.is_none() {
        return Err(ApiError::NotFound {
            kind: "camera lock",
            name: username,
        });
    }
    camera.locked_to = None;
    store_camera(ctx, &username, &camera);
    Ok(Value::Bool(true))
}

/// `CameraService.Shake(Username, ShakeIntensity, Duration)`.
fn shake_camera(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let intensity = args.number(1, "ShakeIntensity")?;
    let duration = args.number(2, "Duration")?;
    if intensity < 0.0 {
        return Err(args.rule("ShakeIntensity must not be negative"));
    }
    if duration <= 0.0 {
        return Err(args.rule("Duration must be positive"));
    }
    let mut camera = camera_for(ctx, &username)?;
    camera.shake = intensity;
    camera.shake_until = ctx.runtime.ticks() as f64 + duration * 100.0;
    store_camera(ctx, &username, &camera);
    Ok(Value::map([
        ("ShakeIntensity", Value::from(intensity)),
        ("Duration", Value::from(duration)),
    ]))
}

/// `CameraService.Reset(Username)` — back to the documented default.
fn reset_camera(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    camera_for(ctx, &username)?;
    store_camera(ctx, &username, &CameraState::default());
    Ok(Value::Bool(true))
}

// --- NotificationService --------------------------------------------------

/// `NotificationService.Send(Username, Title, Message, Type, Duration)` and
/// `SendAll(Title, Message, Type, Duration)`.
///
/// `broadcast` selects which signature's argument positions to read, because the
/// two documented signatures differ only by their leading `Username`.
fn send_notification(ctx: &ApiContext, args: &Args, broadcast: bool) -> ApiResult<Value> {
    const ACCEPTED: [&str; 3] = ["Toast", "Popup", "Banner"];
    let (recipients, base) = if broadcast {
        (ctx.runtime.players.read().online_names(), 0usize)
    } else {
        let username = args.string(0, "Username")?;
        let recipients = if username == "*" {
            ctx.runtime.players.read().online_names()
        } else {
            require_player_exists(&ctx.runtime, &username)?;
            vec![username]
        };
        (recipients, 1usize)
    };

    let title = args.string(base, "Title")?;
    let message = args.string(base + 1, "Message")?;
    let kind = enum_arg(args, base + 2, "Type", &ACCEPTED)?;
    let duration = args.number(base + 3, "Duration")?;
    if duration <= 0.0 {
        return Err(args.rule("Duration must be positive"));
    }

    let mut ids = Vec::with_capacity(recipients.len());
    for to in recipients {
        let id = ctx.runtime.next_id();
        ctx.runtime.notifications.lock().push(Notification {
            id,
            to,
            title: title.clone(),
            message: message.clone(),
            kind: kind.clone(),
            duration,
            dismissed: false,
        });
        ids.push(Value::from(id));
    }
    Ok(Value::Array(ids))
}

/// `NotificationService.Dismiss(Username, NotificationId)`.
fn dismiss_notification(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let id = args.integer(1, "NotificationId")? as u64;

    let mut notifications = ctx.runtime.notifications.lock();
    match notifications
        .iter_mut()
        .find(|n| n.id == id && (n.to == username || username == "*"))
    {
        Some(notification) => {
            notification.dismissed = true;
            Ok(Value::Bool(true))
        }
        None => Err(ApiError::NotFound {
            kind: "notification",
            name: id.to_string(),
        }),
    }
}

/// `NotificationService.OnDismiss(Username, Callback)`.
fn on_dismiss(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    require_player_exists(&ctx.runtime, &username)?;
    let key = ctx
        .runtime
        .register_callback("NotificationService", "OnDismiss", Some(&username));
    Ok(Value::from(key))
}

// --- ChatService ----------------------------------------------------------

/// `ChatService.CreateChannel(ChannelName)`.
fn create_channel(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "ChannelName")?;
    let mut channels = ctx.runtime.chat_channels.write();
    if channels.contains_key(&name) {
        return Err(ApiError::AlreadyExists {
            kind: "chat channel",
            name,
        });
    }
    // `SetFilter` adjusts this; "Normal" is the documented default level.
    channels.insert(name.clone(), "Normal".to_owned());
    drop(channels);
    ctx.runtime
        .chat_members
        .write()
        .entry(name.clone())
        .or_default();
    Ok(Value::from(name))
}

/// `ChatService.DeleteChannel(ChannelName)`.
fn delete_channel(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "ChannelName")?;
    if ctx.runtime.chat_channels.write().remove(&name).is_none() {
        return Err(ApiError::NotFound {
            kind: "chat channel",
            name,
        });
    }
    ctx.runtime.chat_members.write().remove(&name);
    Ok(Value::Bool(true))
}

/// `ChatService.JoinChannel(Username, ChannelName)`.
fn join_channel(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let channel = args.string(1, "ChannelName")?;
    require_player_exists(&ctx.runtime, &username)?;
    if !ctx.runtime.chat_channels.read().contains_key(&channel) {
        return Err(ApiError::NotFound {
            kind: "chat channel",
            name: channel,
        });
    }
    ctx.runtime
        .chat_members
        .write()
        .entry(channel)
        .or_default()
        .insert(username);
    Ok(Value::Bool(true))
}

/// `ChatService.LeaveChannel(Username, ChannelName)`.
fn leave_channel(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let channel = args.string(1, "ChannelName")?;
    require_player_exists(&ctx.runtime, &username)?;

    let removed = ctx
        .runtime
        .chat_members
        .write()
        .get_mut(&channel)
        .map(|members| members.remove(&username))
        .unwrap_or(false);
    if !removed {
        return Err(ApiError::NotFound {
            kind: "channel membership",
            name: format!("{username} in {channel}"),
        });
    }
    Ok(Value::Bool(true))
}

/// `ChatService.SendMessage(Username, ChannelName, Message)`.
///
/// A muted player cannot speak, which is the purpose of `MutePlayer`.
fn send_message(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let channel = args.string(1, "ChannelName")?;
    let message = args.string(2, "Message")?;
    require_player_exists(&ctx.runtime, &username)?;

    if let Some(remaining) = ctx.runtime.chat_mutes.read().get(&username).copied() {
        if remaining > 0.0 {
            return Err(args.rule(format!(
                "{username} is muted for another {remaining} seconds"
            )));
        }
    }

    let filter = ctx
        .runtime
        .chat_channels
        .read()
        .get(&channel)
        .cloned()
        .ok_or_else(|| ApiError::NotFound {
            kind: "chat channel",
            name: channel.clone(),
        })?;

    let filtered = filter_message(&message, &filter);
    ctx.runtime
        .chat_log
        .lock()
        .push((username, channel, filtered.clone()));
    Ok(Value::from(filtered))
}

/// Applies a channel's filter level to a message.
///
/// `SetFilter(ChannelName, FilterLevel)` documents the levels; they behave as:
/// `None` passes text through, `Normal` masks a fixed profanity list, `Strict`
/// additionally masks links. Masking is whole-word and case-insensitive, so
/// `hello` is left alone while `hell` is masked — a substring match would corrupt
/// ordinary words.
pub fn filter_message(message: &str, filter_level: &str) -> String {
    const CENSORED: [&str; 4] = ["damn", "hell", "idiot", "stupid"];
    match filter_level.to_ascii_lowercase().as_str() {
        "none" => message.to_owned(),
        "strict" => censor_links(&censor_words(message, &CENSORED)),
        // Anything unrecognised is treated as the documented default.
        _ => censor_words(message, &CENSORED),
    }
}

/// Masks whole words, preserving separators and case.
fn censor_words(message: &str, words: &[&str]) -> String {
    // Split on the characters that delimit a word, keeping the delimiters.
    let mut out = String::with_capacity(message.len());
    let mut current = String::new();
    for ch in message.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            if !current.is_empty() {
                out.push_str(&mask_if_banned(&current, words));
                current.clear();
            }
            out.push(ch);
        }
    }
    if !current.is_empty() {
        out.push_str(&mask_if_banned(&current, words));
    }
    out
}

/// Masks one word when it is in the list, keeping its length.
fn mask_if_banned(word: &str, words: &[&str]) -> String {
    let lower = word.to_lowercase();
    if words.iter().any(|banned| *banned == lower) {
        "*".repeat(word.chars().count())
    } else {
        word.to_owned()
    }
}

/// Masks bare URLs, which is what `Strict` adds.
fn censor_links(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    let mut current = String::new();
    let flush = |out: &mut String, current: &mut String| {
        if current.is_empty() {
            return;
        }
        let lower = current.to_lowercase();
        let is_link = lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("www.");
        if is_link {
            out.push_str(&"*".repeat(current.chars().count()));
        } else {
            out.push_str(current);
        }
        current.clear();
    };
    for ch in message.chars() {
        if ch.is_whitespace() {
            flush(&mut out, &mut current);
            out.push(ch);
        } else {
            current.push(ch);
        }
    }
    flush(&mut out, &mut current);
    out
}

/// `ChatService.MutePlayer(Username, Duration)`.
fn mute_player(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let duration = args.number(1, "Duration")?;
    require_player_exists(&ctx.runtime, &username)?;
    if duration <= 0.0 {
        return Err(args.rule("Duration must be positive; use UnmutePlayer to lift a mute"));
    }
    ctx.runtime.chat_mutes.write().insert(username, duration);
    Ok(Value::from(duration))
}

/// `ChatService.UnmutePlayer(Username)`.
fn unmute_player(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    require_player_exists(&ctx.runtime, &username)?;
    if ctx.runtime.chat_mutes.write().remove(&username).is_none() {
        return Err(ApiError::NotFound {
            kind: "mute",
            name: username,
        });
    }
    Ok(Value::Bool(true))
}

/// `ChatService.SetFilter(ChannelName, FilterLevel)`.
fn set_filter(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let channel = args.string(0, "ChannelName")?;
    let level = args.string(1, "FilterLevel")?;
    match ctx.runtime.chat_channels.write().get_mut(&channel) {
        Some(slot) => {
            *slot = level.clone();
            Ok(Value::from(level))
        }
        None => Err(ApiError::NotFound {
            kind: "chat channel",
            name: channel,
        }),
    }
}

/// `ChatService.ShowBubble(Username, Message, Duration)` — client only.
fn show_bubble(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let message = args.string(1, "Message")?;
    let duration = args.number(2, "Duration")?;
    if duration <= 0.0 {
        return Err(args.rule("Duration must be positive"));
    }
    ctx.runtime
        .chat_bubbles
        .write()
        .insert(username, message.clone());
    Ok(Value::from(message))
}

/// `ChatService.HideBubble(Username)` — client only.
fn hide_bubble(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    match ctx.runtime.chat_bubbles.write().remove(&username) {
        Some(_) => Ok(Value::Bool(true)),
        None => Err(ApiError::NotFound {
            kind: "chat bubble",
            name: username,
        }),
    }
}

/// `ChatService.OnMessage(ChannelName, Callback)`.
fn on_message(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let channel = args.string(0, "ChannelName")?;
    if !ctx.runtime.chat_channels.read().contains_key(&channel) {
        return Err(ApiError::NotFound {
            kind: "chat channel",
            name: channel,
        });
    }
    let key = ctx
        .runtime
        .register_callback("ChatService", "OnMessage", Some(&channel));
    Ok(Value::from(key))
}

// --- BadgeService ---------------------------------------------------------

/// `BadgeService.Register(BadgeId, Name, Description)`.
fn register_badge(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let id = args.string(0, "BadgeId")?;
    let name = args.string(1, "Name")?;
    let description = args.string_or(2, "Description", "")?;
    let mut badges = ctx.runtime.badges.write();
    if badges.contains_key(&id) {
        return Err(ApiError::AlreadyExists {
            kind: "badge",
            name: id,
        });
    }
    badges.insert(
        id.clone(),
        crate::runtime::Badge {
            id: id.clone(),
            name,
            description,
        },
    );
    Ok(Value::from(id))
}

/// `BadgeService.Unregister(BadgeId)`.
fn unregister_badge(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let id = args.string(0, "BadgeId")?;
    match ctx.runtime.badges.write().remove(&id) {
        Some(_) => Ok(Value::Bool(true)),
        None => Err(ApiError::NotFound {
            kind: "badge",
            name: id,
        }),
    }
}

/// `BadgeService.Award(Username, BadgeId)`.
fn award_badge(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let id = args.string(1, "BadgeId")?;
    require_player_exists(&ctx.runtime, &username)?;
    if !ctx.runtime.badges.read().contains_key(&id) {
        return Err(ApiError::NotFound {
            kind: "badge",
            name: id,
        });
    }
    ctx.runtime
        .badge_owners
        .set(format!("{username}:{id}"), Value::Bool(true))
        .map_err(ApiError::Store)?;
    Ok(Value::Bool(true))
}

/// `BadgeService.Revoke(Username, BadgeId)`.
fn revoke_badge(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let id = args.string(1, "BadgeId")?;
    require_player_exists(&ctx.runtime, &username)?;
    if ctx
        .runtime
        .badge_owners
        .remove(&format!("{username}:{id}"))
        .is_none()
    {
        return Err(ApiError::NotFound {
            kind: "badge ownership",
            name: format!("{username}:{id}"),
        });
    }
    Ok(Value::Bool(true))
}

/// `BadgeService.HasBadge(Username, BadgeId)`.
fn has_badge(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let id = args.string(1, "BadgeId")?;
    Ok(Value::Bool(
        ctx.runtime
            .badge_owners
            .contains(&format!("{username}:{id}")),
    ))
}

/// `BadgeService.GetBadges(Username)` — the ids a player owns, sorted.
fn get_badges(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    Ok(Value::Array(owned_ids(
        &ctx.runtime.badge_owners,
        &username,
    )))
}

/// The ids a player owns in a `Username:Id`-keyed store, sorted.
fn owned_ids(store: &nb_store::KeyedStore, username: &str) -> Vec<Value> {
    let prefix = format!("{username}:");
    store
        .keys()
        .into_iter()
        .filter_map(|key| {
            key.strip_prefix(&prefix)
                .map(|id| Value::from(id.to_owned()))
        })
        .collect()
}

/// `BadgeService.GetAll()` — every registered badge.
fn get_all_badges(ctx: &ApiContext) -> ApiResult<Value> {
    let badges = ctx.runtime.badges.read();
    Ok(Value::array(badges.values().map(|b| {
        Value::map([
            ("BadgeId", Value::from(b.id.clone())),
            ("Name", Value::from(b.name.clone())),
            ("Description", Value::from(b.description.clone())),
        ])
    })))
}

/// `BadgeService.OnAward(Username, Callback)`.
fn on_award(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    require_player_exists(&ctx.runtime, &username)?;
    let key = ctx
        .runtime
        .register_callback("BadgeService", "OnAward", Some(&username));
    Ok(Value::from(key))
}

// --- PassService ----------------------------------------------------------

/// Reads a documented `Permissions` argument.
///
/// The documentation calls the parameter a set of permissions; both shapes a
/// script naturally writes are accepted: an array of names, or a map of name to
/// boolean. A `false` entry grants nothing, so a disabled permission cannot be
/// switched on by its mere presence.
fn parse_permissions(permissions: &Value) -> BTreeSet<String> {
    let mut parsed = BTreeSet::new();
    match permissions {
        Value::Array(items) => {
            for item in items {
                if let Some(name) = item.as_str() {
                    parsed.insert(name.to_owned());
                }
            }
        }
        Value::Map(map) => {
            for (name, value) in map {
                if value.is_truthy() {
                    parsed.insert(name.clone());
                }
            }
        }
        _ => {}
    }
    parsed
}

/// `PassService.Register(PassId, Name, Description, Permissions)`.
fn register_pass(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let id = args.string(0, "PassId")?;
    let name = args.string(1, "Name")?;
    let description = args.string_or(2, "Description", "")?;
    let permissions = args.table_or_empty(3, "Permissions")?;
    let parsed = parse_permissions(&permissions);

    let mut passes = ctx.runtime.passes.write();
    if passes.contains_key(&id) {
        return Err(ApiError::AlreadyExists {
            kind: "pass",
            name: id,
        });
    }
    passes.insert(
        id.clone(),
        crate::runtime::Pass {
            id: id.clone(),
            name,
            description,
            permissions: parsed,
            owners: BTreeSet::new(),
        },
    );
    Ok(Value::from(id))
}

/// `PassService.Unregister(PassId)`.
fn unregister_pass(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let id = args.string(0, "PassId")?;
    match ctx.runtime.passes.write().remove(&id) {
        Some(_) => Ok(Value::Bool(true)),
        None => Err(ApiError::NotFound {
            kind: "pass",
            name: id,
        }),
    }
}

/// `PassService.Grant(Username, PassId)`.
fn grant_pass(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let id = args.string(1, "PassId")?;
    require_player_exists(&ctx.runtime, &username)?;
    if !ctx.runtime.passes.read().contains_key(&id) {
        return Err(ApiError::NotFound {
            kind: "pass",
            name: id,
        });
    }
    if let Some(pass) = ctx.runtime.passes.write().get_mut(&id) {
        pass.owners.insert(username.clone());
    }
    ctx.runtime
        .pass_owners
        .set(format!("{username}:{id}"), Value::Bool(true))
        .map_err(ApiError::Store)?;
    Ok(Value::Bool(true))
}

/// `PassService.Revoke(Username, PassId)`.
fn revoke_pass(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let id = args.string(1, "PassId")?;
    require_player_exists(&ctx.runtime, &username)?;

    let removed = ctx
        .runtime
        .pass_owners
        .remove(&format!("{username}:{id}"))
        .is_some();
    if let Some(pass) = ctx.runtime.passes.write().get_mut(&id) {
        pass.owners.remove(&username);
    }
    if !removed {
        return Err(ApiError::NotFound {
            kind: "pass ownership",
            name: format!("{username}:{id}"),
        });
    }
    Ok(Value::Bool(true))
}

/// `PassService.HasPass(Username, PassId)`.
fn has_pass(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let id = args.string(1, "PassId")?;
    Ok(Value::Bool(
        ctx.runtime
            .pass_owners
            .contains(&format!("{username}:{id}")),
    ))
}

/// `PassService.GetPasses(Username)` — the ids a player owns, sorted.
fn get_passes(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    Ok(Value::Array(owned_ids(&ctx.runtime.pass_owners, &username)))
}

/// `PassService.GetAll()` — every registered pass.
fn get_all_passes(ctx: &ApiContext) -> ApiResult<Value> {
    let passes = ctx.runtime.passes.read();
    Ok(Value::array(passes.values().map(|p| {
        Value::map([
            ("PassId", Value::from(p.id.clone())),
            ("Name", Value::from(p.name.clone())),
            ("Description", Value::from(p.description.clone())),
            (
                "Permissions",
                Value::array(p.permissions.iter().cloned().map(Value::from)),
            ),
        ])
    })))
}

/// `PassService.CheckPermission(Username, Permission)`.
///
/// True when any pass the player owns grants the permission.
fn check_permission(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    let permission = args.string(1, "Permission")?;

    let owned: Vec<String> = {
        let prefix = format!("{username}:");
        ctx.runtime
            .pass_owners
            .keys()
            .into_iter()
            .filter_map(|key| key.strip_prefix(&prefix).map(str::to_owned))
            .collect()
    };

    let passes = ctx.runtime.passes.read();
    let granted = owned.iter().any(|id| {
        passes
            .get(id)
            .is_some_and(|p| p.permissions.contains(&permission))
    });
    Ok(Value::Bool(granted))
}

/// `PassService.OnGrant(Username, Callback)`.
fn on_grant(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let username = args.string(0, "Username")?;
    require_player_exists(&ctx.runtime, &username)?;
    let key = ctx
        .runtime
        .register_callback("PassService", "OnGrant", Some(&username));
    Ok(Value::from(key))
}

/// The translucent black an overlay starts at, named so the value is not a
/// bare literal in a caller.
pub fn default_overlay_colour() -> Rgba {
    Rgba::new(0.0, 0.0, 0.0, 0.5)
}

/// The origin, exposed so a default camera target is named rather than literal.
pub fn default_camera_target() -> Vector3 {
    Vector3::ZERO
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Side;
    use crate::runtime::{ApiContext, PlayerRecord};
    use crate::spec;

    fn server() -> ApiContext {
        let ctx = ApiContext::new(Side::Server);
        ctx.runtime.player_join(PlayerRecord::new("Alice", 1, "VN"));
        ctx.runtime.player_join(PlayerRecord::new("Bob", 2, "US"));
        ctx
    }

    fn client() -> ApiContext {
        let ctx = ApiContext::new(Side::Client);
        ctx.runtime.player_join(PlayerRecord::new("Alice", 1, "VN"));
        ctx
    }

    /// Call a function in this group.
    fn call(ctx: &ApiContext, service: &str, method: &str, values: Vec<Value>) -> ApiResult<Value> {
        let args = Args::new(service, method, values);
        dispatch(ctx, service, method, &args)
    }

    // --- PlayerService ---------------------------------------------------

    #[test]
    fn get_players_lists_only_online_players() {
        let ctx = server();
        let result = call(&ctx, "PlayerService", "GetPlayers", vec![]).unwrap();
        let names: Vec<&str> = result
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(names, vec!["Alice", "Bob"]);

        ctx.runtime.player_leave("Bob");
        assert_eq!(
            call(&ctx, "PlayerService", "GetPlayers", vec![])
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn get_player_returns_a_record_or_nil() {
        let ctx = server();
        let found = call(
            &ctx,
            "PlayerService",
            "GetPlayer",
            vec![Value::from("Alice")],
        )
        .unwrap();
        assert_eq!(found.get("Username").and_then(Value::as_str), Some("Alice"));
        assert_eq!(found.get("UserId").and_then(Value::as_i64), Some(1));
        assert_eq!(found.get("Country").and_then(Value::as_str), Some("VN"));

        let missing = call(
            &ctx,
            "PlayerService",
            "GetPlayer",
            vec![Value::from("Ghost")],
        )
        .unwrap();
        assert!(
            missing.is_nil(),
            "an unknown player reads as nil, not an error"
        );
    }

    #[test]
    fn id_based_accessors_resolve_the_player() {
        let ctx = server();
        assert_eq!(
            call(
                &ctx,
                "PlayerService",
                "GetPlayerName",
                vec![Value::from(2i64)]
            )
            .unwrap()
            .as_str(),
            Some("Bob")
        );
        assert_eq!(
            call(
                &ctx,
                "PlayerService",
                "GetPlayerCountry",
                vec![Value::from(1i64)]
            )
            .unwrap()
            .as_str(),
            Some("VN")
        );
        let err = call(
            &ctx,
            "PlayerService",
            "GetPlayerName",
            vec![Value::from(99i64)],
        )
        .unwrap_err();
        assert!(matches!(err, ApiError::NotFound { kind: "player", .. }));
    }

    #[test]
    fn avatar_and_character_urls_are_derived() {
        let ctx = server();
        {
            let mut players = ctx.runtime.players.write();
            players.get_mut("Alice").unwrap().avatar_id = "av-1".to_owned();
        }
        assert_eq!(
            call(
                &ctx,
                "PlayerService",
                "GetPlayerAvatar",
                vec![Value::from(1i64)]
            )
            .unwrap()
            .as_str(),
            Some("av-1")
        );
        assert_eq!(
            call(
                &ctx,
                "PlayerService",
                "GetPlayerThumbnail",
                vec![Value::from(1i64)]
            )
            .unwrap()
            .as_str(),
            Some("av-1"),
            "the thumbnail is the same asset at another size"
        );
        assert_eq!(
            call(
                &ctx,
                "PlayerService",
                "GetPlayerAvatarUrl",
                vec![Value::from(1i64)]
            )
            .unwrap()
            .as_str(),
            Some("https://assets.noobietoria.local/avatar/av-1")
        );
        assert_eq!(
            call(
                &ctx,
                "PlayerService",
                "GetPlayerThumbnailUrl",
                vec![Value::from(1i64)]
            )
            .unwrap()
            .as_str(),
            Some("https://assets.noobietoria.local/thumbnail/av-1")
        );
        assert_eq!(
            call(
                &ctx,
                "PlayerService",
                "GetPlayerCharacter",
                vec![Value::from(1i64)]
            )
            .unwrap()
            .as_str(),
            Some("Alice:Character")
        );
        assert!(call(
            &ctx,
            "PlayerService",
            "GetPlayerCharacterUrl",
            vec![Value::from(1i64)]
        )
        .unwrap()
        .as_str()
        .unwrap()
        .starts_with("https://assets.noobietoria.local/character/"));
    }

    #[test]
    fn an_empty_avatar_yields_an_empty_url_not_a_broken_one() {
        let ctx = server();
        assert_eq!(
            call(
                &ctx,
                "PlayerService",
                "GetPlayerAvatarUrl",
                vec![Value::from(1i64)]
            )
            .unwrap()
            .as_str(),
            Some("")
        );
    }

    #[test]
    fn get_coors_and_teleport_move_the_player() {
        let ctx = server();
        let start = call(
            &ctx,
            "PlayerService",
            "GetCoors",
            vec![Value::from("Alice")],
        )
        .unwrap();
        assert_eq!(Vector3::from_value(&start), Some(Vector3::ZERO));

        let destination = Vector3::new(10.0, 20.0, -30.0);
        let moved = call(
            &ctx,
            "PlayerService",
            "Teleport",
            vec![Value::from("Alice"), destination.to_value()],
        )
        .unwrap();
        assert_eq!(Vector3::from_value(&moved), Some(destination));

        let now = call(
            &ctx,
            "PlayerService",
            "GetCoors",
            vec![Value::from("Alice")],
        )
        .unwrap();
        assert_eq!(Vector3::from_value(&now), Some(destination));
    }

    #[test]
    fn teleport_needs_a_real_player_and_a_real_position() {
        let ctx = server();
        assert!(matches!(
            call(
                &ctx,
                "PlayerService",
                "Teleport",
                vec![Value::from("Ghost"), Vector3::ZERO.to_value()]
            ),
            Err(ApiError::NotFound { kind: "player", .. })
        ));
        let err = call(
            &ctx,
            "PlayerService",
            "Teleport",
            vec![Value::from("Alice"), Value::from("over there")],
        )
        .unwrap_err();
        assert!(matches!(err, ApiError::BadArgument { param, .. } if param == "Coors"));
    }

    // --- BanService ------------------------------------------------------

    #[test]
    fn kick_disconnects_without_banning() {
        let ctx = server();
        let result = call(
            &ctx,
            "BanService",
            "Kick",
            vec![Value::from("Bob"), Value::from("afk")],
        )
        .unwrap();
        assert_eq!(result.get("Kicked").and_then(Value::as_bool), Some(true));
        assert!(!ctx.runtime.players.read().get("Bob").unwrap().online);
        assert!(
            !call(&ctx, "BanService", "IsBanned", vec![Value::from("Bob")])
                .unwrap()
                .as_bool()
                .unwrap()
        );
    }

    #[test]
    fn permanent_ban_then_unban() {
        let ctx = server();
        let result = call(
            &ctx,
            "BanService",
            "Ban",
            vec![
                Value::from("Bob"),
                Value::from("cheating"),
                Value::from(-1i64),
            ],
        )
        .unwrap();
        assert_eq!(result.get("Permanent").and_then(Value::as_bool), Some(true));
        assert!(result.get("ExpiresAt").unwrap().is_nil());

        assert!(
            call(&ctx, "BanService", "IsBanned", vec![Value::from("Bob")])
                .unwrap()
                .as_bool()
                .unwrap()
        );
        let info = call(&ctx, "BanService", "GetBanInfo", vec![Value::from("Bob")]).unwrap();
        assert_eq!(info.get("Reason").and_then(Value::as_str), Some("cheating"));

        call(&ctx, "BanService", "Unban", vec![Value::from("Bob")]).unwrap();
        assert!(
            !call(&ctx, "BanService", "IsBanned", vec![Value::from("Bob")])
                .unwrap()
                .as_bool()
                .unwrap()
        );
        assert!(
            call(&ctx, "BanService", "GetBanInfo", vec![Value::from("Bob")])
                .unwrap()
                .is_nil()
        );
    }

    #[test]
    fn a_timed_ban_expires_with_the_clock() {
        let ctx = server();
        call(
            &ctx,
            "BanService",
            "Ban",
            vec![Value::from("Bob"), Value::from("spam"), Value::from(10i64)],
        )
        .unwrap();
        assert!(
            call(&ctx, "BanService", "IsBanned", vec![Value::from("Bob")])
                .unwrap()
                .as_bool()
                .unwrap()
        );

        ctx.runtime.advance(9.0);
        assert!(
            call(&ctx, "BanService", "IsBanned", vec![Value::from("Bob")])
                .unwrap()
                .as_bool()
                .unwrap(),
            "nine seconds is still inside a ten-second ban"
        );

        ctx.runtime.advance(1.5);
        assert!(
            !call(&ctx, "BanService", "IsBanned", vec![Value::from("Bob")])
                .unwrap()
                .as_bool()
                .unwrap(),
            "a ten-second ban must not outlive its duration"
        );
    }

    #[test]
    fn unban_without_a_ban_is_reported() {
        let ctx = server();
        let err = call(&ctx, "BanService", "Unban", vec![Value::from("Bob")]).unwrap_err();
        assert!(matches!(err, ApiError::NotFound { kind: "ban", .. }));
    }

    #[test]
    fn warnings_accumulate_and_clear() {
        let ctx = server();
        assert_eq!(
            call(
                &ctx,
                "BanService",
                "Warn",
                vec![Value::from("Alice"), Value::from("spam")]
            )
            .unwrap()
            .as_i64(),
            Some(1)
        );
        assert_eq!(
            call(
                &ctx,
                "BanService",
                "Warn",
                vec![Value::from("Alice"), Value::from("rudeness")]
            )
            .unwrap()
            .as_i64(),
            Some(2)
        );

        let list = call(
            &ctx,
            "BanService",
            "GetWarnings",
            vec![Value::from("Alice")],
        )
        .unwrap();
        assert_eq!(list.as_array().unwrap().len(), 2);
        assert_eq!(
            list.as_array().unwrap()[1].as_str(),
            Some("rudeness"),
            "warnings keep their order"
        );

        call(
            &ctx,
            "BanService",
            "ClearWarnings",
            vec![Value::from("Alice")],
        )
        .unwrap();
        assert!(call(
            &ctx,
            "BanService",
            "GetWarnings",
            vec![Value::from("Alice")]
        )
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    }

    #[test]
    fn banning_preserves_existing_warnings() {
        let ctx = server();
        call(
            &ctx,
            "BanService",
            "Warn",
            vec![Value::from("Bob"), Value::from("spam")],
        )
        .unwrap();
        call(
            &ctx,
            "BanService",
            "Ban",
            vec![
                Value::from("Bob"),
                Value::from("repeat"),
                Value::from(-1i64),
            ],
        )
        .unwrap();
        assert_eq!(
            call(&ctx, "BanService", "GetWarnings", vec![Value::from("Bob")])
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1,
            "a ban must not erase the warning history"
        );
    }

    #[test]
    fn moderation_actions_on_an_unknown_player_are_refused() {
        let ctx = server();
        assert!(matches!(
            call(
                &ctx,
                "BanService",
                "Ban",
                vec![Value::from("Ghost"), Value::from("x"), Value::from(-1i64)]
            ),
            Err(ApiError::NotFound { kind: "player", .. })
        ));
        assert!(matches!(
            call(
                &ctx,
                "BanService",
                "Kick",
                vec![Value::from("Ghost"), Value::from("x")]
            ),
            Err(ApiError::NotFound { kind: "player", .. })
        ));
        assert!(matches!(
            call(
                &ctx,
                "BanService",
                "Warn",
                vec![Value::from("Ghost"), Value::from("x")]
            ),
            Err(ApiError::NotFound { kind: "player", .. })
        ));
    }

    #[test]
    fn ban_history_survives_the_player_leaving_and_honours_limit() {
        let ctx = server();
        call(
            &ctx,
            "BanService",
            "Ban",
            vec![Value::from("Bob"), Value::from("first"), Value::from(-1i64)],
        )
        .unwrap();
        call(&ctx, "BanService", "Unban", vec![Value::from("Bob")]).unwrap();
        call(
            &ctx,
            "BanService",
            "Ban",
            vec![
                Value::from("Bob"),
                Value::from("second"),
                Value::from(-1i64),
            ],
        )
        .unwrap();
        assert!(!ctx.runtime.players.read().get("Bob").unwrap().online);

        let history = call(
            &ctx,
            "BanService",
            "GetBanHistory",
            vec![Value::from("Bob"), Value::from(10i64)],
        )
        .unwrap();
        let entries = history.as_array().unwrap();
        assert_eq!(entries.len(), 3, "two bans plus the unban");
        assert_eq!(
            entries[0].get("Reason").and_then(Value::as_str),
            Some("second"),
            "newest first"
        );

        let limited = call(
            &ctx,
            "BanService",
            "GetBanHistory",
            vec![Value::from("Bob"), Value::from(1i64)],
        )
        .unwrap();
        assert_eq!(limited.as_array().unwrap().len(), 1);
    }

    #[test]
    fn clearing_warnings_that_do_not_exist_is_reported() {
        let ctx = server();
        let err = call(
            &ctx,
            "BanService",
            "ClearWarnings",
            vec![Value::from("Bob")],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ApiError::NotFound {
                kind: "warning",
                ..
            }
        ));
    }

    // --- CameraService ---------------------------------------------------

    #[test]
    fn camera_accepts_settings_and_reports_them() {
        let ctx = client();
        assert_eq!(
            call(
                &ctx,
                "CameraService",
                "SetFOV",
                vec![Value::from("Alice"), Value::from(90i64)]
            )
            .unwrap()
            .as_f64(),
            Some(90.0)
        );
        call(
            &ctx,
            "CameraService",
            "SetType",
            vec![Value::from("Alice"), Value::from("Scriptable")],
        )
        .unwrap();
        let camera = ctx.runtime.cameras.read().get("Alice").cloned().unwrap();
        assert_eq!(camera.kind, "Scriptable");
        assert_eq!(camera.fov, 90.0);
    }

    #[test]
    fn camera_fov_is_range_checked() {
        let ctx = client();
        for bad in [0.0, -10.0, 180.0, 400.0] {
            let err = call(
                &ctx,
                "CameraService",
                "SetFOV",
                vec![Value::from("Alice"), Value::from(bad)],
            )
            .unwrap_err();
            assert!(
                matches!(err, ApiError::Rule(ref m) if m.contains("FieldOfView")),
                "{err}"
            );
        }
    }

    #[test]
    fn camera_position_sets_the_look_target_and_clears_a_lock() {
        let ctx = client();
        call(
            &ctx,
            "CameraService",
            "LockTo",
            vec![Value::from("Alice"), Value::from("Dragon")],
        )
        .unwrap();
        let pos = Vector3::new(0.0, 10.0, 20.0);
        call(
            &ctx,
            "CameraService",
            "SetPosition",
            vec![
                Value::from("Alice"),
                pos.to_value(),
                Vector3::ZERO.to_value(),
            ],
        )
        .unwrap();
        let camera = ctx.runtime.cameras.read().get("Alice").cloned().unwrap();
        assert_eq!(camera.position, pos);
        assert_eq!(camera.look_at, Vector3::ZERO);
        assert!(
            camera.locked_to.is_none(),
            "an explicit position supersedes a lock"
        );
    }

    #[test]
    fn camera_lock_unlock_and_duplicate_unlock() {
        let ctx = client();
        call(
            &ctx,
            "CameraService",
            "LockTo",
            vec![Value::from("Alice"), Value::from("Dragon")],
        )
        .unwrap();
        assert_eq!(
            ctx.runtime
                .cameras
                .read()
                .get("Alice")
                .unwrap()
                .locked_to
                .as_deref(),
            Some("Dragon")
        );
        call(&ctx, "CameraService", "Unlock", vec![Value::from("Alice")]).unwrap();
        let err = call(&ctx, "CameraService", "Unlock", vec![Value::from("Alice")]).unwrap_err();
        assert!(matches!(
            err,
            ApiError::NotFound {
                kind: "camera lock",
                ..
            }
        ));
    }

    #[test]
    fn camera_lock_needs_a_target() {
        let ctx = client();
        assert!(matches!(
            call(
                &ctx,
                "CameraService",
                "LockTo",
                vec![Value::from("Alice"), Value::from("")]
            ),
            Err(ApiError::Rule(_))
        ));
    }

    #[test]
    fn camera_shake_and_reset() {
        let ctx = client();
        call(
            &ctx,
            "CameraService",
            "Shake",
            vec![Value::from("Alice"), Value::from(2.5), Value::from(1.0)],
        )
        .unwrap();
        let camera = ctx.runtime.cameras.read().get("Alice").cloned().unwrap();
        assert_eq!(camera.shake, 2.5);
        assert!(camera.shake_until > 0.0);

        call(&ctx, "CameraService", "Reset", vec![Value::from("Alice")]).unwrap();
        let reset = ctx.runtime.cameras.read().get("Alice").cloned().unwrap();
        assert_eq!(reset.shake, 0.0);
        assert_eq!(reset.fov, 70.0);
        assert_eq!(reset.kind, "Custom");
    }

    #[test]
    fn camera_shake_validates_its_arguments() {
        let ctx = client();
        assert!(matches!(
            call(
                &ctx,
                "CameraService",
                "Shake",
                vec![Value::from("Alice"), Value::from(-1.0), Value::from(1.0)]
            ),
            Err(ApiError::Rule(_))
        ));
        assert!(matches!(
            call(
                &ctx,
                "CameraService",
                "Shake",
                vec![Value::from("Alice"), Value::from(1.0), Value::from(0.0)]
            ),
            Err(ApiError::Rule(_))
        ));
    }

    #[test]
    fn camera_actions_need_a_real_player() {
        let ctx = client();
        let err = call(
            &ctx,
            "CameraService",
            "SetFOV",
            vec![Value::from("Ghost"), Value::from(70i64)],
        )
        .unwrap_err();
        assert!(matches!(err, ApiError::NotFound { kind: "player", .. }));
    }

    // --- NotificationService ---------------------------------------------

    #[test]
    fn send_targets_one_player_and_returns_an_id() {
        let ctx = server();
        let ids = call(
            &ctx,
            "NotificationService",
            "Send",
            vec![
                Value::from("Alice"),
                Value::from("Welcome"),
                Value::from("Have fun"),
                Value::from("Toast"),
                Value::from(3.0),
            ],
        )
        .unwrap();
        assert_eq!(ids.as_array().unwrap().len(), 1);

        let notifications = ctx.runtime.notifications.lock();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].to, "Alice");
        assert_eq!(notifications[0].kind, "Toast");
        assert_eq!(notifications[0].duration, 3.0);
    }

    #[test]
    fn send_all_reaches_every_online_player() {
        let ctx = server();
        let ids = call(
            &ctx,
            "NotificationService",
            "SendAll",
            vec![
                Value::from("Server"),
                Value::from("Restarting soon"),
                Value::from("Banner"),
                Value::from(5.0),
            ],
        )
        .unwrap();
        assert_eq!(ids.as_array().unwrap().len(), 2);
        assert_eq!(ctx.runtime.notifications.lock().len(), 2);
    }

    #[test]
    fn wildcard_send_reaches_every_online_player() {
        let ctx = server();
        let ids = call(
            &ctx,
            "NotificationService",
            "Send",
            vec![
                Value::from("*"),
                Value::from("All"),
                Value::from("Body"),
                Value::from("Toast"),
                Value::from(1.0),
            ],
        )
        .unwrap();
        assert_eq!(ids.as_array().unwrap().len(), 2);
    }

    #[test]
    fn notification_type_and_duration_are_validated() {
        let ctx = server();
        let err = call(
            &ctx,
            "NotificationService",
            "Send",
            vec![
                Value::from("Alice"),
                Value::from("Hi"),
                Value::from("there"),
                Value::from("Marquee"),
                Value::from(1.0),
            ],
        )
        .unwrap_err();
        assert!(
            matches!(err, ApiError::Rule(ref m) if m.contains("Marquee")),
            "{err}"
        );

        assert!(matches!(
            call(
                &ctx,
                "NotificationService",
                "Send",
                vec![
                    Value::from("Alice"),
                    Value::from("Hi"),
                    Value::from("there"),
                    Value::from("Toast"),
                    Value::from(0.0)
                ]
            ),
            Err(ApiError::Rule(_))
        ));
    }

    #[test]
    fn dismissing_marks_the_notification_and_an_unknown_id_fails() {
        let ctx = server();
        let ids = call(
            &ctx,
            "NotificationService",
            "Send",
            vec![
                Value::from("Alice"),
                Value::from("One"),
                Value::from("Two"),
                Value::from("Popup"),
                Value::from(2.0),
            ],
        )
        .unwrap();
        let id = ids.as_array().unwrap()[0].as_i64().unwrap();

        call(
            &ctx,
            "NotificationService",
            "Dismiss",
            vec![Value::from("Alice"), Value::from(id)],
        )
        .unwrap();
        assert!(ctx.runtime.notifications.lock()[0].dismissed);

        let err = call(
            &ctx,
            "NotificationService",
            "Dismiss",
            vec![Value::from("Alice"), Value::from(9999i64)],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ApiError::NotFound {
                kind: "notification",
                ..
            }
        ));
    }

    #[test]
    fn dismissing_with_a_wildcard_reaches_any_recipient() {
        let ctx = server();
        let ids = call(
            &ctx,
            "NotificationService",
            "SendAll",
            vec![
                Value::from("All"),
                Value::from("Body"),
                Value::from("Toast"),
                Value::from(1.0),
            ],
        )
        .unwrap();
        let id = ids.as_array().unwrap()[0].as_i64().unwrap();
        call(
            &ctx,
            "NotificationService",
            "Dismiss",
            vec![Value::from("*"), Value::from(id)],
        )
        .unwrap();
        assert!(ctx.runtime.notifications.lock()[0].dismissed);
    }

    #[test]
    fn on_dismiss_registers_a_callback() {
        let ctx = server();
        call(
            &ctx,
            "NotificationService",
            "OnDismiss",
            vec![Value::from("Alice"), Value::Nil],
        )
        .unwrap();
        assert!(ctx
            .runtime
            .has_callback("NotificationService", "OnDismiss", Some("Alice")));
    }

    // --- ChatService -----------------------------------------------------

    #[test]
    fn channel_lifecycle_rejects_duplicates_and_unknown_names() {
        let ctx = server();
        call(
            &ctx,
            "ChatService",
            "CreateChannel",
            vec![Value::from("Global")],
        )
        .unwrap();
        assert!(ctx.runtime.chat_channels.read().contains_key("Global"));

        let dup = call(
            &ctx,
            "ChatService",
            "CreateChannel",
            vec![Value::from("Global")],
        )
        .unwrap_err();
        assert!(matches!(
            dup,
            ApiError::AlreadyExists {
                kind: "chat channel",
                ..
            }
        ));

        call(
            &ctx,
            "ChatService",
            "DeleteChannel",
            vec![Value::from("Global")],
        )
        .unwrap();
        let err = call(
            &ctx,
            "ChatService",
            "DeleteChannel",
            vec![Value::from("Global")],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ApiError::NotFound {
                kind: "chat channel",
                ..
            }
        ));
    }

    #[test]
    fn joining_and_leaving_a_channel_tracks_membership() {
        let ctx = server();
        call(
            &ctx,
            "ChatService",
            "CreateChannel",
            vec![Value::from("Team")],
        )
        .unwrap();
        call(
            &ctx,
            "ChatService",
            "JoinChannel",
            vec![Value::from("Alice"), Value::from("Team")],
        )
        .unwrap();
        assert!(ctx
            .runtime
            .chat_members
            .read()
            .get("Team")
            .unwrap()
            .contains("Alice"));

        call(
            &ctx,
            "ChatService",
            "LeaveChannel",
            vec![Value::from("Alice"), Value::from("Team")],
        )
        .unwrap();
        assert!(!ctx
            .runtime
            .chat_members
            .read()
            .get("Team")
            .unwrap()
            .contains("Alice"));

        let err = call(
            &ctx,
            "ChatService",
            "LeaveChannel",
            vec![Value::from("Alice"), Value::from("Team")],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ApiError::NotFound {
                kind: "channel membership",
                ..
            }
        ));
    }

    #[test]
    fn joining_an_unknown_channel_is_refused() {
        let ctx = server();
        let err = call(
            &ctx,
            "ChatService",
            "JoinChannel",
            vec![Value::from("Alice"), Value::from("Nowhere")],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ApiError::NotFound {
                kind: "chat channel",
                ..
            }
        ));
    }

    #[test]
    fn messages_are_logged_with_the_channel_filter_applied() {
        let ctx = server();
        call(
            &ctx,
            "ChatService",
            "CreateChannel",
            vec![Value::from("Global")],
        )
        .unwrap();
        let sent = call(
            &ctx,
            "ChatService",
            "SendMessage",
            vec![
                Value::from("Alice"),
                Value::from("Global"),
                Value::from("hello world"),
            ],
        )
        .unwrap();
        assert_eq!(sent.as_str(), Some("hello world"));
        assert_eq!(ctx.runtime.chat_log.lock().len(), 1);

        let filtered = call(
            &ctx,
            "ChatService",
            "SendMessage",
            vec![
                Value::from("Alice"),
                Value::from("Global"),
                Value::from("you idiot"),
            ],
        )
        .unwrap();
        assert_eq!(filtered.as_str(), Some("you *****"));
    }

    #[test]
    fn filter_levels_change_what_is_censored() {
        let ctx = server();
        call(
            &ctx,
            "ChatService",
            "CreateChannel",
            vec![Value::from("Global")],
        )
        .unwrap();
        call(
            &ctx,
            "ChatService",
            "SetFilter",
            vec![Value::from("Global"), Value::from("None")],
        )
        .unwrap();
        assert_eq!(
            call(
                &ctx,
                "ChatService",
                "SendMessage",
                vec![
                    Value::from("Alice"),
                    Value::from("Global"),
                    Value::from("you idiot")
                ]
            )
            .unwrap()
            .as_str(),
            Some("you idiot"),
            "filter None passes text through"
        );

        call(
            &ctx,
            "ChatService",
            "SetFilter",
            vec![Value::from("Global"), Value::from("Strict")],
        )
        .unwrap();
        assert_eq!(
            call(
                &ctx,
                "ChatService",
                "SendMessage",
                vec![
                    Value::from("Alice"),
                    Value::from("Global"),
                    Value::from("see https://bad.example")
                ]
            )
            .unwrap()
            .as_str(),
            Some("see *******************")
        );
    }

    #[test]
    fn a_muted_player_cannot_speak() {
        let ctx = server();
        call(
            &ctx,
            "ChatService",
            "CreateChannel",
            vec![Value::from("Global")],
        )
        .unwrap();
        call(
            &ctx,
            "ChatService",
            "MutePlayer",
            vec![Value::from("Alice"), Value::from(30i64)],
        )
        .unwrap();

        let err = call(
            &ctx,
            "ChatService",
            "SendMessage",
            vec![
                Value::from("Alice"),
                Value::from("Global"),
                Value::from("let me in"),
            ],
        )
        .unwrap_err();
        assert!(
            matches!(err, ApiError::Rule(ref m) if m.contains("muted")),
            "{err}"
        );

        assert!(
            call(
                &ctx,
                "ChatService",
                "SendMessage",
                vec![
                    Value::from("Bob"),
                    Value::from("Global"),
                    Value::from("hello")
                ]
            )
            .is_ok(),
            "another player is unaffected"
        );

        call(
            &ctx,
            "ChatService",
            "UnmutePlayer",
            vec![Value::from("Alice")],
        )
        .unwrap();
        assert!(call(
            &ctx,
            "ChatService",
            "SendMessage",
            vec![
                Value::from("Alice"),
                Value::from("Global"),
                Value::from("thanks")
            ]
        )
        .is_ok());
    }

    #[test]
    fn muting_validates_and_unmuting_without_a_mute_is_reported() {
        let ctx = server();
        assert!(matches!(
            call(
                &ctx,
                "ChatService",
                "MutePlayer",
                vec![Value::from("Alice"), Value::from(0i64)]
            ),
            Err(ApiError::Rule(_))
        ));
        let err = call(
            &ctx,
            "ChatService",
            "UnmutePlayer",
            vec![Value::from("Alice")],
        )
        .unwrap_err();
        assert!(matches!(err, ApiError::NotFound { kind: "mute", .. }));
    }

    #[test]
    fn sending_to_an_unknown_channel_is_refused() {
        let ctx = server();
        let err = call(
            &ctx,
            "ChatService",
            "SendMessage",
            vec![Value::from("Alice"), Value::from("Nope"), Value::from("hi")],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ApiError::NotFound {
                kind: "chat channel",
                ..
            }
        ));
    }

    #[test]
    fn setting_a_filter_on_an_unknown_channel_is_refused() {
        let ctx = server();
        let err = call(
            &ctx,
            "ChatService",
            "SetFilter",
            vec![Value::from("Nope"), Value::from("None")],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ApiError::NotFound {
                kind: "chat channel",
                ..
            }
        ));
    }

    #[test]
    fn bubbles_are_shown_and_hidden() {
        let ctx = client();
        call(
            &ctx,
            "ChatService",
            "ShowBubble",
            vec![Value::from("Alice"), Value::from("Hi!"), Value::from(2.0)],
        )
        .unwrap();
        assert_eq!(
            ctx.runtime
                .chat_bubbles
                .read()
                .get("Alice")
                .map(String::as_str),
            Some("Hi!")
        );
        call(
            &ctx,
            "ChatService",
            "HideBubble",
            vec![Value::from("Alice")],
        )
        .unwrap();
        assert!(ctx.runtime.chat_bubbles.read().is_empty());

        let err = call(
            &ctx,
            "ChatService",
            "HideBubble",
            vec![Value::from("Alice")],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ApiError::NotFound {
                kind: "chat bubble",
                ..
            }
        ));
    }

    #[test]
    fn on_message_needs_a_real_channel() {
        let ctx = server();
        assert!(matches!(
            call(
                &ctx,
                "ChatService",
                "OnMessage",
                vec![Value::from("Global"), Value::Nil]
            ),
            Err(ApiError::NotFound {
                kind: "chat channel",
                ..
            })
        ));
        call(
            &ctx,
            "ChatService",
            "CreateChannel",
            vec![Value::from("Global")],
        )
        .unwrap();
        call(
            &ctx,
            "ChatService",
            "OnMessage",
            vec![Value::from("Global"), Value::Nil],
        )
        .unwrap();
        assert!(ctx
            .runtime
            .has_callback("ChatService", "OnMessage", Some("Global")));
    }

    // --- BadgeService ----------------------------------------------------

    #[test]
    fn badge_registration_award_and_ownership() {
        let ctx = server();
        call(
            &ctx,
            "BadgeService",
            "Register",
            vec![
                Value::from("first_win"),
                Value::from("First Win"),
                Value::from("Win a round"),
            ],
        )
        .unwrap();

        assert!(!call(
            &ctx,
            "BadgeService",
            "HasBadge",
            vec![Value::from("Alice"), Value::from("first_win")]
        )
        .unwrap()
        .as_bool()
        .unwrap());

        call(
            &ctx,
            "BadgeService",
            "Award",
            vec![Value::from("Alice"), Value::from("first_win")],
        )
        .unwrap();
        assert!(call(
            &ctx,
            "BadgeService",
            "HasBadge",
            vec![Value::from("Alice"), Value::from("first_win")]
        )
        .unwrap()
        .as_bool()
        .unwrap());

        assert_eq!(
            call(
                &ctx,
                "BadgeService",
                "GetBadges",
                vec![Value::from("Alice")]
            )
            .unwrap()
            .as_array()
            .unwrap()[0]
                .as_str(),
            Some("first_win")
        );

        call(
            &ctx,
            "BadgeService",
            "Revoke",
            vec![Value::from("Alice"), Value::from("first_win")],
        )
        .unwrap();
        assert!(!call(
            &ctx,
            "BadgeService",
            "HasBadge",
            vec![Value::from("Alice"), Value::from("first_win")]
        )
        .unwrap()
        .as_bool()
        .unwrap());
    }

    #[test]
    fn badge_registration_rejects_duplicates_and_awards_unknown_ids() {
        let ctx = server();
        call(
            &ctx,
            "BadgeService",
            "Register",
            vec![Value::from("b1"), Value::from("B1"), Value::from("")],
        )
        .unwrap();
        let dup = call(
            &ctx,
            "BadgeService",
            "Register",
            vec![Value::from("b1"), Value::from("B1"), Value::from("")],
        )
        .unwrap_err();
        assert!(matches!(dup, ApiError::AlreadyExists { kind: "badge", .. }));

        let unknown = call(
            &ctx,
            "BadgeService",
            "Award",
            vec![Value::from("Alice"), Value::from("ghost")],
        )
        .unwrap_err();
        assert!(matches!(unknown, ApiError::NotFound { kind: "badge", .. }));

        assert!(matches!(
            call(
                &ctx,
                "BadgeService",
                "Unregister",
                vec![Value::from("ghost")]
            ),
            Err(ApiError::NotFound { kind: "badge", .. })
        ));
    }

    #[test]
    fn badge_get_all_lists_every_registered_badge() {
        let ctx = server();
        call(
            &ctx,
            "BadgeService",
            "Register",
            vec![Value::from("b1"), Value::from("One"), Value::from("first")],
        )
        .unwrap();
        call(
            &ctx,
            "BadgeService",
            "Register",
            vec![Value::from("b2"), Value::from("Two"), Value::from("second")],
        )
        .unwrap();
        assert_eq!(
            call(&ctx, "BadgeService", "GetAll", vec![])
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn awarding_to_an_unknown_player_and_revoking_a_missing_badge_are_reported() {
        let ctx = server();
        call(
            &ctx,
            "BadgeService",
            "Register",
            vec![Value::from("b1"), Value::from("B1"), Value::from("")],
        )
        .unwrap();
        assert!(matches!(
            call(
                &ctx,
                "BadgeService",
                "Award",
                vec![Value::from("Ghost"), Value::from("b1")]
            ),
            Err(ApiError::NotFound { kind: "player", .. })
        ));
        assert!(matches!(
            call(
                &ctx,
                "BadgeService",
                "Revoke",
                vec![Value::from("Alice"), Value::from("b1")]
            ),
            Err(ApiError::NotFound {
                kind: "badge ownership",
                ..
            })
        ));
    }

    // --- PassService -----------------------------------------------------

    #[test]
    fn pass_registration_grant_and_permissions() {
        let ctx = server();
        call(
            &ctx,
            "PassService",
            "Register",
            vec![
                Value::from("vip"),
                Value::from("VIP"),
                Value::from("Extra perks"),
                Value::array([Value::from("fly"), Value::from("chat_colour")]),
            ],
        )
        .unwrap();

        assert!(!call(
            &ctx,
            "PassService",
            "CheckPermission",
            vec![Value::from("Alice"), Value::from("fly")]
        )
        .unwrap()
        .as_bool()
        .unwrap());

        call(
            &ctx,
            "PassService",
            "Grant",
            vec![Value::from("Alice"), Value::from("vip")],
        )
        .unwrap();
        assert!(call(
            &ctx,
            "PassService",
            "HasPass",
            vec![Value::from("Alice"), Value::from("vip")]
        )
        .unwrap()
        .as_bool()
        .unwrap());
        assert!(call(
            &ctx,
            "PassService",
            "CheckPermission",
            vec![Value::from("Alice"), Value::from("fly")]
        )
        .unwrap()
        .as_bool()
        .unwrap());
        assert!(!call(
            &ctx,
            "PassService",
            "CheckPermission",
            vec![Value::from("Alice"), Value::from("teleport")]
        )
        .unwrap()
        .as_bool()
        .unwrap());
        assert!(
            !call(
                &ctx,
                "PassService",
                "CheckPermission",
                vec![Value::from("Bob"), Value::from("fly")]
            )
            .unwrap()
            .as_bool()
            .unwrap(),
            "a pass is per player"
        );

        assert_eq!(
            call(&ctx, "PassService", "GetPasses", vec![Value::from("Alice")])
                .unwrap()
                .as_array()
                .unwrap()[0]
                .as_str(),
            Some("vip")
        );

        call(
            &ctx,
            "PassService",
            "Revoke",
            vec![Value::from("Alice"), Value::from("vip")],
        )
        .unwrap();
        assert!(!call(
            &ctx,
            "PassService",
            "CheckPermission",
            vec![Value::from("Alice"), Value::from("fly")]
        )
        .unwrap()
        .as_bool()
        .unwrap());
    }

    #[test]
    fn pass_permissions_may_be_a_map_of_booleans() {
        let ctx = server();
        call(
            &ctx,
            "PassService",
            "Register",
            vec![
                Value::from("vip"),
                Value::from("VIP"),
                Value::from(""),
                Value::map([("fly", Value::Bool(true)), ("banned", Value::Bool(false))]),
            ],
        )
        .unwrap();
        call(
            &ctx,
            "PassService",
            "Grant",
            vec![Value::from("Alice"), Value::from("vip")],
        )
        .unwrap();
        assert!(call(
            &ctx,
            "PassService",
            "CheckPermission",
            vec![Value::from("Alice"), Value::from("fly")]
        )
        .unwrap()
        .as_bool()
        .unwrap());
        assert!(
            !call(
                &ctx,
                "PassService",
                "CheckPermission",
                vec![Value::from("Alice"), Value::from("banned")]
            )
            .unwrap()
            .as_bool()
            .unwrap(),
            "a false entry grants nothing"
        );
    }

    #[test]
    fn pass_registration_and_grant_validate_their_inputs() {
        let ctx = server();
        call(
            &ctx,
            "PassService",
            "Register",
            vec![
                Value::from("vip"),
                Value::from("VIP"),
                Value::from(""),
                Value::empty_map(),
            ],
        )
        .unwrap();
        assert!(matches!(
            call(
                &ctx,
                "PassService",
                "Register",
                vec![
                    Value::from("vip"),
                    Value::from("V"),
                    Value::from(""),
                    Value::empty_map()
                ]
            ),
            Err(ApiError::AlreadyExists { kind: "pass", .. })
        ));
        assert!(matches!(
            call(
                &ctx,
                "PassService",
                "Grant",
                vec![Value::from("Alice"), Value::from("ghost")]
            ),
            Err(ApiError::NotFound { kind: "pass", .. })
        ));
        assert!(matches!(
            call(
                &ctx,
                "PassService",
                "Unregister",
                vec![Value::from("ghost")]
            ),
            Err(ApiError::NotFound { kind: "pass", .. })
        ));
    }

    #[test]
    fn pass_get_all_reports_permissions() {
        let ctx = server();
        call(
            &ctx,
            "PassService",
            "Register",
            vec![
                Value::from("vip"),
                Value::from("VIP"),
                Value::from("desc"),
                Value::array([Value::from("fly")]),
            ],
        )
        .unwrap();
        let all = call(&ctx, "PassService", "GetAll", vec![]).unwrap();
        let first = &all.as_array().unwrap()[0];
        assert_eq!(first.get("PassId").and_then(Value::as_str), Some("vip"));
        assert_eq!(
            first
                .get("Permissions")
                .and_then(Value::as_array)
                .unwrap()
                .len(),
            1
        );
    }

    // --- Callbacks and coverage ------------------------------------------

    #[test]
    fn callback_registrations_are_recorded_and_removable() {
        let ctx = server();
        call(
            &ctx,
            "BadgeService",
            "OnAward",
            vec![Value::from("Alice"), Value::Nil],
        )
        .unwrap();
        assert!(ctx
            .runtime
            .has_callback("BadgeService", "OnAward", Some("Alice")));
        assert!(ctx
            .runtime
            .unregister_callback("BadgeService", "OnAward", Some("Alice")));
        assert!(!ctx
            .runtime
            .has_callback("BadgeService", "OnAward", Some("Alice")));
    }

    #[test]
    fn filtering_is_whole_word_and_case_insensitive() {
        assert_eq!(filter_message("You IDIOT", "Normal"), "You *****");
        assert_eq!(filter_message("You IDIOT", "None"), "You IDIOT");
        assert_eq!(filter_message("plain text", "Normal"), "plain text");
        assert_eq!(filter_message("go to www.x.io", "Strict"), "go to ********");
        assert_eq!(
            filter_message("hello there", "Normal"),
            "hello there",
            "a banned word inside a longer word must not corrupt it"
        );
    }

    #[test]
    fn overlay_defaults_are_documented_values() {
        let colour = default_overlay_colour();
        assert!((colour.a - 0.5).abs() < 1e-9);
        assert_eq!(default_camera_target(), Vector3::ZERO);
    }

    #[test]
    fn every_documented_method_in_this_group_is_wired() {
        let ctx = ApiContext::new(Side::Studio);
        let mut unwired = Vec::new();
        for service in [
            "PlayerService",
            "BanService",
            "CameraService",
            "NotificationService",
            "ChatService",
            "BadgeService",
            "PassService",
        ] {
            for method in spec::methods_of(service) {
                let args = Args::new(service, method, Vec::new());
                if matches!(
                    dispatch(&ctx, service, method, &args),
                    Err(ApiError::UnknownFunction { .. })
                ) {
                    unwired.push(format!("{service}.{method}"));
                }
            }
        }
        assert!(unwired.is_empty(), "unwired: {unwired:?}");
    }

    #[test]
    fn a_method_this_group_does_not_own_is_rejected() {
        let ctx = server();
        for (service, method) in [("ChatService", "NotAThing"), ("EconomyService", "Nope")] {
            let args = Args::new(service, method, Vec::new());
            assert!(matches!(
                dispatch(&ctx, service, method, &args),
                Err(ApiError::UnknownFunction { .. })
            ));
        }
    }
}
