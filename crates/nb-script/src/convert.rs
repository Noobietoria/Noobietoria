/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use mlua::prelude::*;
use nb_types::Value;
use std::collections::BTreeMap;

/// Converts an [`nb_types::Value`] to an [`mlua::Value`].
pub fn value_to_lua<'lua>(lua: &'lua Lua, val: &Value) -> LuaResult<LuaValue> {
    match val {
        Value::Nil => Ok(LuaValue::Nil),
        Value::Bool(b) => Ok(LuaValue::Boolean(*b)),
        Value::Number(n) => Ok(LuaValue::Number(*n)),
        Value::Str(s) => Ok(LuaValue::String(lua.create_string(s)?)),
        Value::Array(arr) => {
            let table = lua.create_table()?;
            for (idx, item) in arr.iter().enumerate() {
                table.set(idx + 1, value_to_lua(lua, item)?)?;
            }
            Ok(LuaValue::Table(table))
        }
        Value::Map(map) => {
            let table = lua.create_table()?;
            for (k, v) in map {
                table.set(k.as_str(), value_to_lua(lua, v)?)?;
            }
            Ok(LuaValue::Table(table))
        }
    }
}

/// Converts an [`mlua::Value`] to an [`nb_types::Value`].
pub fn lua_to_value(val: LuaValue) -> LuaResult<Value> {
    match val {
        LuaValue::Nil => Ok(Value::Nil),
        LuaValue::Boolean(b) => Ok(Value::Bool(b)),
        LuaValue::Integer(i) => Ok(Value::Number(i as f64)),
        LuaValue::Number(n) => Ok(Value::Number(n)),
        LuaValue::String(s) => Ok(Value::Str(s.to_str()?.to_owned())),
        LuaValue::Table(table) => {
            let len = table.raw_len();
            if len > 0 {
                let mut arr = Vec::with_capacity(len);
                for i in 1..=len {
                    let v: LuaValue = table.get(i)?;
                    arr.push(lua_to_value(v)?);
                }
                Ok(Value::Array(arr))
            } else {
                let mut map = BTreeMap::new();
                for pair in table.pairs::<String, LuaValue>() {
                    let (k, v) = pair?;
                    map.insert(k, lua_to_value(v)?);
                }
                Ok(Value::Map(map))
            }
        }
        _ => Ok(Value::Nil),
    }
}
