/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The dynamic value type that crosses every Noobietoria API boundary.
//!
//! The documentation states repeatedly that script arguments must be
//! *serialisable*: `string`, `number`, `boolean` or a plain table of those
//! (`spec/source/instance.md`, `spec/source/api.md`). [`Value`] is exactly that
//! closed set, which makes it safe to move between a Luau script, the network,
//! and the JSON-backed stores without ever smuggling a live pointer across.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// A serialisable Noobietoria value: the intersection of what Luau tables and
/// JSON can both represent.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    /// `nil`
    #[default]
    Nil,
    /// `boolean`
    Bool(bool),
    /// `number`
    Number(f64),
    /// `string`
    Str(String),
    /// An ordered, zero-indexed array — a Luau table with only sequential keys.
    Array(Vec<Value>),
    /// A keyed table with string keys, kept in sorted order for determinism.
    Map(BTreeMap<String, Value>),
}

impl Value {
    /// Discriminant name, used in error messages and in the API registry.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::Str(_) => "string",
            Value::Array(_) => "table",
            Value::Map(_) => "table",
        }
    }

    /// `true` only for [`Value::Nil`].
    pub fn is_nil(&self) -> bool {
        matches!(self, Value::Nil)
    }

    /// Borrow as a string slice.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Borrow as a number. Booleans are deliberately *not* coerced: the
    /// documentation distinguishes `number` from `boolean` in every signature.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Borrow as an integer, truncating a fractional number like Luau does when
    /// an API documents a whole number (for example `Quantity`).
    pub fn as_i64(&self) -> Option<i64> {
        self.as_f64().map(|n| n as i64)
    }

    /// Borrow as a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Borrow as an array.
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Borrow as a keyed table.
    pub fn as_map(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    /// Look up a field of a keyed table.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_map().and_then(|m| m.get(key))
    }

    /// Build a keyed table from pairs.
    pub fn map<K, I>(pairs: I) -> Value
    where
        K: Into<String>,
        I: IntoIterator<Item = (K, Value)>,
    {
        Value::Map(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    /// Build an array from values.
    pub fn array<I: IntoIterator<Item = Value>>(items: I) -> Value {
        Value::Array(items.into_iter().collect())
    }

    /// The value's "truthiness" as Luau defines it: everything except `nil` and
    /// `false` is truthy. Numeric zero and the empty string are truthy.
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }

    /// The empty keyed table, a common default for `Properties`/`Metadata`.
    pub fn empty_map() -> Value {
        Value::Map(BTreeMap::new())
    }

    /// Depth-first structural equality that treats numbers exactly.
    ///
    /// Deriving `PartialEq` already gives structural equality; this exists as a
    /// named, documented entry point because several services need to compare
    /// two received values (offers, reward payloads) and must not accidentally
    /// get float-epsilon semantics.
    pub fn same_as(&self, other: &Value) -> bool {
        self == other
    }
}

impl fmt::Display for Value {
    /// Renders the value the way a script author would write it, which is what
    /// the console and the DataStore browser show.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => f.write_str("nil"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Number(n) => {
                if n.fract() == 0.0 && n.is_finite() {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{n}")
                }
            }
            Value::Str(s) => write!(f, "{s}"),
            Value::Array(items) => {
                f.write_str("{")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("}")
            }
            Value::Map(map) => {
                f.write_str("{")?;
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{k} = {v}")?;
                }
                f.write_str("}")
            }
        }
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Number(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Number(v as f64)
    }
}

impl From<usize> for Value {
    fn from(v: usize) -> Self {
        Value::Number(v as f64)
    }
}

impl From<u64> for Value {
    fn from(v: u64) -> Self {
        Value::Number(v as f64)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Str(v.to_owned())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Str(v)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Value::Array(v.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(inner) => inner.into(),
            None => Value::Nil,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truthiness_matches_luau() {
        assert!(!Value::Nil.is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Bool(true).is_truthy());
        assert!(Value::Number(0.0).is_truthy(), "0 is truthy in Luau");
        assert!(
            Value::Str(String::new()).is_truthy(),
            "'' is truthy in Luau"
        );
    }

    #[test]
    fn numbers_round_trip_through_json() {
        let value = Value::map([
            ("amount", Value::from(42.5)),
            ("quantity", Value::from(3i64)),
            ("flag", Value::from(true)),
            ("name", Value::from("Sword_01")),
            ("tags", Value::array([Value::from("a"), Value::from("b")])),
        ]);
        let json = serde_json::to_string(&value).expect("serialises");
        let back: Value = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(value, back);
    }

    #[test]
    fn nil_round_trips_through_json() {
        let json = serde_json::to_string(&Value::Nil).expect("serialises");
        assert_eq!(json, "null");
        let back: Value = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(back, Value::Nil);
    }

    #[test]
    fn display_reads_like_script_source() {
        assert_eq!(Value::from(3i64).to_string(), "3");
        assert_eq!(Value::from(1.5).to_string(), "1.5");
        assert_eq!(
            Value::map([("Coin", Value::from(100i64))]).to_string(),
            "{Coin = 100}"
        );
        assert_eq!(
            Value::array([Value::from(1i64), Value::from(2i64)]).to_string(),
            "{1, 2}"
        );
    }

    #[test]
    fn accessors_do_not_coerce_across_types() {
        assert_eq!(Value::Bool(true).as_f64(), None);
        assert_eq!(Value::Number(1.0).as_bool(), None);
        assert_eq!(Value::Number(7.9).as_i64(), Some(7));
        assert_eq!(Value::from("x").as_str(), Some("x"));
    }

    #[test]
    fn maps_are_deterministic_regardless_of_insert_order() {
        let a = Value::map([("z", Value::from(1i64)), ("a", Value::from(2i64))]);
        let b = Value::map([("a", Value::from(2i64)), ("z", Value::from(1i64))]);
        assert_eq!(a, b);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap()
        );
    }

    #[test]
    fn nested_lookup_works() {
        let value = Value::map([(
            "Offers",
            Value::array([Value::map([("Coin", Value::from(5i64))])]),
        )]);
        let offers = value
            .get("Offers")
            .and_then(Value::as_array)
            .expect("array");
        assert_eq!(offers[0].get("Coin").and_then(Value::as_f64), Some(5.0));
    }
}
