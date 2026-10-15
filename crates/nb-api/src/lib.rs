/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! # nb-api
//!
//! The Noobietoria API: every service function documented at
//! <https://noobietoria.github.io/Docs/>, callable from Rust and (through
//! [`nb-script`]) from Luau.
//!
//! ## The contract this crate keeps
//!
//! The documented surface is **366 functions across 45 services**, vendored into
//! `spec/api-signatures.json` and embedded at compile time by [`spec`]. Three
//! invariants are enforced by tests, not by documentation:
//!
//! * every documented function has an implementation ([`registry::implemented_functions`]),
//! * no function exists that the documentation does not describe,
//! * dispatching any documented function reaches real code.
//!
//! ## How a call flows
//!
//! ```text
//! Luau            Registry             Services              Runtime
//! ────            ────────             ────────              ───────
//! Currency.Add ─▶ registry::call ─▶ services::dispatch ─▶ CurrencyService::add ─▶ ApiRuntime
//!                   │                      │
//!                   │                      └─ the arm that does the work
//!                   └─ side + arity checks, named-argument errors
//! ```
//!
//! ```
//! use nb_api::{args, call, ApiContext, PlayerRecord, Side};
//!
//! # fn main() -> Result<(), nb_api::ApiError> {
//! let ctx = ApiContext::new(Side::Server);
//! ctx.runtime.player_join(PlayerRecord::new("Alice", 1, "VN"));
//!
//! call(&ctx, "CurrencyService", "Add", args!["Alice", "Coin", 250])?;
//! let balance = call(&ctx, "CurrencyService", "GetBalance", args!["Alice", "Coin"])?;
//! assert_eq!(balance.as_f64(), Some(250.0));
//! # Ok(())
//! # }
//! ```

pub mod args;
pub mod error;
pub mod registry;
pub mod runtime;
pub mod services;
pub mod spec;

pub use args::Args;
pub use error::{ApiError, ApiResult, Side};
pub use registry::{
    availability_of, call, catalog, catalog_by_service, function_info, implemented_functions,
    is_implemented, Availability, FunctionInfo,
};
pub use runtime::{
    ApiContext, ApiRuntime, CameraState, Environment, InvokeHandler, Notification, PendingEvent,
    PlayerRecord, PlayerRegistry, UiElement,
};

/// Builds an [`Args`] list without naming the type at the call site.
///
/// ```
/// use nb_api::{args, call, ApiContext, Side};
/// # fn main() -> Result<(), nb_api::ApiError> {
/// let ctx = ApiContext::new(Side::Server);
/// call(&ctx, "InstanceService", "GetGUIDs", args!["Block"])?;
/// # Ok(())
/// # }
/// ```
#[macro_export]
macro_rules! args {
    () => {
        Vec::<$crate::__private::Value>::new()
    };
    ($($value:expr),+ $(,)?) => {
        vec![$($crate::__private::to_value($value)),+]
    };
}

/// Re-exports used by [`args!`]; not a stable API.
#[doc(hidden)]
pub mod __private {
    pub use nb_types::Value;

    /// Converts a Rust value into a [`Value`] for [`crate::args!`].
    ///
    /// Accepts the shapes scripts actually pass: `&str`, `String`, `bool`,
    /// integers, floats, and anything already a [`Value`].
    pub trait ToValue {
        /// Performs the conversion.
        fn to_api_value(self) -> Value;
    }

    impl ToValue for Value {
        fn to_api_value(self) -> Value {
            self
        }
    }

    impl ToValue for &Value {
        fn to_api_value(self) -> Value {
            self.clone()
        }
    }

    impl ToValue for &str {
        fn to_api_value(self) -> Value {
            Value::from(self)
        }
    }

    impl ToValue for String {
        fn to_api_value(self) -> Value {
            Value::from(self)
        }
    }

    impl ToValue for &String {
        fn to_api_value(self) -> Value {
            Value::from(self.clone())
        }
    }

    impl ToValue for bool {
        fn to_api_value(self) -> Value {
            Value::from(self)
        }
    }

    impl ToValue for f64 {
        fn to_api_value(self) -> Value {
            Value::from(self)
        }
    }

    impl ToValue for f32 {
        fn to_api_value(self) -> Value {
            Value::from(self as f64)
        }
    }

    /// Integers all widen through `i64`, which is the range a Luau number holds
    /// exactly.
    macro_rules! int_to_value {
        ($($t:ty),+) => {
            $(
                impl ToValue for $t {
                    fn to_api_value(self) -> Value {
                        Value::from(self as i64)
                    }
                }
            )+
        };
    }

    int_to_value!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

    /// Converts any supported value.
    pub fn to_value<T: ToValue>(value: T) -> Value {
        value.to_api_value()
    }
}

/// Small helpers for the `args!` macro and for hand-built calls.
pub mod context {
    pub use crate::__private::ToValue;
}
