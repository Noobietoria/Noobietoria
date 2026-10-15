//! Errors every service function can raise.
//!
//! The documentation repeatedly requires a wrong call to *fail loudly* rather
//! than quietly do nothing — a `ModuleScript` without `return` "sẽ lỗi khi
//! require", calling `FireServer` from the server "sẽ báo lỗi", and so on. Every
//! variant here names the offending service function and parameter so the error
//! a script author sees points at their own call site.

use nb_types::Value;

/// Shorthand for service results.
pub type ApiResult<T> = Result<T, ApiError>;

/// A failure raised by a service function.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ApiError {
    /// No such documented function exists.
    #[error("`{service}.{method}` is not part of the Noobietoria API")]
    UnknownFunction {
        /// The service that was called.
        service: String,
        /// The method that was called.
        method: String,
    },
    /// The function exists but does not belong on this side.
    #[error("`{service}.{method}` is {required}-only; this call is running on the {actual}")]
    WrongSide {
        /// The service function.
        service: String,
        /// The method.
        method: String,
        /// Which side may call it.
        required: &'static str,
        /// Which side actually called it.
        actual: &'static str,
    },
    /// Too few arguments were supplied.
    #[error("`{service}.{method}` expects at least {expected} argument(s), got {got}")]
    TooFewArguments {
        /// The service function.
        service: String,
        /// The method.
        method: String,
        /// Documented arity.
        expected: usize,
        /// Arguments actually supplied.
        got: usize,
    },
    /// An argument had the wrong shape or type.
    #[error("`{service}.{method}` argument `{param}` (position {position}) is invalid: {reason}")]
    BadArgument {
        /// The service function.
        service: String,
        /// The method.
        method: String,
        /// Documented parameter name.
        param: String,
        /// Zero-based position.
        position: usize,
        /// What was wrong.
        reason: String,
    },
    /// The referenced instance does not exist.
    #[error(transparent)]
    World(#[from] nb_world::WorldError),
    /// A store operation failed.
    #[error(transparent)]
    Store(#[from] nb_store::StoreError),
    /// A datastore operation failed.
    #[error(transparent)]
    DataStore(#[from] nb_store::DataStoreError),
    /// A domain rule was violated.
    #[error("{0}")]
    Rule(String),
    /// The named entity (player, board, quest, ...) does not exist.
    #[error("no {kind} named `{name}`")]
    NotFound {
        /// What kind of thing was looked up.
        kind: &'static str,
        /// The name used.
        name: String,
    },
    /// The entity already exists.
    #[error("{kind} `{name}` already exists")]
    AlreadyExists {
        /// What kind of thing was created.
        kind: &'static str,
        /// The name used.
        name: String,
    },
}

impl ApiError {
    /// Builds a bad-argument error from a documented signature.
    ///
    /// Parameter names come from the vendored documentation, so the message
    /// quotes the same name an author reads in the Docs.
    pub fn bad_arg(
        service: &str,
        method: &str,
        position: usize,
        param: &str,
        reason: impl Into<String>,
    ) -> Self {
        ApiError::BadArgument {
            service: service.to_owned(),
            method: method.to_owned(),
            param: param.to_owned(),
            position,
            reason: reason.into(),
        }
    }

    /// Builds a bad-argument error that reports the value's actual type.
    pub fn wrong_type(
        service: &str,
        method: &str,
        position: usize,
        param: &str,
        got: &Value,
        expected: &str,
    ) -> Self {
        Self::bad_arg(
            service,
            method,
            position,
            param,
            format!("expected {expected}, got {}", got.type_name()),
        )
    }
}

/// Which side of the client/server split a call is running on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// The authoritative Dedicated server.
    Server,
    /// A player's client.
    Client,
    /// The Studio's editor environment, used for authoring and playtest.
    Studio,
}

impl Side {
    /// The name used in error messages.
    pub fn as_str(self) -> &'static str {
        match self {
            Side::Server => "server",
            Side::Client => "client",
            Side::Studio => "studio",
        }
    }

    /// Whether this side is authoritative for shared state.
    pub fn is_authoritative(self) -> bool {
        matches!(self, Side::Server)
    }

    /// Whether server-only functions may be called.
    pub fn may_call_server_only(self) -> bool {
        matches!(self, Side::Server | Side::Studio)
    }

    /// Whether client-only functions may be called.
    ///
    /// The documentation is explicit that a `ServerScript` "không thể gọi các
    /// API chỉ dành cho client (ví dụ: CameraService, InputService) trực tiếp
    /// ... phải thông qua NetworkEvent".
    pub fn may_call_client_only(self) -> bool {
        matches!(self, Side::Client | Side::Studio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_type_reports_the_actual_type() {
        let err = ApiError::wrong_type(
            "CurrencyService",
            "Add",
            2,
            "Amount",
            &Value::from("lots"),
            "number",
        );
        let text = err.to_string();
        assert!(text.contains("CurrencyService.Add"), "{text}");
        assert!(text.contains("Amount"), "{text}");
        assert!(text.contains("expected number, got string"), "{text}");
    }

    #[test]
    fn wrong_side_message_names_both_sides() {
        let err = ApiError::WrongSide {
            service: "CameraService".into(),
            method: "Shake".into(),
            required: "client",
            actual: "server",
        };
        let text = err.to_string();
        assert!(text.contains("client-only"), "{text}");
        assert!(text.contains("running on the server"), "{text}");
    }

    #[test]
    fn side_capabilities_match_the_documentation() {
        assert!(Side::Server.may_call_server_only());
        assert!(!Side::Server.may_call_client_only());
        assert!(Side::Client.may_call_client_only());
        assert!(!Side::Client.may_call_server_only());
        // The Studio runs both, because it playtests either side.
        assert!(Side::Studio.may_call_server_only());
        assert!(Side::Studio.may_call_client_only());
        assert!(Side::Server.is_authoritative());
        assert!(!Side::Client.is_authoritative());
    }

    #[test]
    fn world_and_store_errors_convert() {
        let e: ApiError = nb_world::WorldError::EmptyName.into();
        assert!(matches!(e, ApiError::World(_)));
        let e: ApiError = nb_store::StoreError::EmptyKey.into();
        assert!(matches!(e, ApiError::Store(_)));
    }
}
