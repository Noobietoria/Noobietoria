/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! # nb-types
//!
//! The value, math and instance-type primitives shared by every Noobietoria
//! component. Nothing here knows about the network, the renderer or scripts;
//! it is the vocabulary the rest of the workspace speaks.
//!
//! The three concerns map onto the three modules:
//!
//! * [`value`] — the closed set of serialisable values that may cross an API
//!   boundary, mirroring the documentation's "must be serialisable" rule.
//! * [`math`] — `Vector3`, `Orientation`, `CFrame` and `Rgba`, in the table
//!   shapes the documentation uses.
//! * [`instance`] — the creatable instance types and the containers they live
//!   in, including which of those replicate to clients.
//!
//! ```
//! use nb_types::{InstanceType, Rgba, Value, Vector3};
//!
//! let position = Vector3::new(0.0, 12.0, -8.0);
//! assert_eq!(Vector3::from_value(&position.to_value()), Some(position));
//!
//! assert_eq!(InstanceType::NetworkEvent.default_container().as_str(), "ReplicatedStorage");
//! assert!(!nb_types::Container::ServerStorage.is_replicated());
//! assert!((Rgba::from_u32(0xFF8800FF).g - 0.533).abs() < 0.01);
//! assert_eq!(Value::from(3i64).as_i64(), Some(3));
//! ```

pub mod avatar;
pub mod instance;
pub mod math;
pub mod value;

pub use avatar::{default_catalog, AvatarCategory, AvatarConfig, CatalogItem};
pub use instance::{Container, InstanceType};
pub use math::{CFrame, Orientation, Rgba, Vector3};
pub use value::Value;

/// The version string every component reports, so a Studio, a Client and a
/// Dedicated server can refuse to talk to an incompatible peer.
pub const PROTOCOL_VERSION: u32 = 1;

/// Default port for a Noobietoria Dedicated server.
pub const DEFAULT_SERVER_PORT: u16 = 3480;

/// Default port for the Noobietoria website.
pub const DEFAULT_WEB_PORT: u16 = 3481;
