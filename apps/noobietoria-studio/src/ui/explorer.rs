//! Explorer hierarchy tree window.

use egui::{Color32, RichText, ScrollArea, Ui};
use nb_types::{Container, InstanceType};

use crate::state::{LogLevel, StudioApp};

pub fn render_explorer(app: &mut StudioApp, ui: &mut Ui) {
    ui.heading("Explorer");
    ui.separator();

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut to_delete: Option<String> = None;

            for container in Container::ALL {
                let container_icon = match container {
                    Container::Workspace => "🌐",
                    Container::ReplicatedStorage => "📦",
                    Container::ServerStorage => "🔒",
                    Container::StarterPlayerScripts => "💻",
                    Container::ScreenGui => "🖥",
                };

                let header_text = format!("{container_icon} {container}");
                egui::CollapsingHeader::new(RichText::new(header_text).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        let children = app.world.iter_container(container);
                        if children.is_empty() {
                            ui.label(
                                RichText::new("  (empty)")
                                    .italics()
                                    .color(Color32::DARK_GRAY),
                            );
                        } else {
                            for instance in children {
                                let guid = instance.id.as_str().to_owned();
                                let is_selected = app.selected_guid.as_deref() == Some(&guid);
                                let kind_icon = instance_type_icon(instance.kind);

                                ui.horizontal(|ui| {
                                    let label_text = format!("{kind_icon} {}", instance.name);
                                    let selectable = ui.selectable_label(
                                        is_selected,
                                        RichText::new(label_text).color(if is_selected {
                                            Color32::WHITE
                                        } else {
                                            Color32::LIGHT_GRAY
                                        }),
                                    );

                                    if selectable.clicked() {
                                        app.selected_guid = Some(guid.clone());
                                        app.status_text = format!("Selected: {}", instance.name);
                                    }

                                    if is_selected {
                                        if ui
                                            .small_button("🗑")
                                            .on_hover_text("Delete Instance")
                                            .clicked()
                                        {
                                            to_delete = Some(guid.clone());
                                        }
                                    }
                                });
                            }
                        }
                    });
            }

            if let Some(guid) = to_delete {
                if let Ok(name) = app.world.by_guid(&guid).map(|i| i.name.clone()) {
                    let _ = app.world.destroy_by_guid(&guid);
                    app.log(
                        LogLevel::Info,
                        format!("Destroyed instance: {name} ({guid})"),
                    );
                    app.selected_guid = None;
                    app.status_text = format!("Deleted {name}");
                }
            }
        });
}

fn instance_type_icon(kind: InstanceType) -> &'static str {
    match kind {
        InstanceType::Part => "🧊",
        InstanceType::TrussPart => "🪜",
        InstanceType::Attachment => "📍",
        InstanceType::Model => "📁",
        InstanceType::SpawnLocation => "🚩",
        InstanceType::Trigger => "⚡",
        InstanceType::ScreenGui => "🖥",
        InstanceType::Frame => "🔲",
        InstanceType::TextLabel => "🔤",
        InstanceType::TextButton => "🔘",
        InstanceType::ImageLabel => "🖼",
        InstanceType::ImageButton => "🖻",
        InstanceType::TextBox => "📝",
        InstanceType::ScrollingFrame => "📜",
        InstanceType::ServerScript => "📜",
        InstanceType::ClientScript => "💻",
        InstanceType::ModuleScript => "🧩",
        InstanceType::NetworkEvent => "📡",
        InstanceType::Audio => "🔊",
    }
}
