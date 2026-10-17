/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! # nb-script
//!
//! Sandboxed Luau script runtime: embeds mlua with Luau compiler/VM and
//! binds the Noobietoria 45 services, world instances, and datastores.

pub mod convert;
pub mod engine;
pub mod log;

pub use convert::{lua_to_value, value_to_lua};
pub use engine::ScriptEngine;
pub use log::{LogLevel, ScriptLog};

#[cfg(test)]
mod tests {
    use super::*;
    use nb_api::{ApiContext, PlayerRecord, Side};
    use nb_types::Value;
    use std::sync::Arc;

    #[test]
    fn test_luau_eval_math() {
        let ctx = Arc::new(ApiContext::new(Side::Server));
        let engine = ScriptEngine::new(ctx).expect("engine creation");
        let res = engine.eval("return 40 + 2").expect("eval math");
        assert_eq!(res, Value::Number(42.0));
    }

    #[test]
    fn test_luau_print_and_logs() {
        let ctx = Arc::new(ApiContext::new(Side::Server));
        let engine = ScriptEngine::new(ctx).expect("engine creation");
        engine
            .eval("print('Hello', 'Luau', 2026)")
            .expect("print call");
        let logs = engine.get_logs();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].level, LogLevel::Info);
        assert_eq!(logs[0].message, "Hello\tLuau\t2026");
    }

    #[test]
    fn test_luau_service_call() {
        let ctx = Arc::new(ApiContext::new(Side::Server));
        ctx.runtime.player_join(PlayerRecord::new("Bob", 2, "VN"));

        let engine = ScriptEngine::new(ctx).expect("engine creation");
        let script = r#"
            local Currency = game:GetService("CurrencyService")
            Currency:Add("Bob", "Coin", 500)
            return Currency:GetBalance("Bob", "Coin")
        "#;
        let res = engine.eval(script).expect("service execution");
        assert_eq!(res, Value::Number(500.0));
    }
}
