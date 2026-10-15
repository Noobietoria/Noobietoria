/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Output console and interactive command bar.

use egui::{Key, RichText, ScrollArea, Ui};
use nb_types::{Container, InstanceType, Rgba, Vector3};
use nb_world::InstanceProperties;

use crate::state::{LogLevel, StudioApp};

pub fn render_output(app: &mut StudioApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.heading("Output");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Clear").clicked() {
                app.logs.clear();
            }
        });
    });
    ui.separator();

    ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .max_height(140.0)
        .show(ui, |ui| {
            if app.logs.is_empty() {
                ui.label(RichText::new("No output logs yet.").weak().italics());
            } else {
                for entry in &app.logs {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&entry.timestamp).weak().size(11.0));
                        ui.label(
                            RichText::new(&entry.message)
                                .color(entry.level.color())
                                .monospace()
                                .size(12.0),
                        );
                    });
                }
            }
        });

    ui.separator();

    // Command Line Bar
    ui.horizontal(|ui| {
        ui.label(RichText::new(">").monospace().strong());
        let text_response = ui.text_edit_singleline(&mut app.command_input);
        if text_response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            let cmd = app.command_input.trim().to_owned();
            if !cmd.is_empty() {
                execute_command(app, &cmd);
                app.command_input.clear();
                text_response.request_focus();
            }
        }
    });
}

fn execute_command(app: &mut StudioApp, cmd: &str) {
    app.log(LogLevel::Info, format!("> {cmd}"));

    if cmd.eq_ignore_ascii_case("help") {
        app.log(
            LogLevel::Info,
            "Available commands: help, clear, new, save, stats, insert <part|spawn>",
        );
    } else if cmd.eq_ignore_ascii_case("clear") {
        app.logs.clear();
    } else if cmd.eq_ignore_ascii_case("new") {
        app.reset_to_starter_place();
    } else if cmd.eq_ignore_ascii_case("stats") {
        app.log(
            LogLevel::Info,
            format!(
                "World Statistics: Total Instances = {}, Gravity = {} studs/s²",
                app.world.len(),
                app.settings.gravity
            ),
        );
    } else if let Some(rest) = cmd.strip_prefix("print(") {
        let msg = rest
            .trim_end_matches(')')
            .trim_matches('"')
            .trim_matches('\'');
        app.log(LogLevel::Script, msg);
    } else if cmd.starts_with("insert ") {
        let what = cmd["insert ".len()..].trim();
        if what.eq_ignore_ascii_case("part") {
            let mut props = InstanceProperties::default();
            props.position = Vector3::new(0.0, 5.0, 0.0);
            props.size = Vector3::new(4.0, 1.2, 2.0);
            props.color = Rgba::new(163.0, 162.0, 165.0, 255.0);
            let id = app
                .world
                .create("Part", InstanceType::Part, Container::Workspace, props)
                .unwrap();
            app.selected_guid = Some(id.to_string());
            app.log(LogLevel::Info, format!("Created Part: {id}"));
        } else if what.eq_ignore_ascii_case("spawn") {
            let mut props = InstanceProperties::default();
            props.position = Vector3::new(0.0, 0.5, 0.0);
            props.size = Vector3::new(12.0, 1.0, 12.0);
            props.color = Rgba::new(196.0, 40.0, 28.0, 255.0);
            let id = app
                .world
                .create(
                    "SpawnLocation",
                    InstanceType::SpawnLocation,
                    Container::Workspace,
                    props,
                )
                .unwrap();
            app.selected_guid = Some(id.to_string());
            app.log(LogLevel::Info, format!("Created SpawnLocation: {id}"));
        } else {
            app.log(
                LogLevel::Warning,
                format!("Unknown asset type to insert: {what}"),
            );
        }
    } else {
        app.log(
            LogLevel::Warning,
            format!("Unknown command: '{cmd}'. Type 'help' for commands list."),
        );
    }
}
