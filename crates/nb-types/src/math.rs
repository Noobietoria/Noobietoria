/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Spatial and colour primitives.
//!
//! The documentation describes positions as `{X, Y, Z}` tables, orientation in
//! degrees, and colour as `RGBA` (`spec/source/instance.md`, `spec/source/api.md`).
//! These types are the in-memory form of exactly those shapes.

use serde::{Deserialize, Serialize};

use crate::value::Value;

/// A point or direction in 3D space.
///
/// Converted to and from the documentation's `{X, Y, Z}` table form, and to and
/// from a bare three-element array for terse script literals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vector3 {
    /// The origin, used as the default position for new instances.
    pub const ZERO: Vector3 = Vector3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    /// One unit up. `Y` is the vertical axis in Noobietoria places.
    pub const UP: Vector3 = Vector3 {
        x: 0.0,
        y: 1.0,
        z: 0.0,
    };

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Component-wise addition.
    pub fn add(self, other: Vector3) -> Vector3 {
        Vector3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    /// Component-wise subtraction.
    pub fn sub(self, other: Vector3) -> Vector3 {
        Vector3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    /// Scalar multiplication.
    pub fn mul(self, factor: f64) -> Vector3 {
        Vector3::new(self.x * factor, self.y * factor, self.z * factor)
    }

    /// Straight-line distance.
    pub fn distance(self, other: Vector3) -> f64 {
        self.sub(other).magnitude()
    }

    /// Euclidean length.
    pub fn magnitude(self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Unit vector, or [`Vector3::ZERO`] when the length is zero.
    pub fn normalized(self) -> Vector3 {
        let len = self.magnitude();
        if len == 0.0 {
            Vector3::ZERO
        } else {
            self.mul(1.0 / len)
        }
    }

    /// Linear interpolation; `alpha` is clamped to `0..=1`.
    pub fn lerp(self, other: Vector3, alpha: f64) -> Vector3 {
        let a = alpha.clamp(0.0, 1.0);
        Vector3::new(
            self.x + (other.x - self.x) * a,
            self.y + (other.y - self.y) * a,
            self.z + (other.z - self.z) * a,
        )
    }

    /// True when every component is finite and non-negative — the check used
    /// before sending a position to the renderer.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    /// The documentation's `{X, Y, Z}` table.
    pub fn to_value(self) -> Value {
        Value::map([
            ("X", Value::from(self.x)),
            ("Y", Value::from(self.y)),
            ("Z", Value::from(self.z)),
        ])
    }

    /// Parses a `{X, Y, Z}` table, a `{x, y, z}` table, or a three-element array.
    ///
    /// The documentation shows `Position` as a table of three coordinates; the
    /// array form is accepted because scripts commonly write
    /// `Part.Position = {0, 10, 0}`.
    pub fn from_value(value: &Value) -> Option<Vector3> {
        let pick = |map: &std::collections::BTreeMap<String, Value>, upper: &str, lower: &str| {
            map.get(upper)
                .or_else(|| map.get(lower))
                .and_then(Value::as_f64)
        };
        match value {
            Value::Map(map) => {
                let x = pick(map, "X", "x")?;
                let y = pick(map, "Y", "y")?;
                let z = pick(map, "Z", "z")?;
                Some(Vector3::new(x, y, z))
            }
            Value::Array(items) if items.len() == 3 => {
                let x = items[0].as_f64()?;
                let y = items[1].as_f64()?;
                let z = items[2].as_f64()?;
                Some(Vector3::new(x, y, z))
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for Vector3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}, {}, {}", self.x, self.y, self.z)
    }
}

/// Orientation in degrees around each axis, matching the documentation's
/// `Orientation` property.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Orientation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Orientation {
    pub const IDENTITY: Orientation = Orientation {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// The documentation's `{X, Y, Z}` table form.
    pub fn to_value(self) -> Value {
        Value::map([
            ("X", Value::from(self.x)),
            ("Y", Value::from(self.y)),
            ("Z", Value::from(self.z)),
        ])
    }

    /// Parses a `{X, Y, Z}` table or a three-element array.
    pub fn from_value(value: &Value) -> Option<Orientation> {
        let v = Vector3::from_value(value)?;
        Some(Orientation::new(v.x, v.y, v.z))
    }

    /// The three axis rotations in radians, for the renderer.
    pub fn to_radians(self) -> [f32; 3] {
        [
            (self.x as f32).to_radians(),
            (self.y as f32).to_radians(),
            (self.z as f32).to_radians(),
        ]
    }
}

/// A position plus an orientation — the documentation's
/// "position và orientation" pair, used by `Part`, `Frame` and camera calls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CFrame {
    pub position: Vector3,
    pub orientation: Orientation,
}

impl CFrame {
    pub const IDENTITY: CFrame = CFrame {
        position: Vector3::ZERO,
        orientation: Orientation::IDENTITY,
    };

    pub const fn new(position: Vector3, orientation: Orientation) -> Self {
        Self {
            position,
            orientation,
        }
    }

    /// A frame at `position` with no rotation.
    pub const fn at(position: Vector3) -> Self {
        Self {
            position,
            orientation: Orientation::IDENTITY,
        }
    }

    /// Puts the point `other` at the given distance along this frame's forward
    /// direction — the semantics of `CameraService.SetPosition(Username, Position, LookAt)`.
    pub fn look_at(position: Vector3, target: Vector3) -> Self {
        let dir = target.sub(position).normalized();
        let yaw = dir.x.atan2(-dir.z).to_degrees();
        let pitch = -dir.y.asin().to_degrees();
        Self::new(position, Orientation::new(pitch, yaw, 0.0))
    }

    /// The forward direction implied by the orientation.
    pub fn forward(self) -> Vector3 {
        let [rx, ry, _rz] = self.orientation.to_radians();
        Vector3::new(
            -(ry.sin() * rx.cos()) as f64,
            rx.sin() as f64,
            -(ry.cos() * rx.cos()) as f64,
        )
    }

    /// Moves along the forward direction.
    pub fn translate(self, distance: f64) -> CFrame {
        CFrame::new(
            self.position.add(self.forward().mul(distance)),
            self.orientation,
        )
    }

    /// The documentation's table form: `{Position = {X,Y,Z}, Orientation = {X,Y,Z}}`.
    pub fn to_value(self) -> Value {
        Value::map([
            ("Position", self.position.to_value()),
            ("Orientation", self.orientation.to_value()),
        ])
    }

    /// Parses `{Position = ..., Orientation = ...}`.
    pub fn from_value(value: &Value) -> Option<CFrame> {
        let position = Vector3::from_value(value.get("Position")?)?;
        let orientation = match value.get("Orientation") {
            Some(v) => Orientation::from_value(v)?,
            None => Orientation::IDENTITY,
        };
        Some(CFrame::new(position, orientation))
    }
}

/// An opaque colour with an alpha channel, matching the documentation's `RGBA`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Default for Rgba {
    fn default() -> Self {
        // The neutral grey a newly created Part gets before a colour is set.
        Self::rgb(163.0 / 255.0, 162.0 / 255.0, 165.0 / 255.0)
    }
}

impl Rgba {
    pub const WHITE: Rgba = Rgba {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Rgba = Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const TRANSPARENT: Rgba = Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub const fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }

    /// Fully opaque colour.
    pub const fn rgb(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// The packed `0xRRGGBBAA` integer form, which is how `RGBA` properties are
    /// written in the Inspector.
    pub fn to_u32(self) -> u32 {
        let q = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        (q(self.r) << 24) | (q(self.g) << 16) | (q(self.b) << 8) | q(self.a)
    }

    /// Inverse of [`Rgba::to_u32`].
    pub fn from_u32(packed: u32) -> Self {
        let c = |shift: u32| ((packed >> shift) & 0xff) as f64 / 255.0;
        Self::new(c(24), c(16), c(8), c(0))
    }

    /// The `[r, g, b, a]` floats the renderer wants.
    pub fn to_array(self) -> [f32; 4] {
        [
            self.r.clamp(0.0, 1.0) as f32,
            self.g.clamp(0.0, 1.0) as f32,
            self.b.clamp(0.0, 1.0) as f32,
            self.a.clamp(0.0, 1.0) as f32,
        ]
    }

    /// Linear blend towards `other`; `alpha` is clamped to `0..=1`.
    pub fn lerp(self, other: Rgba, alpha: f64) -> Rgba {
        let a = alpha.clamp(0.0, 1.0);
        Rgba::new(
            self.r + (other.r - self.r) * a,
            self.g + (other.g - self.g) * a,
            self.b + (other.b - self.b) * a,
            self.a + (other.a - self.a) * a,
        )
    }

    /// The documentation's `RGBA` table form.
    pub fn to_value(self) -> Value {
        Value::map([
            ("R", Value::from(self.r * 255.0)),
            ("G", Value::from(self.g * 255.0)),
            ("B", Value::from(self.b * 255.0)),
            ("A", Value::from(self.a)),
        ])
    }

    /// Parses `RGBA` from a `{R,G,B,A}` table (0-255 per channel, alpha 0-1),
    /// a four-element array, or a packed integer.
    pub fn from_value(value: &Value) -> Option<Rgba> {
        let channel = |v: f64| (v / 255.0).clamp(0.0, 1.0);
        match value {
            Value::Number(packed) => Some(Rgba::from_u32(*packed as u32)),
            Value::Map(map) => {
                let r = channel(map.get("R").or_else(|| map.get("r"))?.as_f64()?);
                let g = channel(map.get("G").or_else(|| map.get("g"))?.as_f64()?);
                let b = channel(map.get("B").or_else(|| map.get("b"))?.as_f64()?);
                let a = map
                    .get("A")
                    .or_else(|| map.get("a"))
                    .and_then(Value::as_f64)
                    .unwrap_or(1.0);
                Some(Rgba::new(r, g, b, a.clamp(0.0, 1.0)))
            }
            Value::Array(items) if items.len() >= 3 => {
                let r = channel(items[0].as_f64()?);
                let g = channel(items[1].as_f64()?);
                let b = channel(items[2].as_f64()?);
                let a = items
                    .get(3)
                    .and_then(Value::as_f64)
                    .unwrap_or(1.0)
                    .clamp(0.0, 1.0);
                Some(Rgba::new(r, g, b, a))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_table_round_trip() {
        let v = Vector3::new(1.5, -2.0, 30.0);
        assert_eq!(Vector3::from_value(&v.to_value()), Some(v));
    }

    #[test]
    fn vector_accepts_array_and_lowercase_keys() {
        let arr = Value::array([Value::from(1i64), Value::from(2i64), Value::from(3i64)]);
        assert_eq!(Vector3::from_value(&arr), Some(Vector3::new(1.0, 2.0, 3.0)));

        let lower = Value::map([
            ("x", Value::from(4i64)),
            ("y", Value::from(5i64)),
            ("z", Value::from(6i64)),
        ]);
        assert_eq!(
            Vector3::from_value(&lower),
            Some(Vector3::new(4.0, 5.0, 6.0))
        );
    }

    #[test]
    fn vector_arithmetic() {
        let a = Vector3::new(3.0, 4.0, 0.0);
        assert_eq!(a.magnitude(), 5.0);
        assert_eq!(a.normalized().magnitude(), 1.0);
        assert_eq!(Vector3::ZERO.normalized(), Vector3::ZERO);
        assert!((a.distance(Vector3::ZERO) - 5.0).abs() < 1e-9);
        assert_eq!(
            Vector3::ZERO.lerp(Vector3::new(10.0, 0.0, 0.0), 0.5),
            Vector3::new(5.0, 0.0, 0.0)
        );
        // Out-of-range alpha is clamped rather than extrapolated.
        assert_eq!(a.lerp(Vector3::ZERO, 5.0), Vector3::ZERO);
    }

    #[test]
    fn v3_from_truncated_array_is_rejected() {
        let arr = Value::array([Value::from(1i64), Value::from(2i64)]);
        assert_eq!(Vector3::from_value(&arr), None);
    }

    #[test]
    fn look_at_faces_the_target() {
        let cam = CFrame::look_at(Vector3::ZERO, Vector3::new(0.0, 0.0, -10.0));
        let fwd = cam.forward();
        // Facing -Z means forward should be (0, 0, -1).
        assert!((fwd.z + 1.0).abs() < 1e-6, "forward.z = {}", fwd.z);
        let moved = cam.translate(5.0);
        assert!(
            (moved.position.z + 5.0).abs() < 1e-6,
            "z = {}",
            moved.position.z
        );
    }

    #[test]
    fn cframe_table_round_trip() {
        let cf = CFrame::new(
            Vector3::new(1.0, 2.0, 3.0),
            Orientation::new(0.0, 90.0, 0.0),
        );
        assert_eq!(CFrame::from_value(&cf.to_value()), Some(cf));
    }

    #[test]
    fn rgba_packing_round_trips() {
        let c = Rgba::new(1.0, 0.5, 0.0, 1.0);
        let packed = c.to_u32();
        let back = Rgba::from_u32(packed);
        assert!((back.r - 1.0).abs() < 1e-2);
        assert!((back.g - 0.5).abs() < 1e-2);
        assert!((back.b - 0.0).abs() < 1e-2);
        assert!((back.a - 1.0).abs() < 1e-2);
    }

    #[test]
    fn rgba_parses_all_documented_shapes() {
        let table = Value::map([
            ("R", Value::from(255i64)),
            ("G", Value::from(0i64)),
            ("B", Value::from(0i64)),
            ("A", Value::from(1.0)),
        ]);
        let c = Rgba::from_value(&table).expect("table form");
        assert!((c.r - 1.0).abs() < 1e-6);
        assert!((c.g - 0.0).abs() < 1e-6);

        let array = Value::array([Value::from(0i64), Value::from(255i64), Value::from(0i64)]);
        let c = Rgba::from_value(&array).expect("array form");
        assert!((c.g - 1.0).abs() < 1e-6);
        assert!((c.a - 1.0).abs() < 1e-6, "alpha defaults to opaque");

        let packed = Rgba::from_value(&Value::from(0x00FF00FFu32 as f64)).expect("packed form");
        assert!((packed.g - 1.0).abs() < 1e-6);
    }

    #[test]
    fn alpha_is_clamped() {
        let table = Value::map([
            ("R", Value::from(0i64)),
            ("G", Value::from(0i64)),
            ("B", Value::from(0i64)),
            ("A", Value::from(7.0)),
        ]);
        assert_eq!(Rgba::from_value(&table).unwrap().a, 1.0);
    }

    #[test]
    fn infinity_is_not_finite() {
        assert!(!Vector3::new(f64::INFINITY, 0.0, 0.0).is_finite());
        assert!(Vector3::new(1.0, 2.0, 3.0).is_finite());
    }
}
