/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Studio state, types and data structures.

use nb_types::Vector3;
use nb_world::{PlaceMetadata, PlaceSettings, World, WorldFile};
use std::path::PathBuf;

/// Active manipulation tool in the viewport ribbon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolMode {
    Select,
    Move,
    Scale,
    Rotate,
}

impl ToolMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Move => "Move",
            Self::Scale => "Scale",
            Self::Rotate => "Rotate",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Select => "👆",
            Self::Move => "✥",
            Self::Scale => "↔",
            Self::Rotate => "⟲",
        }
    }
}

/// Simulation playtest mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaytestState {
    /// Normal editing mode.
    Stopped,
    /// Solo client + server simulation.
    PlayingSolo,
    /// Server-only simulation without local character.
    RunningServer,
}

/// A log entry in the Output / Console panel.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Script,
}

impl LogLevel {
    pub fn color(self) -> egui::Color32 {
        match self {
            Self::Info => egui::Color32::from_rgb(180, 190, 205),
            Self::Warning => egui::Color32::from_rgb(255, 195, 0),
            Self::Error => egui::Color32::from_rgb(255, 80, 80),
            Self::Script => egui::Color32::from_rgb(100, 215, 255),
        }
    }
}

/// Camera orbit and pan parameters for the 3D viewport canvas.
#[derive(Debug, Clone)]
pub struct CameraState {
    /// Target center of view in 3D space.
    pub target: Vector3,
    /// Distance from target.
    pub distance: f32,
    /// Horizontal orbit angle in radians.
    pub yaw: f32,
    /// Vertical pitch angle in radians (-PI/2 to PI/2).
    pub pitch: f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            target: Vector3::new(0.0, 2.0, 0.0),
            distance: 40.0,
            yaw: 0.6,
            pitch: 0.5,
        }
    }
}

impl CameraState {
    /// Computes eye position in world space.
    #[allow(dead_code)]
    pub fn eye_position(&self) -> Vector3 {
        let x = self.target.x
            + (self.distance as f64) * self.pitch.cos() as f64 * self.yaw.sin() as f64;
        let y = self.target.y + (self.distance as f64) * self.pitch.sin() as f64;
        let z = self.target.z
            + (self.distance as f64) * self.pitch.cos() as f64 * self.yaw.cos() as f64;
        Vector3::new(x, y, z)
    }
}

/// Root state of the Noobietoria Studio application.
pub struct StudioApp {
    /// Active world tree with all instances.
    pub world: World,
    /// Metadata for current place.
    pub metadata: PlaceMetadata,
    /// Gameplay & environment settings.
    pub settings: PlaceSettings,
    /// File path on disk if saved/loaded.
    pub current_path: Option<PathBuf>,
    /// GUID of currently selected instance.
    pub selected_guid: Option<String>,
    /// Currently active tool.
    pub tool_mode: ToolMode,
    /// Snap increment (studs).
    pub grid_snap: f32,
    /// Collisions enabled during drag.
    pub collisions_enabled: bool,
    /// Simulation state.
    pub playtest: PlaytestState,
    /// Simulated time in seconds.
    pub simulation_time: f64,
    /// Viewport 3D camera.
    pub camera: CameraState,
    /// Output console logs.
    pub logs: Vec<LogEntry>,
    /// Command line bar input.
    pub command_input: String,
    /// Toolbox search query.
    pub toolbox_query: String,
    /// Toolbox category filter.
    pub toolbox_category: String,
    /// Quick status message on bottom bar.
    pub status_text: String,
    /// Whether the script editor modal is open.
    pub script_editor_open: bool,
    /// Script editor buffer.
    pub script_buffer: String,
    /// GUID of the script currently open in the dedicated editor.
    pub script_editor_target: Option<String>,
    /// State for the inline/dedicated script editor (autocomplete, filter, navigation).
    pub script_editor_state: crate::ui::ScriptEditorState,
    /// Show properties panel.
    pub show_properties: bool,
    /// Show explorer panel.
    pub show_explorer: bool,
    /// Show toolbox panel.
    pub show_toolbox: bool,
    /// Show output panel.
    pub show_output: bool,
}

impl StudioApp {
    pub fn new_headless() -> Self {
        let default_file = WorldFile::default_starter_place();
        let mut app = Self {
            world: default_file.to_world(),
            metadata: default_file.metadata,
            settings: default_file.settings,
            current_path: None,
            selected_guid: None,
            tool_mode: ToolMode::Select,
            grid_snap: 1.0,
            collisions_enabled: true,
            playtest: PlaytestState::Stopped,
            simulation_time: 0.0,
            camera: CameraState::default(),
            logs: Vec::new(),
            command_input: String::new(),
            toolbox_query: String::new(),
            toolbox_category: "All".to_owned(),
            status_text: "Ready - Noobietoria Studio v0.1.0".to_owned(),
            script_editor_open: false,
            script_buffer: String::new(),
            script_editor_target: None,
            script_editor_state: crate::ui::ScriptEditorState::default(),
            show_properties: true,
            show_explorer: true,
            show_toolbox: true,
            show_output: true,
        };
        app.log(LogLevel::Info, "Welcome to Noobietoria Studio!");
        app.log(
            LogLevel::Info,
            "Loaded template: Baseplate Place (3 instances)",
        );
        app
    }

    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::new_headless()
    }

    pub fn log(&mut self, level: LogLevel, message: impl Into<String>) {
        let msg = message.into();
        let timestamp = chrono_like_now();
        self.logs.push(LogEntry {
            timestamp,
            level,
            message: msg,
        });
        if self.logs.len() > 500 {
            self.logs.remove(0);
        }
    }

    pub fn reset_to_starter_place(&mut self) {
        let starter = WorldFile::default_starter_place();
        self.world = starter.to_world();
        self.metadata = starter.metadata;
        self.settings = starter.settings;
        self.current_path = None;
        self.selected_guid = None;
        self.status_text = "Created new Starter Place".to_owned();
        self.log(LogLevel::Info, "Created new place from Baseplate template");
    }
}

fn chrono_like_now() -> String {
    // Light-weight time stamp without extra dependencies
    let now = std::time::SystemTime::now();
    let since_epoch = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = since_epoch.as_secs();
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    let s = secs % 60;
    format!("{hours:02}:{mins:02}:{s:02}")
}
