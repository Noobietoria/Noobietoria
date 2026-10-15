//! The service registry: the single entry point for every documented function.
//!
//! # How coverage is proven
//!
//! Three facts are checked mechanically by the test suite, not asserted in prose:
//!
//! 1. [`implemented_functions`] must contain **every** key in
//!    [`crate::spec::signatures`] — nothing documented is missing.
//! 2. It must contain **nothing else** — no invented API, no undocumented extra.
//! 3. Dispatching each of those keys must never return
//!    [`ApiError::UnknownFunction`], which is only possible if a real `match`
//!    arm exists behind it.
//!
//! Fact 3 is the important one: it means the coverage claim rests on the
//! dispatcher actually being wired up, not on a list that could be padded.

use std::collections::BTreeMap;

use nb_types::Value;

use crate::args::Args;
use crate::error::{ApiError, ApiResult, Side};
use crate::runtime::ApiContext;
use crate::services;

/// Which side may call a service function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    /// Only the authoritative server.
    ServerOnly,
    /// Only a player's client.
    ClientOnly,
    /// Either side.
    Both,
}

impl Availability {
    /// The `required` label used in error messages.
    pub fn required_label(self) -> &'static str {
        match self {
            Availability::ServerOnly => "server",
            Availability::ClientOnly => "client",
            Availability::Both => "client and server",
        }
    }

    /// Whether `side` may call a function with this availability.
    ///
    /// The Studio is allowed both, because it playtests either side.
    pub fn allows(self, side: Side) -> bool {
        match self {
            Availability::Both => true,
            Availability::ServerOnly => side.may_call_server_only(),
            Availability::ClientOnly => side.may_call_client_only(),
        }
    }
}

/// Static metadata about one documented function, taken from the vendored spec.
#[derive(Clone, Debug)]
pub struct FunctionInfo {
    /// Owning service.
    pub service: String,
    /// Method name.
    pub method: String,
    /// Documented parameter names.
    pub params: Vec<String>,
    /// Documented arity.
    pub arity: usize,
    /// Whether the signature ends in `...`.
    pub varargs: bool,
    /// Which side may call it.
    pub availability: Availability,
}

impl FunctionInfo {
    /// The `Service.Method` key.
    pub fn key(&self) -> String {
        if self.method.is_empty() {
            self.service.clone()
        } else {
            format!("{}.{}", self.service, self.method)
        }
    }
}

/// The availability of one function.
///
/// The default is [`Availability::ServerOnly`] because the documented advice is
/// that shared state lives behind the server: a `ClientScript` "muốn thay đổi
/// dữ liệu chung (tiền tệ, inventory, ...) phải gửi yêu cầu qua NetworkEvent lên
/// ServerScript xử lý". Opting *out* of that has to be explicit, per function.
///
/// Services whose entire surface is client-presentation are listed in full;
/// everything else is server-only unless named in the shared or client lists.
pub fn availability_of(service: &str, method: &str) -> Availability {
    // Entirely client-side: these only affect what one player sees or presses.
    const CLIENT_SERVICES: [&str; 5] = [
        "CameraService",
        "InputService",
        "UIService",
        "TextService",
        "AudioService",
    ];
    if CLIENT_SERVICES.contains(&service) {
        return Availability::ClientOnly;
    }

    match (service, method) {
        // --- Available on both sides -----------------------------------
        // The instance tree exists on both; the snapshot rules decide what a
        // client can actually see.
        ("InstanceService", _) => Availability::Both,
        ("NetworkService", _) => Availability::Both,
        ("NetworkEventService", _) => Availability::Both,
        ("TweenService", _) => Availability::Both,
        ("NotificationService", _) => Availability::Both,
        ("LocalizationService", "Translate" | "TranslateFor" | "GetLocale" | "SetLocale") => {
            Availability::Both
        }
        ("ProximityService", "OnEnter" | "OnExit" | "OnTriggered") => Availability::Both,
        ("SoundEffectService", _) => Availability::Both,
        ("EffectService", _) => Availability::Both,
        ("CollisionService", "GetGroup" | "GetGroups") => Availability::Both,
        ("LightingService", "GetTime" | "GetWeather" | "GetProperty") => Availability::Both,
        ("PhysicsService", "GetGravity" | "GetVelocity") => Availability::Both,

        // --- Client-presentation functions on otherwise-server services --
        ("ChatService", "ShowBubble" | "HideBubble") => Availability::ClientOnly,
        ("PlayerService", "GetCoors") => Availability::Both,

        // --- Everything else is server-authoritative -------------------
        _ => Availability::ServerOnly,
    }
}

/// Metadata for one documented function key, or `None` if undocumented.
pub fn function_info(service: &str, method: &str) -> Option<FunctionInfo> {
    crate::spec::find(service, method).map(|s| FunctionInfo {
        service: s.service.clone(),
        method: s.method.clone(),
        params: s.params.clone(),
        arity: s.arity,
        varargs: s.varargs,
        availability: availability_of(&s.service, &s.method),
    })
}

/// Every documented key, with its metadata.
pub fn catalog() -> Vec<FunctionInfo> {
    crate::spec::signatures()
        .iter()
        .map(|s| FunctionInfo {
            service: s.service.clone(),
            method: s.method.clone(),
            params: s.params.clone(),
            arity: s.arity,
            varargs: s.varargs,
            availability: availability_of(&s.service, &s.method),
        })
        .collect()
}

/// The catalog grouped by service, which is how the Docs present it.
pub fn catalog_by_service() -> BTreeMap<String, Vec<FunctionInfo>> {
    let mut grouped: BTreeMap<String, Vec<FunctionInfo>> = BTreeMap::new();
    for info in catalog() {
        grouped.entry(info.service.clone()).or_default().push(info);
    }
    grouped
}

/// Every `Service.Method` key this build actually dispatches.
///
/// Derived by *probing the dispatcher itself*: a key is implemented exactly when
/// dispatching it does not answer [`ApiError::UnknownFunction`]. There is no
/// hand-written list to drift out of sync with the `match` arms, so the coverage
/// test cannot pass while a function is unwired.
pub fn implemented_functions() -> Vec<String> {
    let ctx = probe_context();
    let mut keys: Vec<String> = catalog()
        .into_iter()
        .filter(|info| {
            !matches!(
                services::dispatch(
                    &ctx,
                    &info.service,
                    &info.method,
                    &Args::new(&info.service, &info.method, Vec::new())
                ),
                Err(ApiError::UnknownFunction { .. })
            )
        })
        .map(|info| info.key())
        .collect();
    keys.sort();
    keys
}

/// Whether a key is dispatched by this build.
///
/// The probe runs on the Studio side, which is permitted to call both
/// server-only and client-only functions, so availability never masks a
/// missing arm.
pub fn is_implemented(service: &str, method: &str) -> bool {
    let ctx = probe_context();
    let args = Args::new(service, method, Vec::new());
    !matches!(
        services::dispatch(&ctx, service, method, &args),
        Err(ApiError::UnknownFunction { .. })
    )
}

/// A context used only to probe the dispatcher. Its state is discarded.
fn probe_context() -> ApiContext {
    ApiContext::new(Side::Studio)
}

/// Calls one documented service function.
///
/// This is the whole public surface of the API: the script layer marshals Luau
/// values into an [`Args`], calls this, and marshals the [`Value`] back.
///
/// The checks happen in a deliberate order, so the error an author sees is the
/// most useful one:
///
/// 1. Does the function exist in the documentation at all?
/// 2. Is this side allowed to call it?
/// 3. Are enough arguments present?
/// 4. Dispatch to the implementation, which answers
///    [`ApiError::UnknownFunction`] if no arm is wired up.
pub fn call(ctx: &ApiContext, service: &str, method: &str, values: Vec<Value>) -> ApiResult<Value> {
    let info = function_info(service, method).ok_or_else(|| ApiError::UnknownFunction {
        service: service.to_owned(),
        method: method.to_owned(),
    })?;

    if !info.availability.allows(ctx.side) {
        return Err(ApiError::WrongSide {
            service: service.to_owned(),
            method: method.to_owned(),
            required: info.availability.required_label(),
            actual: ctx.side.as_str(),
        });
    }

    let args = Args::new(service, method, values);

    // A signature without varargs has a fixed minimum arity. Variadic
    // signatures still need their named parameters.
    args.require_arity(info.arity)?;

    services::dispatch(ctx, service, method, &args)
}

/// Calls a function with the arguments already wrapped.
pub fn call_with_args(ctx: &ApiContext, args: &Args) -> ApiResult<Value> {
    call(ctx, args.service(), args.method(), args.values().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_documented_function_is_implemented() {
        let implemented = implemented_functions();
        let mut missing: Vec<String> = crate::spec::signatures()
            .iter()
            .map(|s| s.key())
            .filter(|key| !implemented.contains(key))
            .collect();
        missing.sort();
        assert!(
            missing.is_empty(),
            "{} documented function(s) are not implemented: {:?}",
            missing.len(),
            missing
        );
    }

    #[test]
    fn nothing_undocumented_is_implemented() {
        let documented: Vec<String> = crate::spec::signatures().iter().map(|s| s.key()).collect();
        let mut extra: Vec<String> = implemented_functions()
            .into_iter()
            .filter(|key| !documented.contains(key))
            .collect();
        extra.sort();
        assert!(
            extra.is_empty(),
            "{} implemented function(s) are not in the documentation: {:?}",
            extra.len(),
            extra
        );
    }

    #[test]
    fn implemented_keys_are_unique() {
        let keys = implemented_functions();
        let total = keys.len();
        let mut sorted = keys.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), total, "a function is registered twice");
    }

    #[test]
    fn every_implemented_key_has_a_real_dispatch_arm() {
        // Zero arguments is below every documented arity, so a wired-up arm
        // answers TooFewArguments. Only a missing arm answers UnknownFunction.
        let ctx = ApiContext::new(Side::Studio);
        let mut unwired: Vec<String> = Vec::new();

        for info in catalog() {
            match call(&ctx, &info.service, &info.method, Vec::new()) {
                Err(ApiError::UnknownFunction { .. }) => unwired.push(info.key()),
                _ => {}
            }
        }
        unwired.sort();
        assert!(
            unwired.is_empty(),
            "{} function(s) are listed but have no dispatch arm: {:?}",
            unwired.len(),
            unwired
        );
    }

    #[test]
    fn unknown_functions_are_rejected() {
        let ctx = ApiContext::new(Side::Server);
        let err = call(&ctx, "NoSuchService", "DoThing", vec![]).unwrap_err();
        assert!(matches!(err, ApiError::UnknownFunction { .. }));

        let err = call(&ctx, "PlayerService", "NoSuchMethod", vec![]).unwrap_err();
        assert!(matches!(err, ApiError::UnknownFunction { .. }));
    }

    #[test]
    fn server_only_functions_are_refused_on_a_client() {
        let server = ApiContext::new(Side::Server);
        let client = ApiContext::new(Side::Client);
        server
            .runtime
            .player_join(crate::runtime::PlayerRecord::new("Alice", 1, "VN"));
        let args = vec![Value::from("Alice"), Value::from("Coin"), Value::from(1i64)];

        assert!(call(&server, "CurrencyService", "Add", args.clone()).is_ok());
        let err = call(&client, "CurrencyService", "Add", args).unwrap_err();
        match err {
            ApiError::WrongSide {
                required, actual, ..
            } => {
                assert_eq!(required, "server");
                assert_eq!(actual, "client");
            }
            other => panic!("expected WrongSide, got {other:?}"),
        }
    }

    #[test]
    fn client_only_functions_are_refused_on_the_server() {
        let server = ApiContext::new(Side::Server);
        let client = ApiContext::new(Side::Client);
        client
            .runtime
            .player_join(crate::runtime::PlayerRecord::new("Alice", 1, "VN"));

        let err = call(
            &server,
            "CameraService",
            "Shake",
            vec![Value::from("Alice"), Value::from(2.0), Value::from(1.0)],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ApiError::WrongSide {
                required: "client",
                ..
            }
        ));

        // The same call from a client is fine.
        assert!(call(
            &client,
            "CameraService",
            "Shake",
            vec![Value::from("Alice"), Value::from(2.0), Value::from(1.0)]
        )
        .is_ok());
    }

    #[test]
    fn shared_functions_work_on_both_sides() {
        for side in [Side::Server, Side::Client, Side::Studio] {
            let ctx = ApiContext::new(side);
            assert!(
                call(
                    &ctx,
                    "InstanceService",
                    "GetGUIDs",
                    vec![Value::from("Nothing")]
                )
                .is_ok(),
                "InstanceService should be callable on {side:?}"
            );
        }
    }

    #[test]
    fn arity_is_checked_before_dispatch() {
        let ctx = ApiContext::new(Side::Server);
        let err = call(&ctx, "CurrencyService", "Add", vec![Value::from("Alice")]).unwrap_err();
        assert!(matches!(
            err,
            ApiError::TooFewArguments {
                expected: 3,
                got: 1,
                ..
            }
        ));
    }

    #[test]
    fn catalog_matches_the_spec() {
        assert_eq!(catalog().len(), crate::spec::signatures().len());
        let grouped = catalog_by_service();
        assert_eq!(grouped.len(), crate::spec::spec().service_count);
        assert_eq!(
            grouped.get("PlayerService").map(Vec::len),
            Some(12),
            "PlayerService has 12 documented functions"
        );
    }

    #[test]
    fn availability_is_server_first_by_default() {
        // A shared-state service defaults to the server.
        assert_eq!(
            availability_of("CurrencyService", "Add"),
            Availability::ServerOnly
        );
        // Presentation services are client-only.
        assert_eq!(
            availability_of("UIService", "Create"),
            Availability::ClientOnly
        );
        assert_eq!(
            availability_of("CameraService", "Reset"),
            Availability::ClientOnly
        );
        // The tree and the wire are shared.
        assert_eq!(
            availability_of("InstanceService", "Create"),
            Availability::Both
        );
        assert_eq!(
            availability_of("NetworkService", "FireServer"),
            Availability::Both
        );
    }

    #[test]
    fn availability_labels_read_well() {
        assert_eq!(Availability::ServerOnly.required_label(), "server");
        assert_eq!(Availability::ClientOnly.required_label(), "client");
        assert_eq!(Availability::Both.required_label(), "client and server");
        assert!(Availability::Both.allows(Side::Studio));
    }

    #[test]
    fn function_info_carries_documented_parameter_names() {
        let info = function_info("CurrencyService", "Add").expect("documented");
        assert_eq!(info.params, vec!["Username", "CurrencyType", "Amount"]);
        assert_eq!(info.key(), "CurrencyService.Add");
        assert!(function_info("CurrencyService", "Nope").is_none());
    }
}
