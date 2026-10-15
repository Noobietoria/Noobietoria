/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Lua / Luau Syntax Highlighter and Intelligent Autocomplete Engine
//! Built for Noobietoria Studio with Roblox-compatible Luau API definitions.

use egui::text::LayoutJob;
use egui::{Color32, FontId, Key, RichText, TextFormat, Ui};

// ── Color Theme (VSCode Dark+ style) ───────────────────────────────────────────
const COLOR_KEYWORD: Color32   = Color32::from_rgb(197, 134, 192); // #C586C0 purple
const COLOR_GLOBAL: Color32    = Color32::from_rgb(78, 201, 176);  // #4EC9B0 cyan/teal
const COLOR_FUNCTION: Color32  = Color32::from_rgb(220, 220, 170); // #DCDCAA light yellow
const COLOR_PROPERTY: Color32  = Color32::from_rgb(156, 220, 254); // #9CDCFE sky blue
const COLOR_STRING: Color32    = Color32::from_rgb(206, 145, 120); // #CE9178 warm orange
const COLOR_NUMBER: Color32    = Color32::from_rgb(181, 206, 168); // #B5CEA8 sage green
const COLOR_COMMENT: Color32   = Color32::from_rgb(106, 153, 85);  // #6A9955 green
const COLOR_OPERATOR: Color32  = Color32::from_rgb(212, 212, 212); // #D4D4D4 light gray
const COLOR_DEFAULT: Color32   = Color32::from_rgb(220, 220, 220); // #DCDCDC off-white

// ── Tokenizer / Highlighter ───────────────────────────────────────────────────

pub fn highlight_lua(code: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font = FontId::monospace(13.0);

    let bytes = code.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    let format_for = |color: Color32| TextFormat {
        font_id: font.clone(),
        color,
        ..Default::default()
    };

    while i < len {
        // Multi-line or single-line comment: --[[ ... ]] or -- ...
        if i + 1 < len && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            let start = i;
            i += 2;
            if i + 1 < len && bytes[i] == b'[' && bytes[i + 1] == b'[' {
                i += 2;
                while i + 1 < len && !(bytes[i] == b']' && bytes[i + 1] == b']') {
                    i += 1;
                }
                if i + 1 < len {
                    i += 2;
                }
            } else {
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            job.append(&code[start..i], 0.0, format_for(COLOR_COMMENT));
            continue;
        }

        // Multi-line string: [[ ... ]]
        if i + 1 < len && bytes[i] == b'[' && bytes[i + 1] == b'[' {
            let start = i;
            i += 2;
            while i + 1 < len && !(bytes[i] == b']' && bytes[i + 1] == b']') {
                i += 1;
            }
            if i + 1 < len {
                i += 2;
            }
            job.append(&code[start..i], 0.0, format_for(COLOR_STRING));
            continue;
        }

        // Single-line string: "..." or '...'
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            let start = i;
            i += 1;
            while i < len && bytes[i] != quote && bytes[i] != b'\n' {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if i < len && bytes[i] == quote {
                i += 1;
            }
            job.append(&code[start..i], 0.0, format_for(COLOR_STRING));
            continue;
        }

        // Numbers: digits, 0x hex, decimal point
        if bytes[i].is_ascii_digit() || (bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            if bytes[i] == b'0' && i + 1 < len && (bytes[i + 1] == b'x' || bytes[i + 1] == b'X') {
                i += 2;
                while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
                    i += 1;
                }
            } else {
                while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'e' || bytes[i] == b'E' || bytes[i] == b'_') {
                    i += 1;
                }
            }
            job.append(&code[start..i], 0.0, format_for(COLOR_NUMBER));
            continue;
        }

        // Identifiers & Keywords
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &code[start..i];

            // Check previous non-whitespace char for dot or colon (member access)
            let mut prev_char = None;
            let mut p = start;
            while p > 0 {
                p -= 1;
                if !bytes[p].is_ascii_whitespace() {
                    prev_char = Some(bytes[p]);
                    break;
                }
            }

            let color = match word {
                "local" | "function" | "end" | "if" | "then" | "else" | "elseif"
                | "return" | "for" | "in" | "do" | "while" | "repeat" | "until"
                | "break" | "continue" | "not" | "and" | "or" | "true" | "false"
                | "nil" | "export" | "type" => COLOR_KEYWORD,

                "game" | "workspace" | "script" | "Instance" | "Vector3" | "Color3"
                | "CFrame" | "TweenInfo" | "RaycastParams" | "UDim2" | "UDim" | "math"
                | "string" | "table" | "task" | "coroutine" | "os" | "debug" | "utf8" => {
                    COLOR_GLOBAL
                }

                "print" | "warn" | "error" | "require" | "assert" | "pcall" | "ypcall"
                | "typeof" | "pairs" | "ipairs" | "next" | "tonumber"
                | "tostring" | "select" | "rawget" | "rawset" | "rawequal" | "setmetatable"
                | "getmetatable" => COLOR_FUNCTION,

                "Players" | "Lighting" | "ReplicatedStorage" | "ServerStorage"
                | "ServerScriptService" | "StarterGui" | "StarterPack" | "StarterPlayer"
                | "SoundService" | "TweenService" | "RunService" | "UserInputService"
                | "HttpService" | "Debris" | "TeleportService" => COLOR_GLOBAL,

                _ if prev_char == Some(b':') => COLOR_FUNCTION,
                _ if prev_char == Some(b'.') => COLOR_PROPERTY,
                _ => COLOR_DEFAULT,
            };

            job.append(word, 0.0, format_for(color));
            continue;
        }

        // Operators & Punctuation
        let start = i;
        i += 1;
        job.append(&code[start..i], 0.0, format_for(COLOR_OPERATOR));
    }

    job
}

// ── Intellisense / Autocomplete Dictionary ─────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntellisenseKind {
    Keyword,
    Function,
    Method,
    Property,
    Global,
    Service,
    Snippet,
}

impl IntellisenseKind {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Keyword  => "🔑",
            Self::Function => "ƒ ",
            Self::Method   => "⚡",
            Self::Property => "🏷 ",
            Self::Global   => "🌐",
            Self::Service  => "📦",
            Self::Snippet  => "✂ ",
        }
    }

    pub fn color(&self) -> Color32 {
        match self {
            Self::Keyword  => COLOR_KEYWORD,
            Self::Function => COLOR_FUNCTION,
            Self::Method   => Color32::from_rgb(255, 215, 0),
            Self::Property => COLOR_PROPERTY,
            Self::Global   => COLOR_GLOBAL,
            Self::Service  => Color32::from_rgb(100, 200, 255),
            Self::Snippet  => Color32::from_rgb(255, 160, 100),
        }
    }
}

pub struct IntellisenseEntry {
    pub label: &'static str,
    pub insert_text: &'static str,
    pub kind: IntellisenseKind,
    pub detail: &'static str,
    pub doc: &'static str,
}

pub static INTELLISENSE_CATALOG: &[IntellisenseEntry] = &[
    // ── Globals & Singletons ──────────────────────────────────────────────────
    IntellisenseEntry {
        label: "game",
        insert_text: "game",
        kind: IntellisenseKind::Global,
        detail: "DataModel",
        doc: "The root DataModel of the game hierarchy.",
    },
    IntellisenseEntry {
        label: "workspace",
        insert_text: "workspace",
        kind: IntellisenseKind::Global,
        detail: "Workspace",
        doc: "Root 3D container holding visible instances and terrain.",
    },
    IntellisenseEntry {
        label: "script",
        insert_text: "script",
        kind: IntellisenseKind::Global,
        detail: "LuaSourceContainer",
        doc: "Reference to the currently executing Script instance.",
    },
    IntellisenseEntry {
        label: "script.Parent",
        insert_text: "script.Parent",
        kind: IntellisenseKind::Property,
        detail: "Instance",
        doc: "The parent container of this script.",
    },

    // ── Constructors ──────────────────────────────────────────────────────────
    IntellisenseEntry {
        label: "Instance.new",
        insert_text: "Instance.new(\"Part\", workspace)",
        kind: IntellisenseKind::Function,
        detail: "(className: string, parent?: Instance) -> Instance",
        doc: "Creates a new Instance of the given class name.",
    },
    IntellisenseEntry {
        label: "Vector3.new",
        insert_text: "Vector3.new(0, 0, 0)",
        kind: IntellisenseKind::Function,
        detail: "(x: number, y: number, z: number) -> Vector3",
        doc: "Constructs a 3D vector with the given coordinates.",
    },
    IntellisenseEntry {
        label: "Vector3.zero",
        insert_text: "Vector3.zero",
        kind: IntellisenseKind::Property,
        detail: "Vector3(0, 0, 0)",
        doc: "Constant vector with zero length.",
    },
    IntellisenseEntry {
        label: "Vector3.one",
        insert_text: "Vector3.one",
        kind: IntellisenseKind::Property,
        detail: "Vector3(1, 1, 1)",
        doc: "Constant vector with all components equal to 1.",
    },
    IntellisenseEntry {
        label: "Color3.fromRGB",
        insert_text: "Color3.fromRGB(255, 255, 255)",
        kind: IntellisenseKind::Function,
        detail: "(r: number, g: number, b: number) -> Color3",
        doc: "Constructs a Color3 from 0–255 integer RGB channels.",
    },
    IntellisenseEntry {
        label: "Color3.new",
        insert_text: "Color3.new(1, 1, 1)",
        kind: IntellisenseKind::Function,
        detail: "(r: number, g: number, b: number) -> Color3",
        doc: "Constructs a Color3 from 0.0–1.0 float RGB channels.",
    },
    IntellisenseEntry {
        label: "CFrame.new",
        insert_text: "CFrame.new(0, 0, 0)",
        kind: IntellisenseKind::Function,
        detail: "(x: number, y: number, z: number) -> CFrame",
        doc: "Constructs a coordinate frame at the given 3D position.",
    },
    IntellisenseEntry {
        label: "CFrame.Angles",
        insert_text: "CFrame.Angles(0, 0, 0)",
        kind: IntellisenseKind::Function,
        detail: "(rx: number, ry: number, rz: number) -> CFrame",
        doc: "Constructs a rotated CFrame using radians.",
    },

    // ── Task Library ──────────────────────────────────────────────────────────
    IntellisenseEntry {
        label: "task.wait",
        insert_text: "task.wait()",
        kind: IntellisenseKind::Function,
        detail: "(duration?: number) -> number",
        doc: "Yields the current thread for the specified duration (seconds).",
    },
    IntellisenseEntry {
        label: "task.spawn",
        insert_text: "task.spawn(function()\n    \nend)",
        kind: IntellisenseKind::Function,
        detail: "(fn: function, ...args) -> thread",
        doc: "Schedules and immediately executes a function on a new coroutine thread.",
    },
    IntellisenseEntry {
        label: "task.delay",
        insert_text: "task.delay(1.0, function()\n    \nend)",
        kind: IntellisenseKind::Function,
        detail: "(sec: number, fn: function, ...args) -> thread",
        doc: "Executes a function after the specified delay in seconds.",
    },

    // ── Standard Globals ──────────────────────────────────────────────────────
    IntellisenseEntry {
        label: "print",
        insert_text: "print()",
        kind: IntellisenseKind::Function,
        detail: "(...args: any) -> ()",
        doc: "Logs output to the Studio console.",
    },
    IntellisenseEntry {
        label: "warn",
        insert_text: "warn()",
        kind: IntellisenseKind::Function,
        detail: "(...args: any) -> ()",
        doc: "Logs a yellow warning to the Studio console.",
    },
    IntellisenseEntry {
        label: "error",
        insert_text: "error(\"\")",
        kind: IntellisenseKind::Function,
        detail: "(message: string, level?: number) -> never",
        doc: "Throws an error and halts execution of current thread.",
    },
    IntellisenseEntry {
        label: "require",
        insert_text: "require()",
        kind: IntellisenseKind::Function,
        detail: "(module: ModuleScript) -> any",
        doc: "Executes and returns the exported value from a ModuleScript.",
    },

    // ── Instance Methods ──────────────────────────────────────────────────────
    IntellisenseEntry {
        label: "GetService",
        insert_text: "GetService(\"Players\")",
        kind: IntellisenseKind::Method,
        detail: "(serviceName: string) -> Service",
        doc: "Retrieves a core service singleton from game.",
    },
    IntellisenseEntry {
        label: "FindFirstChild",
        insert_text: "FindFirstChild(\"\")",
        kind: IntellisenseKind::Method,
        detail: "(name: string, recursive?: boolean) -> Instance?",
        doc: "Returns the first child matching the given name, or nil.",
    },
    IntellisenseEntry {
        label: "WaitForChild",
        insert_text: "WaitForChild(\"\")",
        kind: IntellisenseKind::Method,
        detail: "(name: string, timeOut?: number) -> Instance?",
        doc: "Waits until a child with the given name exists and returns it.",
    },
    IntellisenseEntry {
        label: "GetChildren",
        insert_text: "GetChildren()",
        kind: IntellisenseKind::Method,
        detail: "() -> { Instance }",
        doc: "Returns an array of all direct children of the instance.",
    },
    IntellisenseEntry {
        label: "GetDescendants",
        insert_text: "GetDescendants()",
        kind: IntellisenseKind::Method,
        detail: "() -> { Instance }",
        doc: "Returns an array of all recursive descendants in hierarchy.",
    },
    IntellisenseEntry {
        label: "IsA",
        insert_text: "IsA(\"BasePart\")",
        kind: IntellisenseKind::Method,
        detail: "(className: string) -> boolean",
        doc: "Returns true if the instance inherits from the given class.",
    },
    IntellisenseEntry {
        label: "Destroy",
        insert_text: "Destroy()",
        kind: IntellisenseKind::Method,
        detail: "() -> ()",
        doc: "Removes instance from data model, disconnects events and frees memory.",
    },
    IntellisenseEntry {
        label: "Clone",
        insert_text: "Clone()",
        kind: IntellisenseKind::Method,
        detail: "() -> Instance",
        doc: "Creates a deep copy of the instance and all its descendants.",
    },
    IntellisenseEntry {
        label: "Connect",
        insert_text: "Connect(function()\n    \nend)",
        kind: IntellisenseKind::Method,
        detail: "(listener: function) -> RBXScriptConnection",
        doc: "Subscribes a callback function to the event signal.",
    },

    // ── Events & Common Signals ───────────────────────────────────────────────
    IntellisenseEntry {
        label: "Touched:Connect",
        insert_text: "Touched:Connect(function(hit)\n    local humanoid = hit.Parent:FindFirstChild(\"Humanoid\")\n    if humanoid then\n        \n    end\nend)",
        kind: IntellisenseKind::Snippet,
        detail: "Collision trigger callback",
        doc: "Connects a function that executes whenever another part contacts this part.",
    },
    IntellisenseEntry {
        label: "Triggered:Connect",
        insert_text: "Triggered:Connect(function(player)\n    print(\"ProximityPrompt triggered by\", player.Name)\nend)",
        kind: IntellisenseKind::Snippet,
        detail: "ProximityPrompt trigger callback",
        doc: "Connects callback when a player interacts with ProximityPrompt.",
    },
    IntellisenseEntry {
        label: "PlayerAdded:Connect",
        insert_text: "game:GetService(\"Players\").PlayerAdded:Connect(function(player)\n    print(player.Name, \"joined the game!\")\nend)",
        kind: IntellisenseKind::Snippet,
        detail: "Player join event",
        doc: "Runs on server when a new player connects to the server.",
    },

    // ── Instance Properties ───────────────────────────────────────────────────
    IntellisenseEntry {
        label: "Position",
        insert_text: "Position = Vector3.new(0, 5, 0)",
        kind: IntellisenseKind::Property,
        detail: "Vector3",
        doc: "World-space position coordinate of the instance.",
    },
    IntellisenseEntry {
        label: "Size",
        insert_text: "Size = Vector3.new(4, 1, 4)",
        kind: IntellisenseKind::Property,
        detail: "Vector3",
        doc: "Physical bounding dimensions of the part.",
    },
    IntellisenseEntry {
        label: "Color",
        insert_text: "Color = Color3.fromRGB(255, 0, 0)",
        kind: IntellisenseKind::Property,
        detail: "Color3",
        doc: "Tint color of the part surface.",
    },
    IntellisenseEntry {
        label: "Anchored",
        insert_text: "Anchored = true",
        kind: IntellisenseKind::Property,
        detail: "boolean",
        doc: "Whether the part is immovable in 3D physics space.",
    },
    IntellisenseEntry {
        label: "CanCollide",
        insert_text: "CanCollide = true",
        kind: IntellisenseKind::Property,
        detail: "boolean",
        doc: "Whether physical collision detection is active for this part.",
    },
    IntellisenseEntry {
        label: "Transparency",
        insert_text: "Transparency = 0.5",
        kind: IntellisenseKind::Property,
        detail: "number (0.0 to 1.0)",
        doc: "Opacity of the part (0.0 = solid, 1.0 = invisible).",
    },
    IntellisenseEntry {
        label: "Parent",
        insert_text: "Parent = workspace",
        kind: IntellisenseKind::Property,
        detail: "Instance?",
        doc: "Parent container in the instance tree.",
    },
    IntellisenseEntry {
        label: "Name",
        insert_text: "Name = \"NewPart\"",
        kind: IntellisenseKind::Property,
        detail: "string",
        doc: "Identifier name displayed in Explorer and accessible via dot indexing.",
    },

    // ── Core Services ─────────────────────────────────────────────────────────
    IntellisenseEntry {
        label: "Players",
        insert_text: "game:GetService(\"Players\")",
        kind: IntellisenseKind::Service,
        detail: "Players Service",
        doc: "Manages player accounts, characters and team affiliations.",
    },
    IntellisenseEntry {
        label: "ReplicatedStorage",
        insert_text: "game:GetService(\"ReplicatedStorage\")",
        kind: IntellisenseKind::Service,
        detail: "ReplicatedStorage Service",
        doc: "Shared container synced between Server and all Clients.",
    },
    IntellisenseEntry {
        label: "ServerStorage",
        insert_text: "game:GetService(\"ServerStorage\")",
        kind: IntellisenseKind::Service,
        detail: "ServerStorage Service",
        doc: "Storage accessible strictly from server-side scripts.",
    },
    IntellisenseEntry {
        label: "TweenService",
        insert_text: "game:GetService(\"TweenService\")",
        kind: IntellisenseKind::Service,
        detail: "TweenService",
        doc: "Interpolates instance properties smoothly over time.",
    },
    IntellisenseEntry {
        label: "RunService",
        insert_text: "game:GetService(\"RunService\")",
        kind: IntellisenseKind::Service,
        detail: "RunService",
        doc: "Provides access to render steps, heartbeat and simulation lifecycle.",
    },
    IntellisenseEntry {
        label: "UserInputService",
        insert_text: "game:GetService(\"UserInputService\")",
        kind: IntellisenseKind::Service,
        detail: "UserInputService",
        doc: "Reads client keyboard, mouse, gamepad and touch inputs.",
    },

    // ── Luau Keywords & Snippets ──────────────────────────────────────────────
    IntellisenseEntry {
        label: "local",
        insert_text: "local ",
        kind: IntellisenseKind::Keyword,
        detail: "local declaration",
        doc: "Declares variable or function scoped to current block.",
    },
    IntellisenseEntry {
        label: "function snippet",
        insert_text: "local function myFunc()\n    \nend",
        kind: IntellisenseKind::Snippet,
        detail: "Local function snippet",
        doc: "Inserts a local function template.",
    },
    IntellisenseEntry {
        label: "if-then snippet",
        insert_text: "if  then\n    \nend",
        kind: IntellisenseKind::Snippet,
        detail: "If-then conditional",
        doc: "Inserts an if-then block template.",
    },
    IntellisenseEntry {
        label: "for numeric loop",
        insert_text: "for i = 1, 10 do\n    \nend",
        kind: IntellisenseKind::Snippet,
        detail: "Numeric for loop",
        doc: "Loops from start to end with counter variable.",
    },
    IntellisenseEntry {
        label: "for pairs loop",
        insert_text: "for key, val in pairs() do\n    \nend",
        kind: IntellisenseKind::Snippet,
        detail: "Key-value table iteration",
        doc: "Iterates over dictionary keys and values.",
    },
    IntellisenseEntry {
        label: "while loop",
        insert_text: "while true do\n    task.wait(1.0)\n    \nend",
        kind: IntellisenseKind::Snippet,
        detail: "Safe while loop",
        doc: "While loop pre-seeded with task.wait() to prevent freezing.",
    },
    IntellisenseEntry {
        label: "math.clamp",
        insert_text: "math.clamp(val, min, max)",
        kind: IntellisenseKind::Function,
        detail: "(x: number, min: number, max: number) -> number",
        doc: "Restricts value x within [min, max] range.",
    },
    IntellisenseEntry {
        label: "math.random",
        insert_text: "math.random(1, 10)",
        kind: IntellisenseKind::Function,
        detail: "(m?: number, n?: number) -> number",
        doc: "Generates pseudo-random number.",
    },
];

// ── Inline Editor State ────────────────────────────────────────────────────────

#[derive(Default)]
pub struct ScriptEditorState {
    pub selected_index: usize,
    pub is_active: bool,
    pub filter_word: String,
    pub word_start: usize,
}

impl ScriptEditorState {
    pub fn update_filter(&mut self, text: &str, cursor_idx: usize) {
        // Extract word preceding cursor
        let safe_cursor = cursor_idx.min(text.len());
        let prefix = &text[..safe_cursor];

        // Find start of current word: characters alphanumeric, underscore, dot or colon
        let mut start = safe_cursor;
        let bytes = prefix.as_bytes();
        while start > 0 {
            let b = bytes[start - 1];
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b':' {
                start -= 1;
            } else {
                break;
            }
        }

        let word = &prefix[start..safe_cursor];
        self.word_start = start;
        self.filter_word = word.to_owned();
        self.is_active = !word.is_empty();
        self.selected_index = 0;
    }

    pub fn matches(&self) -> Vec<&'static IntellisenseEntry> {
        if self.filter_word.is_empty() {
            return Vec::new();
        }

        let q = self.filter_word.to_lowercase();
        let mut results: Vec<&'static IntellisenseEntry> = INTELLISENSE_CATALOG
            .iter()
            .filter(|e| {
                let lbl = e.label.to_lowercase();
                let ins = e.insert_text.to_lowercase();
                lbl.starts_with(&q) || ins.starts_with(&q) || lbl.contains(&q)
            })
            .collect();

        // Sort: exact prefix match first
        results.sort_by_key(|e| {
            let lbl = e.label.to_lowercase();
            if lbl.starts_with(&q) {
                0
            } else {
                1
            }
        });

        results.truncate(10);
        results
    }
}

// ── UI Render Component: Inline Luau Editor with Intellisense ──────────────────

pub fn render_code_editor(
    ui: &mut Ui,
    source: &mut String,
    state: &mut ScriptEditorState,
    min_height: f32,
) -> bool {
    let mut modified = false;

    // Outer styling: VSCode Dark editor appearance
    egui::Frame::canvas(ui.style())
        .fill(Color32::from_rgb(30, 30, 30))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)))
        .inner_margin(6.0)
        .show(ui, |ui| {
            // Header bar
            ui.horizontal(|ui| {
                ui.label(RichText::new("📜 Luau Source").strong().size(12.0).color(Color32::LIGHT_GRAY));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let lines = source.lines().count().max(1);
                    ui.label(
                        RichText::new(format!("{lines} lines | UTF-8 | Luau 5.1"))
                            .weak()
                            .small(),
                    );
                });
            });
            ui.separator();

            let matches = state.matches();

            // Keyboard navigation for Intellisense popup
            let mut insert_chosen: Option<&str> = None;

            if !matches.is_empty() {
                if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                    state.selected_index = (state.selected_index + 1) % matches.len();
                }
                if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                    state.selected_index = if state.selected_index == 0 {
                        matches.len() - 1
                    } else {
                        state.selected_index - 1
                    };
                }
                if ui.input(|i| i.key_pressed(Key::Tab) || i.key_pressed(Key::Enter)) {
                    if let Some(entry) = matches.get(state.selected_index) {
                        insert_chosen = Some(entry.insert_text);
                    }
                }
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    state.filter_word.clear();
                    state.is_active = false;
                }
            }

            // Apply selected suggestion if Tab/Enter was hit
            if let Some(chosen) = insert_chosen {
                let start = state.word_start.min(source.len());
                let end = (start + state.filter_word.len()).min(source.len());
                source.replace_range(start..end, chosen);
                state.filter_word.clear();
                state.is_active = false;
                modified = true;
            }

            // Editor multiline with syntax-highlighting layouter
            let mut layouter = |ui: &Ui, text: &dyn egui::text_edit::TextBuffer, _wrap_width: f32| {
                let job = highlight_lua(text.as_str());
                ui.fonts_mut(|f| f.layout_job(job))
            };

            let output = egui::TextEdit::multiline(source)
                .code_editor()
                .font(FontId::monospace(13.0))
                .desired_width(ui.available_width())
                .min_size(egui::Vec2::new(ui.available_width(), min_height))
                .layouter(&mut layouter)
                .show(ui);

            if output.response.changed() {
                modified = true;
                // Cursor detection: approximate cursor position
                if let Some(cursor_range) = output.cursor_range {
                    let cursor_idx = cursor_range.primary.index;
                    state.update_filter(source, cursor_idx);
                } else {
                    // Fallback: use end of buffer
                    let len = source.len();
                    state.update_filter(source, len);
                }
            }

            // ── Intellisense Autocomplete Popup (bật ra khi gõ, Tab là ok) ─────
            let current_matches = state.matches();
            if !current_matches.is_empty() {
                ui.add_space(4.0);
                egui::Frame::popup(ui.style())
                    .fill(Color32::from_rgb(37, 37, 38))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0, 122, 204)))
                    .inner_margin(4.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "✨ Intellisense (Tab / Enter để chèn, ↑↓ chọn, Esc đóng):"
                                ))
                                .small()
                                .color(Color32::from_rgb(0, 162, 255)),
                            );
                        });
                        ui.separator();

                        egui::ScrollArea::vertical()
                            .max_height(140.0)
                            .show(ui, |ui| {
                                for (idx, item) in current_matches.iter().enumerate() {
                                    let is_selected = idx == state.selected_index;
                                    let bg = if is_selected {
                                        Color32::from_rgb(4, 57, 94)
                                    } else {
                                        Color32::TRANSPARENT
                                    };

                                    egui::Frame::NONE.fill(bg).show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(item.kind.icon())
                                                    .color(item.kind.color())
                                                    .monospace()
                                                    .size(11.0),
                                            );
                                            let label_btn = ui.selectable_label(
                                                is_selected,
                                                RichText::new(item.label)
                                                    .strong()
                                                    .color(if is_selected { Color32::WHITE } else { Color32::LIGHT_GRAY })
                                                    .size(12.0),
                                            );
                                            ui.label(
                                                RichText::new(item.detail)
                                                    .weak()
                                                    .color(Color32::DARK_GRAY)
                                                    .size(11.0),
                                            );

                                            if label_btn.clicked() {
                                                let start = state.word_start.min(source.len());
                                                let end = (start + state.filter_word.len()).min(source.len());
                                                source.replace_range(start..end, item.insert_text);
                                                state.filter_word.clear();
                                                state.is_active = false;
                                                modified = true;
                                            }
                                        });

                                        if is_selected && !item.doc.is_empty() {
                                            ui.horizontal(|ui| {
                                                ui.add_space(16.0);
                                                ui.label(
                                                    RichText::new(item.doc)
                                                        .italics()
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(180, 180, 180)),
                                                );
                                            });
                                        }
                                    });
                                }
                            });
                    });
            }
        });

    modified
}
