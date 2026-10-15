//! A single node in the instance tree.

use std::collections::BTreeMap;

use nb_types::{Container, InstanceType, Orientation, Rgba, Value, Vector3};
use serde::{Deserialize, Serialize};

/// Stable identity for an instance, independent of its (non-unique) name.
///
/// Wraps a v4 UUID rendered as a lower-case hex string so it survives a
/// round-trip through JSON inside a script table.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InstanceId(String);

impl InstanceId {
    /// Mints a fresh identity.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().simple().to_string())
    }

    /// Wraps an existing hex string, as read back from a saved place.
    pub fn from_raw(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// The hex form scripts see as `GUID`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for InstanceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for InstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which kind of Luau source a script instance carries, and where it may run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptKind {
    /// Runs only on the Dedicated server; the client never receives it.
    Server,
    /// Runs only on the player's machine.
    Client,
    /// Reusable module, run once per environment and cached.
    Module,
}

impl ScriptKind {
    /// Maps an instance type onto its script kind.
    pub fn from_instance_type(t: InstanceType) -> Option<Self> {
        match t {
            InstanceType::ServerScript => Some(ScriptKind::Server),
            InstanceType::ClientScript => Some(ScriptKind::Client),
            InstanceType::ModuleScript => Some(ScriptKind::Module),
            _ => None,
        }
    }

    /// Whether this script may run on the server.
    pub fn runs_on_server(self) -> bool {
        matches!(self, ScriptKind::Server | ScriptKind::Module)
    }

    /// Whether this script may run on a client.
    pub fn runs_on_client(self) -> bool {
        matches!(self, ScriptKind::Client | ScriptKind::Module)
    }

    /// Whether the source itself may be replicated to clients.
    ///
    /// Server code must never leave the server, which is why the documentation
    /// says a `ServerScript` "không replicate xuống Client".
    pub fn source_replicates(self) -> bool {
        !matches!(self, ScriptKind::Server)
    }
}

/// The mutable properties of an instance.
///
/// This is a typed superset of the property bags the documentation passes
/// around: [`InstanceProperties::from_value`] absorbs an arbitrary `Properties`
/// table from a script (ignoring unknown keys, as the documentation implies by
/// listing only the meaningful ones), and [`InstanceProperties::to_value`]
/// renders it back for the Inspector and for replication.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstanceProperties {
    /// World position, for types where [`InstanceType::has_3d_position`] holds.
    pub position: Vector3,
    /// World orientation in degrees.
    pub orientation: Orientation,
    /// Size. `x`/`y` for GUI, all three axes for parts.
    pub size: Vector3,
    /// Display colour.
    pub color: Rgba,
    /// GUI text or `TextBox` content, and also used for `TextLabel`/`TextButton`.
    pub text: String,
    /// `TextBox` placeholder. Only meaningful when `text` is empty.
    pub placeholder: String,
    /// Font face for text rendering.
    pub font: String,
    /// Text size in points.
    pub text_size: f64,
    /// Image asset id, from `NImageAssets`.
    pub image: String,
    /// `ImageTransparency`, 0.0 opaque to 1.0 invisible.
    pub image_transparency: f64,
    /// `ScaleType` for images.
    pub scale_type: String,
    /// `ClearTextOnFocus`, `TextBox` only.
    pub clear_text_on_focus: bool,
    /// `MultiLine`, `TextBox` only.
    pub multi_line: bool,
    /// `CanvasSize`, `ScrollingFrame` only.
    pub canvas_size: Vector3,
    /// `ScrollBarThickness`, `ScrollingFrame` only.
    pub scroll_bar_thickness: f64,
    /// `ScrollingEnabled`, `ScrollingFrame` only.
    pub scrolling_enabled: bool,
    /// `ElasticBehavior`, `ScrollingFrame` only.
    pub elastic_behavior: String,
    /// `ClimbSpeed`, `TrussPart` only.
    pub climb_speed: f64,
    /// Luau source, script types only.
    pub source: String,
    /// Whether the instance is switched on. A disabled script does not run.
    pub enabled: bool,
    /// Whether the instance is drawn and interactive.
    pub visible: bool,
    /// Audio loop flag.
    pub looping: bool,
    /// Audio volume.
    pub volume: f64,
    /// Audio asset id.
    pub audio_id: String,
    /// Anything not covered above, preserved verbatim so no script data is lost.
    pub extra: BTreeMap<String, Value>,
}

impl Default for InstanceProperties {
    fn default() -> Self {
        Self {
            position: Vector3::ZERO,
            orientation: Orientation::IDENTITY,
            // The documentation treats Size as XYZ for parts and XY for GUI;
            // starting at 1x1x1 keeps both interpretations meaningful.
            size: Vector3::new(1.0, 1.0, 1.0),
            color: Rgba::default(),
            text: String::new(),
            placeholder: String::new(),
            font: "Arial".to_owned(),
            text_size: 14.0,
            image: String::new(),
            image_transparency: 0.0,
            scale_type: "Stretch".to_owned(),
            clear_text_on_focus: true,
            multi_line: false,
            canvas_size: Vector3::new(0.0, 0.0, 0.0),
            scroll_bar_thickness: 12.0,
            scrolling_enabled: true,
            elastic_behavior: "WhenScrollable".to_owned(),
            climb_speed: 12.0,
            source: String::new(),
            enabled: true,
            visible: true,
            looping: false,
            volume: 1.0,
            audio_id: String::new(),
            extra: BTreeMap::new(),
        }
    }
}

impl InstanceProperties {
    /// Reads a script's `Properties` table.
    ///
    /// Keys are matched case-insensitively against the documented property
    /// names. Both the capitalised spelling used in the documentation
    /// (`Position`, `TextColor3`) and the plain spelling used in scripts are
    /// accepted. Unrecognised keys are kept in [`InstanceProperties::extra`].
    pub fn from_value(value: &Value) -> Self {
        let mut props = Self::default();
        let Some(map) = value.as_map() else {
            return props;
        };

        // Resolve a key regardless of the case the script used.
        let lookup = |name: &str| -> Option<&Value> {
            map.get(name).or_else(|| {
                map.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(name))
                    .map(|(_, v)| v)
            })
        };

        if let Some(v) = lookup("Position").and_then(Vector3::from_value) {
            props.position = v;
        }
        if let Some(v) = lookup("Orientation").and_then(Orientation::from_value) {
            props.orientation = v;
        }
        if let Some(v) = lookup("Size").and_then(Vector3::from_value) {
            props.size = v;
        }
        if let Some(v) = lookup("RGBA")
            .or_else(|| lookup("Color"))
            .or_else(|| lookup("TextColor3"))
            .and_then(Rgba::from_value)
        {
            props.color = v;
        }
        if let Some(v) = lookup("Text").and_then(Value::as_str) {
            props.text = v.to_owned();
        }
        if let Some(v) = lookup("PlaceholderText").and_then(Value::as_str) {
            props.placeholder = v.to_owned();
        }
        if let Some(v) = lookup("Font").and_then(Value::as_str) {
            props.font = v.to_owned();
        }
        if let Some(v) = lookup("TextSize").and_then(Value::as_f64) {
            props.text_size = v;
        }
        if let Some(v) = lookup("Image").and_then(Value::as_str) {
            props.image = v.to_owned();
        }
        if let Some(v) = lookup("ImageTransparency").and_then(Value::as_f64) {
            props.image_transparency = v;
        }
        if let Some(v) = lookup("ScaleType").and_then(Value::as_str) {
            props.scale_type = v.to_owned();
        }
        if let Some(v) = lookup("ClearTextOnFocus").and_then(Value::as_bool) {
            props.clear_text_on_focus = v;
        }
        if let Some(v) = lookup("MultiLine").and_then(Value::as_bool) {
            props.multi_line = v;
        }
        if let Some(v) = lookup("CanvasSize").and_then(Vector3::from_value) {
            props.canvas_size = v;
        }
        if let Some(v) = lookup("ScrollBarThickness").and_then(Value::as_f64) {
            props.scroll_bar_thickness = v;
        }
        if let Some(v) = lookup("ScrollingEnabled").and_then(Value::as_bool) {
            props.scrolling_enabled = v;
        }
        if let Some(v) = lookup("ElasticBehavior").and_then(Value::as_str) {
            props.elastic_behavior = v.to_owned();
        }
        if let Some(v) = lookup("ClimbSpeed").and_then(Value::as_f64) {
            props.climb_speed = v;
        }
        if let Some(v) = lookup("Source").and_then(Value::as_str) {
            props.source = v.to_owned();
        }
        if let Some(v) = lookup("Enabled").and_then(Value::as_bool) {
            props.enabled = v;
        }
        if let Some(v) = lookup("Visible").and_then(Value::as_bool) {
            props.visible = v;
        }
        if let Some(v) = lookup("Loop").and_then(Value::as_bool) {
            props.looping = v;
        }
        if let Some(v) = lookup("Volume").and_then(Value::as_f64) {
            props.volume = v;
        }
        if let Some(v) = lookup("ID").and_then(Value::as_str) {
            props.audio_id = v.to_owned();
        }

        // Keep anything we did not understand so the Inspector can show it and
        // a save/load cycle does not silently drop author data.
        const KNOWN: [&str; 26] = [
            "Position",
            "Orientation",
            "Size",
            "RGBA",
            "Color",
            "TextColor3",
            "Text",
            "PlaceholderText",
            "Font",
            "TextSize",
            "Image",
            "ImageTransparency",
            "ScaleType",
            "ClearTextOnFocus",
            "MultiLine",
            "CanvasSize",
            "ScrollBarThickness",
            "ScrollingEnabled",
            "ElasticBehavior",
            "ClimbSpeed",
            "Source",
            "Enabled",
            "Visible",
            "Loop",
            "Volume",
            "ID",
        ];
        for (k, v) in map {
            if !KNOWN.iter().any(|known| known.eq_ignore_ascii_case(k)) {
                props.extra.insert(k.clone(), v.clone());
            }
        }

        props
    }

    /// Renders the properties back into the documentation's table shape.
    pub fn to_value(&self) -> Value {
        let mut map = BTreeMap::new();
        map.insert("Position".to_owned(), self.position.to_value());
        map.insert("Orientation".to_owned(), self.orientation.to_value());
        map.insert(
            "Size".to_owned(),
            Value::array([
                Value::from(self.size.x),
                Value::from(self.size.y),
                Value::from(self.size.z),
            ]),
        );
        map.insert("RGBA".to_owned(), self.color.to_value());
        map.insert("Text".to_owned(), Value::from(self.text.clone()));
        map.insert(
            "PlaceholderText".to_owned(),
            Value::from(self.placeholder.clone()),
        );
        map.insert("Font".to_owned(), Value::from(self.font.clone()));
        map.insert("TextSize".to_owned(), Value::from(self.text_size));
        map.insert("Image".to_owned(), Value::from(self.image.clone()));
        map.insert(
            "ImageTransparency".to_owned(),
            Value::from(self.image_transparency),
        );
        map.insert("ScaleType".to_owned(), Value::from(self.scale_type.clone()));
        map.insert(
            "ClearTextOnFocus".to_owned(),
            Value::from(self.clear_text_on_focus),
        );
        map.insert("MultiLine".to_owned(), Value::from(self.multi_line));
        map.insert("CanvasSize".to_owned(), self.canvas_size.to_value());
        map.insert(
            "ScrollBarThickness".to_owned(),
            Value::from(self.scroll_bar_thickness),
        );
        map.insert(
            "ScrollingEnabled".to_owned(),
            Value::from(self.scrolling_enabled),
        );
        map.insert(
            "ElasticBehavior".to_owned(),
            Value::from(self.elastic_behavior.clone()),
        );
        map.insert("ClimbSpeed".to_owned(), Value::from(self.climb_speed));
        map.insert("Source".to_owned(), Value::from(self.source.clone()));
        map.insert("Enabled".to_owned(), Value::from(self.enabled));
        map.insert("Visible".to_owned(), Value::from(self.visible));
        map.insert("Loop".to_owned(), Value::from(self.looping));
        map.insert("Volume".to_owned(), Value::from(self.volume));
        map.insert("ID".to_owned(), Value::from(self.audio_id.clone()));
        for (k, v) in &self.extra {
            map.insert(k.clone(), v.clone());
        }
        Value::Map(map)
    }

    /// Fetches one property by the name a script used in
    /// `UIService.GetProperty` / `LightingService.GetProperty`.
    ///
    /// Returns `None` for a name the instance does not have, which lets the
    /// service layer raise a precise error instead of returning a wrong value.
    pub fn get_named(&self, property: &str) -> Option<Value> {
        let p = property.to_ascii_lowercase();
        let value = match p.as_str() {
            "position" => self.position.to_value(),
            "orientation" => self.orientation.to_value(),
            "size" => self.size.to_value(),
            "rgba" | "color" | "textcolor3" => self.color.to_value(),
            "text" => Value::from(self.text.clone()),
            "placeholdertext" => Value::from(self.placeholder.clone()),
            "font" => Value::from(self.font.clone()),
            "textsize" => Value::from(self.text_size),
            "image" => Value::from(self.image.clone()),
            "imagetransparency" => Value::from(self.image_transparency),
            "scaletype" => Value::from(self.scale_type.clone()),
            "cleartextonfocus" => Value::from(self.clear_text_on_focus),
            "multiline" => Value::from(self.multi_line),
            "canvassize" => self.canvas_size.to_value(),
            "scrollbarthickness" => Value::from(self.scroll_bar_thickness),
            "scrollingenabled" => Value::from(self.scrolling_enabled),
            "elasticbehavior" => Value::from(self.elastic_behavior.clone()),
            "climbspeed" => Value::from(self.climb_speed),
            "source" => Value::from(self.source.clone()),
            "enabled" => Value::from(self.enabled),
            "visible" => Value::from(self.visible),
            "loop" => Value::from(self.looping),
            "volume" => Value::from(self.volume),
            "id" => Value::from(self.audio_id.clone()),
            _ => return self.extra.get(property).cloned(),
        };
        Some(value)
    }

    /// Writes one property by name, mirroring [`InstanceProperties::get_named`].
    ///
    /// Returns `false` when the name is unknown, so callers can report the
    /// problem rather than silently ignoring a typo.
    pub fn set_named(&mut self, property: &str, value: &Value) -> bool {
        let p = property.to_ascii_lowercase();
        match p.as_str() {
            "position" => match Vector3::from_value(value) {
                Some(v) => self.position = v,
                None => return false,
            },
            "orientation" => match Orientation::from_value(value) {
                Some(v) => self.orientation = v,
                None => return false,
            },
            "size" => match Vector3::from_value(value) {
                Some(v) => self.size = v,
                None => return false,
            },
            "rgba" | "color" | "textcolor3" => match Rgba::from_value(value) {
                Some(v) => self.color = v,
                None => return false,
            },
            "text" => self.text = value.as_str().unwrap_or_default().to_owned(),
            "placeholdertext" => self.placeholder = value.as_str().unwrap_or_default().to_owned(),
            "font" => self.font = value.as_str().unwrap_or_default().to_owned(),
            "textsize" => self.text_size = value.as_f64().unwrap_or(self.text_size),
            "image" => self.image = value.as_str().unwrap_or_default().to_owned(),
            "imagetransparency" => {
                self.image_transparency = value.as_f64().unwrap_or(self.image_transparency)
            }
            "scaletype" => self.scale_type = value.as_str().unwrap_or_default().to_owned(),
            "cleartextonfocus" => {
                self.clear_text_on_focus = value.as_bool().unwrap_or(self.clear_text_on_focus)
            }
            "multiline" => self.multi_line = value.as_bool().unwrap_or(self.multi_line),
            "canvassize" => match Vector3::from_value(value) {
                Some(v) => self.canvas_size = v,
                None => return false,
            },
            "scrollbarthickness" => {
                self.scroll_bar_thickness = value.as_f64().unwrap_or(self.scroll_bar_thickness)
            }
            "scrollingenabled" => {
                self.scrolling_enabled = value.as_bool().unwrap_or(self.scrolling_enabled)
            }
            "elasticbehavior" => {
                self.elastic_behavior = value.as_str().unwrap_or_default().to_owned()
            }
            "climbspeed" => self.climb_speed = value.as_f64().unwrap_or(self.climb_speed),
            "source" => self.source = value.as_str().unwrap_or_default().to_owned(),
            "enabled" => self.enabled = value.as_bool().unwrap_or(self.enabled),
            "visible" => self.visible = value.as_bool().unwrap_or(self.visible),
            "loop" => self.looping = value.as_bool().unwrap_or(self.looping),
            "volume" => self.volume = value.as_f64().unwrap_or(self.volume),
            "id" => self.audio_id = value.as_str().unwrap_or_default().to_owned(),
            _ => {
                self.extra.insert(property.to_owned(), value.clone());
            }
        }
        true
    }
}

/// One node in the world tree.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instance {
    /// Stable identity, the `GUID` scripts can address.
    pub id: InstanceId,
    /// Display and lookup name. Not unique.
    pub name: String,
    /// What kind of thing this is, which fixes the legal properties and the
    /// side it runs on.
    pub kind: InstanceType,
    /// The container this node is a direct child of.
    pub parent: Container,
    /// Name of the parent node within the container, or `None` for a root node.
    pub parent_name: Option<String>,
    /// Mutable state.
    pub properties: InstanceProperties,
    /// Sorted property order of creation, used to implement `Index`.
    pub created_at: u64,
}

impl Instance {
    /// Creates a node with default properties.
    pub fn new(
        name: impl Into<String>,
        kind: InstanceType,
        parent: Container,
        created_at: u64,
    ) -> Self {
        Self {
            id: InstanceId::new(),
            name: name.into(),
            kind,
            parent,
            parent_name: None,
            properties: InstanceProperties::default(),
            created_at,
        }
    }

    /// The script kind, when this node is a script.
    pub fn script_kind(&self) -> Option<ScriptKind> {
        ScriptKind::from_instance_type(self.kind)
    }

    /// Whether this node's source may travel to a client.
    ///
    /// A `ServerScript` returns `false`, which is what keeps server logic
    /// off client machines.
    pub fn replicates_to_client(&self) -> bool {
        if !self.parent.is_replicated() {
            return false;
        }
        match self.script_kind() {
            // Server code stays on the server; GUI and world data do not.
            Some(kind) => kind.source_replicates(),
            None => true,
        }
    }

    /// Whether this node exists on the client at all.
    pub fn exists_on_client(&self) -> bool {
        self.parent.is_client_side()
    }

    /// The full path a user sees in the Studio Explorer, e.g.
    /// `Workspace.Shop.Model`.
    pub fn path(&self, ancestors: &[String]) -> String {
        let mut parts = vec![self.parent.as_str().to_owned()];
        parts.extend(ancestors.iter().cloned());
        parts.push(self.name.clone());
        parts.join(".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props_table() -> Value {
        Value::map([
            ("Position", Vector3::new(1.0, 2.0, 3.0).to_value()),
            ("Size", Vector3::new(4.0, 5.0, 6.0).to_value()),
            ("RGBA", Rgba::new(1.0, 0.0, 0.0, 1.0).to_value()),
            ("Text", Value::from("Hello")),
            ("TextSize", Value::from(18i64)),
            ("Enabled", Value::from(false)),
        ])
    }

    #[test]
    fn properties_absorb_a_script_table() {
        let p = InstanceProperties::from_value(&props_table());
        assert_eq!(p.position, Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(p.size, Vector3::new(4.0, 5.0, 6.0));
        assert!((p.color.r - 1.0).abs() < 1e-6);
        assert_eq!(p.text, "Hello");
        assert_eq!(p.text_size, 18.0);
        assert!(!p.enabled);
    }

    #[test]
    fn property_names_are_case_insensitive() {
        let lower = Value::map([
            ("position", Vector3::new(7.0, 8.0, 9.0).to_value()),
            ("text", Value::from("lower")),
        ]);
        let p = InstanceProperties::from_value(&lower);
        assert_eq!(p.position, Vector3::new(7.0, 8.0, 9.0));
        assert_eq!(p.text, "lower");
    }

    #[test]
    fn unknown_properties_are_preserved() {
        let with_extra = Value::map([
            ("Text", Value::from("x")),
            ("CustomTag", Value::from("keep-me")),
        ]);
        let p = InstanceProperties::from_value(&with_extra);
        assert_eq!(
            p.extra.get("CustomTag").and_then(Value::as_str),
            Some("keep-me")
        );
        // And it survives a round trip.
        let back = p.to_value();
        assert_eq!(
            back.get("CustomTag").and_then(Value::as_str),
            Some("keep-me")
        );
    }

    #[test]
    fn properties_round_trip_through_value() {
        let p = InstanceProperties::from_value(&props_table());
        let again = InstanceProperties::from_value(&p.to_value());
        assert_eq!(p.position, again.position);
        assert_eq!(p.size, again.size);
        assert_eq!(p.text, again.text);
        assert_eq!(p.text_size, again.text_size);
        assert_eq!(p.enabled, again.enabled);
        assert_eq!(p.color.to_u32(), again.color.to_u32());
    }

    #[test]
    fn get_and_set_named_agree() {
        let mut p = InstanceProperties::default();
        assert!(p.set_named("Text", &Value::from("abc")));
        assert_eq!(
            p.get_named("Text")
                .and_then(|v| v.as_str().map(str::to_owned)),
            Some("abc".to_owned())
        );
        assert_eq!(
            p.get_named("text")
                .and_then(|v| v.as_str().map(str::to_owned)),
            Some("abc".to_owned())
        );

        assert!(p.set_named("TextSize", &Value::from(22i64)));
        assert_eq!(p.get_named("TextSize").and_then(|v| v.as_f64()), Some(22.0));

        assert!(p.set_named("Visible", &Value::from(false)));
        assert_eq!(
            p.get_named("Visible").and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn setting_an_unknown_property_is_stored_not_dropped() {
        let mut p = InstanceProperties::default();
        assert!(p.set_named("MyFlag", &Value::from(1i64)));
        assert_eq!(p.get_named("MyFlag").and_then(|v| v.as_f64()), Some(1.0));
    }

    #[test]
    fn setting_a_position_with_a_bad_shape_fails_loudly() {
        let mut p = InstanceProperties::default();
        assert!(!p.set_named("Position", &Value::from("not-a-position")));
        // The old value is untouched.
        assert_eq!(p.position, Vector3::ZERO);
    }

    #[test]
    fn script_kinds_map_to_instance_types() {
        assert_eq!(
            ScriptKind::from_instance_type(InstanceType::ServerScript),
            Some(ScriptKind::Server)
        );
        assert_eq!(
            ScriptKind::from_instance_type(InstanceType::ClientScript),
            Some(ScriptKind::Client)
        );
        assert_eq!(
            ScriptKind::from_instance_type(InstanceType::ModuleScript),
            Some(ScriptKind::Module)
        );
        assert_eq!(ScriptKind::from_instance_type(InstanceType::Part), None);
    }

    #[test]
    fn server_script_source_never_replicates() {
        let mut s = Instance::new(
            "Logic",
            InstanceType::ServerScript,
            Container::ServerStorage,
            0,
        );
        assert!(!s.replicates_to_client());
        assert!(!s.exists_on_client());

        s.parent = Container::ReplicatedStorage;
        assert!(
            !s.replicates_to_client(),
            "server source must never replicate"
        );
        assert!(
            s.exists_on_client(),
            "the node exists, only the source is withheld"
        );
    }

    #[test]
    fn module_and_client_scripts_replicate_from_shared_containers() {
        // A module in a shared container reaches clients.
        let shared = Instance::new(
            "MathUtils",
            InstanceType::ModuleScript,
            Container::ReplicatedStorage,
            0,
        );
        assert!(shared.replicates_to_client());

        // A client script always belongs to the player's machine.
        let client = Instance::new(
            "UI",
            InstanceType::ClientScript,
            Container::StarterPlayerScripts,
            0,
        );
        assert!(client.replicates_to_client());
    }

    #[test]
    fn module_script_in_server_storage_stays_on_the_server() {
        // This is the safe default: keeping a module private must actually work,
        // otherwise server-only logic would leak to every client.
        let private = Instance::new(
            "EconomyInternals",
            InstanceType::ModuleScript,
            Container::ServerStorage,
            0,
        );
        assert_eq!(private.kind.default_container(), Container::ServerStorage);
        assert!(!private.replicates_to_client());
    }

    #[test]
    fn parts_replicate_but_server_storage_does_not() {
        let part = Instance::new("Block", InstanceType::Part, Container::Workspace, 0);
        assert!(part.replicates_to_client());

        let hidden = Instance::new("Secret", InstanceType::Part, Container::ServerStorage, 0);
        assert!(!hidden.replicates_to_client());
        assert!(!hidden.exists_on_client());
    }

    #[test]
    fn instance_path_reads_like_the_explorer() {
        let i = Instance::new("Model", InstanceType::Model, Container::Workspace, 0);
        assert_eq!(i.path(&["Shop".to_owned()]), "Workspace.Shop.Model");
        assert_eq!(i.path(&[]), "Workspace.Model");
    }

    #[test]
    fn ids_are_unique_and_hex() {
        let a = InstanceId::new();
        let b = InstanceId::new();
        assert_ne!(a, b);
        assert!(a.as_str().chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(a.as_str().len(), 32, "simple lowercase uuid");
    }
}
