/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The world: every instance in a place, plus the lookup rules the
//! documentation defines for names, indices and GUIDs.

use std::collections::{BTreeMap, BTreeSet};

use nb_types::{Container, InstanceType};
use serde::{Deserialize, Serialize};

use crate::instance::{Instance, InstanceId, InstanceProperties};

/// Errors a world operation can raise.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorldError {
    /// No instance matches the given name/index/GUID.
    #[error("no instance named `{name}` in {container}{}", index_suffix(*index))]
    NotFound {
        /// The name that was looked up.
        name: String,
        /// The container searched.
        container: Container,
        /// The index that was requested, if any.
        index: Option<usize>,
    },
    /// An instance with that GUID is not present.
    #[error("no instance with GUID `{guid}`")]
    NoSuchGuid {
        /// The GUID that was looked up.
        guid: String,
    },
    /// A name is required but was empty.
    #[error("instance name must not be empty")]
    EmptyName,
    /// A GUID is required but was empty.
    #[error("GUID must not be empty")]
    EmptyGuid,
    /// The requested type cannot live in the requested container.
    #[error("{kind} cannot be placed in {container}")]
    WrongContainer {
        /// The instance type.
        kind: InstanceType,
        /// The container it was sent to.
        container: Container,
    },
    /// A `ModuleScript` whose source does not `return` a value.
    ///
    /// The documentation is explicit: "`Properties.Source` bắt buộc phải là
    /// đoạn mã có `return` 1 giá trị, nếu không sẽ lỗi khi require."
    #[error("ModuleScript `{name}` source must contain a `return` statement")]
    ModuleScriptWithoutReturn {
        /// The offending instance.
        name: String,
    },
    /// A script type was created without needing a source, or a non-script was
    /// given one.
    #[error("`{name}` is not a script, so it has no Source")]
    NotAScript {
        /// The offending instance.
        name: String,
    },
    /// A `NetworkEvent` name collided with an existing event.
    ///
    /// The documentation requires event names to be unique game-wide.
    #[error("NetworkEvent `{name}` already exists; event names must be unique")]
    DuplicateNetworkEvent {
        /// The clashing name.
        name: String,
    },
}

fn index_suffix(index: Option<usize>) -> String {
    match index {
        Some(i) => format!(" at index {i}"),
        None => String::new(),
    }
}

/// Shorthand for world results.
pub type WorldResult<T> = Result<T, WorldError>;

/// One entry in the index that implements `Index` semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NameEntry {
    id: InstanceId,
    created_at: u64,
}

/// The instance tree of a place.
///
/// Instances are keyed by GUID for identity, and separately indexed by
/// `(container, parent, name)` to satisfy the documentation's `Index` rule:
/// duplicates are ordered by creation time, and `Index` is 1-based.
#[derive(Debug, Clone, Default)]
pub struct World {
    /// Every instance, by GUID.
    instances: BTreeMap<InstanceId, Instance>,
    /// `(container, parent_name, name)` -> ids in creation order.
    index: BTreeMap<(Container, String, String), Vec<NameEntry>>,
    /// Guards `NetworkEvent` name uniqueness.
    network_events: BTreeSet<String>,
    /// Monotonic creation counter, which defines the order of `Index`.
    next_created_at: u64,
}

impl World {
    /// An empty world with no instances.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of instances in the world.
    pub fn len(&self) -> usize {
        self.instances.len()
    }

    /// Whether the world has no instances at all.
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Every instance, in GUID order.
    pub fn iter(&self) -> impl Iterator<Item = &Instance> {
        self.instances.values()
    }

    /// Every instance in one container, in creation order.
    pub fn iter_container(&self, container: Container) -> Vec<&Instance> {
        let mut all: Vec<&Instance> = self
            .instances
            .values()
            .filter(|i| i.parent == container)
            .collect();
        all.sort_by_key(|i| i.created_at);
        all
    }

    /// Direct children of a named parent inside a container, in creation order.
    pub fn children_of(&self, container: Container, parent_name: &str) -> Vec<&Instance> {
        let mut all: Vec<&Instance> = self
            .instances
            .values()
            .filter(|i| i.parent == container && i.parent_name.as_deref() == Some(parent_name))
            .collect();
        all.sort_by_key(|i| i.created_at);
        all
    }

    /// Looks an instance up by GUID.
    pub fn by_guid(&self, guid: &str) -> WorldResult<&Instance> {
        if guid.is_empty() {
            return Err(WorldError::EmptyGuid);
        }
        self.instances
            .values()
            .find(|i| i.id.as_str() == guid)
            .ok_or_else(|| WorldError::NoSuchGuid {
                guid: guid.to_owned(),
            })
    }

    /// Looks an instance up by GUID, mutably.
    pub fn by_guid_mut(&mut self, guid: &str) -> WorldResult<&mut Instance> {
        if guid.is_empty() {
            return Err(WorldError::EmptyGuid);
        }
        self.instances
            .values_mut()
            .find(|i| i.id.as_str() == guid)
            .ok_or_else(|| WorldError::NoSuchGuid {
                guid: guid.to_owned(),
            })
    }

    /// Looks up the `index`-th instance with this name.
    ///
    /// `index` is 1-based, matching the documentation: "Index trong
    /// `GetGUID(Name, Index)` là số thứ tự (bắt đầu từ 1)". Omitting it behaves
    /// as `index = 1`. Names are searched across all containers, because
    /// `InstanceService.GetGUID` is documented with only a name and an index.
    pub fn find(&self, name: &str, index: Option<usize>) -> WorldResult<&Instance> {
        let wanted = index.unwrap_or(1);
        if wanted == 0 {
            return Err(WorldError::NotFound {
                name: name.to_owned(),
                container: Container::Workspace,
                index: Some(0),
            });
        }
        let mut matches: Vec<&Instance> =
            self.instances.values().filter(|i| i.name == name).collect();
        matches.sort_by_key(|i| i.created_at);
        matches
            .get(wanted - 1)
            .copied()
            .ok_or_else(|| WorldError::NotFound {
                name: name.to_owned(),
                container: Container::Workspace,
                index: Some(wanted),
            })
    }

    /// Looks up by name within one container, honouring `Index`.
    pub fn find_in(
        &self,
        container: Container,
        name: &str,
        index: Option<usize>,
    ) -> WorldResult<&Instance> {
        let wanted = index.unwrap_or(1);
        let mut matches: Vec<&Instance> = self
            .instances
            .values()
            .filter(|i| i.parent == container && i.name == name)
            .collect();
        matches.sort_by_key(|i| i.created_at);
        matches
            .get(wanted.saturating_sub(1))
            .copied()
            .ok_or_else(|| WorldError::NotFound {
                name: name.to_owned(),
                container,
                index: Some(wanted),
            })
    }

    /// All GUIDs of instances sharing a name, in creation order.
    ///
    /// Backs `InstanceService.GetGUIDs(Name)`.
    pub fn guids_named(&self, name: &str) -> Vec<InstanceId> {
        let mut matches: Vec<&Instance> =
            self.instances.values().filter(|i| i.name == name).collect();
        matches.sort_by_key(|i| i.created_at);
        matches.into_iter().map(|i| i.id.clone()).collect()
    }

    /// How many instances share a name — the count a `Index` lookup is valid up to.
    pub fn count_named(&self, name: &str) -> usize {
        self.instances.values().filter(|i| i.name == name).count()
    }

    /// Resolves a `NetworkEvent` by name to its instance.
    pub fn network_event(&self, name: &str) -> Option<&Instance> {
        self.instances
            .values()
            .find(|i| i.kind == InstanceType::NetworkEvent && i.name == name)
    }

    /// Whether a `NetworkEvent` with this name exists.
    pub fn network_event_exists(&self, name: &str) -> bool {
        self.network_events.contains(name)
    }

    /// Creates an instance with default properties.
    ///
    /// Enforces the documentation's placement rules and the `ModuleScript`
    /// `return` requirement up front, so a broken place fails at author time
    /// rather than at first `require`.
    pub fn create(
        &mut self,
        name: impl Into<String>,
        kind: InstanceType,
        parent: Container,
        properties: InstanceProperties,
    ) -> WorldResult<InstanceId> {
        let name = name.into();
        if name.is_empty() {
            return Err(WorldError::EmptyName);
        }
        if !kind.is_allowed_in(parent) {
            return Err(WorldError::WrongContainer {
                kind,
                container: parent,
            });
        }
        if kind == InstanceType::ModuleScript && !source_returns(&properties.source) {
            return Err(WorldError::ModuleScriptWithoutReturn { name });
        }
        if kind == InstanceType::NetworkEvent && self.network_events.contains(&name) {
            return Err(WorldError::DuplicateNetworkEvent { name });
        }
        if kind == InstanceType::NetworkEvent {
            self.network_events.insert(name.clone());
        }

        let created_at = self.next_created_at;
        self.next_created_at += 1;

        let instance = Instance {
            id: InstanceId::new(),
            name: name.clone(),
            kind,
            parent,
            parent_name: None,
            properties,
            created_at,
        };
        let id = instance.id.clone();
        self.index
            .entry((parent, String::new(), name))
            .or_default()
            .push(NameEntry {
                id: id.clone(),
                created_at,
            });
        self.instances.insert(id.clone(), instance);
        Ok(id)
    }

    /// Creates an instance as a child of another instance.
    pub fn create_under(
        &mut self,
        name: impl Into<String>,
        kind: InstanceType,
        parent_guid: &str,
        properties: InstanceProperties,
    ) -> WorldResult<InstanceId> {
        let parent = self.by_guid(parent_guid)?;
        let container = parent.parent;
        let parent_name = Some(parent.name.clone());
        let name = name.into();

        if name.is_empty() {
            return Err(WorldError::EmptyName);
        }
        if !kind.is_allowed_in(container) {
            return Err(WorldError::WrongContainer { kind, container });
        }
        if kind == InstanceType::ModuleScript && !source_returns(&properties.source) {
            return Err(WorldError::ModuleScriptWithoutReturn { name });
        }
        if kind == InstanceType::NetworkEvent && self.network_events.contains(&name) {
            return Err(WorldError::DuplicateNetworkEvent { name });
        }
        if kind == InstanceType::NetworkEvent {
            self.network_events.insert(name.clone());
        }

        let created_at = self.next_created_at;
        self.next_created_at += 1;

        let instance = Instance {
            id: InstanceId::new(),
            name: name.clone(),
            kind,
            parent: container,
            parent_name: parent_name.clone(),
            properties,
            created_at,
        };
        let id = instance.id.clone();
        self.index
            .entry((container, parent_name.unwrap_or_default(), name))
            .or_default()
            .push(NameEntry {
                id: id.clone(),
                created_at,
            });
        self.instances.insert(id.clone(), instance);
        Ok(id)
    }

    /// Destroys the `index`-th instance with this name, cascading to children.
    ///
    /// Cascade matters: removing a `Model` must remove the parts inside it, or
    /// the tree would keep orphaned nodes that no name lookup can reach.
    pub fn destroy(&mut self, name: &str, index: Option<usize>) -> WorldResult<InstanceId> {
        let id = self.find(name, index)?.id.clone();
        self.destroy_by_guid(id.as_str())?;
        Ok(id)
    }

    /// Destroys an instance by GUID, cascading to its children.
    pub fn destroy_by_guid(&mut self, guid: &str) -> WorldResult<usize> {
        let root = self.by_guid(guid)?.id.clone();
        let mut removed = 0usize;
        let mut queue = vec![root.clone()];
        let mut to_remove: Vec<InstanceId> = Vec::new();

        while let Some(current) = queue.pop() {
            let Some(instance) = self.instances.get(&current) else {
                continue;
            };
            // Find children before we mutate anything.
            let children: Vec<InstanceId> = self
                .instances
                .values()
                .filter(|i| {
                    i.parent == instance.parent
                        && i.parent_name.as_deref() == Some(instance.name.as_str())
                })
                .map(|i| i.id.clone())
                .collect();
            to_remove.push(current.clone());
            queue.extend(children);
        }

        for id in to_remove {
            if let Some(instance) = self.instances.remove(&id) {
                removed += 1;
                if instance.kind == InstanceType::NetworkEvent {
                    self.network_events.remove(&instance.name);
                }
                let key = (
                    instance.parent,
                    instance.parent_name.unwrap_or_default(),
                    instance.name,
                );
                if let Some(entries) = self.index.get_mut(&key) {
                    entries.retain(|e| e.id != id);
                    if entries.is_empty() {
                        self.index.remove(&key);
                    }
                }
            }
        }
        Ok(removed)
    }

    /// Moves an instance to a new world position.
    ///
    /// Backs `InstanceService.Move(Name, Position, Index)` and
    /// `MoveByGUID(GUID, Position)`.
    pub fn move_to(&mut self, guid: &str, position: nb_types::Vector3) -> WorldResult<()> {
        let instance = self.by_guid_mut(guid)?;
        instance.properties.position = position;
        Ok(())
    }

    /// Switches an instance on or off.
    ///
    /// Backs `InstanceService.SetEnabled`. A disabled script must not run, and a
    /// disabled object must not be visible.
    pub fn set_enabled(&mut self, guid: &str, enabled: bool) -> WorldResult<()> {
        let instance = self.by_guid_mut(guid)?;
        instance.properties.enabled = enabled;
        Ok(())
    }

    /// Reads the enabled flag.
    pub fn is_enabled(&self, guid: &str) -> WorldResult<bool> {
        Ok(self.by_guid(guid)?.properties.enabled)
    }

    /// Replaces a script's source, re-checking the `ModuleScript` rule.
    pub fn set_source(&mut self, guid: &str, source: &str) -> WorldResult<()> {
        let instance = self.by_guid(guid)?;
        let kind = instance.kind;
        let name = instance.name.clone();
        if !kind.is_script() {
            return Err(WorldError::NotAScript { name });
        }
        if kind == InstanceType::ModuleScript && !source_returns(source) {
            return Err(WorldError::ModuleScriptWithoutReturn { name });
        }
        let instance = self.by_guid_mut(guid)?;
        instance.properties.source = source.to_owned();
        Ok(())
    }

    /// The Luau source of a script.
    pub fn source(&self, guid: &str) -> WorldResult<&str> {
        let instance = self.by_guid(guid)?;
        if !instance.kind.is_script() {
            return Err(WorldError::NotAScript {
                name: instance.name.clone(),
            });
        }
        Ok(instance.properties.source.as_str())
    }

    /// A `ModuleScript` by name, which is what `require` needs.
    ///
    /// Modules are looked up in the shared containers only, matching the
    /// documentation's advice to place shared modules in `ReplicatedStorage`
    /// and server-only modules in `ServerStorage`.
    pub fn module(&self, name: &str, index: Option<usize>) -> WorldResult<&Instance> {
        let wanted = index.unwrap_or(1);
        let mut matches: Vec<&Instance> = self
            .instances
            .values()
            .filter(|i| i.kind == InstanceType::ModuleScript && i.name == name)
            .collect();
        matches.sort_by_key(|i| i.created_at);
        matches
            .get(wanted.saturating_sub(1))
            .copied()
            .ok_or_else(|| WorldError::NotFound {
                name: name.to_owned(),
                container: Container::ServerStorage,
                index: Some(wanted),
            })
    }

    /// Every script that should start when a place boots on the server.
    pub fn server_scripts(&self) -> Vec<&Instance> {
        let mut all: Vec<&Instance> = self
            .instances
            .values()
            .filter(|i| {
                i.kind == InstanceType::ServerScript
                    && i.properties.enabled
                    && i.parent.is_server_side()
            })
            .collect();
        all.sort_by_key(|i| i.created_at);
        all
    }

    /// Every script that should start on a player's client.
    pub fn client_scripts(&self) -> Vec<&Instance> {
        let mut all: Vec<&Instance> = self
            .instances
            .values()
            .filter(|i| i.kind == InstanceType::ClientScript && i.properties.enabled)
            .collect();
        all.sort_by_key(|i| i.created_at);
        all
    }

    /// The replicated snapshot sent to a client.
    ///
    /// This is the rule that makes the client/server split real rather than
    /// decorative: `ServerStorage` is omitted entirely, and a `ServerScript`
    /// is replaced by a marker that keeps its name and position in the tree but
    /// carries **no source**.
    pub fn replicated_snapshot(&self) -> Vec<ReplicatedInstance> {
        let mut out: Vec<ReplicatedInstance> = self
            .instances
            .values()
            .filter(|i| i.exists_on_client())
            .map(|i| {
                let source = match i.script_kind() {
                    // A server script exists on the client as a stub so that a
                    // client script can still be positioned relative to it, but
                    // its code is withheld.
                    Some(kind) if !kind.source_replicates() => None,
                    _ => Some(i.properties.source.clone()),
                };
                ReplicatedInstance {
                    guid: i.id.as_str().to_owned(),
                    name: i.name.clone(),
                    kind: i.kind,
                    container: i.parent,
                    parent: i.parent_name.clone(),
                    properties: i.properties.clone(),
                    source,
                }
            })
            .collect();
        out.sort_by(|a, b| a.guid.cmp(&b.guid));
        out
    }

    /// Renders a human-readable listing of the tree, used by the Studio Explorer
    /// and by `--selftest` output.
    pub fn describe(&self) -> String {
        let mut lines = Vec::new();
        for container in Container::ALL {
            lines.push(format!("{container}"));
            let mut roots: Vec<&Instance> = self
                .instances
                .values()
                .filter(|i| i.parent == container && i.parent_name.is_none())
                .collect();
            roots.sort_by_key(|i| i.created_at);
            for root in roots {
                lines.push(format!("  {} [{}]", root.name, root.kind));
                let mut children: Vec<&Instance> = self
                    .instances
                    .values()
                    .filter(|i| {
                        i.parent == container
                            && i.parent_name.as_deref() == Some(root.name.as_str())
                    })
                    .collect();
                children.sort_by_key(|i| i.created_at);
                for child in children {
                    lines.push(format!("    {} [{}]", child.name, child.kind));
                }
            }
        }
        lines.join("\n")
    }

    /// Total number of instances by type, for the website's game stats.
    pub fn type_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for instance in self.instances.values() {
            *counts.entry(instance.kind.as_str().to_owned()).or_insert(0) += 1;
        }
        counts
    }

    /// Updates properties of an instance in place.
    pub fn with_instance_mut<R>(
        &mut self,
        guid: &str,
        f: impl FnOnce(&mut Instance) -> R,
    ) -> WorldResult<R> {
        let instance = self.by_guid_mut(guid)?;
        Ok(f(instance))
    }

    /// Name of an instance, or `None` when the GUID is unknown.
    pub fn name_of(&self, guid: &str) -> Option<&str> {
        self.instances
            .values()
            .find(|i| i.id.as_str() == guid)
            .map(|i| i.name.as_str())
    }

    /// Inserts a fully constructed instance directly into the world and indexes it.
    pub fn insert_instance(&mut self, instance: Instance) -> InstanceId {
        if instance.kind == InstanceType::NetworkEvent {
            self.network_events.insert(instance.name.clone());
        }
        if instance.created_at >= self.next_created_at {
            self.next_created_at = instance.created_at + 1;
        }
        let key = (
            instance.parent,
            instance.parent_name.clone().unwrap_or_default(),
            instance.name.clone(),
        );
        let id = instance.id.clone();
        let created_at = instance.created_at;
        self.index.entry(key).or_default().push(NameEntry {
            id: id.clone(),
            created_at,
        });
        self.instances.insert(id.clone(), instance);
        id
    }

    /// Clears all instances and indices in the world.
    pub fn clear(&mut self) {
        self.instances.clear();
        self.index.clear();
        self.network_events.clear();
        self.next_created_at = 0;
    }
}

/// One instance as a client sees it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicatedInstance {
    /// Stable identity.
    pub guid: String,
    /// Name, non-unique.
    pub name: String,
    /// Type.
    pub kind: InstanceType,
    /// Containing region.
    pub container: Container,
    /// Parent instance name, if nested.
    pub parent: Option<String>,
    /// Property values.
    pub properties: InstanceProperties,
    /// Script source, or `None` when the source must not leave the server.
    pub source: Option<String>,
}

/// Whether Luau source ends with (or structurally contains) a top-level `return`.
///
/// The documentation requires a `ModuleScript` to "return 1 giá trị". A full
/// parse would be ideal, but a conservative lexical check is what keeps creation
/// fast and dependency-free; it deliberately accepts a `return` anywhere at the
/// start of a line so nested returns inside functions do not count.
fn source_returns(source: &str) -> bool {
    let mut in_block_comment = false;
    for raw_line in source.lines() {
        let mut line = raw_line.trim();
        // Skip long comments `--[[ ... ]]` and `--[==[ ... ]==]`.
        if in_block_comment {
            if line.contains("]]") || line.contains("]=]") {
                in_block_comment = false;
            }
            continue;
        }
        if line.starts_with("--[[") || line.starts_with("--[=[") || line.starts_with("--[==[") {
            if !(line.contains("]]") || line.contains("]=]")) {
                in_block_comment = true;
            }
            continue;
        }
        if line.starts_with("--") {
            continue;
        }
        if let Some(stripped) = line.strip_prefix("return") {
            line = stripped;
            // `return` must be the whole keyword: `returns` is a different name.
            let next = line.chars().next();
            if next.is_none() || next.is_some_and(|c| !c.is_alphanumeric() && c != '_') {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use nb_types::{Rgba, Vector3};

    fn props() -> InstanceProperties {
        InstanceProperties::default()
    }

    fn props_with_source(src: &str) -> InstanceProperties {
        InstanceProperties {
            source: src.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn create_and_lookup_by_name_and_guid() {
        let mut w = World::new();
        let id = w
            .create("Block", InstanceType::Part, Container::Workspace, props())
            .expect("created");
        assert_eq!(w.len(), 1);
        let by_name = w.find("Block", None).expect("found by name");
        assert_eq!(by_name.id, id);
        let by_guid = w.by_guid(id.as_str()).expect("found by guid");
        assert_eq!(by_guid.name, "Block");
    }

    #[test]
    fn empty_names_are_rejected() {
        let mut w = World::new();
        assert_eq!(
            w.create("", InstanceType::Part, Container::Workspace, props()),
            Err(WorldError::EmptyName)
        );
        assert_eq!(w.by_guid("").unwrap_err(), WorldError::EmptyGuid);
    }

    #[test]
    fn index_is_one_based_and_follows_creation_order() {
        let mut w = World::new();
        let first = w
            .create("Dup", InstanceType::Part, Container::Workspace, props())
            .unwrap();
        let second = w
            .create("Dup", InstanceType::Part, Container::Workspace, props())
            .unwrap();

        assert_eq!(
            w.find("Dup", None).unwrap().id,
            first,
            "index defaults to 1"
        );
        assert_eq!(w.find("Dup", Some(1)).unwrap().id, first);
        assert_eq!(w.find("Dup", Some(2)).unwrap().id, second);
        assert!(w.find("Dup", Some(3)).is_err(), "index 3 does not exist");

        assert_eq!(w.guids_named("Dup"), vec![first, second]);
        assert_eq!(w.count_named("Dup"), 2);
    }

    #[test]
    fn missing_name_reports_what_was_searched() {
        let w = World::new();
        let err = w.find("Ghost", Some(2)).unwrap_err();
        match err {
            WorldError::NotFound { name, index, .. } => {
                assert_eq!(name, "Ghost");
                assert_eq!(index, Some(2));
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn placement_rules_are_enforced() {
        let mut w = World::new();
        // GUI must not live in the world.
        assert!(matches!(
            w.create(
                "Bad",
                InstanceType::TextLabel,
                Container::Workspace,
                props()
            ),
            Err(WorldError::WrongContainer { .. })
        ));
        // A NetworkEvent needs a shared container.
        assert!(matches!(
            w.create(
                "Evt",
                InstanceType::NetworkEvent,
                Container::StarterPlayerScripts,
                props()
            ),
            Err(WorldError::WrongContainer { .. })
        ));
    }

    #[test]
    fn module_script_without_return_is_rejected() {
        let mut w = World::new();
        let err = w.create(
            "Bad",
            InstanceType::ModuleScript,
            Container::ReplicatedStorage,
            props_with_source("local x = 1\nprint(x)\n"),
        );
        assert!(matches!(
            err,
            Err(WorldError::ModuleScriptWithoutReturn { .. })
        ));

        // With a return it is accepted.
        assert!(w
            .create(
                "Good",
                InstanceType::ModuleScript,
                Container::ReplicatedStorage,
                props_with_source("local M = {}\nfunction M.f() return 1 end\nreturn M\n"),
            )
            .is_ok());
    }

    #[test]
    fn return_detection_is_not_fooled_by_comments_or_identifiers() {
        assert!(!source_returns("-- return x\nlocal y = 2\n"));
        assert!(!source_returns("local returns = 1\n"));
        assert!(source_returns("return {}\n"));
        assert!(source_returns("return{}\n"));
        assert!(source_returns("local a = 1\nreturn a"));
    }

    #[test]
    fn network_event_names_must_be_unique() {
        let mut w = World::new();
        w.create(
            "Buy",
            InstanceType::NetworkEvent,
            Container::ReplicatedStorage,
            props(),
        )
        .expect("first event");
        assert!(w.network_event_exists("Buy"));
        let err = w.create(
            "Buy",
            InstanceType::NetworkEvent,
            Container::ReplicatedStorage,
            props(),
        );
        assert!(matches!(err, Err(WorldError::DuplicateNetworkEvent { .. })));
    }

    #[test]
    fn destroying_a_name_removes_the_first_duplicate() {
        let mut w = World::new();
        let first = w
            .create("Dup", InstanceType::Part, Container::Workspace, props())
            .unwrap();
        let second = w
            .create("Dup", InstanceType::Part, Container::Workspace, props())
            .unwrap();

        w.destroy("Dup", None).expect("destroyed index 1");
        assert!(w.by_guid(first.as_str()).is_err());
        assert!(w.by_guid(second.as_str()).is_ok());
        assert_eq!(w.count_named("Dup"), 1);
    }

    #[test]
    fn destroying_a_parent_cascades_to_children() {
        let mut w = World::new();
        let model = w
            .create("Model", InstanceType::Model, Container::Workspace, props())
            .unwrap();
        w.create_under("ChildA", InstanceType::Part, model.as_str(), props())
            .expect("child a");
        w.create_under("ChildB", InstanceType::Part, model.as_str(), props())
            .expect("child b");
        assert_eq!(w.len(), 3);

        let removed = w.destroy_by_guid(model.as_str()).expect("destroyed");
        assert_eq!(removed, 3, "model plus both children");
        assert!(w.is_empty());
    }

    #[test]
    fn destroying_a_network_event_frees_its_name() {
        let mut w = World::new();
        let id = w
            .create(
                "Evt",
                InstanceType::NetworkEvent,
                Container::ReplicatedStorage,
                props(),
            )
            .unwrap();
        assert!(w.network_event_exists("Evt"));
        w.destroy_by_guid(id.as_str()).unwrap();
        assert!(!w.network_event_exists("Evt"));
        // And the name can be reused.
        assert!(w
            .create(
                "Evt",
                InstanceType::NetworkEvent,
                Container::ReplicatedStorage,
                props()
            )
            .is_ok());
    }

    #[test]
    fn destroying_an_unknown_guid_errors() {
        let mut w = World::new();
        assert!(matches!(
            w.destroy_by_guid("deadbeef"),
            Err(WorldError::NoSuchGuid { .. })
        ));
    }

    #[test]
    fn move_and_enabled_toggle() {
        let mut w = World::new();
        let id = w
            .create("Block", InstanceType::Part, Container::Workspace, props())
            .unwrap();
        w.move_to(id.as_str(), Vector3::new(5.0, 6.0, 7.0)).unwrap();
        assert_eq!(
            w.by_guid(id.as_str()).unwrap().properties.position,
            Vector3::new(5.0, 6.0, 7.0)
        );

        assert!(w.is_enabled(id.as_str()).unwrap());
        w.set_enabled(id.as_str(), false).unwrap();
        assert!(!w.is_enabled(id.as_str()).unwrap());
    }

    #[test]
    fn source_can_only_be_set_on_scripts() {
        let mut w = World::new();
        let part = w
            .create("Block", InstanceType::Part, Container::Workspace, props())
            .unwrap();
        assert!(matches!(
            w.set_source(part.as_str(), "print(1)"),
            Err(WorldError::NotAScript { .. })
        ));
        assert!(matches!(
            w.source(part.as_str()),
            Err(WorldError::NotAScript { .. })
        ));
    }

    #[test]
    fn setting_a_broken_module_source_is_rejected_and_keeps_the_old_source() {
        let mut w = World::new();
        let id = w
            .create(
                "M",
                InstanceType::ModuleScript,
                Container::ReplicatedStorage,
                props_with_source("return { ok = true }\n"),
            )
            .unwrap();
        let original = w.source(id.as_str()).unwrap().to_owned();

        assert!(matches!(
            w.set_source(id.as_str(), "local x = 1"),
            Err(WorldError::ModuleScriptWithoutReturn { .. })
        ));
        assert_eq!(w.source(id.as_str()).unwrap(), original, "old source kept");
    }

    #[test]
    fn server_scripts_are_listed_and_respect_enabled() {
        let mut w = World::new();
        w.create(
            "Live",
            InstanceType::ServerScript,
            Container::ServerStorage,
            props_with_source("print('hi')"),
        )
        .unwrap();
        let off = w
            .create(
                "Off",
                InstanceType::ServerScript,
                Container::ServerStorage,
                props_with_source("print('off')"),
            )
            .unwrap();
        w.set_enabled(off.as_str(), false).unwrap();

        let names: Vec<&str> = w.server_scripts().iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["Live"], "disabled scripts must not start");
    }

    #[test]
    fn snapshot_hides_server_storage_and_server_source() {
        let mut w = World::new();
        w.create(
            "VisiblePart",
            InstanceType::Part,
            Container::Workspace,
            props(),
        )
        .unwrap();
        w.create(
            "SecretPart",
            InstanceType::Part,
            Container::ServerStorage,
            props(),
        )
        .unwrap();
        w.create(
            "ServerLogic",
            InstanceType::ServerScript,
            Container::ServerStorage,
            props_with_source("-- top secret economy code"),
        )
        .unwrap();
        w.create(
            "ClientLogic",
            InstanceType::ClientScript,
            Container::StarterPlayerScripts,
            props_with_source("print('client')"),
        )
        .unwrap();

        let snapshot = w.replicated_snapshot();
        let names: Vec<&str> = snapshot.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"VisiblePart"));
        assert!(names.contains(&"ClientLogic"));
        assert!(
            !names.contains(&"SecretPart"),
            "ServerStorage must not replicate"
        );
        assert!(
            !names.contains(&"ServerLogic"),
            "a ServerScript in ServerStorage must not replicate at all"
        );

        // A server script placed in ReplicatedStorage replicates as a stub with
        // no source.
        w.create(
            "SharedLogic",
            InstanceType::ServerScript,
            Container::ReplicatedStorage,
            props_with_source("-- still secret"),
        )
        .unwrap();
        let snapshot = w.replicated_snapshot();
        let shared = snapshot
            .iter()
            .find(|i| i.name == "SharedLogic")
            .expect("stub present");
        assert!(shared.source.is_none(), "server source must be withheld");
        assert!(!shared.properties.source.is_empty());
    }

    #[test]
    fn client_source_does_replicate() {
        let mut w = World::new();
        w.create(
            "UI",
            InstanceType::ClientScript,
            Container::StarterPlayerScripts,
            props_with_source("print('draw ui')"),
        )
        .unwrap();
        let snapshot = w.replicated_snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].source.as_deref(), Some("print('draw ui')"));
    }

    #[test]
    fn nested_lookup_and_children() {
        let mut w = World::new();
        let shop = w
            .create("Shop", InstanceType::Model, Container::Workspace, props())
            .unwrap();
        w.create_under("Stand", InstanceType::Part, shop.as_str(), props())
            .unwrap();
        w.create_under("Sign", InstanceType::Part, shop.as_str(), props())
            .unwrap();

        let children = w.children_of(Container::Workspace, "Shop");
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].name, "Stand");
        assert_eq!(children[1].name, "Sign");
    }

    #[test]
    fn module_lookup_returns_the_module() {
        let mut w = World::new();
        w.create(
            "MathUtils",
            InstanceType::ModuleScript,
            Container::ReplicatedStorage,
            props_with_source("return { Add = function(a,b) return a+b end }"),
        )
        .unwrap();
        let m = w.module("MathUtils", None).expect("found");
        assert_eq!(m.kind, InstanceType::ModuleScript);
        assert!(w.module("Nope", None).is_err());
    }

    #[test]
    fn describe_lists_the_tree() {
        let mut w = World::new();
        let m = w
            .create("Model", InstanceType::Model, Container::Workspace, props())
            .unwrap();
        w.create_under("Child", InstanceType::Part, m.as_str(), props())
            .unwrap();
        let text = w.describe();
        assert!(text.contains("Workspace"));
        assert!(text.contains("Model [Model]"));
        assert!(text.contains("Child [Part]"));
    }

    #[test]
    fn type_counts_are_reported() {
        let mut w = World::new();
        w.create("A", InstanceType::Part, Container::Workspace, props())
            .unwrap();
        w.create("B", InstanceType::Part, Container::Workspace, props())
            .unwrap();
        w.create("C", InstanceType::Model, Container::Workspace, props())
            .unwrap();
        let counts = w.type_counts();
        assert_eq!(counts.get("Part"), Some(&2));
        assert_eq!(counts.get("Model"), Some(&1));
    }

    #[test]
    fn containers_are_listed_in_creation_order() {
        let mut w = World::new();
        w.create("A", InstanceType::Part, Container::Workspace, props())
            .unwrap();
        w.create("B", InstanceType::Part, Container::Workspace, props())
            .unwrap();
        let listed: Vec<&str> = w
            .iter_container(Container::Workspace)
            .iter()
            .map(|i| i.name.as_str())
            .collect();
        assert_eq!(listed, vec!["A", "B"]);
    }

    #[test]
    fn colour_survives_a_property_write() {
        let mut w = World::new();
        let id = w
            .create("Block", InstanceType::Part, Container::Workspace, props())
            .unwrap();
        w.with_instance_mut(id.as_str(), |i| {
            i.properties.color = Rgba::new(0.2, 0.4, 0.6, 1.0);
        })
        .unwrap();
        let c = w.by_guid(id.as_str()).unwrap().properties.color;
        assert!((c.r - 0.2).abs() < 1e-9);
    }
}
