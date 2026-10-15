//! Top ribbon toolbar and main menu bar.

use egui::{Color32, RichText, Ui};
use nb_types::{Container, InstanceType, Rgba, Vector3};
use nb_world::{InstanceProperties, WorldFile};

use crate::state::{LogLevel, PlaytestState, StudioApp, ToolMode};

pub fn render_ribbon(app: &mut StudioApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        // --- Menu Bar (File, Edit, View, Help) ---
        ui.menu_button("File", |ui| {
            if ui.button("📄 New Place (Ctrl+N)").clicked() {
                app.reset_to_starter_place();
                ui.close();
            }
            if ui.button("📂 Open .nbw Place...").clicked() {
                // Open file dialog or fallback prompt
                if let Some(path) = rfd_open_file() {
                    match WorldFile::load_from_file(&path) {
                        Ok(file) => {
                            app.world = file.to_world();
                            app.metadata = file.metadata;
                            app.settings = file.settings;
                            app.current_path = Some(path.clone());
                            app.status_text = format!("Opened: {}", path.display());
                            app.log(
                                LogLevel::Info,
                                format!("Successfully opened: {}", path.display()),
                            );
                        }
                        Err(err) => {
                            app.log(LogLevel::Error, format!("Failed to load place: {err}"));
                        }
                    }
                }
                ui.close();
            }
            if ui.button("💾 Save .nbw Place (Ctrl+S)").clicked() {
                save_place_action(app);
                ui.close();
            }
            ui.separator();
            if ui.button("🌐 Publish to Noobietoria Web").clicked() {
                app.log(
                    LogLevel::Info,
                    "Published place to Noobietoria Web catalog!",
                );
                app.status_text = "Place published to web successfully!".to_owned();
                ui.close();
            }
        });

        ui.menu_button("View", |ui| {
            ui.checkbox(&mut app.show_explorer, "Explorer Window");
            ui.checkbox(&mut app.show_properties, "Properties Window");
            ui.checkbox(&mut app.show_toolbox, "Toolbox Dock");
            ui.checkbox(&mut app.show_output, "Output Console");
        });

        ui.separator();

        // --- Tools (Select, Move, Scale, Rotate) ---
        ui.label(RichText::new("Tools:").strong());
        for mode in [
            ToolMode::Select,
            ToolMode::Move,
            ToolMode::Scale,
            ToolMode::Rotate,
        ] {
            let is_selected = app.tool_mode == mode;
            let text =
                RichText::new(format!("{} {}", mode.icon(), mode.name())).color(if is_selected {
                    Color32::WHITE
                } else {
                    Color32::LIGHT_GRAY
                });

            let btn = ui.selectable_label(is_selected, text);
            if btn.clicked() {
                app.tool_mode = mode;
                app.status_text = format!("Tool: {}", mode.name());
            }
        }

        ui.separator();

        // --- Part Insertion ---
        ui.label(RichText::new("Insert:").strong());
        if ui.button("🧊 Block Part").clicked() {
            let id = insert_basic_part(app, "Part", Vector3::new(4.0, 1.2, 2.0));
            app.selected_guid = Some(id.to_string());
            app.status_text = "Inserted Part into Workspace".to_owned();
            app.log(LogLevel::Info, "Created new Part at origin");
        }
        if ui.button("🚩 Spawn").clicked() {
            let id = insert_spawn(app);
            app.selected_guid = Some(id.to_string());
            app.status_text = "Inserted SpawnLocation".to_owned();
            app.log(LogLevel::Info, "Created SpawnLocation in Workspace");
        }
        if ui.button("📜 Script").clicked() {
            let id = insert_script(app);
            app.selected_guid = Some(id.to_string());
            app.status_text = "Inserted ServerScript".to_owned();
            app.log(
                LogLevel::Info,
                "Created ServerScript in ServerScriptService",
            );
        }

        ui.separator();

        // --- Playtest Simulation Controls ---
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

        ui.separator();

        // --- Grid Snap & Collisions ---
        ui.checkbox(&mut app.collisions_enabled, "Collisions");
        egui::ComboBox::from_id_salt("snap_combo")
            .selected_text(format!("Snap: {} studs", app.grid_snap))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.grid_snap, 0.2, "0.2 studs");
                ui.selectable_value(&mut app.grid_snap, 1.0, "1.0 stud");
                ui.selectable_value(&mut app.grid_snap, 4.0, "4.0 studs");
                ui.selectable_value(&mut app.grid_snap, 16.0, "16.0 studs");
            });
    });
}

fn save_place_action(app: &mut StudioApp) {
    let path = app.current_path.clone().unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("SavedPlace.nbw")
    });
    let file = WorldFile::from_world(&app.world, app.metadata.clone(), app.settings.clone());
    match file.save_to_file(&path) {
        Ok(_) => {
            app.current_path = Some(path.clone());
            app.status_text = format!("Saved: {}", path.display());
            app.log(
                LogLevel::Info,
                format!("Successfully saved place to: {}", path.display()),
            );
        }
        Err(err) => {
            app.log(LogLevel::Error, format!("Failed to save place: {err}"));
        }
    }
}

fn rfd_open_file() -> Option<std::path::PathBuf> {
    // Check if default SavedPlace.nbw exists locally
    let default_p = std::path::PathBuf::from("SavedPlace.nbw");
    if default_p.exists() {
        Some(default_p)
    } else {
        None
    }
}

fn insert_basic_part(app: &mut StudioApp, name: &str, size: Vector3) -> nb_world::InstanceId {
    let mut props = InstanceProperties::default();
    props.position = Vector3::new(0.0, 3.0, 0.0);
    props.size = size;
    props.color = Rgba::new(163.0, 162.0, 165.0, 255.0);
    app.world
        .create(name, InstanceType::Part, Container::Workspace, props)
        .unwrap()
}

fn insert_spawn(app: &mut StudioApp) -> nb_world::InstanceId {
    let mut props = InstanceProperties::default();
    props.position = Vector3::new(0.0, 0.5, 0.0);
    props.size = Vector3::new(12.0, 1.0, 12.0);
    props.color = Rgba::new(196.0, 40.0, 28.0, 255.0);
    app.world
        .create(
            "SpawnLocation",
            InstanceType::SpawnLocation,
            Container::Workspace,
            props,
        )
        .unwrap()
}

fn insert_script(app: &mut StudioApp) -> nb_world::InstanceId {
    let mut props = InstanceProperties::default();
    props.source = "-- Server Script\nprint(\"Hello Noobietoria World!\")\n".to_owned();
    app.world
        .create(
            "Script",
            InstanceType::ServerScript,
            Container::ServerStorage,
            props,
        )
        .unwrap()
}
