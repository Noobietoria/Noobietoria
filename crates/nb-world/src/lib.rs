/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! # nb-world
//!
//! The instance tree that a place is made of.
//!
//! Every Noobietoria service ultimately operates on this tree: `InstanceService`
//! creates and moves nodes, `UIService` mutates GUI nodes, `NetworkService` reads
//! `NetworkEvent` nodes, and the renderer walks `Workspace` to draw frames.
//!
//! ## Naming and identity
//!
//! The documentation exposes three ways to address an instance, and all three
//! exist here because services are documented against each of them:
//!
//! * **Name** — `"Sword_01"`, the common case. Names are *not* unique.
//! * **Name + Index** — when duplicates exist, "Index là số thứ tự (bắt đầu từ 1)
//!   của Instance trong danh sách các Instance trùng tên, theo thứ tự tạo"
//!   (`spec/source/api.md`, `InstanceService`). Index is therefore creation order.
//! * **GUID** — unique, the escape hatch the documentation recommends when many
//!   instances share a name.
//!
//! ## Containers
//!
//! Placement decides replication: `Workspace` and `ReplicatedStorage` reach
//! clients, `ServerStorage` never does, and `StarterPlayerScripts` only exists
//! client-side. [`World::replicated_snapshot`] implements exactly that rule, so
//! a client cannot observe server-only state even by accident.

pub mod instance;
pub mod toolbox;
pub mod world;
pub mod world_file;

pub use instance::{Instance, InstanceId, InstanceProperties, ScriptKind};
pub use toolbox::{ToolboxAsset, ToolboxCatalog, BUILTIN_CATALOG};
pub use world::{ReplicatedInstance, World, WorldError, WorldResult};
pub use world_file::{PlaceMetadata, PlaceSettings, WorldFile, NBW_MAGIC};
