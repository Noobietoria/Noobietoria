//! Toolbox window: Roblox-style asset catalog with search and 1-click insertion.

use egui::{Color32, RichText, ScrollArea, Ui};
use nb_types::Vector3;
use nb_world::ToolboxCatalog;

use crate::state::{LogLevel, StudioApp};

pub fn render_toolbox(app: &mut StudioApp, ui: &mut Ui) {
    ui.heading("Toolbox");
    ui.separator();

    // Category Selector
    ui.horizontal(|ui| {
        for cat in ["All", "Models", "Parts", "Scripts"] {
            let is_sel = app.toolbox_category == cat;
            if ui.selectable_label(is_sel, cat).clicked() {
                app.toolbox_category = cat.to_owned();
            }
        }
    });

    // Search bar
    ui.horizontal(|ui| {
        ui.label("🔍");
        ui.text_edit_singleline(&mut app.toolbox_query);
        if !app.toolbox_query.is_empty() {
            if ui.small_button("✖").clicked() {
                app.toolbox_query.clear();
            }
        }
    });

    ui.separator();

    let category_filter = if app.toolbox_category == "All" {
        None
    } else {
        Some(app.toolbox_category.as_str())
    };

    let items = ToolboxCatalog::search(&app.toolbox_query, category_filter);

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if items.is_empty() {
                ui.label(RichText::new("No assets found matching query.").italics());
                return;
            }

            for asset in items {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(asset.icon).size(24.0));
                        ui.vertical(|ui| {
                            ui.label(RichText::new(asset.name).strong());
                            ui.label(
                                RichText::new(format!("By: {} | {}", asset.author, asset.category))
                                    .weak()
                                    .size(11.0),
                            );
                        });
                    });

                    ui.label(RichText::new(asset.description).size(12.0));

                    ui.horizontal(|ui| {
                        if ui
                            .button(
                                RichText::new("➕ Insert into Workspace")
                                    .color(Color32::from_rgb(120, 220, 255)),
                            )
                            .clicked()
                        {
                            let origin = Vector3::new(
                                app.camera.target.x,
                                app.camera.target.y.max(1.0),
                                app.camera.target.z,
                            );
                            match ToolboxCatalog::insert_into_world(
                                &mut app.world,
                                asset.id,
                                origin,
                            ) {
                                Ok(ids) => {
                                    if let Some(first) = ids.first() {
                                        app.selected_guid = Some(first.to_string());
                                    }
                                    app.log(
                                        LogLevel::Info,
                                        format!("Inserted asset '{}' into Workspace", asset.name),
                                    );
                                    app.status_text = format!("Inserted {}", asset.name);
                                }
                                Err(err) => {
                                    app.log(
                                        LogLevel::Error,
                                        format!("Failed to insert asset: {err}"),
                                    );
                                }
                            }
                        }
                    });
                });
                ui.add_space(4.0);
            }
        });
}
