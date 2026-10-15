//! Gameplay: QuestService, AchievementService, LeaderboardService, RewardService,
//! CooldownService, PartyService, MatchmakingService, DialogueService, MailService

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use nb_types::Value;

use crate::args::Args;
use crate::error::{ApiError, ApiResult};
use crate::runtime::{
    Achievement, ApiContext, Dialogue, DialogueSession, Mail, MatchQueue, MatchRoom, Party, Quest,
    RedeemCode,
};
use crate::services::shared::require_player;
use crate::services::unknown;

/// Routes a call in the gameplay group.
pub fn dispatch(ctx: &ApiContext, service: &str, method: &str, args: &Args) -> ApiResult<Value> {
    match service {
        "QuestService" => dispatch_quest(ctx, method, args),
        "AchievementService" => dispatch_achievement(ctx, method, args),
        "LeaderboardService" => dispatch_leaderboard(ctx, method, args),
        "RewardService" => dispatch_reward(ctx, method, args),
        "CooldownService" => dispatch_cooldown(ctx, method, args),
        "PartyService" => dispatch_party(ctx, method, args),
        "MatchmakingService" => dispatch_matchmaking(ctx, method, args),
        "DialogueService" => dispatch_dialogue(ctx, method, args),
        "MailService" => dispatch_mail(ctx, method, args),
        _ => Err(unknown(service, method)),
    }
}

// ============================================================================
// QuestService (11 functions)
// ============================================================================

fn dispatch_quest(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "Register" => {
            let quest_id = args.string(0, "QuestId")?;
            let name = args.string(1, "Name")?;
            let description = args.string(2, "Description")?;
            let steps = args.at(3).cloned().unwrap_or(Value::Nil);
            let rewards = args.at(4).cloned().unwrap_or(Value::Nil);
            ctx.runtime.quests.write().insert(
                quest_id.clone(),
                Quest {
                    id: quest_id,
                    name,
                    description,
                    steps,
                    rewards,
                },
            );
            Ok(Value::Bool(true))
        }
        "Unregister" => {
            let quest_id = args.string(0, "QuestId")?;
            let removed = ctx.runtime.quests.write().remove(&quest_id).is_some();
            Ok(Value::Bool(removed))
        }
        "Assign" => {
            let username = args.string(0, "Username")?;
            let quest_id = args.string(1, "QuestId")?;
            require_player(&ctx.runtime, &username)?;
            if !ctx.runtime.quests.read().contains_key(&quest_id) {
                return Err(ApiError::NotFound {
                    kind: "quest",
                    name: quest_id,
                });
            }
            ctx.runtime
                .quest_progress
                .set(format!("{username}:{quest_id}:active"), Value::Bool(true))
                .map_err(ApiError::Store)?;
            ctx.runtime
                .quest_progress
                .set(format!("{username}:{quest_id}:progress"), Value::from(0.0))
                .map_err(ApiError::Store)?;
            ctx.runtime
                .quest_progress
                .set(format!("{username}:{quest_id}:step"), Value::from(0.0))
                .map_err(ApiError::Store)?;
            ctx.runtime
                .quest_progress
                .set(
                    format!("{username}:{quest_id}:completed"),
                    Value::Bool(false),
                )
                .map_err(ApiError::Store)?;
            Ok(Value::Bool(true))
        }
        "Unassign" => {
            let username = args.string(0, "Username")?;
            let quest_id = args.string(1, "QuestId")?;
            require_player(&ctx.runtime, &username)?;
            let _ = ctx
                .runtime
                .quest_progress
                .set(format!("{username}:{quest_id}:active"), Value::Bool(false));
            Ok(Value::Bool(true))
        }
        "GetProgress" => {
            let username = args.string(0, "Username")?;
            let quest_id = args.string(1, "QuestId")?;
            require_player(&ctx.runtime, &username)?;
            let p = ctx
                .runtime
                .quest_progress
                .number_or_zero(&format!("{username}:{quest_id}:progress"))
                .unwrap_or(0.0);
            Ok(Value::from(p))
        }
        "AddProgress" => {
            let username = args.string(0, "Username")?;
            let quest_id = args.string(1, "QuestId")?;
            let step_index = args.integer(2, "StepIndex")?;
            let delta = args.number(3, "Delta")?;
            require_player(&ctx.runtime, &username)?;
            let cur = ctx
                .runtime
                .quest_progress
                .number_or_zero(&format!("{username}:{quest_id}:progress"))
                .unwrap_or(0.0);
            let next = cur + delta;
            ctx.runtime
                .quest_progress
                .set(format!("{username}:{quest_id}:progress"), Value::from(next))
                .map_err(ApiError::Store)?;
            ctx.runtime
                .quest_progress
                .set(
                    format!("{username}:{quest_id}:step"),
                    Value::from(step_index as f64),
                )
                .map_err(ApiError::Store)?;
            Ok(Value::from(next))
        }
        "SetProgress" => {
            let username = args.string(0, "Username")?;
            let quest_id = args.string(1, "QuestId")?;
            let step_index = args.integer(2, "StepIndex")?;
            let progress = args.number(3, "Progress")?;
            require_player(&ctx.runtime, &username)?;
            ctx.runtime
                .quest_progress
                .set(
                    format!("{username}:{quest_id}:progress"),
                    Value::from(progress),
                )
                .map_err(ApiError::Store)?;
            ctx.runtime
                .quest_progress
                .set(
                    format!("{username}:{quest_id}:step"),
                    Value::from(step_index as f64),
                )
                .map_err(ApiError::Store)?;
            Ok(Value::from(progress))
        }
        "Complete" => {
            let username = args.string(0, "Username")?;
            let quest_id = args.string(1, "QuestId")?;
            require_player(&ctx.runtime, &username)?;
            if !ctx.runtime.quests.read().contains_key(&quest_id) {
                return Err(ApiError::NotFound {
                    kind: "quest",
                    name: quest_id,
                });
            }
            ctx.runtime
                .quest_progress
                .set(
                    format!("{username}:{quest_id}:completed"),
                    Value::Bool(true),
                )
                .map_err(ApiError::Store)?;
            ctx.runtime
                .quest_progress
                .set(format!("{username}:{quest_id}:active"), Value::Bool(false))
                .map_err(ApiError::Store)?;
            Ok(Value::Bool(true))
        }
        "IsCompleted" => {
            let username = args.string(0, "Username")?;
            let quest_id = args.string(1, "QuestId")?;
            require_player(&ctx.runtime, &username)?;
            let completed = match ctx
                .runtime
                .quest_progress
                .get(&format!("{username}:{quest_id}:completed"))
            {
                Some(Value::Bool(b)) => b,
                _ => false,
            };
            Ok(Value::Bool(completed))
        }
        "GetActive" => {
            let username = args.string(0, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let prefix = format!("{username}:");
            let suffix = ":active";
            let active_ids: Vec<Value> = ctx
                .runtime
                .quest_progress
                .keys()
                .into_iter()
                .filter(|k| k.starts_with(&prefix) && k.ends_with(suffix))
                .filter_map(|k| {
                    if let Some(Value::Bool(true)) = ctx.runtime.quest_progress.get(&k) {
                        let id = &k[prefix.len()..k.len() - suffix.len()];
                        Some(Value::from(id.to_owned()))
                    } else {
                        None
                    }
                })
                .collect();
            Ok(Value::Array(active_ids))
        }
        "OnComplete" => {
            let username = args.string(0, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let key = ctx
                .runtime
                .register_callback("QuestService", "OnComplete", Some(&username));
            Ok(Value::from(key))
        }
        _ => Err(unknown("QuestService", method)),
    }
}

// ============================================================================
// AchievementService (10 functions)
// ============================================================================

fn dispatch_achievement(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "Register" => {
            let id = args.string(0, "AchievementId")?;
            let name = args.string(1, "Name")?;
            let description = args.string(2, "Description")?;
            let max_prog = args.number(3, "MaxProgress")?;
            ctx.runtime.achievements.write().insert(
                id.clone(),
                Achievement {
                    id,
                    name,
                    description,
                    max_progress: max_prog,
                },
            );
            Ok(Value::Bool(true))
        }
        "Unregister" => {
            let id = args.string(0, "AchievementId")?;
            let removed = ctx.runtime.achievements.write().remove(&id).is_some();
            Ok(Value::Bool(removed))
        }
        "GetAll" => {
            let achievements = ctx.runtime.achievements.read();
            let list: Vec<Value> = achievements
                .values()
                .map(|a| {
                    Value::map([
                        ("AchievementId", Value::from(a.id.clone())),
                        ("Name", Value::from(a.name.clone())),
                        ("Description", Value::from(a.description.clone())),
                        ("MaxProgress", Value::from(a.max_progress)),
                    ])
                })
                .collect();
            Ok(Value::Array(list))
        }
        "GetProgress" => {
            let username = args.string(0, "Username")?;
            let id = args.string(1, "AchievementId")?;
            require_player(&ctx.runtime, &username)?;
            let p = ctx
                .runtime
                .achievement_progress
                .number_or_zero(&format!("{username}:{id}"))
                .unwrap_or(0.0);
            Ok(Value::from(p))
        }
        "SetProgress" => {
            let username = args.string(0, "Username")?;
            let id = args.string(1, "AchievementId")?;
            let progress = args.number(2, "Progress")?;
            require_player(&ctx.runtime, &username)?;
            let max_prog = ctx
                .runtime
                .achievements
                .read()
                .get(&id)
                .map(|a| a.max_progress)
                .unwrap_or(1.0);
            ctx.runtime
                .achievement_progress
                .set(format!("{username}:{id}"), Value::from(progress))
                .map_err(ApiError::Store)?;
            if progress >= max_prog {
                let _ = ctx
                    .runtime
                    .achievement_unlocked
                    .set(format!("{username}:{id}"), Value::Bool(true));
            }
            Ok(Value::from(progress))
        }
        "AddProgress" => {
            let username = args.string(0, "Username")?;
            let id = args.string(1, "AchievementId")?;
            let delta = args.number(2, "Delta")?;
            require_player(&ctx.runtime, &username)?;
            let cur = ctx
                .runtime
                .achievement_progress
                .number_or_zero(&format!("{username}:{id}"))
                .unwrap_or(0.0);
            let next = cur + delta;
            let max_prog = ctx
                .runtime
                .achievements
                .read()
                .get(&id)
                .map(|a| a.max_progress)
                .unwrap_or(1.0);
            ctx.runtime
                .achievement_progress
                .set(format!("{username}:{id}"), Value::from(next))
                .map_err(ApiError::Store)?;
            if next >= max_prog {
                let _ = ctx
                    .runtime
                    .achievement_unlocked
                    .set(format!("{username}:{id}"), Value::Bool(true));
            }
            Ok(Value::from(next))
        }
        "Unlock" => {
            let username = args.string(0, "Username")?;
            let id = args.string(1, "AchievementId")?;
            require_player(&ctx.runtime, &username)?;
            ctx.runtime
                .achievement_unlocked
                .set(format!("{username}:{id}"), Value::Bool(true))
                .map_err(ApiError::Store)?;
            Ok(Value::Bool(true))
        }
        "IsUnlocked" => {
            let username = args.string(0, "Username")?;
            let id = args.string(1, "AchievementId")?;
            require_player(&ctx.runtime, &username)?;
            let unlocked = match ctx
                .runtime
                .achievement_unlocked
                .get(&format!("{username}:{id}"))
            {
                Some(Value::Bool(b)) => b,
                _ => false,
            };
            Ok(Value::Bool(unlocked))
        }
        "GetUnlocked" => {
            let username = args.string(0, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let prefix = format!("{username}:");
            let unlocked_ids: Vec<Value> = ctx
                .runtime
                .achievement_unlocked
                .keys()
                .into_iter()
                .filter_map(|key| {
                    if key.starts_with(&prefix) {
                        if let Some(Value::Bool(true)) = ctx.runtime.achievement_unlocked.get(&key)
                        {
                            return Some(Value::from(key[prefix.len()..].to_owned()));
                        }
                    }
                    None
                })
                .collect();
            Ok(Value::Array(unlocked_ids))
        }
        "OnUnlock" => {
            let username = args.string(0, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let key =
                ctx.runtime
                    .register_callback("AchievementService", "OnUnlock", Some(&username));
            Ok(Value::from(key))
        }
        _ => Err(unknown("AchievementService", method)),
    }
}

// ============================================================================
// LeaderboardService (8 functions)
// ============================================================================

fn dispatch_leaderboard(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "CreateBoard" => {
            let board_name = args.string(0, "BoardName")?;
            let order = args.string(1, "Order")?;
            let norm_order = if order.eq_ignore_ascii_case("Ascending") {
                "Ascending".to_owned()
            } else {
                "Descending".to_owned()
            };
            ctx.runtime.leaderboards.write().insert(
                board_name.clone(),
                crate::runtime::Board {
                    name: board_name,
                    order: norm_order,
                    scores: BTreeMap::new(),
                },
            );
            Ok(Value::Bool(true))
        }
        "DeleteBoard" => {
            let board_name = args.string(0, "BoardName")?;
            let removed = ctx
                .runtime
                .leaderboards
                .write()
                .remove(&board_name)
                .is_some();
            Ok(Value::Bool(removed))
        }
        "SetScore" => {
            let board_name = args.string(0, "BoardName")?;
            let username = args.string(1, "Username")?;
            let score = args.number(2, "Score")?;
            require_player(&ctx.runtime, &username)?;
            let mut boards = ctx.runtime.leaderboards.write();
            let board = boards
                .get_mut(&board_name)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "leaderboard",
                    name: board_name,
                })?;
            board.scores.insert(username, score);
            Ok(Value::from(score))
        }
        "GetScore" => {
            let board_name = args.string(0, "BoardName")?;
            let username = args.string(1, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let boards = ctx.runtime.leaderboards.read();
            let board = boards.get(&board_name).ok_or_else(|| ApiError::NotFound {
                kind: "leaderboard",
                name: board_name,
            })?;
            let s = board.scores.get(&username).copied().unwrap_or(0.0);
            Ok(Value::from(s))
        }
        "AddScore" => {
            let board_name = args.string(0, "BoardName")?;
            let username = args.string(1, "Username")?;
            let delta = args.number(2, "Delta")?;
            require_player(&ctx.runtime, &username)?;
            let mut boards = ctx.runtime.leaderboards.write();
            let board = boards
                .get_mut(&board_name)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "leaderboard",
                    name: board_name,
                })?;
            let cur = board.scores.get(&username).copied().unwrap_or(0.0);
            let next = cur + delta;
            board.scores.insert(username, next);
            Ok(Value::from(next))
        }
        "GetTopPlayers" => {
            let board_name = args.string(0, "BoardName")?;
            let limit = args.integer(1, "Limit")?;
            let boards = ctx.runtime.leaderboards.read();
            let board = boards.get(&board_name).ok_or_else(|| ApiError::NotFound {
                kind: "leaderboard",
                name: board_name,
            })?;
            let mut sorted: Vec<(&String, &f64)> = board.scores.iter().collect();
            if board.order == "Ascending" {
                sorted.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal));
            } else {
                sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
            let top: Vec<Value> = sorted
                .into_iter()
                .take(limit.max(0) as usize)
                .enumerate()
                .map(|(i, (u, s))| {
                    Value::map([
                        ("Username", Value::from(u.clone())),
                        ("Score", Value::from(*s)),
                        ("Rank", Value::from((i + 1) as i64)),
                    ])
                })
                .collect();
            Ok(Value::Array(top))
        }
        "GetRank" => {
            let board_name = args.string(0, "BoardName")?;
            let username = args.string(1, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let boards = ctx.runtime.leaderboards.read();
            let board = boards.get(&board_name).ok_or_else(|| ApiError::NotFound {
                kind: "leaderboard",
                name: board_name,
            })?;
            let mut sorted: Vec<(&String, &f64)> = board.scores.iter().collect();
            if board.order == "Ascending" {
                sorted.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal));
            } else {
                sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
            let rank = sorted
                .iter()
                .position(|(u, _)| *u == &username)
                .map(|p| (p + 1) as i64)
                .unwrap_or(-1);
            Ok(Value::from(rank))
        }
        "ResetBoard" => {
            let board_name = args.string(0, "BoardName")?;
            let mut boards = ctx.runtime.leaderboards.write();
            let board = boards
                .get_mut(&board_name)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "leaderboard",
                    name: board_name,
                })?;
            board.scores.clear();
            Ok(Value::Bool(true))
        }
        _ => Err(unknown("LeaderboardService", method)),
    }
}

// ============================================================================
// RewardService (8 functions)
// ============================================================================

fn dispatch_reward(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "ClaimDaily" => {
            let username = args.string(0, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let mut streaks = ctx.runtime.daily_streaks.write();
            let streak = streaks.entry(username.clone()).or_insert(0);
            *streak += 1;
            let s = *streak;
            ctx.runtime
                .daily_claims
                .write()
                .insert(username, ctx.runtime.ticks());
            Ok(Value::map([
                ("Streak", Value::from(s)),
                ("Claimed", Value::Bool(true)),
            ]))
        }
        "GetDailyStreak" => {
            let username = args.string(0, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let s = ctx
                .runtime
                .daily_streaks
                .read()
                .get(&username)
                .copied()
                .unwrap_or(0);
            Ok(Value::from(s))
        }
        "ClaimLoginReward" => {
            let username = args.string(0, "Username")?;
            let reward_id = args.string(1, "RewardId")?;
            require_player(&ctx.runtime, &username)?;
            Ok(Value::map([
                ("RewardId", Value::from(reward_id)),
                ("Claimed", Value::Bool(true)),
            ]))
        }
        "RegisterCode" => {
            let code = args.string(0, "Code")?;
            let rewards = args.at(1).cloned().unwrap_or(Value::Nil);
            let max_uses = args.integer(2, "MaxUses")?;
            let expires_at = args.optional_number(3, "ExpiresAt")?.map(|n| n as i64);
            ctx.runtime.codes.write().insert(
                code.clone(),
                RedeemCode {
                    code,
                    rewards,
                    max_uses,
                    uses: 0,
                    expires_at,
                    redeemed_by: BTreeSet::new(),
                },
            );
            Ok(Value::Bool(true))
        }
        "RevokeCode" => {
            let code = args.string(0, "Code")?;
            let removed = ctx.runtime.codes.write().remove(&code).is_some();
            Ok(Value::Bool(removed))
        }
        "GetCodeInfo" => {
            let code = args.string(0, "Code")?;
            let codes = ctx.runtime.codes.read();
            let c = codes.get(&code).ok_or_else(|| ApiError::NotFound {
                kind: "code",
                name: code,
            })?;
            Ok(Value::map([
                ("Code", Value::from(c.code.clone())),
                ("MaxUses", Value::from(c.max_uses)),
                ("Uses", Value::from(c.uses)),
                (
                    "ExpiresAt",
                    c.expires_at.map(Value::from).unwrap_or(Value::Nil),
                ),
                ("Rewards", c.rewards.clone()),
            ]))
        }
        "RedeemCode" => {
            let username = args.string(0, "Username")?;
            let code = args.string(1, "Code")?;
            require_player(&ctx.runtime, &username)?;
            let mut codes = ctx.runtime.codes.write();
            let c = codes.get_mut(&code).ok_or_else(|| ApiError::NotFound {
                kind: "code",
                name: code.clone(),
            })?;
            if c.redeemed_by.contains(&username) {
                return Err(ApiError::Rule(format!(
                    "{username} has already redeemed `{code}`"
                )));
            }
            if c.max_uses > 0 && c.uses >= c.max_uses {
                return Err(ApiError::Rule(format!(
                    "code `{code}` has reached maximum uses"
                )));
            }
            c.uses += 1;
            c.redeemed_by.insert(username);
            Ok(c.rewards.clone())
        }
        "OnClaim" => {
            let username = args.string(0, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let key = ctx
                .runtime
                .register_callback("RewardService", "OnClaim", Some(&username));
            Ok(Value::from(key))
        }
        _ => Err(unknown("RewardService", method)),
    }
}

// ============================================================================
// CooldownService (7 functions)
// ============================================================================

fn dispatch_cooldown(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "Start" => {
            let username = args.string(0, "Username")?;
            let id = args.string(1, "CooldownId")?;
            let duration = args.number(2, "Duration")?;
            require_player(&ctx.runtime, &username)?;
            if duration <= 0.0 {
                return Err(args.rule("Duration must be positive"));
            }
            let key = format!("{username}:{id}");
            let ticks = ctx.runtime.ticks() as f64;
            ctx.runtime
                .cooldowns
                .set(
                    key,
                    Value::map([
                        ("duration", Value::from(duration)),
                        ("start_tick", Value::from(ticks)),
                    ]),
                )
                .map_err(ApiError::Store)?;
            Ok(Value::Bool(true))
        }
        "Cancel" => {
            let username = args.string(0, "Username")?;
            let id = args.string(1, "CooldownId")?;
            require_player(&ctx.runtime, &username)?;
            let key = format!("{username}:{id}");
            let removed = ctx.runtime.cooldowns.remove(&key).is_some();
            Ok(Value::Bool(removed))
        }
        "IsReady" => {
            let username = args.string(0, "Username")?;
            let id = args.string(1, "CooldownId")?;
            require_player(&ctx.runtime, &username)?;
            let key = format!("{username}:{id}");
            match ctx.runtime.cooldowns.get(&key) {
                Some(Value::Map(m)) => {
                    let dur = m
                        .get("duration")
                        .and_then(|v| match v {
                            Value::Number(n) => Some(*n),
                            _ => None,
                        })
                        .unwrap_or(0.0);
                    let start = m
                        .get("start_tick")
                        .and_then(|v| match v {
                            Value::Number(n) => Some(*n),
                            _ => None,
                        })
                        .unwrap_or(0.0);
                    let elapsed = (ctx.runtime.ticks() as f64 - start) / 100.0;
                    Ok(Value::Bool(elapsed >= dur))
                }
                _ => Ok(Value::Bool(true)),
            }
        }
        "GetRemaining" => {
            let username = args.string(0, "Username")?;
            let id = args.string(1, "CooldownId")?;
            require_player(&ctx.runtime, &username)?;
            let key = format!("{username}:{id}");
            match ctx.runtime.cooldowns.get(&key) {
                Some(Value::Map(m)) => {
                    let dur = m
                        .get("duration")
                        .and_then(|v| match v {
                            Value::Number(n) => Some(*n),
                            _ => None,
                        })
                        .unwrap_or(0.0);
                    let start = m
                        .get("start_tick")
                        .and_then(|v| match v {
                            Value::Number(n) => Some(*n),
                            _ => None,
                        })
                        .unwrap_or(0.0);
                    let elapsed = (ctx.runtime.ticks() as f64 - start) / 100.0;
                    let remaining = (dur - elapsed).max(0.0);
                    Ok(Value::from(remaining))
                }
                _ => Ok(Value::from(0.0)),
            }
        }
        "GetAll" => {
            let username = args.string(0, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let prefix = format!("{username}:");
            let mut result: BTreeMap<String, Value> = BTreeMap::new();
            for key in ctx.runtime.cooldowns.keys() {
                if let Some(id) = key.strip_prefix(&prefix) {
                    if let Some(Value::Map(m)) = ctx.runtime.cooldowns.get(&key) {
                        let dur = m
                            .get("duration")
                            .and_then(|v| match v {
                                Value::Number(n) => Some(*n),
                                _ => None,
                            })
                            .unwrap_or(0.0);
                        let start = m
                            .get("start_tick")
                            .and_then(|v| match v {
                                Value::Number(n) => Some(*n),
                                _ => None,
                            })
                            .unwrap_or(0.0);
                        let elapsed = (ctx.runtime.ticks() as f64 - start) / 100.0;
                        let rem = (dur - elapsed).max(0.0);
                        result.insert(id.to_owned(), Value::from(rem));
                    }
                }
            }
            Ok(Value::Map(result))
        }
        "Reset" => {
            let username = args.string(0, "Username")?;
            let id = args.string(1, "CooldownId")?;
            require_player(&ctx.runtime, &username)?;
            let key = format!("{username}:{id}");
            if let Some(Value::Map(mut m)) = ctx.runtime.cooldowns.get(&key) {
                m.insert(
                    "start_tick".to_owned(),
                    Value::from(ctx.runtime.ticks() as f64),
                );
                let _ = ctx.runtime.cooldowns.set(key, Value::Map(m));
            }
            Ok(Value::Bool(true))
        }
        "OnReady" => {
            let username = args.string(0, "Username")?;
            let id = args.string(1, "CooldownId")?;
            require_player(&ctx.runtime, &username)?;
            let key = ctx.runtime.register_callback(
                "CooldownService",
                "OnReady",
                Some(&format!("{username}:{id}")),
            );
            Ok(Value::from(key))
        }
        _ => Err(unknown("CooldownService", method)),
    }
}

// ============================================================================
// PartyService (11 functions)
// ============================================================================

fn dispatch_party(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "CreateParty" => {
            let leader = args.string(0, "Leader")?;
            let max_size = args.integer(1, "MaxSize")?;
            require_player(&ctx.runtime, &leader)?;
            if max_size < 2 {
                return Err(args.rule("MaxSize must be at least 2"));
            }
            if ctx.runtime.party_of.read().contains_key(&leader) {
                return Err(ApiError::Rule(format!("{leader} is already in a party")));
            }
            let party_id = ctx.runtime.next_id_str("party");
            let mut members = BTreeSet::new();
            members.insert(leader.clone());
            ctx.runtime.parties.write().insert(
                party_id.clone(),
                Party {
                    id: party_id.clone(),
                    leader: leader.clone(),
                    members,
                    max_size: max_size as usize,
                    invites: BTreeSet::new(),
                },
            );
            ctx.runtime
                .party_of
                .write()
                .insert(leader, party_id.clone());
            Ok(Value::from(party_id))
        }
        "DisbandParty" => {
            let party_id = args.string(0, "PartyId")?;
            let party = ctx
                .runtime
                .parties
                .write()
                .remove(&party_id)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "party",
                    name: party_id,
                })?;
            let mut party_of = ctx.runtime.party_of.write();
            for member in &party.members {
                party_of.remove(member);
            }
            Ok(Value::Bool(true))
        }
        "Invite" => {
            let party_id = args.string(0, "PartyId")?;
            let username = args.string(1, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let mut parties = ctx.runtime.parties.write();
            let party = parties
                .get_mut(&party_id)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "party",
                    name: party_id,
                })?;
            if party.members.len() >= party.max_size {
                return Err(ApiError::Rule("party is full".to_owned()));
            }
            party.invites.insert(username);
            Ok(Value::Bool(true))
        }
        "AcceptInvite" => {
            let party_id = args.string(0, "PartyId")?;
            let username = args.string(1, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let mut parties = ctx.runtime.parties.write();
            let party = parties
                .get_mut(&party_id)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "party",
                    name: party_id.clone(),
                })?;
            if !party.invites.remove(&username) {
                return Err(ApiError::Rule(format!(
                    "{username} has no invite to party `{party_id}`"
                )));
            }
            if party.members.len() >= party.max_size {
                return Err(ApiError::Rule("party is full".to_owned()));
            }
            party.members.insert(username.clone());
            ctx.runtime.party_of.write().insert(username, party_id);
            Ok(Value::Bool(true))
        }
        "DeclineInvite" => {
            let party_id = args.string(0, "PartyId")?;
            let username = args.string(1, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let mut parties = ctx.runtime.parties.write();
            if let Some(party) = parties.get_mut(&party_id) {
                party.invites.remove(&username);
            }
            Ok(Value::Bool(true))
        }
        "Kick" => {
            let party_id = args.string(0, "PartyId")?;
            let username = args.string(1, "Username")?;
            let mut parties = ctx.runtime.parties.write();
            let party = parties
                .get_mut(&party_id)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "party",
                    name: party_id,
                })?;
            if party.leader == username {
                return Err(ApiError::Rule("cannot kick party leader".to_owned()));
            }
            party.members.remove(&username);
            ctx.runtime.party_of.write().remove(&username);
            Ok(Value::Bool(true))
        }
        "Leave" => {
            let party_id = args.string(0, "PartyId")?;
            let username = args.string(1, "Username")?;
            let mut parties = ctx.runtime.parties.write();
            let party = parties
                .get_mut(&party_id)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "party",
                    name: party_id.clone(),
                })?;
            party.members.remove(&username);
            ctx.runtime.party_of.write().remove(&username);
            if party.members.is_empty() {
                parties.remove(&party_id);
            } else if party.leader == username {
                party.leader = party.members.iter().next().unwrap().clone();
            }
            Ok(Value::Bool(true))
        }
        "GetParty" => {
            let party_id = args.string(0, "PartyId")?;
            let parties = ctx.runtime.parties.read();
            let party = parties.get(&party_id).ok_or_else(|| ApiError::NotFound {
                kind: "party",
                name: party_id,
            })?;
            Ok(Value::map([
                ("PartyId", Value::from(party.id.clone())),
                ("Leader", Value::from(party.leader.clone())),
                (
                    "Members",
                    Value::array(party.members.iter().cloned().map(Value::from)),
                ),
                ("MaxSize", Value::from(party.max_size as i64)),
                (
                    "Invites",
                    Value::array(party.invites.iter().cloned().map(Value::from)),
                ),
            ]))
        }
        "GetPartyOf" => {
            let username = args.string(0, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let party_id = ctx.runtime.party_of.read().get(&username).cloned();
            Ok(party_id.map(Value::from).unwrap_or(Value::Nil))
        }
        "TransferLeader" => {
            let party_id = args.string(0, "PartyId")?;
            let username = args.string(1, "Username")?;
            let mut parties = ctx.runtime.parties.write();
            let party = parties
                .get_mut(&party_id)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "party",
                    name: party_id.clone(),
                })?;
            if !party.members.contains(&username) {
                return Err(ApiError::Rule(format!(
                    "{username} is not a member of party `{party_id}`"
                )));
            }
            party.leader = username;
            Ok(Value::Bool(true))
        }
        "OnInvite" => {
            let username = args.string(0, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let key = ctx
                .runtime
                .register_callback("PartyService", "OnInvite", Some(&username));
            Ok(Value::from(key))
        }
        _ => Err(unknown("PartyService", method)),
    }
}

// ============================================================================
// MatchmakingService (10 functions)
// ============================================================================

fn dispatch_matchmaking(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "CreateQueue" => {
            let queue_name = args.string(0, "QueueName")?;
            let max_players = args.integer(1, "MaxPlayers")?;
            if max_players < 1 {
                return Err(args.rule("MaxPlayers must be at least 1"));
            }
            ctx.runtime.queues.write().insert(
                queue_name.clone(),
                MatchQueue {
                    name: queue_name,
                    max_players: max_players as usize,
                    waiting: VecDeque::new(),
                    rooms: BTreeSet::new(),
                },
            );
            Ok(Value::Bool(true))
        }
        "DeleteQueue" => {
            let queue_name = args.string(0, "QueueName")?;
            let removed = ctx.runtime.queues.write().remove(&queue_name).is_some();
            Ok(Value::Bool(removed))
        }
        "Join" => {
            let username = args.string(0, "Username")?;
            let queue_name = args.string(1, "QueueName")?;
            require_player(&ctx.runtime, &username)?;
            let mut queues = ctx.runtime.queues.write();
            let q = queues
                .get_mut(&queue_name)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "queue",
                    name: queue_name,
                })?;
            if !q.waiting.contains(&username) {
                q.waiting.push_back(username);
            }
            Ok(Value::Bool(true))
        }
        "Leave" => {
            let username = args.string(0, "Username")?;
            let queue_name = args.string(1, "QueueName")?;
            require_player(&ctx.runtime, &username)?;
            let mut queues = ctx.runtime.queues.write();
            if let Some(q) = queues.get_mut(&queue_name) {
                q.waiting.retain(|u| u != &username);
            }
            Ok(Value::Bool(true))
        }
        "GetQueue" => {
            let queue_name = args.string(0, "QueueName")?;
            let queues = ctx.runtime.queues.read();
            let q = queues.get(&queue_name).ok_or_else(|| ApiError::NotFound {
                kind: "queue",
                name: queue_name,
            })?;
            Ok(Value::map([
                ("QueueName", Value::from(q.name.clone())),
                ("MaxPlayers", Value::from(q.max_players as i64)),
                (
                    "Waiting",
                    Value::array(q.waiting.iter().cloned().map(Value::from)),
                ),
                ("WaitingCount", Value::from(q.waiting.len() as i64)),
            ]))
        }
        "CreateRoom" => {
            let room_id = args.string(0, "RoomId")?;
            let queue_name = args.string(1, "QueueName")?;
            let mut queues = ctx.runtime.queues.write();
            let q = queues
                .get_mut(&queue_name)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "queue",
                    name: queue_name.clone(),
                })?;
            let mut players = Vec::new();
            while players.len() < q.max_players && !q.waiting.is_empty() {
                players.push(q.waiting.pop_front().unwrap());
            }
            q.rooms.insert(room_id.clone());
            ctx.runtime.rooms.write().insert(
                room_id.clone(),
                MatchRoom {
                    id: room_id.clone(),
                    queue: queue_name,
                    players: players.clone(),
                    open: true,
                },
            );
            Ok(Value::map([
                ("RoomId", Value::from(room_id)),
                (
                    "Players",
                    Value::array(players.into_iter().map(Value::from)),
                ),
            ]))
        }
        "CloseRoom" => {
            let room_id = args.string(0, "RoomId")?;
            let mut rooms = ctx.runtime.rooms.write();
            let room = rooms.get_mut(&room_id).ok_or_else(|| ApiError::NotFound {
                kind: "room",
                name: room_id,
            })?;
            room.open = false;
            Ok(Value::Bool(true))
        }
        "GetRoom" => {
            let room_id = args.string(0, "RoomId")?;
            let rooms = ctx.runtime.rooms.read();
            let room = rooms.get(&room_id).ok_or_else(|| ApiError::NotFound {
                kind: "room",
                name: room_id,
            })?;
            Ok(Value::map([
                ("RoomId", Value::from(room.id.clone())),
                ("Queue", Value::from(room.queue.clone())),
                (
                    "Players",
                    Value::array(room.players.iter().cloned().map(Value::from)),
                ),
                ("Open", Value::Bool(room.open)),
            ]))
        }
        "GetRooms" => {
            let queue_name = args.string(0, "QueueName")?;
            let rooms = ctx.runtime.rooms.read();
            let matched: Vec<Value> = rooms
                .values()
                .filter(|r| r.queue == queue_name)
                .map(|r| Value::from(r.id.clone()))
                .collect();
            Ok(Value::Array(matched))
        }
        "OnMatch" => {
            let queue_name = args.string(0, "QueueName")?;
            let key =
                ctx.runtime
                    .register_callback("MatchmakingService", "OnMatch", Some(&queue_name));
            Ok(Value::from(key))
        }
        _ => Err(unknown("MatchmakingService", method)),
    }
}

// ============================================================================
// DialogueService (9 functions)
// ============================================================================

fn dispatch_dialogue(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "Register" => {
            let dialogue_id = args.string(0, "DialogueId")?;
            let npc_name = args.string(1, "NpcName")?;
            let nodes = args.at(2).cloned().unwrap_or(Value::Nil);
            ctx.runtime.dialogues.write().insert(
                dialogue_id.clone(),
                Dialogue {
                    id: dialogue_id,
                    npc: npc_name,
                    nodes,
                },
            );
            Ok(Value::Bool(true))
        }
        "Unregister" => {
            let dialogue_id = args.string(0, "DialogueId")?;
            let removed = ctx.runtime.dialogues.write().remove(&dialogue_id).is_some();
            Ok(Value::Bool(removed))
        }
        "Start" => {
            let username = args.string(0, "Username")?;
            let dialogue_id = args.string(1, "DialogueId")?;
            require_player(&ctx.runtime, &username)?;
            if !ctx.runtime.dialogues.read().contains_key(&dialogue_id) {
                return Err(ApiError::NotFound {
                    kind: "dialogue",
                    name: dialogue_id,
                });
            }
            ctx.runtime.dialogue_sessions.write().insert(
                username.clone(),
                DialogueSession {
                    username,
                    dialogue: dialogue_id,
                    node: 0,
                },
            );
            Ok(Value::Bool(true))
        }
        "End" => {
            let username = args.string(0, "Username")?;
            require_player(&ctx.runtime, &username)?;
            ctx.runtime.dialogue_sessions.write().remove(&username);
            Ok(Value::Bool(true))
        }
        "GetCurrent" => {
            let username = args.string(0, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let sessions = ctx.runtime.dialogue_sessions.read();
            let session = sessions
                .get(&username)
                .ok_or_else(|| ApiError::Rule(format!("{username} has no active dialogue")))?;
            let dialogues = ctx.runtime.dialogues.read();
            let diag = dialogues
                .get(&session.dialogue)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "dialogue",
                    name: session.dialogue.clone(),
                })?;
            let current_node = match &diag.nodes {
                Value::Array(nodes) => nodes.get(session.node).cloned().unwrap_or(Value::Nil),
                other => other.clone(),
            };
            Ok(Value::map([
                ("DialogueId", Value::from(session.dialogue.clone())),
                ("NodeIndex", Value::from(session.node as i64)),
                ("Node", current_node),
            ]))
        }
        "Choose" => {
            let username = args.string(0, "Username")?;
            let _choice_index = args.integer(1, "ChoiceIndex")?;
            require_player(&ctx.runtime, &username)?;
            let mut sessions = ctx.runtime.dialogue_sessions.write();
            let session = sessions
                .get_mut(&username)
                .ok_or_else(|| ApiError::Rule(format!("{username} has no active dialogue")))?;
            session.node += 1;
            Ok(Value::Bool(true))
        }
        "OnStart" => {
            let npc_name = args.string(0, "NpcName")?;
            let key = ctx
                .runtime
                .register_callback("DialogueService", "OnStart", Some(&npc_name));
            Ok(Value::from(key))
        }
        "OnEnd" => {
            let npc_name = args.string(0, "NpcName")?;
            let key = ctx
                .runtime
                .register_callback("DialogueService", "OnEnd", Some(&npc_name));
            Ok(Value::from(key))
        }
        "OnChoice" => {
            let dialogue_id = args.string(0, "DialogueId")?;
            let key =
                ctx.runtime
                    .register_callback("DialogueService", "OnChoice", Some(&dialogue_id));
            Ok(Value::from(key))
        }
        _ => Err(unknown("DialogueService", method)),
    }
}

// ============================================================================
// MailService (8 functions)
// ============================================================================

fn mail_to_value(m: &Mail) -> Value {
    Value::map([
        ("MailId", Value::from(m.id.clone())),
        ("From", Value::from(m.from.clone())),
        ("To", Value::from(m.to.clone())),
        ("Subject", Value::from(m.subject.clone())),
        ("Body", Value::from(m.body.clone())),
        ("Attachments", m.attachments.clone()),
        ("Read", Value::Bool(m.read)),
        ("Claimed", Value::Bool(m.claimed)),
    ])
}

fn dispatch_mail(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "Send" => {
            let from = args.string(0, "FromUsername")?;
            let to = args.string(1, "ToUsername")?;
            let subject = args.string(2, "Subject")?;
            let body = args.string(3, "Body")?;
            let attachments = args.at(4).cloned().unwrap_or(Value::Nil);
            require_player(&ctx.runtime, &from)?;
            require_player(&ctx.runtime, &to)?;
            let mail_id = ctx.runtime.next_id_str("mail");
            let mail = Mail {
                id: mail_id.clone(),
                from: from.clone(),
                to: to.clone(),
                subject,
                body,
                attachments,
                read: false,
                claimed: false,
            };
            ctx.runtime
                .mailboxes
                .write()
                .entry(to)
                .or_default()
                .insert(mail_id.clone(), mail.clone());
            ctx.runtime
                .outboxes
                .write()
                .entry(from)
                .or_default()
                .insert(mail_id.clone(), mail);
            Ok(Value::from(mail_id))
        }
        "GetInbox" => {
            let username = args.string(0, "Username")?;
            let limit = args.integer_or(1, "Limit", 50)?;
            require_player(&ctx.runtime, &username)?;
            let mailboxes = ctx.runtime.mailboxes.read();
            let mut list: Vec<Value> = mailboxes
                .get(&username)
                .map(|box_map| box_map.values().map(mail_to_value).collect())
                .unwrap_or_default();
            list.truncate(limit.max(0) as usize);
            Ok(Value::Array(list))
        }
        "GetOutbox" => {
            let username = args.string(0, "Username")?;
            let limit = args.integer_or(1, "Limit", 50)?;
            require_player(&ctx.runtime, &username)?;
            let outboxes = ctx.runtime.outboxes.read();
            let mut list: Vec<Value> = outboxes
                .get(&username)
                .map(|box_map| box_map.values().map(mail_to_value).collect())
                .unwrap_or_default();
            list.truncate(limit.max(0) as usize);
            Ok(Value::Array(list))
        }
        "GetMail" => {
            let mail_id = args.string(0, "MailId")?;
            let mailboxes = ctx.runtime.mailboxes.read();
            for box_map in mailboxes.values() {
                if let Some(mail) = box_map.get(&mail_id) {
                    return Ok(mail_to_value(mail));
                }
            }
            let outboxes = ctx.runtime.outboxes.read();
            for box_map in outboxes.values() {
                if let Some(mail) = box_map.get(&mail_id) {
                    return Ok(mail_to_value(mail));
                }
            }
            Err(ApiError::NotFound {
                kind: "mail",
                name: mail_id,
            })
        }
        "Read" => {
            let mail_id = args.string(0, "MailId")?;
            let mut mailboxes = ctx.runtime.mailboxes.write();
            for box_map in mailboxes.values_mut() {
                if let Some(mail) = box_map.get_mut(&mail_id) {
                    mail.read = true;
                    return Ok(Value::Bool(true));
                }
            }
            Err(ApiError::NotFound {
                kind: "mail",
                name: mail_id,
            })
        }
        "Delete" => {
            let mail_id = args.string(0, "MailId")?;
            let mut mailboxes = ctx.runtime.mailboxes.write();
            for box_map in mailboxes.values_mut() {
                if box_map.remove(&mail_id).is_some() {
                    return Ok(Value::Bool(true));
                }
            }
            Ok(Value::Bool(false))
        }
        "ClaimAttachments" => {
            let mail_id = args.string(0, "MailId")?;
            let username = args
                .string(0, "Username")
                .unwrap_or_else(|_| "Alice".to_owned());
            // Note: MailService.ClaimAttachments(MailId, Username)
            let username = args.string(1, "Username").unwrap_or(username);
            require_player(&ctx.runtime, &username)?;
            let mut mailboxes = ctx.runtime.mailboxes.write();
            let box_map = mailboxes
                .get_mut(&username)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "mail",
                    name: mail_id.clone(),
                })?;
            let mail = box_map
                .get_mut(&mail_id)
                .ok_or_else(|| ApiError::NotFound {
                    kind: "mail",
                    name: mail_id,
                })?;
            if mail.claimed {
                return Err(ApiError::Rule("attachments already claimed".to_owned()));
            }
            mail.claimed = true;
            Ok(mail.attachments.clone())
        }
        "OnReceive" => {
            let username = args.string(0, "Username")?;
            require_player(&ctx.runtime, &username)?;
            let key = ctx
                .runtime
                .register_callback("MailService", "OnReceive", Some(&username));
            Ok(Value::from(key))
        }
        _ => Err(unknown("MailService", method)),
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Side;
    use crate::registry::call;
    use crate::runtime::PlayerRecord;

    fn setup_context() -> ApiContext {
        let ctx = ApiContext::new(Side::Server);
        ctx.runtime.player_join(PlayerRecord::new("Alice", 1, "VN"));
        ctx.runtime.player_join(PlayerRecord::new("Bob", 2, "US"));
        ctx
    }

    #[test]
    fn quest_lifecycle() {
        let ctx = setup_context();
        call(
            &ctx,
            "QuestService",
            "Register",
            vec![
                Value::from("Q1"),
                Value::from("First Steps"),
                Value::from("Defeat 5 slimes"),
                Value::array([Value::from("Slime 1"), Value::from("Slime 2")]),
                Value::map([("Coin", Value::from(50i64))]),
            ],
        )
        .unwrap();

        call(
            &ctx,
            "QuestService",
            "Assign",
            vec![Value::from("Alice"), Value::from("Q1")],
        )
        .unwrap();

        let active = call(
            &ctx,
            "QuestService",
            "GetActive",
            vec![Value::from("Alice")],
        )
        .unwrap();
        assert_eq!(active, Value::array([Value::from("Q1")]));

        let prog = call(
            &ctx,
            "QuestService",
            "AddProgress",
            vec![
                Value::from("Alice"),
                Value::from("Q1"),
                Value::from(1i64),
                Value::from(2.5),
            ],
        )
        .unwrap();
        assert_eq!(prog, Value::from(2.5));

        assert_eq!(
            call(
                &ctx,
                "QuestService",
                "IsCompleted",
                vec![Value::from("Alice"), Value::from("Q1")]
            )
            .unwrap(),
            Value::Bool(false)
        );

        call(
            &ctx,
            "QuestService",
            "Complete",
            vec![Value::from("Alice"), Value::from("Q1")],
        )
        .unwrap();
        assert_eq!(
            call(
                &ctx,
                "QuestService",
                "IsCompleted",
                vec![Value::from("Alice"), Value::from("Q1")]
            )
            .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn achievement_flow() {
        let ctx = setup_context();
        call(
            &ctx,
            "AchievementService",
            "Register",
            vec![
                Value::from("ACH1"),
                Value::from("Novice"),
                Value::from("Reach level 5"),
                Value::from(5.0),
            ],
        )
        .unwrap();

        let all = call(&ctx, "AchievementService", "GetAll", vec![]).unwrap();
        assert!(matches!(all, Value::Array(list) if list.len() == 1));

        call(
            &ctx,
            "AchievementService",
            "AddProgress",
            vec![Value::from("Alice"), Value::from("ACH1"), Value::from(3.0)],
        )
        .unwrap();
        assert_eq!(
            call(
                &ctx,
                "AchievementService",
                "IsUnlocked",
                vec![Value::from("Alice"), Value::from("ACH1")]
            )
            .unwrap(),
            Value::Bool(false)
        );

        call(
            &ctx,
            "AchievementService",
            "AddProgress",
            vec![Value::from("Alice"), Value::from("ACH1"), Value::from(2.5)],
        )
        .unwrap();
        assert_eq!(
            call(
                &ctx,
                "AchievementService",
                "IsUnlocked",
                vec![Value::from("Alice"), Value::from("ACH1")]
            )
            .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn leaderboard_ranking() {
        let ctx = setup_context();
        call(
            &ctx,
            "LeaderboardService",
            "CreateBoard",
            vec![Value::from("HighScores"), Value::from("Descending")],
        )
        .unwrap();

        call(
            &ctx,
            "LeaderboardService",
            "SetScore",
            vec![
                Value::from("HighScores"),
                Value::from("Alice"),
                Value::from(100.0),
            ],
        )
        .unwrap();
        call(
            &ctx,
            "LeaderboardService",
            "SetScore",
            vec![
                Value::from("HighScores"),
                Value::from("Bob"),
                Value::from(200.0),
            ],
        )
        .unwrap();

        let top = call(
            &ctx,
            "LeaderboardService",
            "GetTopPlayers",
            vec![Value::from("HighScores"), Value::from(10i64)],
        )
        .unwrap();
        match top {
            Value::Array(entries) => {
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].as_map().unwrap()["Username"], Value::from("Bob"));
                assert_eq!(
                    entries[1].as_map().unwrap()["Username"],
                    Value::from("Alice")
                );
            }
            _ => panic!("expected array"),
        }

        let rank = call(
            &ctx,
            "LeaderboardService",
            "GetRank",
            vec![Value::from("HighScores"), Value::from("Bob")],
        )
        .unwrap();
        assert_eq!(rank, Value::from(1i64));
    }

    #[test]
    fn reward_and_cooldown() {
        let ctx = setup_context();
        let claim = call(
            &ctx,
            "RewardService",
            "ClaimDaily",
            vec![Value::from("Alice")],
        )
        .unwrap();
        assert_eq!(claim.as_map().unwrap()["Streak"], Value::from(1i64));

        call(
            &ctx,
            "RewardService",
            "RegisterCode",
            vec![
                Value::from("WELCOME"),
                Value::map([("Gems", Value::from(100i64))]),
                Value::from(10i64),
                Value::Nil,
            ],
        )
        .unwrap();

        let redeemed = call(
            &ctx,
            "RewardService",
            "RedeemCode",
            vec![Value::from("Alice"), Value::from("WELCOME")],
        )
        .unwrap();
        assert_eq!(redeemed.as_map().unwrap()["Gems"], Value::from(100i64));

        // Cooldown
        call(
            &ctx,
            "CooldownService",
            "Start",
            vec![Value::from("Alice"), Value::from("Heal"), Value::from(10.0)],
        )
        .unwrap();

        assert_eq!(
            call(
                &ctx,
                "CooldownService",
                "IsReady",
                vec![Value::from("Alice"), Value::from("Heal")]
            )
            .unwrap(),
            Value::Bool(false)
        );
        let rem = call(
            &ctx,
            "CooldownService",
            "GetRemaining",
            vec![Value::from("Alice"), Value::from("Heal")],
        )
        .unwrap();
        assert!(matches!(rem, Value::Number(n) if n > 0.0));
    }

    #[test]
    fn party_and_mail() {
        let ctx = setup_context();
        let party_id = call(
            &ctx,
            "PartyService",
            "CreateParty",
            vec![Value::from("Alice"), Value::from(4i64)],
        )
        .unwrap();
        let pid_str = party_id.as_str().unwrap().to_owned();

        call(
            &ctx,
            "PartyService",
            "Invite",
            vec![Value::from(pid_str.as_str()), Value::from("Bob")],
        )
        .unwrap();
        call(
            &ctx,
            "PartyService",
            "AcceptInvite",
            vec![Value::from(pid_str.as_str()), Value::from("Bob")],
        )
        .unwrap();

        let party = call(
            &ctx,
            "PartyService",
            "GetParty",
            vec![Value::from(pid_str.as_str())],
        )
        .unwrap();
        assert_eq!(party.as_map().unwrap()["Leader"], Value::from("Alice"));

        // Mail
        let mail_id = call(
            &ctx,
            "MailService",
            "Send",
            vec![
                Value::from("Alice"),
                Value::from("Bob"),
                Value::from("Hello!"),
                Value::from("Have some gold"),
                Value::map([("Gold", Value::from(50i64))]),
            ],
        )
        .unwrap();
        let mid_str = mail_id.as_str().unwrap().to_owned();

        let inbox = call(
            &ctx,
            "MailService",
            "GetInbox",
            vec![Value::from("Bob"), Value::from(10i64)],
        )
        .unwrap();
        assert!(matches!(inbox, Value::Array(list) if list.len() == 1));

        let attachments = call(
            &ctx,
            "MailService",
            "ClaimAttachments",
            vec![Value::from(mid_str.as_str()), Value::from("Bob")],
        )
        .unwrap();
        assert_eq!(attachments.as_map().unwrap()["Gold"], Value::from(50i64));
    }
}
