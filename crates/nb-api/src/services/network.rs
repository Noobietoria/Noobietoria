/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Mạng & Script: `NetworkService` and `NetworkEventService`.
//!
//! # The side rules are the whole point
//!
//! The documentation is unusually specific about who may call what:
//!
//! * `FireServer` / `OnClientEvent` — client only.
//! * `FireClient` / `FireAllClients` / `FireAllClientsExcept` / `OnServerEvent`
//!   / `OnServerInvoke` — server only.
//! * `Create` / `Destroy` / `Disconnect` — both sides.
//!
//! Calling from the wrong side "sẽ báo lỗi", so this module returns
//! [`ApiError::Rule`] rather than quietly doing nothing. That matters for more
//! than pedantry: a `ClientScript` that could fire `FireAllClients` would be a
//! way to forge server messages, so the check is a security boundary, not a
//! style preference.
//!
//! # Payload rules
//!
//! The documentation caps a payload at 64 KB and asks authors not to call
//! `FireAllClients` more than 20 times a second. Both are enforced here, because
//! a limit that is only written down is a limit that a busy game will discover
//! in production.

use std::collections::BTreeMap;

use nb_types::{InstanceType, Value};

use crate::args::Args;
use crate::error::{ApiError, ApiResult, Side};
use crate::runtime::{ApiContext, InvokeHandler, PendingEvent};
use crate::services::unknown;

/// The documented maximum payload size for one network call, in bytes.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

/// The documented advice for `FireAllClients`, in calls per second.
pub const MAX_BROADCASTS_PER_SECOND: u32 = 20;

/// Routes a network-group call.
pub fn dispatch(ctx: &ApiContext, service: &str, method: &str, args: &Args) -> ApiResult<Value> {
    match (service, method) {
        ("NetworkService", "Create") => create(ctx, args),
        ("NetworkService", "Destroy") => destroy(ctx, args),
        ("NetworkService", "FireClient") => fire_client(ctx, args),
        ("NetworkService", "FireAllClients") => fire_all_clients(ctx, args, None),
        ("NetworkService", "OnServerEvent") => on_server_event(ctx, args),
        ("NetworkService", "FireServer") => fire_server(ctx, args),
        ("NetworkService", "OnClientEvent") => on_client_event(ctx, args),
        ("NetworkService", "Disconnect") => disconnect(ctx, args),

        ("NetworkEventService", "OnServerEvent") => on_server_event(ctx, args),
        ("NetworkEventService", "FireClient") => fire_client(ctx, args),
        ("NetworkEventService", "FireAllClients") => fire_all_clients(ctx, args, None),
        ("NetworkEventService", "FireAllClientsExcept") => fire_all_clients_except(ctx, args),
        ("NetworkEventService", "OnClientEvent") => on_client_event(ctx, args),
        ("NetworkEventService", "FireServer") => fire_server(ctx, args),
        ("NetworkEventService", "InvokeServer") => invoke_server(ctx, args),
        ("NetworkEventService", "OnServerInvoke") => on_server_invoke(ctx, args),
        ("NetworkEventService", "Disconnect") => disconnect(ctx, args),
        ("NetworkEventService", "Exists") => exists(ctx, args),

        _ => Err(unknown(service, method)),
    }
}

/// The payload arguments of a call whose trailing `Index` must be excluded.
///
/// `FireClient(Name, Username, ..., Index)` and `FireAllClients(Name, ..., Index)`
/// both end in an optional `Index`, so the values between the named parameters
/// and the end are the payload.
fn payload_between(args: &Args, after: usize, trailing: usize) -> Vec<Value> {
    let total = args.len();
    let end = total.saturating_sub(trailing);
    if end <= after {
        return Vec::new();
    }
    args.values()[after..end].to_vec()
}

/// Rejects a payload over the documented 64 KB limit.
///
/// The check is on the serialised size, which is what actually crosses the wire.
fn check_payload(args: &Args, payload: &[Value]) -> ApiResult<usize> {
    let size = serde_json::to_string(&Value::array(payload.to_vec()))
        .map(|s| s.len())
        .unwrap_or(0);
    if size > MAX_PAYLOAD_BYTES {
        return Err(args.rule(format!(
            "payload is {size} bytes, over the {MAX_PAYLOAD_BYTES}-byte limit"
        )));
    }
    Ok(size)
}

/// The recipient names for a broadcast, honouring `"*"`.
#[allow(dead_code)]
fn recipients(ctx: &ApiContext, to: &str) -> Vec<String> {
    if to == "*" {
        ctx.runtime.players.read().online_names()
    } else {
        vec![to.to_owned()]
    }
}

// --- NetworkService.Create / NetworkEventService --------------------------

/// `NetworkService.Create(Name, Parent)` — makes a `NetworkEvent` instance.
///
/// The documentation requires the event to live where both sides can see it, so
/// the instance is created through the world, which enforces that rule.
fn create(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let parent = args.string_or(1, "Parent", "ReplicatedStorage")?;

    let container: nb_types::Container = parent.parse().map_err(|e: String| args.rule(e))?;
    let mut world = ctx.runtime.world.write();
    let id = world.create(
        name.clone(),
        InstanceType::NetworkEvent,
        container,
        nb_world::InstanceProperties::default(),
    )?;
    Ok(Value::map([
        ("Name", Value::from(name)),
        ("GUID", Value::from(id.as_str())),
        ("Parent", Value::from(container.as_str())),
    ]))
}

/// `NetworkService.Destroy(Name, Index)` — removes a `NetworkEvent`.
fn destroy(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let index = args.optional_index(1)?;
    let mut world = ctx.runtime.world.write();
    let id = world.destroy(&name, index)?;
    drop(world);
    ctx.runtime.unregister_all(&name);
    Ok(Value::map([
        ("Name", Value::from(name)),
        ("GUID", Value::from(id.as_str())),
    ]))
}

/// `NetworkEventService.Exists(EventName)` — whether the event was created.
fn exists(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "EventName")?;
    let world = ctx.runtime.world.read();
    Ok(Value::Bool(world.network_event_exists(&name)))
}

/// Requires an event to have been created before it is used.
///
/// The documentation says `EventName` must be one "đã được tạo trong Explorer",
/// so a typo should be an error rather than a message into the void.
fn require_event(ctx: &ApiContext, name: &str, args: &Args) -> ApiResult<()> {
    if ctx.runtime.world.read().network_event_exists(name) {
        Ok(())
    } else {
        Err(args.rule(format!(
            "NetworkEvent `{name}` does not exist; create it first"
        )))
    }
}

// --- Server -> Client -----------------------------------------------------

/// `FireClient(Name, Username, ..., Index)` — server only.
fn fire_client(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    require_server(ctx, "NetworkService", "FireClient")?;
    let name = args.string(0, "Name")?;
    let username = args.string(1, "Username")?;
    require_event(ctx, &name, args)?;

    // The documented `Index` is the final argument; everything between the
    // named parameters and the end is the payload.
    let payload = payload_between(args, 2, trailing_index_len(args));
    check_payload(args, &payload)?;

    let delivered = if username == "*" {
        let names = ctx.runtime.players.read().online_names();
        for to in &names {
            ctx.runtime.push_outbound(PendingEvent {
                event: name.clone(),
                to: to.clone(),
                args: payload.clone(),
                invoke: false,
            });
        }
        names.len()
    } else {
        if !ctx.runtime.has_player(&username) {
            return Err(ApiError::NotFound {
                kind: "player",
                name: username,
            });
        }
        ctx.runtime.push_outbound(PendingEvent {
            event: name,
            to: username,
            args: payload,
            invoke: false,
        });
        1
    };
    Ok(Value::from(delivered))
}

/// How many trailing arguments are the documented `Index` rather than payload.
///
/// `Index` is only recognised when a payload is already present, because a
/// single argument after the named parameters is the payload itself.
fn trailing_index_len(args: &Args) -> usize {
    if args.len() > 0
        && args
            .at(args.len() - 1)
            .is_some_and(|v| matches!(v, Value::Number(_)))
    {
        // `Index` is documented as an optional trailing number; it only exists
        // when named parameters plus at least one payload value precede it.
        1
    } else {
        0
    }
}

/// `FireAllClients(Name, ..., Index)` — server only.
///
/// `except` implements the documented `FireAllClientsExcept`.
fn fire_all_clients(ctx: &ApiContext, args: &Args, except: Option<&str>) -> ApiResult<Value> {
    let method = if except.is_some() {
        "FireAllClientsExcept"
    } else {
        "FireAllClients"
    };
    require_server(ctx, "NetworkService", method)?;

    let (name, payload, skip_from) = if let Some(excluded) = except {
        let name = args.string(0, "EventName")?;
        let payload = payload_between(args, 2, 0);
        (name, payload, Some(excluded.to_owned()))
    } else {
        let name = args.string(0, "Name")?;
        let payload = payload_between(args, 1, trailing_index_len(args));
        (name, payload, None)
    };

    require_event(ctx, &name, args)?;
    check_payload(args, &payload)?;

    let recipients: Vec<String> = ctx
        .runtime
        .players
        .read()
        .online_names()
        .into_iter()
        .filter(|p| skip_from.as_deref() != Some(p.as_str()))
        .collect();

    // The documentation advises at most 20 broadcasts per second; refusing
    // beyond that keeps a runaway loop from flooding every client.
    ctx.runtime
        .charge_broadcast(ctx.runtime.ticks(), MAX_BROADCASTS_PER_SECOND)?;

    for to in &recipients {
        ctx.runtime.push_outbound(PendingEvent {
            event: name.clone(),
            to: to.clone(),
            args: payload.clone(),
            invoke: false,
        });
    }
    Ok(Value::from(recipients.len()))
}

/// `NetworkEventService.FireAllClientsExcept(EventName, ExcludeUsername, ...)`.
fn fire_all_clients_except(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let exclude = args.string(1, "ExcludeUsername")?;
    fire_all_clients(ctx, args, Some(exclude.as_str()))
}

// --- Client -> Server -----------------------------------------------------

/// `FireServer(Name, ..., Index)` — client only.
fn fire_server(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    require_client(ctx, "FireServer")?;
    let name = args.string(0, "Name")?;
    require_event(ctx, &name, args)?;
    let payload = payload_between(args, 1, trailing_index_len(args));
    check_payload(args, &payload)?;

    let from = ctx.require_caller()?;
    ctx.runtime.push_inbound(PendingEvent {
        event: name,
        to: from,
        args: payload,
        invoke: false,
    });
    Ok(Value::Bool(true))
}

/// `NetworkEventService.InvokeServer(EventName, ...)` — client only.
///
/// The reply is produced synchronously by the handler the server registered with
/// `OnServerInvoke`, and returned to the caller.
fn invoke_server(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    require_client(ctx, "InvokeServer")?;
    let name = args.string(0, "EventName")?;
    require_event(ctx, &name, args)?;
    let payload = payload_between(args, 1, 0);
    check_payload(args, &payload)?;

    let from = ctx.require_caller()?;
    let reply = ctx.runtime.invoke(&name, &from, &payload)?;

    ctx.runtime.push_inbound(PendingEvent {
        event: name,
        to: from,
        args: payload,
        invoke: true,
    });
    Ok(reply)
}

// --- Handler registration -------------------------------------------------

/// `OnServerEvent(Name, Callback, Index)` — server only.
///
/// Registers that the server is listening, so the script layer can route inbound
/// messages, and `Disconnect` can report whether anything was removed.
fn on_server_event(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    require_server(ctx, "NetworkService", "OnServerEvent")?;
    let name = args.string(0, "Name")?;
    require_event(ctx, &name, args)?;
    ctx.runtime.register_handler(&name, Side::Server);
    Ok(Value::Bool(true))
}

/// `NetworkEventService.OnServerInvoke(EventName, Callback)` — server only.
///
/// Also registers a default handler that echoes the payload, which is what an
/// author gets before they attach real logic. The script layer replaces it.
fn on_server_invoke(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    require_server(ctx, "NetworkService", "OnServerInvoke")?;
    let name = args.string(0, "EventName")?;
    require_event(ctx, &name, args)?;
    ctx.runtime.register_handler(&name, Side::Server);
    if !ctx.runtime.has_invoke_handler(&name) {
        ctx.runtime.register_invoke_handler(
            &name,
            InvokeHandler::new(|_caller, payload| Ok(Value::array(payload.to_vec()))),
        );
    }
    Ok(Value::Bool(true))
}

/// `OnClientEvent(Name, Callback, Index)` — client only.
fn on_client_event(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    require_client(ctx, "OnClientEvent")?;
    let name = args.string(0, "Name")?;
    require_event(ctx, &name, args)?;
    ctx.runtime.register_handler(&name, Side::Client);
    Ok(Value::Bool(true))
}

/// `Disconnect(Name, Index)` — both sides.
fn disconnect(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    let name = args.string(0, "Name")?;
    let had_event = ctx.runtime.unregister_handler(&name);
    let had_invoke = ctx.runtime.unregister_invoke_handler(&name);
    if !had_event && !had_invoke {
        return Err(args.rule(format!("no handler is registered for `{name}`")));
    }
    Ok(Value::Bool(true))
}

// --- Side guards ----------------------------------------------------------

/// Refuses a server-only call made from a client.
fn require_server(ctx: &ApiContext, service: &str, method: &str) -> ApiResult<()> {
    if ctx.side.may_call_server_only() {
        Ok(())
    } else {
        Err(ApiError::WrongSide {
            service: service.to_owned(),
            method: method.to_owned(),
            required: "server",
            actual: ctx.side.as_str(),
        })
    }
}

/// Refuses a client-only call made from the server.
fn require_client(ctx: &ApiContext, method: &str) -> ApiResult<()> {
    if ctx.side.may_call_client_only() {
        Ok(())
    } else {
        Err(ApiError::WrongSide {
            service: "NetworkService".to_owned(),
            method: method.to_owned(),
            required: "client",
            actual: ctx.side.as_str(),
        })
    }
}

/// A convenience the script layer uses to learn which events exist.
pub fn event_inventory(ctx: &ApiContext) -> BTreeMap<String, bool> {
    let world = ctx.runtime.world.read();
    ctx.runtime
        .handler_names()
        .into_iter()
        .map(|name| {
            let exists = world.network_event_exists(&name);
            (name, exists)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::PlayerRecord;

    fn server() -> ApiContext {
        ApiContext::new(Side::Server)
    }

    fn client(caller: &str) -> ApiContext {
        let ctx = ApiContext::new(Side::Client);
        ctx.runtime.player_join(PlayerRecord::new("Alice", 1, "VN"));
        ctx.runtime.player_join(PlayerRecord::new("Bob", 2, "US"));
        ctx.as_caller(caller)
    }

    fn make_event(ctx: &ApiContext, name: &str) {
        dispatch(
            ctx,
            "NetworkService",
            "Create",
            &Args::new(
                "NetworkService",
                "Create",
                vec![Value::from(name), Value::from("ReplicatedStorage")],
            ),
        )
        .expect("event created");
    }

    #[test]
    fn create_makes_a_network_event_instance() {
        let ctx = server();
        let result = dispatch(
            &ctx,
            "NetworkService",
            "Create",
            &Args::new(
                "NetworkService",
                "Create",
                vec![Value::from("BuyItem"), Value::from("ReplicatedStorage")],
            ),
        )
        .expect("created");
        assert_eq!(result.get("Name").and_then(Value::as_str), Some("BuyItem"));
        assert!(ctx.runtime.world.read().network_event_exists("BuyItem"));
        assert!(result.get("GUID").and_then(Value::as_str).is_some());
    }

    #[test]
    fn duplicate_event_names_are_refused() {
        let ctx = server();
        make_event(&ctx, "Buy");
        let err = dispatch(
            &ctx,
            "NetworkService",
            "Create",
            &Args::new(
                "NetworkService",
                "Create",
                vec![Value::from("Buy"), Value::from("ReplicatedStorage")],
            ),
        )
        .unwrap_err();
        assert!(matches!(err, ApiError::World(_)));
    }

    #[test]
    fn using_an_undefined_event_is_an_error() {
        let ctx = server();
        let err = dispatch(
            &ctx,
            "NetworkService",
            "FireAllClients",
            &Args::new(
                "NetworkService",
                "FireAllClients",
                vec![Value::from("Ghost")],
            ),
        )
        .unwrap_err();
        assert!(matches!(err, ApiError::Rule(ref m) if m.contains("does not exist")));
    }

    #[test]
    fn exists_reports_event_presence() {
        let ctx = server();
        make_event(&ctx, "Buy");
        let yes = dispatch(
            &ctx,
            "NetworkEventService",
            "Exists",
            &Args::new("NetworkEventService", "Exists", vec![Value::from("Buy")]),
        )
        .unwrap();
        assert_eq!(yes.as_bool(), Some(true));

        let no = dispatch(
            &ctx,
            "NetworkEventService",
            "Exists",
            &Args::new("NetworkEventService", "Exists", vec![Value::from("Nope")]),
        )
        .unwrap();
        assert_eq!(no.as_bool(), Some(false));
    }

    #[test]
    fn server_can_fire_at_one_client() {
        let ctx = server();
        ctx.runtime.player_join(PlayerRecord::new("Alice", 1, "VN"));
        make_event(&ctx, "Buy");
        let result = dispatch(
            &ctx,
            "NetworkService",
            "FireClient",
            &Args::new(
                "NetworkService",
                "FireClient",
                vec![
                    Value::from("Buy"),
                    Value::from("Alice"),
                    Value::from("ok"),
                    Value::from(100i64),
                    Value::from(1i64),
                ],
            ),
        )
        .expect("fired");
        assert_eq!(result.as_f64(), Some(1.0));

        let out = ctx.runtime.take_outbound();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].event, "Buy");
        assert_eq!(out[0].to, "Alice");
        assert_eq!(out[0].args.len(), 2, "both payload arguments survive");
        assert_eq!(out[0].args[0].as_str(), Some("ok"));
    }

    #[test]
    fn firing_at_an_unknown_player_is_an_error() {
        let ctx = server();
        make_event(&ctx, "Buy");
        let err = dispatch(
            &ctx,
            "NetworkService",
            "FireClient",
            &Args::new(
                "NetworkService",
                "FireClient",
                vec![Value::from("Buy"), Value::from("Ghost"), Value::from(1i64)],
            ),
        )
        .unwrap_err();
        assert!(matches!(err, ApiError::NotFound { kind: "player", .. }));
    }

    #[test]
    fn wildcard_reaches_every_online_client() {
        let ctx = server();
        ctx.runtime.player_join(PlayerRecord::new("Alice", 1, "VN"));
        ctx.runtime.player_join(PlayerRecord::new("Bob", 2, "US"));
        ctx.runtime.player_join(PlayerRecord::new("Cara", 3, "US"));
        ctx.runtime.player_leave("Cara");
        make_event(&ctx, "Announce");

        let result = dispatch(
            &ctx,
            "NetworkService",
            "FireClient",
            &Args::new(
                "NetworkService",
                "FireClient",
                vec![
                    Value::from("Announce"),
                    Value::from("*"),
                    Value::from("hello"),
                ],
            ),
        )
        .unwrap();
        assert_eq!(result.as_f64(), Some(2.0), "only online players");

        let out = ctx.runtime.take_outbound();
        let mut targets: Vec<&str> = out.iter().map(|e| e.to.as_str()).collect();
        targets.sort();
        assert_eq!(targets, vec!["Alice", "Bob"]);
    }

    #[test]
    fn fire_all_clients_except_skips_one_player() {
        let ctx = server();
        ctx.runtime.player_join(PlayerRecord::new("Alice", 1, "VN"));
        ctx.runtime.player_join(PlayerRecord::new("Bob", 2, "US"));
        make_event(&ctx, "Tick");

        let result = dispatch(
            &ctx,
            "NetworkEventService",
            "FireAllClientsExcept",
            &Args::new(
                "NetworkEventService",
                "FireAllClientsExcept",
                vec![Value::from("Tick"), Value::from("Alice"), Value::from(1i64)],
            ),
        )
        .unwrap();
        assert_eq!(result.as_f64(), Some(1.0));
        let out = ctx.runtime.take_outbound();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].to, "Bob");
    }

    #[test]
    fn client_cannot_impersonate_the_server() {
        let ctx = client("Alice");
        make_event(&ctx, "Buy");
        for method in ["FireClient", "FireAllClients"] {
            let args = match method {
                "FireClient" => vec![
                    Value::from("Buy"),
                    Value::from("Bob"),
                    Value::from("forged"),
                ],
                _ => vec![Value::from("Buy"), Value::from("forged")],
            };
            let err = dispatch(
                &ctx,
                "NetworkService",
                method,
                &Args::new("NetworkService", method, args),
            )
            .unwrap_err();
            assert!(
                matches!(
                    err,
                    ApiError::WrongSide {
                        required: "server",
                        ..
                    }
                ),
                "{method} must be server-only, got {err:?}"
            );
        }
        assert!(ctx.runtime.take_outbound().is_empty(), "nothing was sent");
    }

    #[test]
    fn server_cannot_fire_server() {
        let ctx = server();
        make_event(&ctx, "Buy");
        let err = dispatch(
            &ctx,
            "NetworkService",
            "FireServer",
            &Args::new(
                "NetworkService",
                "FireServer",
                vec![Value::from("Buy"), Value::from(1i64)],
            ),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ApiError::WrongSide {
                required: "client",
                ..
            }
        ));
    }

    #[test]
    fn client_fires_server_and_the_payload_arrives_attributed() {
        let ctx = client("Alice");
        make_event(&ctx, "Buy");
        dispatch(
            &ctx,
            "NetworkService",
            "FireServer",
            &Args::new(
                "NetworkService",
                "FireServer",
                // The documented signature is (Name, ..., Index), so the trailing
                // number is the Index and `Sword_01` is the payload.
                vec![
                    Value::from("Buy"),
                    Value::from("Sword_01"),
                    Value::from(2i64),
                ],
            ),
        )
        .expect("fired");

        let inbound = ctx.runtime.take_inbound();
        assert_eq!(inbound.len(), 1);
        assert_eq!(inbound[0].to, "Alice", "the server is told who sent it");
        assert_eq!(inbound[0].event, "Buy");
        assert_eq!(inbound[0].args.len(), 1, "Index is not part of the payload");
        assert_eq!(inbound[0].args[0].as_str(), Some("Sword_01"));
    }

    #[test]
    fn fire_server_passes_a_multi_value_payload_through() {
        let ctx = client("Alice");
        make_event(&ctx, "Buy");
        // Two payload values and no trailing Index: both are delivered.
        dispatch(
            &ctx,
            "NetworkService",
            "FireServer",
            &Args::new(
                "NetworkService",
                "FireServer",
                vec![
                    Value::from("Buy"),
                    Value::from("Sword_01"),
                    Value::from("x2"),
                ],
            ),
        )
        .expect("fired");

        let inbound = ctx.runtime.take_inbound();
        assert_eq!(inbound[0].args.len(), 2);
        assert_eq!(inbound[0].args[0].as_str(), Some("Sword_01"));
        assert_eq!(inbound[0].args[1].as_str(), Some("x2"));
    }

    #[test]
    fn a_fire_from_nowhere_is_refused() {
        // A context with no caller cannot have come from a client script.
        let ctx = ApiContext::new(Side::Client);
        make_event(&ctx, "Buy");
        let err = dispatch(
            &ctx,
            "NetworkService",
            "FireServer",
            &Args::new(
                "NetworkService",
                "FireServer",
                vec![Value::from("Buy"), Value::from(1i64)],
            ),
        )
        .unwrap_err();
        assert!(matches!(err, ApiError::Rule(_)));
    }

    #[test]
    fn handlers_register_on_the_right_side() {
        let server_ctx = server();
        make_event(&server_ctx, "Buy");
        dispatch(
            &server_ctx,
            "NetworkService",
            "OnServerEvent",
            &Args::new(
                "NetworkService",
                "OnServerEvent",
                vec![Value::from("Buy"), Value::Nil],
            ),
        )
        .expect("registered");
        assert!(server_ctx.runtime.has_handler("Buy"));

        // A client may not register a server handler.
        let client_ctx = client("Alice");
        make_event(&client_ctx, "Buy");
        let err = dispatch(
            &client_ctx,
            "NetworkService",
            "OnServerEvent",
            &Args::new(
                "NetworkService",
                "OnServerEvent",
                vec![Value::from("Buy"), Value::Nil],
            ),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ApiError::WrongSide {
                required: "server",
                ..
            }
        ));

        // But it may register a client handler.
        dispatch(
            &client_ctx,
            "NetworkService",
            "OnClientEvent",
            &Args::new(
                "NetworkService",
                "OnClientEvent",
                vec![Value::from("Buy"), Value::Nil],
            ),
        )
        .expect("registered client handler");
    }

    #[test]
    fn disconnect_reports_whether_anything_was_registered() {
        let ctx = server();
        make_event(&ctx, "Buy");
        dispatch(
            &ctx,
            "NetworkService",
            "OnServerEvent",
            &Args::new(
                "NetworkService",
                "OnServerEvent",
                vec![Value::from("Buy"), Value::Nil],
            ),
        )
        .unwrap();

        let removed = dispatch(
            &ctx,
            "NetworkService",
            "Disconnect",
            &Args::new("NetworkService", "Disconnect", vec![Value::from("Buy")]),
        )
        .unwrap();
        assert_eq!(removed.as_bool(), Some(true));
        assert!(!ctx.runtime.has_handler("Buy"));

        // Disconnecting twice is an error rather than a silent success.
        let err = dispatch(
            &ctx,
            "NetworkService",
            "Disconnect",
            &Args::new("NetworkService", "Disconnect", vec![Value::from("Buy")]),
        )
        .unwrap_err();
        assert!(matches!(err, ApiError::Rule(_)));
    }

    #[test]
    fn oversized_payloads_are_refused() {
        let ctx = server();
        make_event(&ctx, "Big");
        let huge = Value::from("x".repeat(MAX_PAYLOAD_BYTES + 1));
        let err = dispatch(
            &ctx,
            "NetworkService",
            "FireAllClients",
            &Args::new(
                "NetworkService",
                "FireAllClients",
                vec![Value::from("Big"), huge],
            ),
        )
        .unwrap_err();
        assert!(
            matches!(err, ApiError::Rule(ref m) if m.contains("limit")),
            "got {err:?}"
        );
        assert!(ctx.runtime.take_outbound().is_empty());
    }

    #[test]
    fn payload_just_under_the_limit_is_allowed() {
        let ctx = server();
        ctx.runtime.player_join(PlayerRecord::new("Alice", 1, "VN"));
        make_event(&ctx, "Big");
        let ok = Value::from("x".repeat(1000));
        assert!(dispatch(
            &ctx,
            "NetworkService",
            "FireAllClients",
            &Args::new(
                "NetworkService",
                "FireAllClients",
                vec![Value::from("Big"), ok]
            ),
        )
        .is_ok());
    }

    #[test]
    fn destroy_frees_the_event_name_and_its_handler() {
        let ctx = server();
        make_event(&ctx, "Buy");
        dispatch(
            &ctx,
            "NetworkService",
            "OnServerEvent",
            &Args::new(
                "NetworkService",
                "OnServerEvent",
                vec![Value::from("Buy"), Value::Nil],
            ),
        )
        .unwrap();

        dispatch(
            &ctx,
            "NetworkService",
            "Destroy",
            &Args::new("NetworkService", "Destroy", vec![Value::from("Buy")]),
        )
        .expect("destroyed");
        assert!(!ctx.runtime.world.read().network_event_exists("Buy"));
        assert!(!ctx.runtime.has_handler("Buy"));
        // The name can be reused now.
        make_event(&ctx, "Buy");
    }

    #[test]
    fn event_inventory_reports_created_and_bound_events() {
        let ctx = server();
        make_event(&ctx, "Buy");
        dispatch(
            &ctx,
            "NetworkService",
            "OnServerEvent",
            &Args::new(
                "NetworkService",
                "OnServerEvent",
                vec![Value::from("Buy"), Value::Nil],
            ),
        )
        .unwrap();
        let inventory = event_inventory(&ctx);
        assert_eq!(inventory.get("Buy"), Some(&true));
    }

    #[test]
    fn invoke_server_without_a_handler_reports_that_clearly() {
        let ctx = client("Alice");
        make_event(&ctx, "Query");
        let err = dispatch(
            &ctx,
            "NetworkEventService",
            "InvokeServer",
            &Args::new(
                "NetworkEventService",
                "InvokeServer",
                vec![Value::from("Query")],
            ),
        )
        .unwrap_err();
        assert!(
            matches!(err, ApiError::Rule(ref m) if m.contains("OnServerInvoke")),
            "got {err:?}"
        );
    }
}
