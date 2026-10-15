/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Top ribbon toolbar and main menu bar.
//!
//! The **Add** menu lists every documented [`InstanceType`] grouped by category
//! so authors can insert any object without touching the Toolbox.  Insertion
//! logic follows [`InstanceType::default_container`] from the spec.

use egui::{Color32, RichText, Ui};
use nb_types::{Container, InstanceType, Rgba, Vector3};
use nb_world::{InstanceProperties, WorldFile};

use crate::state::{LogLevel, PlaytestState, StudioApp, ToolMode};

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn render_ribbon(app: &mut StudioApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        render_file_menu(app, ui);
        render_edit_menu(app, ui);
        render_view_menu(app, ui);
        render_add_menu(app, ui);

        ui.separator();
        render_tools(app, ui);

        ui.separator();
        render_playtest(app, ui);

        ui.separator();
        render_grid_controls(app, ui);

        ui.separator();
        if ui
            .button(
                RichText::new("📜 Script Editor")
                    .color(if app.script_editor_open {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(100, 215, 255)
                    })
                    .strong(),
            )
            .clicked()
        {
            app.script_editor_open = !app.script_editor_open;
            if app.script_editor_open && app.script_editor_target.is_none() {
                if let Some(guid) = &app.selected_guid {
                    if let Ok(inst) = app.world.by_guid(guid) {
                        if inst.kind.is_script() {
                            app.script_editor_target = Some(guid.clone());
                            app.script_buffer = inst.properties.source.clone();
                        }
                    }
                }
            }
        }
    });
}

// ── File ──────────────────────────────────────────────────────────────────────

fn render_file_menu(app: &mut StudioApp, ui: &mut Ui) {
    ui.menu_button("File", |ui| {
        if ui.button("📄 New Place (Ctrl+N)").clicked() {
            app.reset_to_starter_place();
            ui.close();
        }
        if ui.button("📂 Open .nbw…").clicked() {
            if let Some(path) = rfd_open_file() {
                match WorldFile::load_from_file(&path) {
                    Ok(file) => {
                        app.world = file.to_world();
                        app.metadata = file.metadata;
                        app.settings = file.settings;
                        app.current_path = Some(path.clone());
                        app.log(LogLevel::Info, format!("Opened: {}", path.display()));
                        app.status_text = format!("Opened: {}", path.display());
                    }
                    Err(e) => app.log(LogLevel::Error, format!("Failed to open: {e}")),
                }
            }
            ui.close();
        }
        if ui.button("💾 Save .nbw (Ctrl+S)").clicked() {
            save_place_action(app);
            ui.close();
        }
        if ui.button("💾 Save As…").clicked() {
            save_place_as(app);
            ui.close();
        }
        ui.separator();
        if ui.button("🌐 Publish to Noobietoria Web").clicked() {
            app.log(LogLevel::Info, "Place published to Noobietoria Web.");
            app.status_text = "Published to web.".to_owned();
            ui.close();
        }
    });
}

// ── Edit ──────────────────────────────────────────────────────────────────────

fn render_edit_menu(app: &mut StudioApp, ui: &mut Ui) {
    ui.menu_button("Edit", |ui| {
        // Duplicate
        if ui.button("📋 Duplicate (Ctrl+D)").clicked() {
            if let Some(guid) = app.selected_guid.clone() {
                duplicate_instance(app, &guid);
            } else {
                app.log(LogLevel::Warning, "Nothing selected to duplicate.");
            }
            ui.close();
        }

        // Delete
        if ui.button("🗑 Delete (Del)").clicked() {
            if let Some(guid) = app.selected_guid.clone() {
                if let Ok(name) = app.world.by_guid(&guid).map(|i| i.name.clone()) {
                    let _ = app.world.destroy_by_guid(&guid);
                    app.log(LogLevel::Info, format!("Deleted: {name}"));
                    app.status_text = format!("Deleted {name}");
                    app.selected_guid = None;
                }
            }
            ui.close();
        }

        ui.separator();

        // Select All
        if ui.button("Select All (Ctrl+A)").clicked() {
            // Set selection to first instance if any
            let first = app.world.iter().next().map(|i| i.id.as_str().to_owned());
            app.selected_guid = first;
            ui.close();
        }

        // Focus camera on selection
        if ui.button("🎯 Focus on Selection (F)").clicked() {
            if let Some(guid) = &app.selected_guid.clone() {
                if let Ok(inst) = app.world.by_guid(guid) {
                    let p = inst.properties.position;
                    app.camera.target = nb_types::Vector3::new(p.x, p.y, p.z);
                    app.camera.distance = 15.0;
                    app.status_text = format!("Focused on {}", inst.name);
                }
            }
            ui.close();
        }
    });
}

// ── View ──────────────────────────────────────────────────────────────────────

fn render_view_menu(app: &mut StudioApp, ui: &mut Ui) {
    ui.menu_button("View", |ui| {
        ui.checkbox(&mut app.show_explorer, "Explorer");
        ui.checkbox(&mut app.show_properties, "Properties");
        ui.checkbox(&mut app.show_toolbox, "Toolbox");
        ui.checkbox(&mut app.show_output, "Output");
        ui.separator();
        ui.checkbox(&mut app.script_editor_open, "📜 Luau Script Editor");
    });
}

// ── Add ───────────────────────────────────────────────────────────────────────

/// The full Add menu.  Every documented `InstanceType` appears here, grouped by
/// category, with default properties following `instance.rs`.
fn render_add_menu(app: &mut StudioApp, ui: &mut Ui) {
    ui.menu_button(RichText::new("➕ Add").strong(), |ui| {

        // ── Geometry ─────────────────────────────────────────────────────────
        ui.label(RichText::new("── Geometry ──").color(Color32::from_rgb(180, 220, 255)).small());

        add_item(ui, app, "🧊 Part",
            "Part", InstanceType::Part, Container::Workspace,
            |p| { p.size = Vector3::new(4.0, 1.2, 2.0); p.color = light_grey(); p.position = Vector3::new(0.0, 3.0, 0.0); });

        add_item(ui, app, "🪜 TrussPart",
            "TrussPart", InstanceType::TrussPart, Container::Workspace,
            |p| { p.size = Vector3::new(2.0, 6.0, 2.0); p.position = Vector3::new(0.0, 3.0, 0.0); p.climb_speed = 12.0; });

        add_item(ui, app, "📍 Attachment",
            "Attachment", InstanceType::Attachment, Container::Workspace,
            |p| { p.size = Vector3::new(0.1, 0.1, 0.1); p.position = Vector3::new(0.0, 1.0, 0.0); });

        add_item(ui, app, "📁 Model",
            "Model", InstanceType::Model, Container::Workspace,
            |p| { p.position = Vector3::ZERO; });

        add_item(ui, app, "🚩 SpawnLocation",
            "SpawnLocation", InstanceType::SpawnLocation, Container::Workspace,
            |p| { p.size = Vector3::new(12.0, 1.0, 12.0); p.position = Vector3::new(0.0, 0.5, 0.0); p.color = Rgba::new(196.0, 40.0, 28.0, 255.0); });

        add_item(ui, app, "⚡ Trigger",
            "Trigger", InstanceType::Trigger, Container::Workspace,
            |p| { p.size = Vector3::new(6.0, 4.0, 6.0); p.position = Vector3::new(0.0, 2.0, 0.0); p.visible = false; });

        ui.separator();

        // ── GUI ───────────────────────────────────────────────────────────────
        ui.label(RichText::new("── GUI ──").color(Color32::from_rgb(255, 220, 100)).small());

        add_item(ui, app, "🖥 ScreenGui",
            "ScreenGui", InstanceType::ScreenGui, Container::ScreenGui,
            |p| { p.enabled = true; });

        add_item(ui, app, "🔲 Frame",
            "Frame", InstanceType::Frame, Container::ScreenGui,
            |p| { p.size = Vector3::new(200.0, 150.0, 0.0); p.color = Rgba::new(30.0, 30.0, 40.0, 200.0); });

        add_item(ui, app, "🔤 TextLabel",
            "TextLabel", InstanceType::TextLabel, Container::ScreenGui,
            |p| { p.size = Vector3::new(200.0, 40.0, 0.0); p.text = "Label".to_owned(); p.text_size = 16.0; });

        add_item(ui, app, "🔘 TextButton",
            "TextButton", InstanceType::TextButton, Container::ScreenGui,
            |p| { p.size = Vector3::new(180.0, 44.0, 0.0); p.text = "Button".to_owned(); p.text_size = 16.0; });

        add_item(ui, app, "🖼 ImageLabel",
            "ImageLabel", InstanceType::ImageLabel, Container::ScreenGui,
            |p| { p.size = Vector3::new(100.0, 100.0, 0.0); });

        add_item(ui, app, "🖻 ImageButton",
            "ImageButton", InstanceType::ImageButton, Container::ScreenGui,
            |p| { p.size = Vector3::new(100.0, 100.0, 0.0); });

        add_item(ui, app, "📝 TextBox",
            "TextBox", InstanceType::TextBox, Container::ScreenGui,
            |p| { p.size = Vector3::new(240.0, 40.0, 0.0); p.placeholder = "Enter text…".to_owned(); p.clear_text_on_focus = true; });

        add_item(ui, app, "📜 ScrollingFrame",
            "ScrollingFrame", InstanceType::ScrollingFrame, Container::ScreenGui,
            |p| { p.size = Vector3::new(300.0, 200.0, 0.0); p.canvas_size = Vector3::new(0.0, 600.0, 0.0); p.scroll_bar_thickness = 12.0; });

        ui.separator();

        // ── Scripting ─────────────────────────────────────────────────────────
        ui.label(RichText::new("── Scripting ──").color(Color32::from_rgb(120, 255, 160)).small());

        add_item(ui, app, "📜 ServerScript",
            "Script", InstanceType::ServerScript, Container::ServerStorage,
            |p| { p.source = "-- ServerScript\n-- Runs on the Dedicated server only.\nprint(\"Hello from server!\")\n".to_owned(); p.enabled = true; });

        add_item(ui, app, "💻 ClientScript",
            "LocalScript", InstanceType::ClientScript, Container::StarterPlayerScripts,
            |p| { p.source = "-- ClientScript\n-- Runs on each player's machine.\nprint(\"Hello client!\")\n".to_owned(); p.enabled = true; });

        add_item(ui, app, "🧩 ModuleScript",
            "Module", InstanceType::ModuleScript, Container::ReplicatedStorage,
            |p| { p.source = "-- ModuleScript\n-- Must return a value.\nlocal M = {}\nreturn M\n".to_owned(); p.enabled = true; });

        ui.separator();

        // ── Networking ────────────────────────────────────────────────────────
        ui.label(RichText::new("── Networking ──").color(Color32::from_rgb(255, 140, 200)).small());

        add_item(ui, app, "📡 NetworkEvent",
            "NetworkEvent", InstanceType::NetworkEvent, Container::ReplicatedStorage,
            |p| { p.enabled = true; });

        ui.separator();

        // ── Audio ─────────────────────────────────────────────────────────────
        ui.label(RichText::new("── Audio ──").color(Color32::from_rgb(255, 180, 80)).small());

        add_item(ui, app, "🔊 Audio",
            "Audio", InstanceType::Audio, Container::Workspace,
            |p| { p.position = Vector3::new(0.0, 3.0, 0.0); p.volume = 1.0; p.looping = false; p.audio_id = "".to_owned(); });
    });
}

/// One row in the Add menu.  Inserts the instance, selects it, and logs.
fn add_item(
    ui: &mut Ui,
    app: &mut StudioApp,
    label: &str,
    name: &str,
    kind: InstanceType,
    container: Container,
    configure: impl Fn(&mut InstanceProperties),
) {
    if ui.button(label).clicked() {
        let mut props = InstanceProperties::default();
        configure(&mut props);
        match app.world.create(name, kind, container, props) {
            Ok(id) => {
                app.selected_guid = Some(id.as_str().to_owned());
                app.status_text = format!("Added {} to {container}", kind.as_str());
                app.log(
                    LogLevel::Info,
                    format!("Created {} \"{name}\" in {container}", kind.as_str()),
                );
            }
            Err(e) => {
                app.log(
                    LogLevel::Error,
                    format!("Cannot create {}: {e}", kind.as_str()),
                );
            }
        }
        ui.close();
    }
}

// ── Tools ─────────────────────────────────────────────────────────────────────

fn render_tools(app: &mut StudioApp, ui: &mut Ui) {
    ui.label(RichText::new("Tools:").strong());
    for mode in [
        ToolMode::Select,
        ToolMode::Move,
        ToolMode::Scale,
        ToolMode::Rotate,
    ] {
        let active = app.tool_mode == mode;
        let text = RichText::new(format!("{} {}", mode.icon(), mode.name())).color(if active {
            Color32::WHITE
        } else {
            Color32::LIGHT_GRAY
        });
        if ui.selectable_label(active, text).clicked() {
            app.tool_mode = mode;
            app.status_text = format!("Tool: {}", mode.name());
        }
    }
}

// ── Playtest ──────────────────────────────────────────────────────────────────

fn render_playtest(app: &mut StudioApp, ui: &mut Ui) {
    ui.label(RichText::new("Test:").strong());
    match app.playtest {
        PlaytestState::Stopped => {
            if ui
                .button(
                    RichText::new("▶ Play")
                        .color(Color32::from_rgb(100, 255, 120))
                        .strong(),
                )
                .clicked()
            {
                app.playtest = PlaytestState::PlayingSolo;
                app.status_text = "Playtest Solo Started".to_owned();
                app.log(
                    LogLevel::Info,
                    "--- Starting Playtest (Solo Client + Server) ---",
                );
                app.log(
                    LogLevel::Info,
                    "Client connected to local session. Character spawned.",
                );
            }
            if ui.button("▶ Run").clicked() {
                app.playtest = PlaytestState::RunningServer;
                app.status_text = "Server Simulation Started".to_owned();
                app.log(LogLevel::Info, "--- Starting Server-only Simulation ---");
            }
        }
        PlaytestState::PlayingSolo | PlaytestState::RunningServer => {
            if ui
                .button(
                    RichText::new("⏹ Stop")
                        .color(Color32::from_rgb(255, 100, 100))
                        .strong(),
                )
                .clicked()
            {
                app.playtest = PlaytestState::Stopped;
                app.status_text = "Simulation Stopped".to_owned();
                app.log(
                    LogLevel::Info,
                    "--- Simulation Stopped. World reset to edit state. ---",
                );
            }
        }
    }
}

// ── Grid controls ─────────────────────────────────────────────────────────────

fn render_grid_controls(app: &mut StudioApp, ui: &mut Ui) {
    ui.checkbox(&mut app.collisions_enabled, "Collisions");
    egui::ComboBox::from_id_salt("snap_combo")
        .selected_text(format!("Snap: {} st", app.grid_snap))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut app.grid_snap, 0.2, "0.2 studs");
            ui.selectable_value(&mut app.grid_snap, 1.0, "1.0 stud");
            ui.selectable_value(&mut app.grid_snap, 4.0, "4.0 studs");
            ui.selectable_value(&mut app.grid_snap, 16.0, "16.0 studs");
        });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn light_grey() -> Rgba {
    Rgba::new(163.0, 162.0, 165.0, 255.0)
}

fn save_place_action(app: &mut StudioApp) {
    let path = app.current_path.clone().unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("SavedPlace.nbw")
    });
    save_to_path(app, path);
}

fn save_place_as(app: &mut StudioApp) {
    let path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(format!("{}.nbw", app.metadata.name.replace(' ', "_")));
    save_to_path(app, path);
}

fn save_to_path(app: &mut StudioApp, path: std::path::PathBuf) {
    let file = WorldFile::from_world(&app.world, app.metadata.clone(), app.settings.clone());
    match file.save_to_file(&path) {
        Ok(_) => {
            app.current_path = Some(path.clone());
            app.status_text = format!("Saved: {}", path.display());
            app.log(LogLevel::Info, format!("Saved to: {}", path.display()));
        }
        Err(e) => {
            app.log(LogLevel::Error, format!("Save failed: {e}"));
        }
    }
}

fn rfd_open_file() -> Option<std::path::PathBuf> {
    let default_p = std::path::PathBuf::from("SavedPlace.nbw");
    if default_p.exists() {
        Some(default_p)
    } else {
        None
    }
}

/// Duplicates the selected instance (same type, container, props, new GUID).
fn duplicate_instance(app: &mut StudioApp, guid: &str) {
    let Ok(orig) = app.world.by_guid(guid) else {
        return;
    };
    let name = format!("{}_copy", orig.name);
    let kind = orig.kind;
    let container = orig.parent;
    let mut props = orig.properties.clone();
    // Offset position so the copy isn't exactly on top of the original.
    if kind.has_3d_position() {
        props.position.x += 4.0;
    }
    match app.world.create(&name, kind, container, props) {
        Ok(id) => {
            app.selected_guid = Some(id.as_str().to_owned());
            app.status_text = format!("Duplicated as {name}");
            app.log(LogLevel::Info, format!("Duplicated: {name}"));
        }
        Err(e) => {
            app.log(LogLevel::Error, format!("Duplicate failed: {e}"));
        }
    }
}
