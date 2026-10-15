/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Thế giới, âm thanh & hiệu ứng: InstanceService, CollisionService, LightingService,
//! PhysicsService, TweenService, SpawnService, ProximityService, PathfindingService,
//! AccessoryService, AudioService, SoundEffectService, EffectService

use nb_types::{Container, InstanceType, Value, Vector3};
use nb_world::InstanceProperties;

use crate::args::Args;
use crate::error::{ApiError, ApiResult};
use crate::runtime::{
    ApiContext, AudioHandle, CollisionGroup, EffectInstance, PathAgent, ProximityPrompt,
    ScreenEffect, SoundEmitter, SpawnPoint, Tween,
};
use crate::services::unknown;

/// Routes a call in the world group.
pub fn dispatch(ctx: &ApiContext, service: &str, method: &str, args: &Args) -> ApiResult<Value> {
    match service {
        "InstanceService" => dispatch_instance(ctx, method, args),
        "CollisionService" => dispatch_collision(ctx, method, args),
        "LightingService" => dispatch_lighting(ctx, method, args),
        "PhysicsService" => dispatch_physics(ctx, method, args),
        "TweenService" => dispatch_tween(ctx, method, args),
        "SpawnService" => dispatch_spawn(ctx, method, args),
        "ProximityService" => dispatch_proximity(ctx, method, args),
        "PathfindingService" => dispatch_pathfinding(ctx, method, args),
        "AccessoryService" => dispatch_accessory(ctx, method, args),
        "AudioService" => dispatch_audio(ctx, method, args),
        "SoundEffectService" => dispatch_sound_effect(ctx, method, args),
        "EffectService" => dispatch_effect(ctx, method, args),
        _ => Err(unknown(service, method)),
    }
}

// ============================================================================
// InstanceService
// ============================================================================

fn dispatch_instance(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "Create" => {
            let name = args.string(0, "Name")?;
            let kind_str = args.string(1, "Type")?;
            let parent_str = args.string_or(2, "Parent", "")?;
            let props_val = args.table_or_empty(3, "Properties")?;

            let kind: InstanceType = kind_str.parse().map_err(|e: String| args.rule(e))?;
            let container: Container = if parent_str.is_empty() {
                kind.default_container()
            } else {
                parent_str.parse().map_err(|e: String| args.rule(e))?
            };
            let props = InstanceProperties::from_value(&props_val);
            let id = ctx
                .runtime
                .world
                .write()
                .create(name.clone(), kind, container, props)?;
            Ok(Value::map([
                ("Name", Value::from(name)),
                ("GUID", Value::from(id.as_str())),
                ("Type", Value::from(kind.as_str())),
                ("Parent", Value::from(container.as_str())),
            ]))
        }
        "Destroy" => {
            let name = args.string(0, "Name")?;
            let index = args.optional_index(1)?;
            let id = ctx.runtime.world.write().destroy(&name, index)?;
            Ok(Value::map([
                ("Name", Value::from(name)),
                ("GUID", Value::from(id.as_str())),
            ]))
        }
        "DestroyByGUID" => {
            let guid = args.string(0, "GUID")?;
            ctx.runtime.world.write().destroy_by_guid(&guid)?;
            Ok(Value::Bool(true))
        }
        "GetByGUID" => {
            let guid = args.string(0, "GUID")?;
            let world = ctx.runtime.world.read();
            let inst = world.by_guid(&guid)?;
            Ok(Value::map([
                ("Name", Value::from(inst.name.clone())),
                ("GUID", Value::from(inst.id.as_str())),
                ("Type", Value::from(inst.kind.as_str())),
            ]))
        }
        "GetGUID" => {
            let name = args.string(0, "Name")?;
            let index = args.optional_index(1)?;
            let world = ctx.runtime.world.read();
            let inst = world.find(&name, index)?;
            Ok(Value::from(inst.id.as_str()))
        }
        "GetGUIDs" => {
            let name = args.string(0, "Name")?;
            let world = ctx.runtime.world.read();
            let guids = world.guids_named(&name);
            Ok(Value::array(
                guids.iter().map(|id| Value::from(id.as_str())),
            ))
        }
        "IsEnabled" => {
            let name = args.string(0, "Name")?;
            let index = args.optional_index(1)?;
            let world = ctx.runtime.world.read();
            let inst = world.find(&name, index)?;
            let enabled = world.is_enabled(inst.id.as_str())?;
            Ok(Value::Bool(enabled))
        }
        "Move" => {
            let name = args.string(0, "Name")?;
            let pos = args.position(1, "Position")?;
            let index = args.optional_index(2)?;
            let guid = {
                let world = ctx.runtime.world.read();
                let inst = world.find(&name, index)?;
                inst.id.as_str().to_owned()
            };
            ctx.runtime.world.write().move_to(&guid, pos)?;
            Ok(Value::Bool(true))
        }
        "MoveByGUID" => {
            let guid = args.string(0, "GUID")?;
            let pos = args.position(1, "Position")?;
            ctx.runtime.world.write().move_to(&guid, pos)?;
            Ok(Value::Bool(true))
        }
        "SetEnabled" => {
            let name = args.string(0, "Name")?;
            let enabled = args.boolean(1, "Boolean")?;
            let index = args.optional_index(2)?;
            let guid = {
                let world = ctx.runtime.world.read();
                let inst = world.find(&name, index)?;
                inst.id.as_str().to_owned()
            };
            ctx.runtime.world.write().set_enabled(&guid, enabled)?;
            Ok(Value::Bool(true))
        }
        "require" => {
            let name = args.string(0, "Name")?;
            let index = args.optional_index(1)?;
            let world = ctx.runtime.world.read();
            let inst = world.module(&name, index)?;
            Ok(Value::map([
                ("Name", Value::from(inst.name.clone())),
                ("GUID", Value::from(inst.id.as_str())),
            ]))
        }
        "requireByGUID" => {
            let guid = args.string(0, "GUID")?;
            let world = ctx.runtime.world.read();
            let inst = world.by_guid(&guid)?;
            Ok(Value::map([
                ("Name", Value::from(inst.name.clone())),
                ("GUID", Value::from(inst.id.as_str())),
            ]))
        }
        _ => Err(unknown("InstanceService", method)),
    }
}

// ============================================================================
// CollisionService
// ============================================================================

fn dispatch_collision(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "AssignGroup" => {
            let instance = args.string(0, "Instance")?;
            let group_name = args.string(1, "GroupName")?;
            {
                let groups = ctx.runtime.collision_groups.read();
                if !groups.contains_key(&group_name) {
                    drop(groups);
                    return Err(ApiError::NotFound {
                        kind: "collision group",
                        name: group_name,
                    });
                }
                drop(groups);
            }
            ctx.runtime
                .collision_groups
                .write()
                .get_mut(&group_name)
                .map(|g| g.members.insert(instance.clone()));
            ctx.runtime
                .collision_membership
                .write()
                .insert(instance, group_name);
            Ok(Value::Bool(true))
        }
        "CreateGroup" => {
            let name = args.string(0, "GroupName")?;
            let mut groups = ctx.runtime.collision_groups.write();
            if groups.contains_key(&name) {
                return Err(ApiError::AlreadyExists {
                    kind: "collision group",
                    name,
                });
            }
            groups.insert(
                name.clone(),
                CollisionGroup {
                    name,
                    members: Default::default(),
                    ignores: Default::default(),
                },
            );
            Ok(Value::Bool(true))
        }
        "DeleteGroup" => {
            let name = args.string(0, "GroupName")?;
            let removed = ctx.runtime.collision_groups.write().remove(&name).is_some();
            if removed {
                ctx.runtime
                    .collision_membership
                    .write()
                    .retain(|_, g| g != &name);
            }
            Ok(Value::Bool(removed))
        }
        "GetGroup" => {
            let instance = args.string(0, "Instance")?;
            let membership = ctx.runtime.collision_membership.read();
            match membership.get(&instance) {
                Some(g) => Ok(Value::from(g.clone())),
                None => Ok(Value::Nil),
            }
        }
        "GetGroups" => {
            let groups = ctx.runtime.collision_groups.read();
            let list: Vec<Value> = groups
                .values()
                .map(|g| {
                    Value::map([
                        ("Name", Value::from(g.name.clone())),
                        (
                            "Members",
                            Value::array(g.members.iter().map(|m| Value::from(m.clone()))),
                        ),
                    ])
                })
                .collect();
            Ok(Value::array(list))
        }
        "SetCollision" => {
            let group_a = args.string(0, "GroupA")?;
            let group_b = args.string(1, "GroupB")?;
            let collides = args.boolean(2, "Boolean")?;
            let mut groups = ctx.runtime.collision_groups.write();
            if let Some(g) = groups.get_mut(&group_a) {
                if collides {
                    g.ignores.remove(&group_b);
                } else {
                    g.ignores.insert(group_b.clone());
                }
            }
            if let Some(g) = groups.get_mut(&group_b) {
                if collides {
                    g.ignores.remove(&group_a);
                } else {
                    g.ignores.insert(group_a.clone());
                }
            }
            Ok(Value::Bool(true))
        }
        _ => Err(unknown("CollisionService", method)),
    }
}

// ============================================================================
// LightingService
// ============================================================================

fn dispatch_lighting(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "GetProperty" => {
            let prop = args.string(0, "Property")?;
            let env = ctx.runtime.environment.read();
            let val = match prop.as_str() {
                "Time" => Value::from(env.time.clone()),
                "Weather" => Value::from(env.weather.clone()),
                "Ambient" => env.ambient.to_value(),
                "FogEnabled" => Value::Bool(env.fog_enabled),
                "FogColor" => env.fog_color.to_value(),
                "FogStart" => Value::Number(env.fog_start),
                "FogEnd" => Value::Number(env.fog_end),
                other => env.properties.get(other).cloned().unwrap_or(Value::Nil),
            };
            Ok(val)
        }
        "GetTime" => Ok(Value::from(ctx.runtime.environment.read().time.clone())),
        "GetWeather" => {
            let env = ctx.runtime.environment.read();
            Ok(Value::map([
                ("Weather", Value::from(env.weather.clone())),
                ("Intensity", Value::Number(env.weather_intensity)),
            ]))
        }
        "SetAmbient" => {
            let color = args.color(0, "RGBA")?;
            ctx.runtime.environment.write().ambient = color;
            Ok(Value::Bool(true))
        }
        "SetFog" => {
            let enabled = args.boolean(0, "Enabled")?;
            let color = args.optional_color(1, "Color")?;
            let start = args.number_or(2, "Start", 0.0)?;
            let end = args.number_or(3, "End", 500.0)?;
            let mut env = ctx.runtime.environment.write();
            env.fog_enabled = enabled;
            if let Some(c) = color {
                env.fog_color = c;
            }
            env.fog_start = start;
            env.fog_end = end;
            Ok(Value::Bool(true))
        }
        "SetProperty" => {
            let prop = args.string(0, "Property")?;
            let val = args.at(1).cloned().unwrap_or(Value::Nil);
            ctx.runtime.environment.write().properties.insert(prop, val);
            Ok(Value::Bool(true))
        }
        "SetTime" => {
            let time_of_day = args.string(0, "TimeOfDay")?;
            ctx.runtime.environment.write().time = time_of_day;
            Ok(Value::Bool(true))
        }
        "SetWeather" => {
            let weather_type = args.string(0, "WeatherType")?;
            let intensity = args.number_or(1, "Intensity", 0.5)?;
            let mut env = ctx.runtime.environment.write();
            env.weather = weather_type;
            env.weather_intensity = intensity;
            Ok(Value::Bool(true))
        }
        _ => Err(unknown("LightingService", method)),
    }
}

// ============================================================================
// PhysicsService
// ============================================================================

fn dispatch_physics(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "ApplyForce" => {
            let instance = args.string(0, "Instance")?;
            let force = args.position(1, "Force")?;
            let duration = args.number_or(2, "Duration", 1.0)?;
            ctx.runtime
                .bodies
                .write()
                .entry(instance)
                .or_default()
                .force = Some((force, duration));
            Ok(Value::Bool(true))
        }
        "ApplyImpulse" => {
            let instance = args.string(0, "Instance")?;
            let impulse = args.position(1, "Force")?;
            let mut bodies = ctx.runtime.bodies.write();
            let body = bodies.entry(instance).or_default();
            body.velocity = Vector3::new(
                body.velocity.x + impulse.x,
                body.velocity.y + impulse.y,
                body.velocity.z + impulse.z,
            );
            Ok(Value::Bool(true))
        }
        "GetGravity" => {
            let gravity = ctx
                .runtime
                .globals
                .get("Physics.Gravity")
                .and_then(|v| {
                    if let Value::Number(n) = v {
                        Some(n)
                    } else {
                        None
                    }
                })
                .unwrap_or(-196.2);
            Ok(Value::Number(gravity))
        }
        "GetVelocity" => {
            let instance = args.string(0, "Instance")?;
            let bodies = ctx.runtime.bodies.read();
            let vel = bodies
                .get(&instance)
                .map(|b| b.velocity)
                .unwrap_or(Vector3::ZERO);
            Ok(vel.to_value())
        }
        "SetAnchored" => {
            let instance = args.string(0, "Instance")?;
            let anchored = args.boolean(1, "Boolean")?;
            ctx.runtime
                .bodies
                .write()
                .entry(instance)
                .or_default()
                .anchored = anchored;
            Ok(Value::Bool(true))
        }
        "SetElasticity" => {
            let instance = args.string(0, "Instance")?;
            let val = args.number(1, "Elasticity")?;
            ctx.runtime
                .bodies
                .write()
                .entry(instance)
                .or_default()
                .elasticity = val;
            Ok(Value::Bool(true))
        }
        "SetFriction" => {
            let instance = args.string(0, "Instance")?;
            let val = args.number(1, "Friction")?;
            ctx.runtime
                .bodies
                .write()
                .entry(instance)
                .or_default()
                .friction = val;
            Ok(Value::Bool(true))
        }
        "SetGravity" => {
            let gravity = args.number(0, "Gravity")?;
            ctx.runtime
                .globals
                .set("Physics.Gravity", Value::Number(gravity))
                .map_err(ApiError::Store)?;
            Ok(Value::Bool(true))
        }
        "SetMass" => {
            let instance = args.string(0, "Instance")?;
            let mass = args.number(1, "Mass")?;
            ctx.runtime.bodies.write().entry(instance).or_default().mass = mass;
            Ok(Value::Bool(true))
        }
        "SetVelocity" => {
            let instance = args.string(0, "Instance")?;
            let vel = args.position(1, "Velocity")?;
            ctx.runtime
                .bodies
                .write()
                .entry(instance)
                .or_default()
                .velocity = vel;
            Ok(Value::Bool(true))
        }
        _ => Err(unknown("PhysicsService", method)),
    }
}

// ============================================================================
// TweenService
// ============================================================================

fn dispatch_tween(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "Move" => {
            let instance = args.string(0, "Instance")?;
            let end_pos = args.position(1, "EndPosition")?;
            let time = args.number_or(2, "Time", 1.0)?;
            let _easing_dir = args.string_or(3, "EasingDirection", "Out")?;
            let _easing_style = args.string_or(4, "EasingStyle", "Linear")?;
            let looping = args.boolean_or(5, "Loop", false)?;
            let color = args.optional_color(6, "RGBA")?;

            // Get start from current world position or zero
            let start = {
                let world = ctx.runtime.world.read();
                world
                    .guids_named(&instance)
                    .first()
                    .and_then(|id| world.by_guid(id.as_str()).ok())
                    .and_then(|inst| Some(inst.properties.position))
                    .unwrap_or(Vector3::ZERO)
            };

            ctx.runtime.tweens.write().insert(
                instance.clone(),
                Tween {
                    instance: instance.clone(),
                    start,
                    end: end_pos,
                    duration: time,
                    elapsed: 0.0,
                    looping,
                    paused: false,
                    finished: false,
                    target_color: color,
                },
            );
            ctx.runtime.tween_order.lock().push(instance);
            Ok(Value::Bool(true))
        }
        "Pause" => {
            let instance = args.string(0, "Instance")?;
            if let Some(t) = ctx.runtime.tweens.write().get_mut(&instance) {
                t.paused = true;
            }
            Ok(Value::Bool(true))
        }
        "Resume" => {
            let instance = args.string(0, "Instance")?;
            if let Some(t) = ctx.runtime.tweens.write().get_mut(&instance) {
                t.paused = false;
            }
            Ok(Value::Bool(true))
        }
        "Stop" => {
            let instance = args.string(0, "Instance")?;
            if let Some(t) = ctx.runtime.tweens.write().get_mut(&instance) {
                t.finished = true;
            }
            Ok(Value::Bool(true))
        }
        "Back" => {
            // Reverses the tween - swaps start/end
            let instance = args.string(0, "Instance")?;
            if let Some(t) = ctx.runtime.tweens.write().get_mut(&instance) {
                let old_start = t.start;
                t.start = t.end;
                t.end = old_start;
                t.elapsed = 0.0;
                t.paused = false;
                t.finished = false;
            }
            Ok(Value::Bool(true))
        }
        _ => Err(unknown("TweenService", method)),
    }
}

// ============================================================================
// SpawnService
// ============================================================================

fn dispatch_spawn(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    fn spawn_to_value(sp: &SpawnPoint) -> Value {
        Value::map([
            ("Name", Value::from(sp.name.clone())),
            ("Position", sp.position.to_value()),
        ])
    }

    match method {
        "AssignSpawn" => {
            let _target_type = args.string(0, "TargetType")?;
            let username = args.string(1, "Username")?;
            let spawn_name = args.string(2, "SpawnName")?;
            let mut spawns = ctx.runtime.spawns.write();
            if let Some(sp) = spawns.get_mut(&spawn_name) {
                sp.assigned.insert(username);
                Ok(Value::Bool(true))
            } else {
                Err(ApiError::NotFound {
                    kind: "spawn",
                    name: spawn_name,
                })
            }
        }
        "CreateSpawn" => {
            let name = args.string(0, "SpawnName")?;
            let pos = args
                .optional_position(1, "Position")?
                .unwrap_or(Vector3::ZERO);
            let mut spawns = ctx.runtime.spawns.write();
            if spawns.contains_key(&name) {
                return Err(ApiError::AlreadyExists {
                    kind: "spawn",
                    name,
                });
            }
            spawns.insert(
                name.clone(),
                SpawnPoint {
                    name,
                    position: pos,
                    assigned: Default::default(),
                },
            );
            Ok(Value::Bool(true))
        }
        "DeleteSpawn" => {
            let name = args.string(0, "SpawnName")?;
            let removed = ctx.runtime.spawns.write().remove(&name).is_some();
            Ok(Value::Bool(removed))
        }
        "GetAllSpawns" => {
            let spawns = ctx.runtime.spawns.read();
            Ok(Value::array(spawns.values().map(spawn_to_value)))
        }
        "GetSpawn" => {
            let name = args.string(0, "SpawnName")?;
            let spawns = ctx.runtime.spawns.read();
            match spawns.get(&name) {
                Some(sp) => Ok(spawn_to_value(sp)),
                None => Err(ApiError::NotFound {
                    kind: "spawn",
                    name,
                }),
            }
        }
        "Respawn" => {
            let _target_type = args.string(0, "TargetType")?;
            let username = args.string(1, "Username")?;
            // Find assigned spawn or first spawn
            let pos = {
                let spawns = ctx.runtime.spawns.read();
                spawns
                    .values()
                    .find(|sp| sp.assigned.contains(&username))
                    .or_else(|| spawns.values().next())
                    .map(|sp| sp.position)
                    .unwrap_or(Vector3::ZERO)
            };
            if let Some(p) = ctx.runtime.players.write().get_mut(&username) {
                p.position = pos;
            }
            Ok(pos.to_value())
        }
        "RespawnAt" => {
            let _target_type = args.string(0, "TargetType")?;
            let username = args.string(1, "Username")?;
            let spawn_name = args.string(2, "SpawnName")?;
            let pos = ctx
                .runtime
                .spawns
                .read()
                .get(&spawn_name)
                .map(|sp| sp.position)
                .unwrap_or(Vector3::ZERO);
            if let Some(p) = ctx.runtime.players.write().get_mut(&username) {
                p.position = pos;
            }
            Ok(pos.to_value())
        }
        "SetSpawn" => {
            let name = args.string(0, "SpawnName")?;
            let pos = args.position(1, "Position")?;
            let mut spawns = ctx.runtime.spawns.write();
            if let Some(sp) = spawns.get_mut(&name) {
                sp.position = pos;
                Ok(Value::Bool(true))
            } else {
                Err(ApiError::NotFound {
                    kind: "spawn",
                    name,
                })
            }
        }
        _ => Err(unknown("SpawnService", method)),
    }
}

// ============================================================================
// ProximityService
// ============================================================================

fn dispatch_proximity(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    fn prompt_to_value(p: &ProximityPrompt) -> Value {
        Value::map([
            ("Id", Value::from(p.id.clone())),
            ("Instance", Value::from(p.instance.clone())),
            ("Label", Value::from(p.label.clone())),
            ("KeyCode", Value::from(p.key_code.clone())),
            ("MaxDistance", Value::Number(p.max_distance)),
            ("HoldDuration", Value::Number(p.hold_duration)),
            ("Enabled", Value::Bool(p.enabled)),
            ("Visible", Value::Bool(p.visible)),
        ])
    }

    match method {
        "Create" => {
            let id = args.string(0, "PromptId")?;
            let instance = args.string(1, "Instance")?;
            let label = args.string_or(2, "Label", "")?;
            let key_code = args.string_or(3, "KeyCode", "E")?;
            let max_distance = args.number_or(4, "MaxDistance", 10.0)?;
            let hold_duration = args.number_or(5, "HoldDuration", 0.0)?;
            ctx.runtime.prompts.write().insert(
                id.clone(),
                ProximityPrompt {
                    id,
                    instance,
                    label,
                    key_code,
                    max_distance,
                    hold_duration,
                    visible: true,
                    enabled: true,
                },
            );
            Ok(Value::Bool(true))
        }
        "GetAllPrompts" => {
            let prompts = ctx.runtime.prompts.read();
            Ok(Value::array(prompts.values().map(prompt_to_value)))
        }
        "GetPrompt" => {
            let id = args.string(0, "PromptId")?;
            let prompts = ctx.runtime.prompts.read();
            match prompts.get(&id) {
                Some(p) => Ok(prompt_to_value(p)),
                None => Err(ApiError::NotFound {
                    kind: "proximity prompt",
                    name: id,
                }),
            }
        }
        "OnEnter" | "OnExit" | "OnTriggered" => {
            let id = args.string(0, "PromptId")?;
            let cb = args.at(1).cloned().unwrap_or(Value::Nil);
            let key = format!("ProximityService.{method}:{id}");
            ctx.runtime
                .callbacks
                .set(key, cb)
                .map_err(ApiError::Store)?;
            Ok(Value::Bool(true))
        }
        "Remove" => {
            let id = args.string(0, "PromptId")?;
            let removed = ctx.runtime.prompts.write().remove(&id).is_some();
            Ok(Value::Bool(removed))
        }
        "SetEnabled" => {
            let id = args.string(0, "PromptId")?;
            let enabled = args.boolean(1, "Boolean")?;
            if let Some(p) = ctx.runtime.prompts.write().get_mut(&id) {
                p.enabled = enabled;
                Ok(Value::Bool(true))
            } else {
                Err(ApiError::NotFound {
                    kind: "proximity prompt",
                    name: id,
                })
            }
        }
        "SetVisible" => {
            let id = args.string(0, "PromptId")?;
            let visible = args.boolean(1, "Boolean")?;
            if let Some(p) = ctx.runtime.prompts.write().get_mut(&id) {
                p.visible = visible;
                Ok(Value::Bool(true))
            } else {
                Err(ApiError::NotFound {
                    kind: "proximity prompt",
                    name: id,
                })
            }
        }
        _ => Err(unknown("ProximityService", method)),
    }
}

// ============================================================================
// PathfindingService
// ============================================================================

fn dispatch_pathfinding(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "ComputePath" => {
            let npc = args.string(0, "NpcName")?;
            let dest = args.position(1, "Destination")?;
            let radius = args.number_or(2, "AgentRadius", 1.0)?;
            let height = args.number_or(3, "AgentHeight", 2.0)?;
            // Simple straight-line path (no nav mesh yet)
            let start = ctx
                .runtime
                .players
                .read()
                .get(&npc)
                .map(|p| p.position)
                .unwrap_or(Vector3::ZERO);
            let waypoints = vec![start, dest];
            ctx.runtime.path_agents.write().insert(
                npc.clone(),
                PathAgent {
                    npc,
                    waypoints: waypoints.clone(),
                    destination: dest,
                    state: "Idle".to_owned(),
                    radius,
                    height,
                },
            );
            Ok(Value::array(waypoints.iter().map(|w| w.to_value())))
        }
        "GetWaypoints" => {
            let npc = args.string(0, "NpcName")?;
            let agents = ctx.runtime.path_agents.read();
            match agents.get(&npc) {
                Some(a) => Ok(Value::array(a.waypoints.iter().map(|w| w.to_value()))),
                None => Ok(Value::array([] as [Value; 0])),
            }
        }
        "MoveAlongWaypoints" => {
            let npc = args.string(0, "NpcName")?;
            let waypoints_val = args.table_or_empty(1, "Waypoints")?;
            let waypoints: Vec<Vector3> = match waypoints_val {
                Value::Array(arr) => arr.iter().filter_map(|v| Vector3::from_value(v)).collect(),
                _ => vec![],
            };
            if let Some(dest) = waypoints.last().copied() {
                let mut agents = ctx.runtime.path_agents.write();
                let agent = agents.entry(npc.clone()).or_insert_with(|| PathAgent {
                    npc: npc.clone(),
                    waypoints: vec![],
                    destination: Vector3::ZERO,
                    state: "Idle".to_owned(),
                    radius: 1.0,
                    height: 2.0,
                });
                agent.waypoints = waypoints;
                agent.destination = dest;
                agent.state = "Moving".to_owned();
            }
            Ok(Value::Bool(true))
        }
        "MoveTo" => {
            let npc = args.string(0, "NpcName")?;
            let dest = args.position(1, "Destination")?;
            let mut agents = ctx.runtime.path_agents.write();
            let agent = agents.entry(npc.clone()).or_insert_with(|| PathAgent {
                npc: npc.clone(),
                waypoints: vec![],
                destination: Vector3::ZERO,
                state: "Idle".to_owned(),
                radius: 1.0,
                height: 2.0,
            });
            agent.destination = dest;
            agent.state = "Moving".to_owned();
            Ok(Value::Bool(true))
        }
        "OnBlocked" | "OnReached" => {
            let npc = args.string(0, "NpcName")?;
            let cb = args.at(1).cloned().unwrap_or(Value::Nil);
            let key = format!("PathfindingService.{method}:{npc}");
            ctx.runtime
                .callbacks
                .set(key, cb)
                .map_err(ApiError::Store)?;
            Ok(Value::Bool(true))
        }
        "Pause" => {
            let npc = args.string(0, "NpcName")?;
            if let Some(a) = ctx.runtime.path_agents.write().get_mut(&npc) {
                a.state = "Paused".to_owned();
            }
            Ok(Value::Bool(true))
        }
        "Resume" => {
            let npc = args.string(0, "NpcName")?;
            if let Some(a) = ctx.runtime.path_agents.write().get_mut(&npc) {
                a.state = "Moving".to_owned();
            }
            Ok(Value::Bool(true))
        }
        "Stop" => {
            let npc = args.string(0, "NpcName")?;
            if let Some(a) = ctx.runtime.path_agents.write().get_mut(&npc) {
                a.state = "Idle".to_owned();
                a.waypoints.clear();
            }
            Ok(Value::Bool(true))
        }
        _ => Err(unknown("PathfindingService", method)),
    }
}

// ============================================================================
// AccessoryService
// ============================================================================

fn dispatch_accessory(ctx: &ApiContext, _method: &str, args: &Args) -> ApiResult<Value> {
    // AccessoryService(Target, TargetType, AssetIds)  — one function, no method
    let target = args.string(0, "arg1")?;
    let _target_type = args.string_or(1, "arg2", "Player")?;
    let ids_raw = args.string_or(2, "arg3", "")?;
    let ids: Vec<String> = ids_raw
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    ctx.runtime.accessories.write().insert(target, ids);
    Ok(Value::Bool(true))
}

// ============================================================================
// AudioService
// ============================================================================

fn dispatch_audio(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "New" => {
            let name = args.string(0, "Name")?;
            let id = args.string(1, "ID")?;
            ctx.runtime.audio.write().insert(
                name.clone(),
                AudioHandle {
                    name,
                    id,
                    volume: 1.0,
                    looping: false,
                    state: "Stopped".to_owned(),
                },
            );
            Ok(Value::Bool(true))
        }
        "Change" => {
            let name = args.string(0, "Name")?;
            let id = args.string(1, "ID")?;
            if let Some(h) = ctx.runtime.audio.write().get_mut(&name) {
                h.id = id;
                Ok(Value::Bool(true))
            } else {
                Err(ApiError::NotFound {
                    kind: "audio handle",
                    name,
                })
            }
        }
        "Loop" => {
            let name = args.string(0, "Name")?;
            let looping = args.boolean(1, "Boolean")?;
            if let Some(h) = ctx.runtime.audio.write().get_mut(&name) {
                h.looping = looping;
                Ok(Value::Bool(true))
            } else {
                Err(ApiError::NotFound {
                    kind: "audio handle",
                    name,
                })
            }
        }
        "Pause" => {
            let name = args.string(0, "Name")?;
            if let Some(h) = ctx.runtime.audio.write().get_mut(&name) {
                h.state = "Paused".to_owned();
                Ok(Value::Bool(true))
            } else {
                Err(ApiError::NotFound {
                    kind: "audio handle",
                    name,
                })
            }
        }
        "Resume" => {
            let name = args.string(0, "Name")?;
            if let Some(h) = ctx.runtime.audio.write().get_mut(&name) {
                h.state = "Playing".to_owned();
                Ok(Value::Bool(true))
            } else {
                Err(ApiError::NotFound {
                    kind: "audio handle",
                    name,
                })
            }
        }
        "SetVolume" => {
            let name = args.string(0, "Name")?;
            let vol = args.number(1, "Volume")?;
            if let Some(h) = ctx.runtime.audio.write().get_mut(&name) {
                h.volume = vol;
                Ok(Value::Bool(true))
            } else {
                Err(ApiError::NotFound {
                    kind: "audio handle",
                    name,
                })
            }
        }
        "Stop" => {
            let name = args.string(0, "Name")?;
            if let Some(h) = ctx.runtime.audio.write().get_mut(&name) {
                h.state = "Stopped".to_owned();
                Ok(Value::Bool(true))
            } else {
                Err(ApiError::NotFound {
                    kind: "audio handle",
                    name,
                })
            }
        }
        _ => Err(unknown("AudioService", method)),
    }
}

// ============================================================================
// SoundEffectService
// ============================================================================

fn dispatch_sound_effect(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "Play" => {
            let sound_id = args.string(0, "SoundId")?;
            let pos = args
                .optional_position(1, "Position")?
                .unwrap_or(Vector3::ZERO);
            let volume = args.number_or(2, "Volume", 1.0)?;
            let rolloff = args.number_or(3, "RolloffDistance", 40.0)?;
            ctx.runtime.sounds.write().insert(
                sound_id.clone(),
                SoundEmitter {
                    id: sound_id,
                    position: pos,
                    attached_to: None,
                    volume,
                    rolloff,
                    playing: true,
                },
            );
            Ok(Value::Bool(true))
        }
        "PlayOn" => {
            let sound_id = args.string(0, "SoundId")?;
            let instance = args.string(1, "Instance")?;
            let volume = args.number_or(2, "Volume", 1.0)?;
            let rolloff = args.number_or(3, "RolloffDistance", 40.0)?;
            ctx.runtime.sounds.write().insert(
                sound_id.clone(),
                SoundEmitter {
                    id: sound_id,
                    position: Vector3::ZERO,
                    attached_to: Some(instance),
                    volume,
                    rolloff,
                    playing: true,
                },
            );
            Ok(Value::Bool(true))
        }
        "Preload" => {
            // Just register existence
            let sound_id = args.string(0, "SoundId")?;
            ctx.runtime
                .sound_volumes
                .write()
                .entry(sound_id)
                .or_insert(1.0);
            Ok(Value::Bool(true))
        }
        "Release" => {
            let sound_id = args.string(0, "SoundId")?;
            ctx.runtime.sounds.write().remove(&sound_id);
            ctx.runtime.sound_volumes.write().remove(&sound_id);
            Ok(Value::Bool(true))
        }
        "SetVolume" => {
            let sound_id = args.string(0, "SoundId")?;
            let vol = args.number(1, "Volume")?;
            ctx.runtime
                .sound_volumes
                .write()
                .insert(sound_id.clone(), vol);
            if let Some(s) = ctx.runtime.sounds.write().get_mut(&sound_id) {
                s.volume = vol;
            }
            Ok(Value::Bool(true))
        }
        "Stop" => {
            let sound_id = args.string(0, "SoundId")?;
            if let Some(s) = ctx.runtime.sounds.write().get_mut(&sound_id) {
                s.playing = false;
            }
            Ok(Value::Bool(true))
        }
        _ => Err(unknown("SoundEffectService", method)),
    }
}

// ============================================================================
// EffectService
// ============================================================================

fn dispatch_effect(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "ClearScreenEffect" => {
            let username = args.string(0, "Username")?;
            ctx.runtime.screen_effects.write().remove(&username);
            Ok(Value::Bool(true))
        }
        "Emit" => {
            let effect_id = args.string(0, "EffectId")?;
            let pos = args
                .optional_position(1, "Position")?
                .unwrap_or(Vector3::ZERO);
            let props = args.table_or_empty(2, "Properties")?;
            ctx.runtime.effects.write().insert(
                effect_id.clone(),
                EffectInstance {
                    id: effect_id,
                    position: pos,
                    attached_to: None,
                    properties: props,
                    emitting: true,
                },
            );
            Ok(Value::Bool(true))
        }
        "EmitOn" => {
            let effect_id = args.string(0, "EffectId")?;
            let instance = args.string(1, "Instance")?;
            let props = args.table_or_empty(2, "Properties")?;
            ctx.runtime.effects.write().insert(
                effect_id.clone(),
                EffectInstance {
                    id: effect_id,
                    position: Vector3::ZERO,
                    attached_to: Some(instance),
                    properties: props,
                    emitting: true,
                },
            );
            Ok(Value::Bool(true))
        }
        "ScreenEffect" => {
            let username = args.string(0, "Username")?;
            let effect_type = args.string(1, "EffectType")?;
            let props = args.table_or_empty(2, "Properties")?;
            let duration = args.number_or(3, "Duration", 1.0)?;
            ctx.runtime.screen_effects.write().insert(
                username.clone(),
                ScreenEffect {
                    username,
                    kind: effect_type,
                    properties: props,
                    duration,
                    elapsed: 0.0,
                },
            );
            Ok(Value::Bool(true))
        }
        "Stop" => {
            let effect_id = args.string(0, "EffectId")?;
            if let Some(e) = ctx.runtime.effects.write().get_mut(&effect_id) {
                e.emitting = false;
            }
            Ok(Value::Bool(true))
        }
        "StopAll" => {
            for e in ctx.runtime.effects.write().values_mut() {
                e.emitting = false;
            }
            Ok(Value::Bool(true))
        }
        _ => Err(unknown("EffectService", method)),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use nb_types::Value;

    use crate::args::Args;
    use crate::error::Side;
    use crate::runtime::ApiContext;

    fn mk(service: &str, method: &str, values: Vec<Value>) -> Args {
        Args::new(service, method, values)
    }

    fn ctx() -> ApiContext {
        ApiContext::new(Side::Server)
    }

    #[test]
    fn lighting_round_trip() {
        let ctx = ctx();

        super::dispatch_lighting(
            &ctx,
            "SetTime",
            &mk("LightingService", "SetTime", vec![Value::from("18:00:00")]),
        )
        .unwrap();
        let t =
            super::dispatch_lighting(&ctx, "GetTime", &mk("LightingService", "GetTime", vec![]))
                .unwrap();
        assert_eq!(t, Value::from("18:00:00"));

        super::dispatch_lighting(
            &ctx,
            "SetWeather",
            &mk(
                "LightingService",
                "SetWeather",
                vec![Value::from("Rain"), Value::Number(0.8)],
            ),
        )
        .unwrap();
        let w = super::dispatch_lighting(
            &ctx,
            "GetWeather",
            &mk("LightingService", "GetWeather", vec![]),
        )
        .unwrap();
        assert!(matches!(w, Value::Map(m) if m.get("Weather") == Some(&Value::from("Rain"))));
    }

    #[test]
    fn physics_velocity() {
        let ctx = ctx();

        super::dispatch_physics(
            &ctx,
            "SetVelocity",
            &mk(
                "PhysicsService",
                "SetVelocity",
                vec![
                    Value::from("Block"),
                    Value::map([
                        ("X", Value::Number(10.0)),
                        ("Y", Value::Number(0.0)),
                        ("Z", Value::Number(0.0)),
                    ]),
                ],
            ),
        )
        .unwrap();

        let vel = super::dispatch_physics(
            &ctx,
            "GetVelocity",
            &mk("PhysicsService", "GetVelocity", vec![Value::from("Block")]),
        )
        .unwrap();
        // Should be a map/array with X=10
        let vx = match &vel {
            Value::Map(m) => m.get("X").and_then(|v| {
                if let Value::Number(n) = v {
                    Some(*n)
                } else {
                    None
                }
            }),
            _ => None,
        };
        assert_eq!(vx, Some(10.0));
    }

    #[test]
    fn spawn_service_flow() {
        let ctx = ctx();

        super::dispatch_spawn(
            &ctx,
            "CreateSpawn",
            &mk(
                "SpawnService",
                "CreateSpawn",
                vec![
                    Value::from("Main"),
                    Value::map([
                        ("X", Value::Number(0.0)),
                        ("Y", Value::Number(5.0)),
                        ("Z", Value::Number(0.0)),
                    ]),
                ],
            ),
        )
        .unwrap();

        let sp = super::dispatch_spawn(
            &ctx,
            "GetSpawn",
            &mk("SpawnService", "GetSpawn", vec![Value::from("Main")]),
        )
        .unwrap();
        assert!(matches!(sp, Value::Map(m) if m.contains_key("Name")));

        let all = super::dispatch_spawn(
            &ctx,
            "GetAllSpawns",
            &mk("SpawnService", "GetAllSpawns", vec![]),
        )
        .unwrap();
        assert!(matches!(all, Value::Array(v) if v.len() == 1));

        super::dispatch_spawn(
            &ctx,
            "DeleteSpawn",
            &mk("SpawnService", "DeleteSpawn", vec![Value::from("Main")]),
        )
        .unwrap();

        let all = super::dispatch_spawn(
            &ctx,
            "GetAllSpawns",
            &mk("SpawnService", "GetAllSpawns", vec![]),
        )
        .unwrap();
        assert!(matches!(all, Value::Array(v) if v.is_empty()));
    }

    #[test]
    fn collision_groups() {
        let ctx = ctx();

        super::dispatch_collision(
            &ctx,
            "CreateGroup",
            &mk(
                "CollisionService",
                "CreateGroup",
                vec![Value::from("Walls")],
            ),
        )
        .unwrap();
        super::dispatch_collision(
            &ctx,
            "CreateGroup",
            &mk(
                "CollisionService",
                "CreateGroup",
                vec![Value::from("Players")],
            ),
        )
        .unwrap();

        super::dispatch_collision(
            &ctx,
            "SetCollision",
            &mk(
                "CollisionService",
                "SetCollision",
                vec![
                    Value::from("Walls"),
                    Value::from("Players"),
                    Value::Bool(false),
                ],
            ),
        )
        .unwrap();

        let groups = super::dispatch_collision(
            &ctx,
            "GetGroups",
            &mk("CollisionService", "GetGroups", vec![]),
        )
        .unwrap();
        assert!(matches!(groups, Value::Array(v) if v.len() == 2));
    }

    #[test]
    fn audio_lifecycle() {
        let ctx = ctx();

        super::dispatch_audio(
            &ctx,
            "New",
            &mk(
                "AudioService",
                "New",
                vec![Value::from("bg"), Value::from("rbxasset://12345")],
            ),
        )
        .unwrap();
        super::dispatch_audio(
            &ctx,
            "SetVolume",
            &mk(
                "AudioService",
                "SetVolume",
                vec![Value::from("bg"), Value::Number(0.5)],
            ),
        )
        .unwrap();
        super::dispatch_audio(
            &ctx,
            "Loop",
            &mk(
                "AudioService",
                "Loop",
                vec![Value::from("bg"), Value::Bool(true)],
            ),
        )
        .unwrap();

        let audio = ctx.runtime.audio.read();
        let h = audio.get("bg").unwrap();
        assert_eq!(h.volume, 0.5);
        assert!(h.looping);
    }

    #[test]
    fn tween_lifecycle() {
        let ctx = ctx();

        // Create a world instance first
        {
            use nb_types::InstanceType;
            use nb_world::InstanceProperties;
            ctx.runtime
                .world
                .write()
                .create(
                    "Block",
                    InstanceType::Part,
                    nb_types::Container::Workspace,
                    InstanceProperties::default(),
                )
                .unwrap();
        }

        super::dispatch_tween(
            &ctx,
            "Move",
            &mk(
                "TweenService",
                "Move",
                vec![
                    Value::from("Block"),
                    Value::map([
                        ("X", Value::Number(10.0)),
                        ("Y", Value::Number(0.0)),
                        ("Z", Value::Number(0.0)),
                    ]),
                    Value::Number(2.0),
                    Value::from("Out"),
                    Value::from("Linear"),
                    Value::Bool(false),
                    Value::Nil,
                ],
            ),
        )
        .unwrap();

        let tweens = ctx.runtime.tweens.read();
        assert!(tweens.contains_key("Block"));
        let tween = tweens.get("Block").unwrap();
        assert_eq!(tween.end.x, 10.0);
    }
}
