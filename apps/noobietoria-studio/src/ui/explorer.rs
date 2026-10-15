/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Explorer hierarchy tree – lists all containers and their instances.
//!
//! Features:
//!  - All five containers shown as collapsible sections.
//!  - Click to select; selected item is highlighted.
//!  - Right-click → context menu: Duplicate, Delete, Focus Camera.
//!  - Each container header has a quick-add icon strip for all allowed types.

use egui::{Color32, RichText, ScrollArea, Ui};
use nb_types::{Container, InstanceType};

use crate::state::{LogLevel, StudioApp};

// ── Public entry point ────────────────────────────────────────────────────────

pub fn render_explorer(app: &mut StudioApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.heading("Explorer");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let n = app.world.len();
            ui.label(RichText::new(format!("{n}")).size(10.0).color(Color32::DARK_GRAY));
        });
    });
    ui.separator();

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut action: Option<ExplorerAction> = None;
            for container in Container::ALL {
                render_container(app, ui, container, &mut action);
            }
            if let Some(act) = action {
                apply_action(app, act);
            }
        });
}

// ── Deferred actions ──────────────────────────────────────────────────────────

enum ExplorerAction {
    Select(String),
    Delete(String),
    Duplicate(String),
    Focus(String),
    OpenScript(String),
    QuickAdd { kind: InstanceType, container: Container },
}

fn apply_action(app: &mut StudioApp, act: ExplorerAction) {
    match act {
        ExplorerAction::Select(guid) => {
            if let Ok(i) = app.world.by_guid(&guid) {
                app.status_text = format!("Selected: {} ({})", i.name, i.kind.as_str());
            }
            app.selected_guid = Some(guid);
        }

        ExplorerAction::Delete(guid) => {
            if let Ok(name) = app.world.by_guid(&guid).map(|i| i.name.clone()) {
                let _ = app.world.destroy_by_guid(&guid);
                app.log(LogLevel::Info, format!("Deleted: {name}"));
                app.status_text = format!("Deleted {name}");
                app.selected_guid = None;
            }
        }

        ExplorerAction::Duplicate(guid) => {
            let Ok(orig) = app.world.by_guid(&guid) else { return; };
            let name = format!("{}_copy", orig.name);
            let kind = orig.kind;
            let container = orig.parent;
            let mut props = orig.properties.clone();
            if kind.has_3d_position() {
                props.position.x += 4.0;
            }
            match app.world.create(&name, kind, container, props) {
                Ok(id) => {
                    app.selected_guid = Some(id.as_str().to_owned());
                    app.status_text = format!("Duplicated as {name}");
                    app.log(LogLevel::Info, format!("Duplicated: {name}"));
                }
                Err(e) => app.log(LogLevel::Error, format!("Duplicate failed: {e}")),
            }
        }

        ExplorerAction::Focus(guid) => {
            if let Ok(inst) = app.world.by_guid(&guid) {
                let p = inst.properties.position;
                app.camera.target = nb_types::Vector3::new(p.x, p.y, p.z);
                app.camera.distance = 15.0;
                app.status_text = format!("Focused on {}", inst.name);
            }
        }

        ExplorerAction::OpenScript(guid) => {
            if let Ok(inst) = app.world.by_guid(&guid) {
                app.selected_guid = Some(guid.clone());
                app.script_editor_target = Some(guid.clone());
                app.script_buffer = inst.properties.source.clone();
                app.script_editor_open = true;
                app.status_text = format!("Editing script: {}", inst.name);
                app.log(LogLevel::Info, format!("Opened script in Editor: {}", inst.name));
            }
        }

        ExplorerAction::QuickAdd { kind, container } => {
            use nb_world::InstanceProperties;
            let mut props = InstanceProperties::default();
            if kind.has_3d_position() {
                props.position = nb_types::Vector3::new(0.0, 3.0, 0.0);
            }
            if kind.is_script() {
                props.source = format!("-- {}\n", kind.as_str());
            }
            match app.world.create(kind.as_str(), kind, container, props) {
                Ok(id) => {
                    app.selected_guid = Some(id.as_str().to_owned());
                    app.status_text = format!("Added {} to {container}", kind.as_str());
                    app.log(LogLevel::Info, format!("Quick-added {} to {container}", kind.as_str()));
                }
                Err(e) => app.log(LogLevel::Error, format!("Add failed: {e}")),
            }
        }
    }
}

// ── Container section ─────────────────────────────────────────────────────────

fn render_container(
    app: &mut StudioApp,
    ui: &mut Ui,
    container: Container,
    action: &mut Option<ExplorerAction>,
) {
    let (icon, color) = container_style(container);
    let children_n = app.world.iter_container(container).len();
    let header = RichText::new(if children_n > 0 {
        format!("{icon} {container}  ({children_n})")
    } else {
        format!("{icon} {container}")
    })
    .strong()
    .color(color);

    egui::CollapsingHeader::new(header)
        .default_open(true)
        .show(ui, |ui| {
            // Quick-add icon strip.
            quick_add_strip(ui, container, action);
            ui.separator();

            let items: Vec<_> = app
                .world
                .iter_container(container)
                .into_iter()
                .map(|i| (i.id.as_str().to_owned(), i.name.clone(), i.kind))
                .collect();

            if items.is_empty() {
                ui.label(RichText::new("  (empty)").italics().color(Color32::DARK_GRAY));
            } else {
                for (guid, name, kind) in items {
                    instance_row(app, ui, guid, name, kind, action);
                }
            }
        });
}

// ── Quick-add strip ───────────────────────────────────────────────────────────

fn quick_add_strip(ui: &mut Ui, container: Container, action: &mut Option<ExplorerAction>) {
    let allowed: Vec<InstanceType> = InstanceType::ALL
        .into_iter()
        .filter(|t| t.is_allowed_in(container))
        .collect();

    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("➕").color(Color32::DARK_GRAY).small());
        for kind in allowed {
            let resp = ui
                .add(egui::Button::new(
                    RichText::new(instance_type_icon(kind)).size(14.0),
                )
                .frame(false)
                .min_size(egui::Vec2::new(20.0, 16.0)))
                .on_hover_text(format!("Add {}", kind.as_str()));
            if resp.clicked() {
                *action = Some(ExplorerAction::QuickAdd { kind, container });
            }
        }
    });
}

// ── Instance row ──────────────────────────────────────────────────────────────

fn instance_row(
    app: &mut StudioApp,
    ui: &mut Ui,
    guid: String,
    name: String,
    kind: InstanceType,
    action: &mut Option<ExplorerAction>,
) {
    let is_sel = app.selected_guid.as_deref() == Some(&guid);
    let icon = instance_type_icon(kind);

    ui.horizontal(|ui| {
        let sel = ui.selectable_label(
            is_sel,
            RichText::new(format!("{icon} {name}"))
                .color(if is_sel { Color32::WHITE } else { Color32::LIGHT_GRAY })
                .size(13.0),
        );

        if sel.double_clicked() && kind.is_script() {
            *action = Some(ExplorerAction::OpenScript(guid.clone()));
        } else if sel.clicked() {
            *action = Some(ExplorerAction::Select(guid.clone()));
        }

        // Right-click context menu.
        sel.context_menu(|ui| {
            ui.label(
                RichText::new(format!("{} – {name}", kind.as_str()))
                    .small()
                    .color(Color32::GRAY),
            );
            ui.separator();
            if kind.is_script() {
                if ui.button("📜  Open in Script Editor").clicked() {
                    *action = Some(ExplorerAction::OpenScript(guid.clone()));
                    ui.close();
                }
                ui.separator();
            }
            if ui.button("🎯  Focus Camera").clicked() {
                *action = Some(ExplorerAction::Focus(guid.clone()));
                ui.close();
            }
            if ui.button("📋  Duplicate").clicked() {
                *action = Some(ExplorerAction::Duplicate(guid.clone()));
                ui.close();
            }
            ui.separator();
            if ui
                .button(RichText::new("🗑  Delete").color(Color32::from_rgb(255, 100, 100)))
                .clicked()
            {
                *action = Some(ExplorerAction::Delete(guid.clone()));
                ui.close();
            }
        });

        // Inline action buttons when selected.
        if is_sel {
            if kind.is_script() {
                if ui.small_button("📜").on_hover_text("Open in Editor").clicked() {
                    *action = Some(ExplorerAction::OpenScript(guid.clone()));
                }
            }
            if ui.small_button("🎯").on_hover_text("Focus").clicked() {
                *action = Some(ExplorerAction::Focus(guid.clone()));
            }
            if ui.small_button("📋").on_hover_text("Duplicate").clicked() {
                *action = Some(ExplorerAction::Duplicate(guid.clone()));
            }
            if ui.small_button("🗑").on_hover_text("Delete").clicked() {
                *action = Some(ExplorerAction::Delete(guid.clone()));
            }
        }
    });
}

// ── Styling helpers ───────────────────────────────────────────────────────────

fn container_style(c: Container) -> (&'static str, Color32) {
    match c {
        Container::Workspace            => ("🌐", Color32::from_rgb(130, 190, 255)),
        Container::ReplicatedStorage    => ("📦", Color32::from_rgb(140, 230, 140)),
        Container::ServerStorage        => ("🔒", Color32::from_rgb(255, 180, 80)),
        Container::StarterPlayerScripts => ("💻", Color32::from_rgb(190, 160, 255)),
        Container::ScreenGui            => ("🖥",  Color32::from_rgb(255, 220, 100)),
    }
}

pub fn instance_type_icon(kind: InstanceType) -> &'static str {
    match kind {
        InstanceType::Part           => "🧊",
        InstanceType::TrussPart      => "🪜",
        InstanceType::Attachment     => "📍",
        InstanceType::Model          => "📁",
        InstanceType::SpawnLocation  => "🚩",
        InstanceType::Trigger        => "⚡",
        InstanceType::ScreenGui      => "🖥",
        InstanceType::Frame          => "🔲",
        InstanceType::TextLabel      => "🔤",
        InstanceType::TextButton     => "🔘",
        InstanceType::ImageLabel     => "🖼",
        InstanceType::ImageButton    => "🖻",
        InstanceType::TextBox        => "📝",
        InstanceType::ScrollingFrame => "📜",
        InstanceType::ServerScript   => "📜",
        InstanceType::ClientScript   => "💻",
        InstanceType::ModuleScript   => "🧩",
        InstanceType::NetworkEvent   => "📡",
        InstanceType::Audio          => "🔊",
    }
}

