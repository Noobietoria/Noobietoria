//! Reading documented arguments out of a call.
//!
//! Every service function receives an [`Args`] holding the values a script
//! passed. Accessors are positional but *named*: they take the parameter name
//! from the vendored documentation so a malformed call produces an error such as
//! `` `CurrencyService.Add` argument `Amount` (position 2) is invalid: expected
//! number, got string `` rather than a generic "bad argument".
//!
//! Optional accessors return `Option`, so a service can express the
//! documentation's own optionality — `Index` defaults to 1, `Metadata` "có thể
//! nil", `Duration` may be omitted.

use nb_types::{Rgba, Value, Vector3};

use crate::error::{ApiError, ApiResult};

/// The values passed to one service function.
#[derive(Clone, Debug, Default)]
pub struct Args {
    service: String,
    method: String,
    values: Vec<Value>,
}

impl Args {
    /// Builds an argument list for a named service function.
    pub fn new(service: &str, method: &str, values: Vec<Value>) -> Self {
        Self {
            service: service.to_owned(),
            method: method.to_owned(),
            values,
        }
    }

    /// The service this call targets.
    pub fn service(&self) -> &str {
        &self.service
    }

    /// The method this call targets.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// How many values were passed.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether no values were passed.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// All values, in order.
    pub fn values(&self) -> &[Value] {
        &self.values
    }

    /// The value at a position, if present.
    pub fn at(&self, position: usize) -> Option<&Value> {
        self.values.get(position)
    }

    /// The raw error used when a position is missing.
    fn missing(&self, position: usize, param: &str) -> ApiError {
        ApiError::bad_arg(
            &self.service,
            &self.method,
            position,
            param,
            "argument missing",
        )
    }

    /// A string argument, required.
    pub fn string(&self, position: usize, param: &str) -> ApiResult<String> {
        let value = self
            .at(position)
            .ok_or_else(|| self.missing(position, param))?;
        match value {
            Value::Str(s) => Ok(s.clone()),
            other => Err(ApiError::wrong_type(
                &self.service,
                &self.method,
                position,
                param,
                other,
                "string",
            )),
        }
    }

    /// A string argument, or `None` when absent or `nil`.
    pub fn optional_string(&self, position: usize, param: &str) -> ApiResult<Option<String>> {
        match self.at(position) {
            None | Some(Value::Nil) => Ok(None),
            Some(Value::Str(s)) => Ok(Some(s.clone())),
            Some(other) => Err(ApiError::wrong_type(
                &self.service,
                &self.method,
                position,
                param,
                other,
                "string",
            )),
        }
    }

    /// A string argument that falls back to `default` when absent or `nil`.
    pub fn string_or(&self, position: usize, param: &str, default: &str) -> ApiResult<String> {
        Ok(self
            .optional_string(position, param)?
            .unwrap_or_else(|| default.to_owned()))
    }

    /// A number argument, required.
    pub fn number(&self, position: usize, param: &str) -> ApiResult<f64> {
        let value = self
            .at(position)
            .ok_or_else(|| self.missing(position, param))?;
        match value {
            Value::Number(n) => Ok(*n),
            other => Err(ApiError::wrong_type(
                &self.service,
                &self.method,
                position,
                param,
                other,
                "number",
            )),
        }
    }

    /// A number argument, or `None` when absent or `nil`.
    pub fn optional_number(&self, position: usize, param: &str) -> ApiResult<Option<f64>> {
        match self.at(position) {
            None | Some(Value::Nil) => Ok(None),
            Some(Value::Number(n)) => Ok(Some(*n)),
            Some(other) => Err(ApiError::wrong_type(
                &self.service,
                &self.method,
                position,
                param,
                other,
                "number",
            )),
        }
    }

    /// A number argument that falls back to `default`.
    pub fn number_or(&self, position: usize, param: &str, default: f64) -> ApiResult<f64> {
        Ok(self.optional_number(position, param)?.unwrap_or(default))
    }

    /// An integer argument, required. A fractional number is truncated.
    pub fn integer(&self, position: usize, param: &str) -> ApiResult<i64> {
        Ok(self.number(position, param)? as i64)
    }

    /// An integer argument that falls back to `default`.
    pub fn integer_or(&self, position: usize, param: &str, default: i64) -> ApiResult<i64> {
        Ok(self.number_or(position, param, default as f64)? as i64)
    }

    /// A boolean argument, required.
    pub fn boolean(&self, position: usize, param: &str) -> ApiResult<bool> {
        let value = self
            .at(position)
            .ok_or_else(|| self.missing(position, param))?;
        match value {
            Value::Bool(b) => Ok(*b),
            other => Err(ApiError::wrong_type(
                &self.service,
                &self.method,
                position,
                param,
                other,
                "boolean",
            )),
        }
    }

    /// A boolean argument that falls back to `default`.
    pub fn boolean_or(&self, position: usize, param: &str, default: bool) -> ApiResult<bool> {
        match self.at(position) {
            None | Some(Value::Nil) => Ok(default),
            Some(Value::Bool(b)) => Ok(*b),
            Some(other) => Err(ApiError::wrong_type(
                &self.service,
                &self.method,
                position,
                param,
                other,
                "boolean",
            )),
        }
    }

    /// The `Index` argument, defaulting to 1 as the documentation specifies.
    pub fn index(&self, position: usize) -> ApiResult<usize> {
        let raw = self.number_or(position, "Index", 1.0)?;
        if raw < 1.0 {
            return Err(ApiError::bad_arg(
                &self.service,
                &self.method,
                position,
                "Index",
                "Index is 1-based, so it must be at least 1",
            ));
        }
        Ok(raw as usize)
    }

    /// An optional `Index` that stays `None` when the argument was not passed.
    pub fn optional_index(&self, position: usize) -> ApiResult<Option<usize>> {
        Ok(self
            .optional_number(position, "Index")?
            .map(|n| n.max(1.0) as usize))
    }

    /// A table argument (`Properties`, `Metadata`, `Rewards`, ...), required.
    pub fn table(&self, position: usize, param: &str) -> ApiResult<Value> {
        let value = self
            .at(position)
            .ok_or_else(|| self.missing(position, param))?;
        match value {
            Value::Map(_) | Value::Array(_) => Ok(value.clone()),
            other => Err(ApiError::wrong_type(
                &self.service,
                &self.method,
                position,
                param,
                other,
                "table",
            )),
        }
    }

    /// A table argument that falls back to an empty table.
    pub fn table_or_empty(&self, position: usize, param: &str) -> ApiResult<Value> {
        match self.at(position) {
            None | Some(Value::Nil) => Ok(Value::empty_map()),
            Some(Value::Map(_)) | Some(Value::Array(_)) => Ok(self.values[position].clone()),
            Some(other) => Err(ApiError::wrong_type(
                &self.service,
                &self.method,
                position,
                param,
                other,
                "table",
            )),
        }
    }

    /// An optional table argument, as documented for `Metadata` and `Filters`.
    pub fn optional_table(&self, position: usize, param: &str) -> ApiResult<Option<Value>> {
        match self.at(position) {
            None | Some(Value::Nil) => Ok(None),
            Some(Value::Map(_)) | Some(Value::Array(_)) => Ok(Some(self.values[position].clone())),
            Some(other) => Err(ApiError::wrong_type(
                &self.service,
                &self.method,
                position,
                param,
                other,
                "table",
            )),
        }
    }

    /// The remaining values from `position` onwards, used by `...` signatures.
    pub fn varargs(&self, position: usize) -> Vec<Value> {
        self.values
            .get(position..)
            .map(|s| s.to_vec())
            .unwrap_or_default()
    }

    /// A `{X, Y, Z}` position argument, required.
    pub fn position(&self, position: usize, param: &str) -> ApiResult<Vector3> {
        let value = self
            .at(position)
            .ok_or_else(|| self.missing(position, param))?;
        Vector3::from_value(value).ok_or_else(|| {
            ApiError::bad_arg(
                &self.service,
                &self.method,
                position,
                param,
                "expected a position table {X, Y, Z} or a three-element array",
            )
        })
    }

    /// An optional position argument.
    pub fn optional_position(&self, position: usize, param: &str) -> ApiResult<Option<Vector3>> {
        match self.at(position) {
            None | Some(Value::Nil) => Ok(None),
            Some(v) => match Vector3::from_value(v) {
                Some(p) => Ok(Some(p)),
                None => Err(ApiError::bad_arg(
                    &self.service,
                    &self.method,
                    position,
                    param,
                    "expected a position table {X, Y, Z} or a three-element array",
                )),
            },
        }
    }

    /// An `RGBA` colour argument, required.
    pub fn color(&self, position: usize, param: &str) -> ApiResult<Rgba> {
        let value = self
            .at(position)
            .ok_or_else(|| self.missing(position, param))?;
        Rgba::from_value(value).ok_or_else(|| {
            ApiError::bad_arg(
                &self.service,
                &self.method,
                position,
                param,
                "expected an RGBA table, a four-element array, or a packed integer",
            )
        })
    }

    /// An optional `RGBA` colour argument.
    pub fn optional_color(&self, position: usize, param: &str) -> ApiResult<Option<Rgba>> {
        match self.at(position) {
            None | Some(Value::Nil) => Ok(None),
            Some(v) => match Rgba::from_value(v) {
                Some(c) => Ok(Some(c)),
                None => Err(ApiError::bad_arg(
                    &self.service,
                    &self.method,
                    position,
                    param,
                    "expected an RGBA table, a four-element array, or a packed integer",
                )),
            },
        }
    }

    /// Checks that at least `expected` arguments were supplied.
    ///
    /// Used by the registry to reject an under-length call before dispatch, so
    /// a service body can assume its documented parameters are present.
    pub fn require_arity(&self, expected: usize) -> ApiResult<()> {
        if self.values.len() < expected {
            return Err(ApiError::TooFewArguments {
                service: self.service.clone(),
                method: self.method.clone(),
                expected,
                got: self.values.len(),
            });
        }
        Ok(())
    }

    /// Builds an error naming a rule the call broke.
    pub fn rule(&self, message: impl Into<String>) -> ApiError {
        ApiError::Rule(format!(
            "{}::{}: {}",
            self.service,
            self.method,
            message.into()
        ))
    }

    /// Wraps a value as an `ok` result, the common return for setters.
    pub fn ok(&self) -> ApiResult<Value> {
        Ok(Value::Bool(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: Vec<Value>) -> Args {
        Args::new("CurrencyService", "Add", values)
    }

    #[test]
    fn reads_required_values() {
        let a = args(vec![
            Value::from("Alice"),
            Value::from("Coin"),
            Value::from(100i64),
        ]);
        assert_eq!(a.string(0, "Username").unwrap(), "Alice");
        assert_eq!(a.string(1, "CurrencyType").unwrap(), "Coin");
        assert_eq!(a.number(2, "Amount").unwrap(), 100.0);
        assert_eq!(a.len(), 3);
    }

    #[test]
    fn missing_argument_names_the_documented_parameter() {
        let a = args(vec![Value::from("Alice")]);
        let err = a.number(2, "Amount").unwrap_err();
        let text = err.to_string();
        assert!(text.contains("CurrencyService.Add"), "{text}");
        assert!(text.contains("Amount"), "{text}");
        assert!(text.contains("position 2"), "{text}");
    }

    #[test]
    fn type_mismatch_reports_actual_and_expected() {
        let a = args(vec![
            Value::from("Alice"),
            Value::from("Coin"),
            Value::from("lots"),
        ]);
        let err = a.number(2, "Amount").unwrap_err();
        let text = err.to_string();
        assert!(text.contains("expected number, got string"), "{text}");
    }

    #[test]
    fn optional_accessors_tolerate_nil_and_absence() {
        let a = args(vec![Value::from("Alice"), Value::Nil]);
        assert_eq!(a.optional_string(1, "CurrencyType").unwrap(), None);
        assert_eq!(a.optional_string(9, "Missing").unwrap(), None);
        assert_eq!(a.optional_number(1, "Amount").unwrap(), None);
        assert_eq!(a.string_or(1, "CurrencyType", "Coin").unwrap(), "Coin");
        assert_eq!(a.number_or(1, "Amount", 5.0).unwrap(), 5.0);
        assert!(a.optional_table(1, "Metadata").unwrap().is_none());
    }

    #[test]
    fn index_defaults_to_one_and_is_one_based() {
        let a = args(vec![Value::from("Part")]);
        assert_eq!(a.index(1).unwrap(), 1, "omitted Index means 1");
        let b = args(vec![Value::from("Part"), Value::from(3i64)]);
        assert_eq!(b.index(1).unwrap(), 3);
        let c = args(vec![Value::from("Part"), Value::from(0i64)]);
        assert!(c.index(1).is_err(), "index 0 is not 1-based");
    }

    #[test]
    fn optional_index_stays_none_when_omitted() {
        let a = args(vec![Value::from("Part")]);
        assert_eq!(a.optional_index(1).unwrap(), None);
        let b = args(vec![Value::from("Part"), Value::from(2i64)]);
        assert_eq!(b.optional_index(1).unwrap(), Some(2));
    }

    #[test]
    fn varargs_collect_the_tail() {
        let a = Args::new(
            "NetworkService",
            "FireClient",
            vec![
                Value::from("Buy"),
                Value::from("Alice"),
                Value::from("ok"),
                Value::from(3i64),
            ],
        );
        assert_eq!(a.varargs(2).len(), 2);
        assert_eq!(a.varargs(4).len(), 0);
    }

    #[test]
    fn positions_accept_the_documented_shapes() {
        let table = Vector3::new(1.0, 2.0, 3.0).to_value();
        let a = args(vec![table.clone()]);
        assert_eq!(
            a.position(0, "Position").unwrap(),
            Vector3::new(1.0, 2.0, 3.0)
        );

        let array = Value::array([Value::from(4i64), Value::from(5i64), Value::from(6i64)]);
        let b = args(vec![array]);
        assert_eq!(
            b.position(0, "Position").unwrap(),
            Vector3::new(4.0, 5.0, 6.0)
        );

        let bad = args(vec![Value::from("here")]);
        assert!(bad.position(0, "Position").is_err());
    }

    #[test]
    fn colours_accept_the_documented_shapes() {
        let packed = args(vec![Value::from(0xFF0000FFu32 as f64)]);
        let c = packed.color(0, "RGBA").unwrap();
        assert!((c.r - 1.0).abs() < 1e-6);

        let bad = args(vec![Value::from("red")]);
        assert!(bad.color(0, "RGBA").is_err());
    }

    #[test]
    fn arity_check_precedes_dispatch() {
        let a = args(vec![Value::from("Alice")]);
        let err = a.require_arity(3).unwrap_err();
        assert!(matches!(
            err,
            ApiError::TooFewArguments {
                expected: 3,
                got: 1,
                ..
            }
        ));
        assert!(a.require_arity(1).is_ok());
    }

    #[test]
    fn tables_and_booleans() {
        let a = args(vec![
            Value::map([("Coin", Value::from(10i64))]),
            Value::from(false),
        ]);
        assert!(a.table(0, "Properties").unwrap().as_map().is_some());
        assert!(!a.boolean(1, "Enabled").unwrap());
        assert!(a.boolean_or(5, "Visible", true).unwrap());
        assert!(a.table(5, "Props").is_err(), "a table is required here");
        assert!(a.table_or_empty(5, "Props").unwrap().as_map().is_some());
    }

    #[test]
    fn rule_errors_carry_the_call_site() {
        let a = args(vec![]);
        let err = a.rule("cannot trade with yourself");
        let text = err.to_string();
        assert!(text.contains("CurrencyService::Add"), "{text}");
        assert!(text.contains("cannot trade with yourself"), "{text}");
    }
}
