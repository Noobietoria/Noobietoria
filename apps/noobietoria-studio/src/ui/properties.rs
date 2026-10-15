/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Properties inspector – shows every editable field for the selected instance.
//!
//! Sections follow the documented property names from `instance.rs`:
//!   Identity, Transform (3D only), Appearance, GUI (GUI types only),
//!   Script Source (script types), Audio (Audio type), Networking (NetworkEvent).

use egui::{Color32, DragValue, RichText, ScrollArea, Ui};
use nb_types::{InstanceType, Rgba};

use crate::state::StudioApp;

// ── Public entry point ────────────────────────────────────────────────────────

pub fn render_properties(app: &mut StudioApp, ui: &mut Ui) {
    ui.heading("Properties");
    ui.separator();

    let mut open_editor_req = None;

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if let Some(guid) = app.selected_guid.clone() {
                render_instance_properties(
                    &mut app.world,
                    &mut app.script_editor_state,
                    &guid,
                    ui,
                    &mut open_editor_req,
                );
            } else {
                render_place_settings(app, ui);
            }
        });

    if let Some(guid) = open_editor_req {
        if let Ok(inst) = app.world.by_guid(&guid) {
            app.script_editor_target = Some(guid.clone());
            app.script_buffer = inst.properties.source.clone();
            app.script_editor_open = true;
            app.status_text = format!("Editing script: {}", inst.name);
        }
    }
}

// ── Instance inspector ────────────────────────────────────────────────────────

fn render_instance_properties(
    world: &mut nb_world::World,
    editor_state: &mut crate::ui::ScriptEditorState,
    guid: &str,
    ui: &mut Ui,
    open_editor_req: &mut Option<String>,
) {
    let Ok(instance) = world.by_guid_mut(guid) else {
        ui.label(RichText::new("Selected instance no longer exists.").italics());
        return;
    };

    // ── Identity ─────────────────────────────────────────────────────────────
    ui.collapsing(RichText::new("🪪 Identity").strong(), |ui| {
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut instance.name);
        });
        ui.horizontal(|ui| {
            ui.label("Class:");
            ui.label(
                RichText::new(instance.kind.as_str())
                    .color(Color32::from_rgb(120, 190, 255))
                    .monospace(),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Container:");
            ui.label(RichText::new(instance.parent.as_str()).color(Color32::LIGHT_GRAY));
        });
        ui.horizontal(|ui| {
            ui.label("GUID:");
            ui.label(
                RichText::new(instance.id.as_str())
                    .monospace()
                    .weak()
                    .small(),
            );
        });
    });

    ui.separator();

    // ── Transform (3D objects) ────────────────────────────────────────────────
    if instance.kind.has_3d_position() {
        ui.collapsing(RichText::new("📐 Transform").strong(), |ui| {
            // Position
            ui.label(RichText::new("Position (X Y Z):").weak().small());
            ui.horizontal(|ui| {
                ui.add(
                    DragValue::new(&mut instance.properties.position.x)
                        .prefix("X ")
                        .speed(0.5),
                );
                ui.add(
                    DragValue::new(&mut instance.properties.position.y)
                        .prefix("Y ")
                        .speed(0.5),
                );
                ui.add(
                    DragValue::new(&mut instance.properties.position.z)
                        .prefix("Z ")
                        .speed(0.5),
                );
            });

            // Orientation
            ui.label(RichText::new("Orientation deg (X Y Z):").weak().small());
            ui.horizontal(|ui| {
                ui.add(
                    DragValue::new(&mut instance.properties.orientation.x)
                        .prefix("X ")
                        .speed(1.0),
                );
                ui.add(
                    DragValue::new(&mut instance.properties.orientation.y)
                        .prefix("Y ")
                        .speed(1.0),
                );
                ui.add(
                    DragValue::new(&mut instance.properties.orientation.z)
                        .prefix("Z ")
                        .speed(1.0),
                );
            });

            // Size
            ui.label(RichText::new("Size (X Y Z):").weak().small());
            ui.horizontal(|ui| {
                ui.add(
                    DragValue::new(&mut instance.properties.size.x)
                        .prefix("X ")
                        .speed(0.25)
                        .range(0.05..=f64::MAX),
                );
                ui.add(
                    DragValue::new(&mut instance.properties.size.y)
                        .prefix("Y ")
                        .speed(0.25)
                        .range(0.05..=f64::MAX),
                );
                ui.add(
                    DragValue::new(&mut instance.properties.size.z)
                        .prefix("Z ")
                        .speed(0.25)
                        .range(0.05..=f64::MAX),
                );
            });

            // TrussPart extras
            if instance.kind == InstanceType::TrussPart {
                ui.horizontal(|ui| {
                    ui.label("ClimbSpeed (studs/s):");
                    ui.add(
                        DragValue::new(&mut instance.properties.climb_speed)
                            .speed(0.5)
                            .range(0.0..=200.0),
                    );
                });
            }
        });
        ui.separator();
    }

    // ── Appearance ────────────────────────────────────────────────────────────
    ui.collapsing(RichText::new("🎨 Appearance").strong(), |ui| {
        ui.horizontal(|ui| {
            ui.label("Color (RGBA):");
            let mut rgb = [
                (instance.properties.color.r / 255.0) as f32,
                (instance.properties.color.g / 255.0) as f32,
                (instance.properties.color.b / 255.0) as f32,
            ];
            if ui.color_edit_button_rgb(&mut rgb).changed() {
                instance.properties.color = Rgba::new(
                    (rgb[0] * 255.0) as f64,
                    (rgb[1] * 255.0) as f64,
                    (rgb[2] * 255.0) as f64,
                    255.0,
                );
            }
        });
        ui.checkbox(&mut instance.properties.visible, "Visible");
        ui.checkbox(&mut instance.properties.enabled, "Enabled");
    });

    ui.separator();

    // ── GUI properties ────────────────────────────────────────────────────────
    if instance.kind.is_gui() {
        ui.collapsing(RichText::new("🖥 GUI").strong(), |ui| {
            // Size (XY for GUI)
            ui.label(RichText::new("Size (W H):").weak().small());
            ui.horizontal(|ui| {
                ui.add(
                    DragValue::new(&mut instance.properties.size.x)
                        .prefix("W ")
                        .speed(1.0)
                        .range(0.0..=f64::MAX),
                );
                ui.add(
                    DragValue::new(&mut instance.properties.size.y)
                        .prefix("H ")
                        .speed(1.0)
                        .range(0.0..=f64::MAX),
                );
            });

            // Text fields
            if matches!(
                instance.kind,
                InstanceType::TextLabel | InstanceType::TextButton | InstanceType::TextBox
            ) {
                ui.horizontal(|ui| {
                    ui.label("Text:");
                    ui.text_edit_singleline(&mut instance.properties.text);
                });
                if instance.kind == InstanceType::TextBox {
                    ui.horizontal(|ui| {
                        ui.label("Placeholder:");
                        ui.text_edit_singleline(&mut instance.properties.placeholder);
                    });
                    ui.checkbox(
                        &mut instance.properties.clear_text_on_focus,
                        "ClearTextOnFocus",
                    );
                    ui.checkbox(&mut instance.properties.multi_line, "MultiLine");
                }
                ui.horizontal(|ui| {
                    ui.label("Font:");
                    ui.text_edit_singleline(&mut instance.properties.font);
                });
                ui.horizontal(|ui| {
                    ui.label("TextSize:");
                    ui.add(
                        DragValue::new(&mut instance.properties.text_size)
                            .speed(0.5)
                            .range(6.0..=200.0),
                    );
                });
            }

            // Image fields
            if matches!(
                instance.kind,
                InstanceType::ImageLabel | InstanceType::ImageButton
            ) {
                ui.horizontal(|ui| {
                    ui.label("Image (asset ID):");
                    ui.text_edit_singleline(&mut instance.properties.image);
                });
                ui.horizontal(|ui| {
                    ui.label("ImageTransparency:");
                    ui.add(
                        DragValue::new(&mut instance.properties.image_transparency)
                            .speed(0.01)
                            .range(0.0..=1.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("ScaleType:");
                    ui.text_edit_singleline(&mut instance.properties.scale_type);
                });
            }

            // ScrollingFrame extras
            if instance.kind == InstanceType::ScrollingFrame {
                ui.label(RichText::new("CanvasSize (W H):").weak().small());
                ui.horizontal(|ui| {
                    ui.add(
                        DragValue::new(&mut instance.properties.canvas_size.x)
                            .prefix("W ")
                            .speed(1.0),
                    );
                    ui.add(
                        DragValue::new(&mut instance.properties.canvas_size.y)
                            .prefix("H ")
                            .speed(1.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("ScrollBarThickness:");
                    ui.add(
                        DragValue::new(&mut instance.properties.scroll_bar_thickness)
                            .speed(0.5)
                            .range(0.0..=50.0),
                    );
                });
                ui.checkbox(
                    &mut instance.properties.scrolling_enabled,
                    "ScrollingEnabled",
                );
                ui.horizontal(|ui| {
                    ui.label("ElasticBehavior:");
                    ui.text_edit_singleline(&mut instance.properties.elastic_behavior);
                });
            }
        });
        ui.separator();
    }

    // ── Script source ─────────────────────────────────────────────────────────
    if instance.kind.is_script() {
        ui.collapsing(
            RichText::new("📜 Script Source & Intellisense").strong(),
            |ui| {
                let kind_label = match instance.kind {
                    InstanceType::ServerScript => "ServerScript  – runs on Dedicated server only",
                    InstanceType::ClientScript => "ClientScript  – runs on each player's machine",
                    InstanceType::ModuleScript => {
                        "ModuleScript  – cached module, must return a value"
                    }
                    _ => "",
                };
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(kind_label)
                            .small()
                            .color(Color32::DARK_GRAY)
                            .italics(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("↗ Full Editor")
                            .on_hover_text("Open dedicated floating script editor window")
                            .clicked()
                        {
                            *open_editor_req = Some(instance.id.as_str().to_owned());
                        }
                    });
                });
                ui.add_space(2.0);
                crate::ui::render_code_editor(
                    ui,
                    &mut instance.properties.source,
                    editor_state,
                    220.0,
                );
            },
        );
        ui.separator();
    }

    // ── Audio ─────────────────────────────────────────────────────────────────
    if instance.kind == InstanceType::Audio {
        ui.collapsing(RichText::new("🔊 Audio").strong(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Asset ID:");
                ui.text_edit_singleline(&mut instance.properties.audio_id);
            });
            ui.horizontal(|ui| {
                ui.label("Volume:");
                ui.add(
                    DragValue::new(&mut instance.properties.volume)
                        .speed(0.02)
                        .range(0.0..=1.0),
                );
            });
            ui.checkbox(&mut instance.properties.looping, "Looped");
        });
        ui.separator();
    }

    // ── NetworkEvent ──────────────────────────────────────────────────────────
    if instance.kind == InstanceType::NetworkEvent {
        ui.collapsing(RichText::new("📡 NetworkEvent").strong(), |ui| {
            ui.label(
                RichText::new(
                    "Two-way client/server channel.\n\
                     Place in ReplicatedStorage so both sides can reach it.\n\
                     Fire with: NetworkService.FireEvent(name, ...args)",
                )
                .small()
                .color(Color32::DARK_GRAY),
            );
            ui.checkbox(&mut instance.properties.enabled, "Enabled");
        });
        ui.separator();
    }
}

// ── Place settings (nothing selected) ────────────────────────────────────────

fn render_place_settings(app: &mut StudioApp, ui: &mut Ui) {
    ui.label(
        RichText::new("Place Settings")
            .strong()
            .color(Color32::LIGHT_GREEN),
    );
    ui.label(
        RichText::new("(Select an instance in Explorer to view properties)")
            .weak()
            .italics(),
    );
    ui.separator();

    ui.collapsing(RichText::new("📄 Metadata").strong(), |ui| {
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut app.metadata.name);
        });
        ui.horizontal(|ui| {
            ui.label("Creator:");
            ui.text_edit_singleline(&mut app.metadata.creator);
        });
        ui.horizontal(|ui| {
            ui.label("Description:");
            ui.text_edit_singleline(&mut app.metadata.description);
        });
    });

    ui.separator();

    ui.collapsing(RichText::new("⚙ Physics & Gameplay").strong(), |ui| {
        ui.horizontal(|ui| {
            ui.label("Gravity (studs/s²):");
            ui.add(
                DragValue::new(&mut app.settings.gravity)
                    .speed(1.0)
                    .range(0.0..=1000.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Max Players:");
            ui.add(
                DragValue::new(&mut app.settings.max_players)
                    .speed(1)
                    .range(1..=100),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Walk Speed:");
            ui.add(
                DragValue::new(&mut app.settings.walk_speed)
                    .speed(0.5)
                    .range(0.0..=200.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Jump Power:");
            ui.add(
                DragValue::new(&mut app.settings.jump_power)
                    .speed(1.0)
                    .range(0.0..=500.0),
            );
        });
    });

    ui.separator();

    ui.collapsing(RichText::new("🌤 Lighting & Sky").strong(), |ui| {
        ui.horizontal(|ui| {
            ui.label("Time of Day:");
            ui.text_edit_singleline(&mut app.settings.time_of_day);
        });
        ui.horizontal(|ui| {
            ui.label("Ambient Color:");
            ui.color_edit_button_rgb(&mut app.settings.ambient_color);
        });
        ui.horizontal(|ui| {
            ui.label("Brightness:");
            ui.add(
                DragValue::new(&mut app.settings.brightness)
                    .speed(0.1)
                    .range(0.0..=10.0),
            );
        });
    });
}
