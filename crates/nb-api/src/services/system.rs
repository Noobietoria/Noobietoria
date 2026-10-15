//! Hệ thống: DataStoreService, DataStore, HttpService, AnalyticsService, LocalizationService, TeleportService

use nb_store::SessionId;
use nb_types::{Value, Vector3};

use crate::args::Args;
use crate::error::{ApiError, ApiResult};
use crate::runtime::{AnalyticsEvent, ApiContext, HttpRequestSpec, Zone};
use crate::services::unknown;

/// Routes a call in the system group.
pub fn dispatch(ctx: &ApiContext, service: &str, method: &str, args: &Args) -> ApiResult<Value> {
    match service {
        "DataStoreService" => dispatch_datastore_service(ctx, method, args),
        "DataStore" => dispatch_datastore(ctx, method, args),
        "HttpService" => dispatch_http_service(ctx, method, args),
        "AnalyticsService" => dispatch_analytics_service(ctx, method, args),
        "LocalizationService" => dispatch_localization_service(ctx, method, args),
        "TeleportService" => dispatch_teleport_service(ctx, method, args),
        _ => Err(unknown(service, method)),
    }
}

// ============================================================================
// DataStoreService
// ============================================================================

fn parse_session(raw: &str) -> ApiResult<SessionId> {
    SessionId::new(raw).map_err(|e| ApiError::Rule(e.to_string()))
}

fn dispatch_datastore_service(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "Close" => {
            let session_raw = args.string(0, "SessionID")?;
            let session = parse_session(&session_raw)?;
            ctx.runtime
                .datastore
                .close(&session)
                .map_err(ApiError::DataStore)?;
            Ok(Value::Nil)
        }
        "Delete" => {
            let session_raw = args.string(0, "SessionID")?;
            let key = args.string(1, "Key")?;
            let session = parse_session(&session_raw)?;
            let removed = ctx
                .runtime
                .datastore
                .delete(&session, &key)
                .map_err(ApiError::DataStore)?;
            Ok(removed.unwrap_or(Value::Nil))
        }
        "Get" => {
            let session_raw = args.string(0, "SessionID")?;
            let key = args.string(1, "Key")?;
            let session = parse_session(&session_raw)?;
            let val = ctx
                .runtime
                .datastore
                .get(&session, &key)
                .map_err(ApiError::DataStore)?;
            Ok(val.unwrap_or(Value::Nil))
        }
        "GetDataStore" => {
            let name = args.string(0, "name")?;
            let scope = args.string_or(1, "scope", "")?;
            ctx.runtime
                .datastore
                .get_data_store(&name, &scope)
                .map_err(ApiError::DataStore)?;
            Ok(Value::map([
                ("__type", Value::from("DataStore")),
                ("Name", Value::from(name)),
                ("Scope", Value::from(scope)),
            ]))
        }
        "GetKeys" => {
            let session_raw = args.string(0, "SessionID")?;
            let session = parse_session(&session_raw)?;
            let keys = ctx
                .runtime
                .datastore
                .get_keys(&session)
                .map_err(ApiError::DataStore)?;
            Ok(Value::array(keys.into_iter().map(Value::Str)))
        }
        "GetOrderedDataStore" => {
            let name = args.string(0, "name")?;
            let scope = args.string_or(1, "scope", "")?;
            ctx.runtime
                .datastore
                .get_ordered_data_store(&name, &scope)
                .map_err(ApiError::DataStore)?;
            Ok(Value::map([
                ("__type", Value::from("OrderedDataStore")),
                ("Name", Value::from(name)),
                ("Scope", Value::from(scope)),
            ]))
        }
        "GetRequestBudgetForRequestType" => {
            let request_type = args.string(0, "requestType")?;
            let budget = ctx
                .runtime
                .datastore
                .get_request_budget_for_request_type(&request_type);
            Ok(Value::Number(budget as f64))
        }
        "Init" => {
            let session_raw = args.string(0, "SessionID")?;
            let session = parse_session(&session_raw)?;
            ctx.runtime
                .datastore
                .init(&session)
                .map_err(ApiError::DataStore)?;
            Ok(Value::Nil)
        }
        "Insert" => {
            let session_raw = args.string(0, "SessionID")?;
            let key = args.string(1, "Key")?;
            let val = args.at(2).cloned().unwrap_or(Value::Nil);
            let session = parse_session(&session_raw)?;
            ctx.runtime
                .datastore
                .insert(&session, &key, &val)
                .map_err(ApiError::DataStore)?;
            Ok(Value::Nil)
        }
        _ => Err(unknown("DataStoreService", method)),
    }
}

// ============================================================================
// DataStore (Roblox-style handle, routed through the global datastore)
// ============================================================================

fn dispatch_datastore(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    let handle = ctx
        .runtime
        .datastore
        .get_data_store("GlobalDataStore", "")
        .map_err(ApiError::DataStore)?;

    match method {
        "GetAsync" => {
            let key = args.string(0, "key")?;
            let val = handle.get_async(&key).map_err(ApiError::DataStore)?;
            Ok(val.unwrap_or(Value::Nil))
        }
        "RemoveAsync" => {
            let key = args.string(0, "key")?;
            let val = handle.remove_async(&key).map_err(ApiError::DataStore)?;
            Ok(val.unwrap_or(Value::Nil))
        }
        "SetAsync" => {
            let key = args.string(0, "key")?;
            let val = args.at(1).cloned().unwrap_or(Value::Nil);
            handle.set_async(&key, &val).map_err(ApiError::DataStore)?;
            Ok(Value::Nil)
        }
        "UpdateAsync" => {
            let key = args.string(0, "key")?;
            // argument 1 is a transform function — we accept an already-evaluated
            // replacement value (the script layer calls the Luau closure before dispatch)
            let replacement = args.at(1).cloned();
            let updated = handle
                .update_async(&key, move |current| replacement.or(current))
                .map_err(ApiError::DataStore)?;
            Ok(updated.unwrap_or(Value::Nil))
        }
        _ => Err(unknown("DataStore", method)),
    }
}

// ============================================================================
// HttpService
// ============================================================================

fn dispatch_http_service(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "Prepare" => {
            let id = args.number(0, "Id")? as i64;
            let req_method = args.string(1, "Method")?;
            let url = args.string(2, "Url")?;
            let headers = args.at(3).cloned().unwrap_or_else(Value::empty_map);
            let body = args.string_or(4, "Body", "")?;
            let content_type = args.string_or(5, "ContentType", "application/json")?;
            ctx.runtime.http_requests.write().insert(
                id,
                HttpRequestSpec {
                    id,
                    method: req_method,
                    url,
                    headers,
                    body,
                    content_type,
                    released: false,
                },
            );
            Ok(Value::Number(id as f64))
        }
        "Release" => {
            let id = args.number(0, "Id")? as i64;
            let mut reqs = ctx.runtime.http_requests.write();
            if let Some(req) = reqs.get_mut(&id) {
                req.released = true;
                Ok(Value::Bool(true))
            } else {
                Ok(Value::Bool(false))
            }
        }
        "Request" => {
            let id = args.number(0, "Id")? as i64;
            let req_opt = ctx.runtime.http_requests.read().get(&id).cloned();
            match req_opt {
                Some(req) => Ok(Value::map([
                    ("Success", Value::Bool(true)),
                    ("StatusCode", Value::Number(200.0)),
                    ("StatusMessage", Value::from("OK")),
                    ("Body", Value::from(req.body)),
                    ("Headers", req.headers),
                ])),
                None => Err(ApiError::Rule(format!("no prepared request with id {id}"))),
            }
        }
        _ => Err(unknown("HttpService", method)),
    }
}

// ============================================================================
// AnalyticsService
// ============================================================================

fn dispatch_analytics_service(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "ClearEvents" => {
            let event_name = args.string(0, "EventName")?;
            let mut events = ctx.runtime.analytics_events.lock();
            let before = events.len();
            events.retain(|e| e.name != event_name);
            let cleared = before - events.len();
            Ok(Value::Number(cleared as f64))
        }
        "GetCounter" => {
            let metric = args.string(0, "MetricName")?;
            let val = ctx.runtime.analytics.get(&metric);
            match val {
                Some(Value::Number(n)) => Ok(Value::Number(n)),
                _ => Ok(Value::Number(0.0)),
            }
        }
        "GetEvents" => {
            let event_name = args.string(0, "EventName")?;
            let limit = args.number_or(1, "Limit", 50.0)? as usize;
            let events = ctx.runtime.analytics_events.lock();
            let matching: Vec<Value> = events
                .iter()
                .filter(|e| e.name == event_name)
                .take(limit)
                .map(|e| {
                    let mut pairs: Vec<(&str, Value)> = vec![
                        ("Name", Value::from(e.name.clone())),
                        ("Properties", e.properties.clone()),
                        ("IsError", Value::Bool(e.is_error)),
                    ];
                    if let Some(ref u) = e.username {
                        pairs.push(("Username", Value::from(u.clone())));
                    }
                    Value::map(pairs)
                })
                .collect();
            Ok(Value::array(matching))
        }
        "IncrementCounter" => {
            let metric = args.string(0, "MetricName")?;
            let delta = args.number_or(1, "Delta", 1.0)?;
            let current = match ctx.runtime.analytics.get(&metric) {
                Some(Value::Number(n)) => n,
                _ => 0.0,
            };
            let new_val = current + delta;
            ctx.runtime
                .analytics
                .set(&metric, Value::Number(new_val))
                .map_err(ApiError::Store)?;
            Ok(Value::Number(new_val))
        }
        "SetCounter" => {
            let metric = args.string(0, "MetricName")?;
            let val = args.number(1, "Value")?;
            ctx.runtime
                .analytics
                .set(&metric, Value::Number(val))
                .map_err(ApiError::Store)?;
            Ok(Value::Number(val))
        }
        "TrackError" => {
            let error_msg = args.string(0, "ErrorMessage")?;
            let username = args.string_or(1, "Username", "")?;
            let properties = args.at(2).cloned().unwrap_or_else(Value::empty_map);
            ctx.runtime.analytics_events.lock().push(AnalyticsEvent {
                name: error_msg,
                username: if username.is_empty() {
                    None
                } else {
                    Some(username)
                },
                properties,
                is_error: true,
            });
            Ok(Value::Bool(true))
        }
        "TrackEvent" => {
            let event_name = args.string(0, "EventName")?;
            let username = args.string_or(1, "Username", "")?;
            let properties = args.at(2).cloned().unwrap_or_else(Value::empty_map);
            ctx.runtime.analytics_events.lock().push(AnalyticsEvent {
                name: event_name,
                username: if username.is_empty() {
                    None
                } else {
                    Some(username)
                },
                properties,
                is_error: false,
            });
            Ok(Value::Bool(true))
        }
        _ => Err(unknown("AnalyticsService", method)),
    }
}

// ============================================================================
// LocalizationService
// ============================================================================

fn dispatch_localization_service(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "GetLocale" => {
            let username = args.string(0, "Username")?;
            let locale = ctx
                .runtime
                .player_locales
                .read()
                .get(&username)
                .cloned()
                .unwrap_or_else(|| "en-US".to_owned());
            Ok(Value::from(locale))
        }
        "GetSupportedLocales" => {
            let locales = [
                "en-US", "vi-VN", "es-ES", "fr-FR", "de-DE", "ja-JP", "ko-KR", "zh-CN", "pt-BR",
            ];
            Ok(Value::array(locales.iter().map(|s| Value::from(*s))))
        }
        "ImportTable" => {
            // Accepts { Locale -> { Key -> text } } or an array of { Locale, Key, Text }
            let translations = args.table_or_empty(0, "Translations")?;
            if let Value::Map(outer) = translations {
                for (locale, sub) in outer {
                    if let Value::Map(inner) = sub {
                        for (k, v) in inner {
                            let store_key = format!("{}:{}", locale, k);
                            let _ = ctx.runtime.locale_tables.set(store_key, v);
                        }
                    }
                }
            }
            Ok(Value::Bool(true))
        }
        "Register" => {
            let key = args.string(0, "Key")?;
            let translations = args.table_or_empty(1, "Translations")?;
            if let Value::Map(map) = translations {
                for (locale, text) in map {
                    let store_key = format!("{}:{}", locale, key);
                    let _ = ctx.runtime.locale_tables.set(store_key, text);
                }
            }
            Ok(Value::Bool(true))
        }
        "SetLocale" => {
            let username = args.string(0, "Username")?;
            let locale = args.string(1, "Locale")?;
            ctx.runtime.player_locales.write().insert(username, locale);
            Ok(Value::Bool(true))
        }
        "Translate" => {
            let key = args.string(0, "Key")?;
            let locale = args.string(1, "Locale")?;
            let store_key = format!("{}:{}", locale, key);
            match ctx.runtime.locale_tables.get(&store_key) {
                Some(Value::Str(s)) => Ok(Value::from(s)),
                _ => Ok(Value::from(key)),
            }
        }
        "TranslateFor" => {
            let key = args.string(0, "Key")?;
            let username = args.string(1, "Username")?;
            let locale = ctx
                .runtime
                .player_locales
                .read()
                .get(&username)
                .cloned()
                .unwrap_or_else(|| "en-US".to_owned());
            let store_key = format!("{}:{}", locale, key);
            match ctx.runtime.locale_tables.get(&store_key) {
                Some(Value::Str(s)) => Ok(Value::from(s)),
                _ => Ok(Value::from(key)),
            }
        }
        "Unregister" => {
            let key = args.string(0, "Key")?;
            let suffix = format!(":{}", key);
            let all_keys = ctx.runtime.locale_tables.keys();
            for k in all_keys {
                if k.ends_with(&suffix) {
                    ctx.runtime.locale_tables.remove(&k);
                }
            }
            Ok(Value::Bool(true))
        }
        _ => Err(unknown("LocalizationService", method)),
    }
}

// ============================================================================
// TeleportService
// ============================================================================

fn vec3_from_value(v: Option<&Value>) -> Vector3 {
    match v {
        Some(Value::Map(m)) => {
            let x = m
                .get("X")
                .and_then(|v| {
                    if let Value::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0);
            let y = m
                .get("Y")
                .and_then(|v| {
                    if let Value::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0);
            let z = m
                .get("Z")
                .and_then(|v| {
                    if let Value::Number(n) = v {
                        Some(*n)
                    } else {
                        None
                    }
                })
                .unwrap_or(0.0);
            Vector3::new(x, y, z)
        }
        Some(Value::Array(arr)) => {
            let x = if let Some(Value::Number(n)) = arr.get(0) {
                *n
            } else {
                0.0
            };
            let y = if let Some(Value::Number(n)) = arr.get(1) {
                *n
            } else {
                0.0
            };
            let z = if let Some(Value::Number(n)) = arr.get(2) {
                *n
            } else {
                0.0
            };
            Vector3::new(x, y, z)
        }
        _ => Vector3::ZERO,
    }
}

fn zone_to_value(zone: &Zone) -> Value {
    Value::map([
        ("Name", Value::from(zone.name.clone())),
        ("ServerId", Value::from(zone.server_id.clone())),
        ("SpawnPosition", zone.spawn.to_value()),
    ])
}

fn dispatch_teleport_service(ctx: &ApiContext, method: &str, args: &Args) -> ApiResult<Value> {
    match method {
        "GetZone" => {
            let name = args.string(0, "ZoneName")?;
            let zones = ctx.runtime.zones.read();
            if let Some(zone) = zones.get(&name) {
                Ok(zone_to_value(zone))
            } else {
                Err(ApiError::NotFound { kind: "zone", name })
            }
        }
        "GetZones" => {
            let zones = ctx.runtime.zones.read();
            Ok(Value::array(zones.values().map(zone_to_value)))
        }
        "OnTeleport" => {
            let username = args.string(0, "Username")?;
            let cb = args.at(1).cloned().unwrap_or(Value::Nil);
            let key = format!("TeleportService.OnTeleport:{username}");
            ctx.runtime
                .callbacks
                .set(key, cb)
                .map_err(ApiError::Store)?;
            Ok(Value::Bool(true))
        }
        "RegisterZone" => {
            let name = args.string(0, "ZoneName")?;
            let server_id = args.string(1, "ServerId")?;
            let spawn = vec3_from_value(args.at(2));
            ctx.runtime.zones.write().insert(
                name.clone(),
                Zone {
                    name,
                    server_id,
                    spawn,
                },
            );
            Ok(Value::Bool(true))
        }
        "TeleportGroup" => {
            let usernames: Vec<String> = match args.at(0) {
                Some(Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| match v {
                        Value::Str(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect(),
                _ => vec![],
            };
            let zone_name = args.string(1, "ZoneName")?;
            let pos_override = match args.at(2) {
                Some(v @ (Value::Map(_) | Value::Array(_))) => Some(vec3_from_value(Some(v))),
                _ => None,
            };
            let zone_spawn = ctx.runtime.zones.read().get(&zone_name).map(|z| z.spawn);
            for u in &usernames {
                if let Some(p) = ctx.runtime.players.write().get_mut(u) {
                    p.position = pos_override.or(zone_spawn).unwrap_or(Vector3::ZERO);
                }
            }
            Ok(Value::Bool(true))
        }
        "TeleportToServer" => {
            let username = args.string(0, "Username")?;
            let _server_id = args.string(1, "ServerId")?;
            let pos = vec3_from_value(args.at(2));
            if let Some(p) = ctx.runtime.players.write().get_mut(&username) {
                p.position = pos;
            }
            Ok(Value::Bool(true))
        }
        "TeleportToZone" => {
            let username = args.string(0, "Username")?;
            let zone_name = args.string(1, "ZoneName")?;
            let pos_arg = args.at(2);
            let dest = if pos_arg.map(|v| !v.is_nil()).unwrap_or(false) {
                vec3_from_value(pos_arg)
            } else {
                ctx.runtime
                    .zones
                    .read()
                    .get(&zone_name)
                    .map(|z| z.spawn)
                    .unwrap_or(Vector3::ZERO)
            };
            if let Some(p) = ctx.runtime.players.write().get_mut(&username) {
                p.position = dest;
            }
            Ok(Value::Bool(true))
        }
        "UnregisterZone" => {
            let name = args.string(0, "ZoneName")?;
            let removed = ctx.runtime.zones.write().remove(&name).is_some();
            Ok(Value::Bool(removed))
        }
        _ => Err(unknown("TeleportService", method)),
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
    use crate::runtime::{ApiContext, PlayerRecord};

    fn mk(service: &str, method: &str, values: Vec<Value>) -> Args {
        Args::new(service, method, values)
    }

    fn ctx() -> ApiContext {
        ApiContext::new(Side::Server)
    }

    #[test]
    fn analytics_counters_and_events() {
        let ctx = ctx();

        // IncrementCounter
        let result = super::dispatch_analytics_service(
            &ctx,
            "IncrementCounter",
            &mk(
                "AnalyticsService",
                "IncrementCounter",
                vec![Value::from("hits"), Value::Number(3.0)],
            ),
        )
        .unwrap();
        assert_eq!(result, Value::Number(3.0));

        // GetCounter
        let result = super::dispatch_analytics_service(
            &ctx,
            "GetCounter",
            &mk("AnalyticsService", "GetCounter", vec![Value::from("hits")]),
        )
        .unwrap();
        assert_eq!(result, Value::Number(3.0));

        // TrackEvent
        super::dispatch_analytics_service(
            &ctx,
            "TrackEvent",
            &mk(
                "AnalyticsService",
                "TrackEvent",
                vec![
                    Value::from("login"),
                    Value::from("Alice"),
                    Value::empty_map(),
                ],
            ),
        )
        .unwrap();

        // GetEvents
        let result = super::dispatch_analytics_service(
            &ctx,
            "GetEvents",
            &mk(
                "AnalyticsService",
                "GetEvents",
                vec![Value::from("login"), Value::Number(10.0)],
            ),
        )
        .unwrap();
        assert!(matches!(result, Value::Array(v) if v.len() == 1));

        // ClearEvents
        super::dispatch_analytics_service(
            &ctx,
            "ClearEvents",
            &mk(
                "AnalyticsService",
                "ClearEvents",
                vec![Value::from("login")],
            ),
        )
        .unwrap();
        let result = super::dispatch_analytics_service(
            &ctx,
            "GetEvents",
            &mk(
                "AnalyticsService",
                "GetEvents",
                vec![Value::from("login"), Value::Number(10.0)],
            ),
        )
        .unwrap();
        assert!(matches!(result, Value::Array(v) if v.is_empty()));
    }

    #[test]
    fn localization_round_trip() {
        let ctx = ctx();

        // Register
        super::dispatch_localization_service(
            &ctx,
            "Register",
            &mk(
                "LocalizationService",
                "Register",
                vec![
                    Value::from("hello"),
                    Value::map([
                        ("en-US", Value::from("Hello")),
                        ("vi-VN", Value::from("Xin chào")),
                    ]),
                ],
            ),
        )
        .unwrap();

        // Translate
        let result = super::dispatch_localization_service(
            &ctx,
            "Translate",
            &mk(
                "LocalizationService",
                "Translate",
                vec![Value::from("hello"), Value::from("vi-VN")],
            ),
        )
        .unwrap();
        assert_eq!(result, Value::from("Xin chào"));

        // SetLocale + TranslateFor
        super::dispatch_localization_service(
            &ctx,
            "SetLocale",
            &mk(
                "LocalizationService",
                "SetLocale",
                vec![Value::from("Alice"), Value::from("vi-VN")],
            ),
        )
        .unwrap();
        let result = super::dispatch_localization_service(
            &ctx,
            "TranslateFor",
            &mk(
                "LocalizationService",
                "TranslateFor",
                vec![Value::from("hello"), Value::from("Alice")],
            ),
        )
        .unwrap();
        assert_eq!(result, Value::from("Xin chào"));

        // Unregister + fallback
        super::dispatch_localization_service(
            &ctx,
            "Unregister",
            &mk(
                "LocalizationService",
                "Unregister",
                vec![Value::from("hello")],
            ),
        )
        .unwrap();
        let result = super::dispatch_localization_service(
            &ctx,
            "Translate",
            &mk(
                "LocalizationService",
                "Translate",
                vec![Value::from("hello"), Value::from("en-US")],
            ),
        )
        .unwrap();
        assert_eq!(result, Value::from("hello"));
    }

    #[test]
    fn teleport_zones() {
        let ctx = ctx();

        // Add a player
        {
            let mut players = ctx.runtime.players.write();
            players.join(PlayerRecord::new("Alice", 1, "VN"));
        }

        // RegisterZone
        let pos = Value::map([
            ("X", Value::Number(10.0)),
            ("Y", Value::Number(0.0)),
            ("Z", Value::Number(5.0)),
        ]);
        super::dispatch_teleport_service(
            &ctx,
            "RegisterZone",
            &mk(
                "TeleportService",
                "RegisterZone",
                vec![Value::from("Hub"), Value::from("server1"), pos],
            ),
        )
        .unwrap();

        // GetZone
        let zone = super::dispatch_teleport_service(
            &ctx,
            "GetZone",
            &mk("TeleportService", "GetZone", vec![Value::from("Hub")]),
        )
        .unwrap();
        assert!(matches!(&zone, Value::Map(m) if m.contains_key("Name")));

        // TeleportToZone
        super::dispatch_teleport_service(
            &ctx,
            "TeleportToZone",
            &mk(
                "TeleportService",
                "TeleportToZone",
                vec![Value::from("Alice"), Value::from("Hub"), Value::Nil],
            ),
        )
        .unwrap();
        let pos = ctx.runtime.players.read().get("Alice").unwrap().position;
        assert_eq!(pos.x, 10.0);

        // UnregisterZone
        let result = super::dispatch_teleport_service(
            &ctx,
            "UnregisterZone",
            &mk(
                "TeleportService",
                "UnregisterZone",
                vec![Value::from("Hub")],
            ),
        )
        .unwrap();
        assert_eq!(result, Value::Bool(true));

        // GetZones should now be empty
        let zones = super::dispatch_teleport_service(
            &ctx,
            "GetZones",
            &mk("TeleportService", "GetZones", vec![]),
        )
        .unwrap();
        assert!(matches!(zones, Value::Array(v) if v.is_empty()));
    }

    #[test]
    fn http_service_prepare_request_release() {
        let ctx = ctx();

        // Prepare
        let id = super::dispatch_http_service(
            &ctx,
            "Prepare",
            &mk(
                "HttpService",
                "Prepare",
                vec![
                    Value::Number(42.0),
                    Value::from("GET"),
                    Value::from("https://example.com"),
                    Value::empty_map(),
                    Value::from(""),
                    Value::from("application/json"),
                ],
            ),
        )
        .unwrap();
        assert_eq!(id, Value::Number(42.0));

        // Request
        let resp = super::dispatch_http_service(
            &ctx,
            "Request",
            &mk("HttpService", "Request", vec![Value::Number(42.0)]),
        )
        .unwrap();
        assert!(matches!(&resp, Value::Map(m) if m.get("Success") == Some(&Value::Bool(true))));

        // Release
        let ok = super::dispatch_http_service(
            &ctx,
            "Release",
            &mk("HttpService", "Release", vec![Value::Number(42.0)]),
        )
        .unwrap();
        assert_eq!(ok, Value::Bool(true));
    }

    #[test]
    fn datastore_session_flow() {
        let ctx = ctx();

        // Init
        super::dispatch_datastore_service(
            &ctx,
            "Init",
            &mk(
                "DataStoreService",
                "Init",
                vec![Value::from("AAAAAAAAAAAAAAAAAAAAAAAAAAAA")],
            ),
        )
        .unwrap();

        // Insert
        super::dispatch_datastore_service(
            &ctx,
            "Insert",
            &mk(
                "DataStoreService",
                "Insert",
                vec![
                    Value::from("AAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
                    Value::from("score"),
                    Value::Number(100.0),
                ],
            ),
        )
        .unwrap();

        // Get
        let val = super::dispatch_datastore_service(
            &ctx,
            "Get",
            &mk(
                "DataStoreService",
                "Get",
                vec![
                    Value::from("AAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
                    Value::from("score"),
                ],
            ),
        )
        .unwrap();
        assert_eq!(val, Value::Number(100.0));

        // GetKeys
        let keys = super::dispatch_datastore_service(
            &ctx,
            "GetKeys",
            &mk(
                "DataStoreService",
                "GetKeys",
                vec![Value::from("AAAAAAAAAAAAAAAAAAAAAAAAAAAA")],
            ),
        )
        .unwrap();
        assert!(matches!(keys, Value::Array(v) if v.len() == 1));

        // Delete
        super::dispatch_datastore_service(
            &ctx,
            "Delete",
            &mk(
                "DataStoreService",
                "Delete",
                vec![
                    Value::from("AAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
                    Value::from("score"),
                ],
            ),
        )
        .unwrap();

        // Close
        super::dispatch_datastore_service(
            &ctx,
            "Close",
            &mk(
                "DataStoreService",
                "Close",
                vec![Value::from("AAAAAAAAAAAAAAAAAAAAAAAAAAAA")],
            ),
        )
        .unwrap();
    }
}
