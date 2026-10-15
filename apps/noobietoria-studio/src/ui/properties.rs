//! Properties inspector window and in-engine script editor.

use egui::{Color32, DragValue, RichText, ScrollArea, Ui};
use nb_types::Rgba;

use crate::state::StudioApp;

pub fn render_properties(app: &mut StudioApp, ui: &mut Ui) {
    ui.heading("Properties");
    ui.separator();

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if let Some(guid) = app.selected_guid.clone() {
                render_instance_properties(app, &guid, ui);
            } else {
                render_place_settings(app, ui);
            }
        });
}

fn render_instance_properties(app: &mut StudioApp, guid: &str, ui: &mut Ui) {
    let Ok(instance) = app.world.by_guid_mut(guid) else {
        ui.label(RichText::new("Selected instance no longer exists.").italics());
        return;
    };

    ui.collapsing(RichText::new("Identity").strong(), |ui| {
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut instance.name);
        });
        ui.horizontal(|ui| {
            ui.label("Class:");
            ui.label(RichText::new(format!("{:?}", instance.kind)).color(Color32::LIGHT_BLUE));
        });
        ui.horizontal(|ui| {
            ui.label("Container:");
            ui.label(format!("{}", instance.parent));
        });
        ui.horizontal(|ui| {
            ui.label("GUID:");
            ui.label(RichText::new(instance.id.as_str()).monospace().weak());
        });
    });

    ui.separator();

    // Transform
    ui.collapsing(RichText::new("Transform").strong(), |ui| {
        ui.label("Position (X, Y, Z):");
        ui.horizontal(|ui| {
            ui.add(
                DragValue::new(&mut instance.properties.position.x)
                    .prefix("X: ")
                    .speed(0.5),
            );
            ui.add(
                DragValue::new(&mut instance.properties.position.y)
                    .prefix("Y: ")
                    .speed(0.5),
            );
            ui.add(
                DragValue::new(&mut instance.properties.position.z)
                    .prefix("Z: ")
                    .speed(0.5),
            );
        });

        ui.label("Size (X, Y, Z):");
        ui.horizontal(|ui| {
            ui.add(
                DragValue::new(&mut instance.properties.size.x)
                    .prefix("X: ")
                    .speed(0.5),
            );
            ui.add(
                DragValue::new(&mut instance.properties.size.y)
                    .prefix("Y: ")
                    .speed(0.5),
            );
            ui.add(
                DragValue::new(&mut instance.properties.size.z)
                    .prefix("Z: ")
                    .speed(0.5),
            );
        });
    });

    ui.separator();

    // Appearance
    ui.collapsing(RichText::new("Appearance").strong(), |ui| {
        ui.horizontal(|ui| {
            ui.label("Color (RGB):");
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

    // Script properties
    if instance.kind.is_script() {
        ui.separator();
        ui.collapsing(RichText::new("Script Source").strong(), |ui| {
            ui.label("Luau Script Code:");
            ui.text_edit_multiline(&mut instance.properties.source);
        });
    }

    // Audio properties
    if instance.kind == nb_types::InstanceType::Audio {
        ui.separator();
        ui.collapsing(RichText::new("Audio").strong(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Asset ID:");
                ui.text_edit_singleline(&mut instance.properties.audio_id);
            });
            ui.horizontal(|ui| {
                ui.label("Volume:");
                ui.add(
                    DragValue::new(&mut instance.properties.volume)
                        .speed(0.05)
                        .range(0.0..=1.0),
                );
            });
            ui.checkbox(&mut instance.properties.looping, "Looped");
        });
    }
}

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

    ui.collapsing(RichText::new("Metadata").strong(), |ui| {
        ui.horizontal(|ui| {
            ui.label("Place Name:");
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

    ui.collapsing(RichText::new("Physics & Gameplay").strong(), |ui| {
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

    ui.collapsing(RichText::new("Lighting & Sky").strong(), |ui| {
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
