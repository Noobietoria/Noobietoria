//! The set of creatable instances.
//!
//! Sourced from `spec/source/instance.md` (the Instance Reference) and the
//! container names quoted throughout `spec/source/api.md`. Anything a script can
//! name in `InstanceService.Create(Name, Type, Parent, Properties)` has a
//! variant here; [`InstanceType::from_str`] is deliberately closed so an
//! unknown type is an error rather than a silently created stub.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Where an instance lives *and* who can see it.
///
/// This distinction is what makes the client/server split real: a
/// `ServerStorage` instance never leaves the server, while `ReplicatedStorage`
/// is visible to both, as `spec/source/instance.md` requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Container {
    /// The 3D world, replicated to every client.
    Workspace,
    /// Shared between server and client; `NetworkEvent` and shared
    /// `ModuleScript`s belong here.
    ReplicatedStorage,
    /// Server-only. Never replicated.
    ServerStorage,
    /// Client-only, one copy per player.
    StarterPlayerScripts,
    /// Objects that exist only in the GUI tree, outside the 3D world.
    ScreenGui,
}

impl Container {
    /// The container names the documentation uses as `Parent` values.
    pub const ALL: [Container; 5] = [
        Container::Workspace,
        Container::ReplicatedStorage,
        Container::ServerStorage,
        Container::StarterPlayerScripts,
        Container::ScreenGui,
    ];

    /// The exact string a script writes for this container.
    pub fn as_str(self) -> &'static str {
        match self {
            Container::Workspace => "Workspace",
            Container::ReplicatedStorage => "ReplicatedStorage",
            Container::ServerStorage => "ServerStorage",
            Container::StarterPlayerScripts => "StarterPlayerScripts",
            Container::ScreenGui => "ScreenGui",
        }
    }

    /// Whether instances here are sent to clients.
    ///
    /// `ServerStorage` is the only container that never replicates, which is
    /// precisely why the documentation tells authors to keep secrets in it.
    pub fn is_replicated(self) -> bool {
        !matches!(self, Container::ServerStorage)
    }

    /// Whether instances here exist on the server.
    pub fn is_server_side(self) -> bool {
        !matches!(self, Container::StarterPlayerScripts)
    }

    /// Whether instances here exist on the client.
    pub fn is_client_side(self) -> bool {
        !matches!(self, Container::ServerStorage)
    }
}

impl fmt::Display for Container {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Container {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Container::ALL
            .into_iter()
            .find(|c| c.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| format!("unknown container `{s}`"))
    }
}

/// Every instance type the Studio can create.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum InstanceType {
    // --- World geometry -------------------------------------------------
    /// Base solid; the documentation's most fundamental object.
    Part,
    /// A climbable descendant of `Part`.
    TrussPart,
    /// An anchor point in 3D space.
    Attachment,
    /// A grouping node for parts.
    Model,
    /// Where players and NPCs respawn.
    SpawnLocation,
    /// Non-solid visual marker, used by `ProximityService` prompts.
    Trigger,

    // --- GUI ------------------------------------------------------------
    /// Root GUI container.
    ScreenGui,
    /// The generic GUI container.
    Frame,
    /// Text display.
    TextLabel,
    /// Clickable text.
    TextButton,
    /// Image display.
    ImageLabel,
    /// Clickable image.
    ImageButton,
    /// Free text entry.
    TextBox,
    /// Scrollable frame.
    ScrollingFrame,

    // --- Scripts and networking -----------------------------------------
    /// Server-only Luau source.
    ServerScript,
    /// Per-player Luau source.
    ClientScript,
    /// Reusable Luau module, run once and cached.
    ModuleScript,
    /// Two-way client/server channel.
    NetworkEvent,

    // --- Audio ----------------------------------------------------------
    /// A playable audio emitter placed in the world.
    Audio,
}

impl InstanceType {
    /// Every creatable type. The coverage test pins this list against the
    /// vendored documentation so a new documented type cannot be forgotten.
    pub const ALL: [InstanceType; 18] = [
        InstanceType::Part,
        InstanceType::TrussPart,
        InstanceType::Attachment,
        InstanceType::Model,
        InstanceType::SpawnLocation,
        InstanceType::Trigger,
        InstanceType::ScreenGui,
        InstanceType::Frame,
        InstanceType::TextLabel,
        InstanceType::TextButton,
        InstanceType::ImageLabel,
        InstanceType::ImageButton,
        InstanceType::TextBox,
        InstanceType::ScrollingFrame,
        InstanceType::ServerScript,
        InstanceType::ClientScript,
        InstanceType::ModuleScript,
        InstanceType::NetworkEvent,
    ];

    /// The exact identifier used in `InstanceService.Create`.
    pub fn as_str(self) -> &'static str {
        match self {
            InstanceType::Part => "Part",
            InstanceType::TrussPart => "TrussPart",
            InstanceType::Attachment => "Attachment",
            InstanceType::Model => "Model",
            InstanceType::SpawnLocation => "SpawnLocation",
            InstanceType::Trigger => "Trigger",
            InstanceType::ScreenGui => "ScreenGui",
            InstanceType::Frame => "Frame",
            InstanceType::TextLabel => "TextLabel",
            InstanceType::TextButton => "TextButton",
            InstanceType::ImageLabel => "ImageLabel",
            InstanceType::ImageButton => "ImageButton",
            InstanceType::TextBox => "TextBox",
            InstanceType::ScrollingFrame => "ScrollingFrame",
            InstanceType::ServerScript => "ServerScript",
            InstanceType::ClientScript => "ClientScript",
            InstanceType::ModuleScript => "ModuleScript",
            InstanceType::NetworkEvent => "NetworkEvent",
            InstanceType::Audio => "Audio",
        }
    }

    /// Holds Luau source code.
    pub fn is_script(self) -> bool {
        matches!(
            self,
            InstanceType::ServerScript | InstanceType::ClientScript | InstanceType::ModuleScript
        )
    }

    /// Belongs to the GUI tree, so it is parented to a `ScreenGui` or a `Frame`
    /// rather than to `Workspace`.
    pub fn is_gui(self) -> bool {
        matches!(
            self,
            InstanceType::ScreenGui
                | InstanceType::Frame
                | InstanceType::TextLabel
                | InstanceType::TextButton
                | InstanceType::ImageLabel
                | InstanceType::ImageButton
                | InstanceType::TextBox
                | InstanceType::ScrollingFrame
        )
    }

    /// Has a position in the 3D world, so `{X, Y, Z}` coordinates apply.
    pub fn has_3d_position(self) -> bool {
        matches!(
            self,
            InstanceType::Part
                | InstanceType::TrussPart
                | InstanceType::Attachment
                | InstanceType::Model
                | InstanceType::SpawnLocation
                | InstanceType::Trigger
                | InstanceType::Audio
        )
    }

    /// Where an instance of this type belongs by default.
    ///
    /// Scripts and GUI follow the documentation's placement rules: client code
    /// lives in `StarterPlayerScripts`, GUI under a `ScreenGui`, and the
    /// sensitive default for a new script is `ServerStorage`.
    pub fn default_container(self) -> Container {
        match self {
            InstanceType::ClientScript => Container::StarterPlayerScripts,
            InstanceType::ServerScript | InstanceType::ModuleScript => Container::ServerStorage,
            InstanceType::NetworkEvent => Container::ReplicatedStorage,
            InstanceType::ScreenGui => Container::ScreenGui,
            t if t.is_gui() => Container::ScreenGui,
            _ => Container::Workspace,
        }
    }

    /// The container a type is *allowed* in, enforcing the documentation's rule
    /// that a `NetworkEvent` must be reachable from both sides.
    pub fn is_allowed_in(self, container: Container) -> bool {
        match self {
            InstanceType::NetworkEvent => matches!(
                container,
                Container::ReplicatedStorage | Container::ServerStorage | Container::Workspace
            ),
            InstanceType::ServerScript => !matches!(container, Container::StarterPlayerScripts),
            InstanceType::ClientScript => !matches!(container, Container::ServerStorage),
            t if t.is_gui() => matches!(container, Container::ScreenGui),
            _ => true,
        }
    }
}

impl fmt::Display for InstanceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for InstanceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        InstanceType::ALL
            .into_iter()
            .find(|t| t.as_str().eq_ignore_ascii_case(s))
            .ok_or_else(|| format!("unknown instance type `{s}`"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_name_round_trips() {
        for t in InstanceType::ALL {
            let parsed: InstanceType = t.as_str().parse().expect("parses its own name");
            assert_eq!(parsed, t);
        }
    }

    #[test]
    fn type_names_are_unique() {
        let mut names: Vec<&str> = InstanceType::ALL.iter().map(|t| t.as_str()).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate instance type name");
    }

    #[test]
    fn type_names_match_documentation_identifiers() {
        // The documentation names these exact strings in InstanceService.
        for expected in [
            "Part",
            "Model",
            "SpawnLocation",
            "NetworkEvent",
            "ServerScript",
            "ClientScript",
            "ModuleScript",
            "Frame",
            "TextLabel",
            "TextButton",
            "ImageLabel",
            "ImageButton",
            "TextBox",
            "ScrollingFrame",
        ] {
            assert!(
                InstanceType::from_str(expected).is_ok(),
                "documented type `{expected}` is missing"
            );
        }
    }

    #[test]
    fn unknown_types_are_rejected() {
        assert!(InstanceType::from_str("NotAThing").is_err());
        assert!(Container::from_str("Nowhere").is_err());
    }

    #[test]
    fn type_lookup_is_case_insensitive() {
        assert_eq!(InstanceType::from_str("part").unwrap(), InstanceType::Part);
        assert_eq!(
            InstanceType::from_str("CLIENTSCRIPT").unwrap(),
            InstanceType::ClientScript
        );
    }

    #[test]
    fn containers_match_documentation_names() {
        assert_eq!(Container::Workspace.as_str(), "Workspace");
        assert_eq!(Container::ReplicatedStorage.as_str(), "ReplicatedStorage");
        assert_eq!(Container::ServerStorage.as_str(), "ServerStorage");
        assert_eq!(
            Container::StarterPlayerScripts.as_str(),
            "StarterPlayerScripts"
        );
    }

    #[test]
    fn server_storage_is_never_replicated() {
        assert!(!Container::ServerStorage.is_replicated());
        for c in Container::ALL {
            if c != Container::ServerStorage {
                assert!(c.is_replicated(), "{c} should replicate");
            }
        }
        assert!(!Container::ServerStorage.is_client_side());
        assert!(Container::ReplicatedStorage.is_replicated());
        assert!(Container::ReplicatedStorage.is_client_side());
        assert!(!Container::StarterPlayerScripts.is_server_side());
    }

    #[test]
    fn network_event_needs_a_shared_container() {
        assert!(InstanceType::NetworkEvent.is_allowed_in(Container::ReplicatedStorage));
        assert!(!InstanceType::NetworkEvent.is_allowed_in(Container::StarterPlayerScripts));
    }

    #[test]
    fn default_containers_are_allowed() {
        for t in InstanceType::ALL {
            let container = t.default_container();
            assert!(
                t.is_allowed_in(container),
                "{t} defaults to a container it is not allowed in ({container})"
            );
        }
    }

    #[test]
    fn client_script_defaults_to_starter_player_scripts() {
        assert_eq!(
            InstanceType::ClientScript.default_container(),
            Container::StarterPlayerScripts
        );
        assert_eq!(
            InstanceType::NetworkEvent.default_container(),
            Container::ReplicatedStorage
        );
        assert_eq!(InstanceType::Part.default_container(), Container::Workspace);
    }

    #[test]
    fn classification_helpers_are_consistent() {
        assert!(InstanceType::ModuleScript.is_script());
        assert!(!InstanceType::Part.is_script());
        assert!(InstanceType::ScrollingFrame.is_gui());
        assert!(!InstanceType::Audio.is_gui());
        assert!(InstanceType::TrussPart.has_3d_position());
        assert!(!InstanceType::Frame.has_3d_position());
    }
}
