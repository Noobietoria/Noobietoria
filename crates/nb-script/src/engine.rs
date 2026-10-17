/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Luau Script Runtime Engine: binds Noobietoria world, services, and execution VM.

use std::sync::Arc;
use mlua::prelude::*;
use parking_lot::Mutex;

use nb_api::ApiContext;
use nb_types::Value;
use nb_world::instance::ScriptKind;

use crate::convert::{lua_to_value, value_to_lua};
use crate::log::{LogLevel, ScriptLog};

/// The execution host for Luau scripts in Noobietoria.
pub struct ScriptEngine {
    lua: Lua,
    api_ctx: Arc<ApiContext>,
    logs: Arc<Mutex<Vec<ScriptLog>>>,
}

impl ScriptEngine {
    /// Initializes a new Luau script engine bound to the given [`ApiContext`].
    pub fn new(api_ctx: Arc<ApiContext>) -> LuaResult<Self> {
        let lua = Lua::new();
        let logs = Arc::new(Mutex::new(Vec::new()));

        // --- Standard Output Functions: print, warn ---
        let print_fn = {
            let logs = Arc::clone(&logs);
            lua.create_function(move |_, args: LuaMultiValue| {
                let parts: Vec<String> = args
                    .iter()
                    .map(|v| match v {
                        LuaValue::Nil => "nil".to_owned(),
                        LuaValue::Boolean(b) => b.to_string(),
                        LuaValue::Integer(i) => i.to_string(),
                        LuaValue::Number(n) => n.to_string(),
                        LuaValue::String(s) => s.to_str().map(|b| b.to_string()).unwrap_or_else(|_| "<invalid utf8>".to_owned()),
                        LuaValue::Table(_) => "table".to_owned(),
                        LuaValue::Function(_) => "function".to_owned(),
                        _ => format!("{:?}", v),
                    })
                    .collect();
                let msg = parts.join("\t");
                logs.lock().push(ScriptLog::new(LogLevel::Info, msg));
                Ok(())
            })?
        };
        lua.globals().set("print", print_fn)?;

        let warn_fn = {
            let logs = Arc::clone(&logs);
            lua.create_function(move |_, args: LuaMultiValue| {
                let parts: Vec<String> = args
                    .iter()
                    .map(|v| match v {
                        LuaValue::String(s) => s.to_str().map(|b| b.to_string()).unwrap_or_else(|_| "<invalid utf8>".to_owned()),
                        other => format!("{:?}", other),
                    })
                    .collect();
                let msg = parts.join("\t");
                logs.lock().push(ScriptLog::new(LogLevel::Warning, msg));
                Ok(())
            })?
        };
        lua.globals().set("warn", warn_fn)?;

        // --- task library (task.wait, task.spawn) ---
        let task_tbl = lua.create_table()?;
        task_tbl.set(
            "wait",
            lua.create_function(|_, seconds: Option<f64>| {
                let s = seconds.unwrap_or(0.03);
                if s > 0.0 {
                    std::thread::sleep(std::time::Duration::from_secs_f64(s.min(5.0)));
                }
                Ok(s)
            })?,
        )?;
        task_tbl.set(
            "spawn",
            lua.create_function(|_, (func, args): (LuaFunction, LuaMultiValue)| {
                func.call::<LuaValue>(args)?;
                Ok(())
            })?,
        )?;
        lua.globals().set("task", task_tbl)?;

        // --- game & game:GetService(service_name) ---
        let game_tbl = lua.create_table()?;
        {
            let ctx = Arc::clone(&api_ctx);
            let get_service = lua.create_function(move |lua, (_self_tbl, service_name): (LuaValue, String)| {
                let s_tbl = lua.create_table()?;
                s_tbl.set("__service_name", service_name.clone())?;

                let meta = lua.create_table()?;
                let ctx_inner = Arc::clone(&ctx);
                let s_name = service_name.clone();

                meta.set(
                    "__index",
                    lua.create_function(move |lua, (_tbl, method_name): (LuaTable, String)| {
                        let ctx_call = Arc::clone(&ctx_inner);
                        let s_call = s_name.clone();
                        let func = lua.create_function(move |lua, args: LuaMultiValue| {
                            let mut api_args = Vec::new();
                            let mut iter = args.into_iter();
                            if let Some(first) = iter.next() {
                                let is_self = match &first {
                                    LuaValue::Table(t) => {
                                        t.get::<Option<String>>("__service_name").unwrap_or(None).is_some()
                                    }
                                    _ => false,
                                };
                                if !is_self {
                                    api_args.push(lua_to_value(first)?);
                                }
                            }
                            for arg in iter {
                                api_args.push(lua_to_value(arg)?);
                            }

                            match nb_api::call(&ctx_call, &s_call, &method_name, api_args) {
                                Ok(val) => value_to_lua(lua, &val),
                                Err(e) => Err(LuaError::RuntimeError(format!("{s_call}:{method_name} failed: {e}"))),
                            }
                        })?;
                        Ok(func)
                    })?,
                )?;

                s_tbl.set_metatable(Some(meta))?;
                Ok(s_tbl)
            })?;
            game_tbl.set("GetService", get_service)?;
        }
        lua.globals().set("game", game_tbl.clone())?;

        // --- workspace shortcut ---
        let ws_tbl = lua.create_table()?;
        ws_tbl.set("__service_name", "Workspace")?;
        lua.globals().set("workspace", ws_tbl)?;

        // --- Instance.new constructor ---
        let instance_tbl = lua.create_table()?;
        {
            let ctx = Arc::clone(&api_ctx);
            instance_tbl.set(
                "new",
                lua.create_function(move |lua, (class_name, parent_opt): (String, Option<String>)| {
                    let mut args = vec![Value::Str(class_name)];
                    if let Some(p) = parent_opt {
                        args.push(Value::Str(p));
                    }
                    match nb_api::call(&ctx, "InstanceService", "Create", args) {
                        Ok(val) => value_to_lua(lua, &val),
                        Err(e) => Err(LuaError::RuntimeError(format!("Instance.new error: {e}"))),
                    }
                })?,
            )?;
        }
        lua.globals().set("Instance", instance_tbl)?;

        // --- require(module_name) support ---
        {
            let ctx = Arc::clone(&api_ctx);
            let require_fn = lua.create_function(move |lua, target: String| {
                // Look for ModuleScript in world
                let world = ctx.runtime.world.read();
                let module_inst = world
                    .iter()
                    .find(|i| i.name == target || i.id.as_str() == target)
                    .cloned();
                drop(world);

                let inst = match module_inst {
                    Some(i) => i,
                    None => {
                        return Err(LuaError::RuntimeError(format!(
                            "Module `{target}` not found in World instances"
                        )))
                    }
                };

                let loaded_table: LuaTable = lua
                    .globals()
                    .get("_LOADED_MODULES")
                    .unwrap_or_else(|_| lua.create_table().unwrap());
                lua.globals().set("_LOADED_MODULES", loaded_table.clone())?;

                if let Ok(cached) = loaded_table.get::<LuaValue>(inst.id.as_str()) {
                    if !matches!(cached, LuaValue::Nil) {
                        return Ok(cached);
                    }
                }

                // Execute the module script chunk
                let chunk = lua.load(&inst.properties.source).set_name(&inst.name);
                let result: LuaValue = chunk.eval()?;
                if matches!(result, LuaValue::Nil) {
                    return Err(LuaError::RuntimeError(format!(
                        "ModuleScript `{}` must return a value",
                        inst.name
                    )));
                }

                loaded_table.set(inst.id.as_str(), result.clone())?;
                Ok(result)
            })?;
            lua.globals().set("require", require_fn)?;
        }

        Ok(Self { lua, api_ctx, logs })
    }

    /// Evaluates Luau source code and returns the resulting [`Value`].
    pub fn eval(&self, source: &str) -> Result<Value, String> {
        self.lua
            .load(source)
            .eval::<LuaValue>()
            .map_err(|e| e.to_string())
            .and_then(|lv| lua_to_value(lv).map_err(|e| e.to_string()))
    }

    /// Executes a script instance with `script` global populated.
    pub fn run_script(&self, name: &str, guid: &str, source: &str) -> Result<Value, String> {
        let script_obj = self.lua.create_table().map_err(|e| e.to_string())?;
        script_obj.set("Name", name).map_err(|e| e.to_string())?;
        script_obj.set("GUID", guid).map_err(|e| e.to_string())?;
        script_obj.set("Disabled", false).map_err(|e| e.to_string())?;
        self.lua
            .globals()
            .set("script", script_obj)
            .map_err(|e| e.to_string())?;

        let chunk = self.lua.load(source).set_name(name);
        match chunk.eval::<LuaValue>() {
            Ok(lv) => lua_to_value(lv).map_err(|e| e.to_string()),
            Err(e) => {
                let err_msg = format!("Runtime error in script `{name}`: {e}");
                self.logs
                    .lock()
                    .push(ScriptLog::new(LogLevel::Error, err_msg.clone()));
                Err(err_msg)
            }
        }
    }

    /// Discovers and executes all active `ServerScript` instances in the world.
    pub fn run_server_scripts(&self) -> Vec<Result<String, String>> {
        let world = self.api_ctx.runtime.world.read();
        let scripts: Vec<(String, String, String)> = world
            .iter()
            .filter(|i| {
                ScriptKind::from_instance_type(i.kind) == Some(ScriptKind::Server)
                    && i.properties.enabled
            })
            .map(|i| (i.name.clone(), i.id.as_str().to_owned(), i.properties.source.clone()))
            .collect();
        drop(world);

        let mut results = Vec::new();
        for (name, guid, source) in scripts {
            match self.run_script(&name, &guid, &source) {
                Ok(_) => results.push(Ok(format!("Script `{name}` executed successfully"))),
                Err(e) => results.push(Err(e)),
            }
        }
        results
    }

    /// Returns a copy of all accumulated logs.
    pub fn get_logs(&self) -> Vec<ScriptLog> {
        self.logs.lock().clone()
    }

    /// Clears the log buffer.
    pub fn clear_logs(&self) {
        self.logs.lock().clear();
    }
}
