/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The eframe App lifecycle and main layout orchestration.

use eframe::App;
use egui::{Color32, RichText};

use crate::state::{PlaytestState, StudioApp};
use crate::ui::{
    render_explorer, render_output, render_properties, render_ribbon, render_toolbox,
    render_viewport,
};

impl App for StudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Playtest simulation tick (Inner Server Simulation)
        if self.playtest != PlaytestState::Stopped {
            let dt = ctx.input(|i| i.stable_dt as f32).min(0.05);
            self.simulation_time += dt as f64;

            if self.playtest == PlaytestState::PlayingSolo {
                // Collect keyboard inputs for character movement
                let (w, s, a, d, space, shift) = ctx.input(|i| {
                    (
                        i.key_down(egui::Key::W) || i.key_down(egui::Key::ArrowUp),
                        i.key_down(egui::Key::S) || i.key_down(egui::Key::ArrowDown),
                        i.key_down(egui::Key::A) || i.key_down(egui::Key::ArrowLeft),
                        i.key_down(egui::Key::D) || i.key_down(egui::Key::ArrowRight),
                        i.key_down(egui::Key::Space),
                        i.modifiers.shift,
                    )
                });

                let input = nb_render::PlayerInput {
                    move_forward: w,
                    move_backward: s,
                    move_left: a,
                    move_right: d,
                    jump: space,
                    sprint: shift,
                };

                if let Some(ref mut character) = self.playtest_character {
                    let gravity = self.settings.gravity as f32;
                    let gravity = if gravity <= 0.0 { 196.2 * 0.15 } else { gravity * 0.15 };
                    character.update(dt, &input, self.camera.yaw, gravity);

                    // Camera smoothly follows the player character
                    let target_cam = nb_types::Vector3::new(
                        character.position.x,
                        character.position.y + 2.5,
                        character.position.z,
                    );
                    self.camera.target = target_cam;
                }
            }

            ctx.request_repaint();
        }

        // --- Top Ribbon Bar ---
        egui::TopBottomPanel::top("studio_ribbon")
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(3.0);
                render_ribbon(self, ui);
                ui.add_space(3.0);
            });

        // --- Bottom Status and Output Panel ---
        egui::TopBottomPanel::bottom("studio_bottom")
            .resizable(true)
            .default_height(160.0)
            .min_height(60.0)
            .show(ctx, |ui| {
                if self.show_output {
                    render_output(self, ui);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.status_text).size(11.0).weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "Time: {:.1}s | Instances: {}",
                                self.simulation_time,
                                self.world.len()
                            ))
                            .size(11.0)
                            .color(Color32::GRAY),
                        );
                    });
                });
            });

        // --- Left Panel: Toolbox Dock ---
        if self.show_toolbox {
            egui::SidePanel::left("studio_toolbox")
                .resizable(true)
                .default_width(280.0)
                .min_width(200.0)
                .show(ctx, |ui| {
                    render_toolbox(self, ui);
                });
        }

        // --- Right Panel: Explorer & Properties ---
        if self.show_explorer || self.show_properties {
            egui::SidePanel::right("studio_explorer_properties")
                .resizable(true)
                .default_width(300.0)
                .min_width(220.0)
                .show(ctx, |ui| {
                    if self.show_explorer && self.show_properties {
                        egui::TopBottomPanel::top("inner_explorer")
                            .resizable(true)
                            .default_height(260.0)
                            .show_inside(ui, |ui| {
                                render_explorer(self, ui);
                            });
                        egui::CentralPanel::default().show_inside(ui, |ui| {
                            render_properties(self, ui);
                        });
                    } else if self.show_explorer {
                        render_explorer(self, ui);
                    } else {
                        render_properties(self, ui);
                    }
                });
        }

        // --- Central Panel: 3D Interactive Viewport ---
        egui::CentralPanel::default().show(ctx, |ui| {
            render_viewport(self, ui);
        });

        // --- Dedicated Script Editor Window (Luau IDE) ---
        if self.script_editor_open {
            let mut is_open = self.script_editor_open;
            let script_title = self
                .script_editor_target
                .as_ref()
                .and_then(|g| self.world.by_guid(g).ok())
                .map(|i| format!("📜 Script Editor: {} ({})", i.name, i.kind.as_str()))
                .unwrap_or_else(|| "📜 Script Editor".to_owned());

            let mut close_requested = false;
            let mut save_requested = false;

            egui::Window::new(script_title)
                .open(&mut is_open)
                .default_size([760.0, 520.0])
                .min_size([400.0, 300.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("💾 Apply to Script").clicked() {
                            save_requested = true;
                        }
                        if ui.button("❌ Close").clicked() {
                            close_requested = true;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new("Luau 5.1 Intellisense Active")
                                    .color(Color32::from_rgb(0, 162, 255))
                                    .small(),
                            );
                        });
                    });
                    ui.separator();

                    let modified = crate::ui::render_code_editor(
                        ui,
                        &mut self.script_buffer,
                        &mut self.script_editor_state,
                        (ui.available_height() - 10.0).max(200.0),
                    );

                    // Auto-sync buffer back to target instance when changed
                    if modified {
                        if let Some(guid) = &self.script_editor_target {
                            if let Ok(inst) = self.world.by_guid_mut(guid) {
                                inst.properties.source = self.script_buffer.clone();
                            }
                        }
                    }
                });

            if save_requested {
                if let Some(guid) = &self.script_editor_target {
                    if let Ok(inst) = self.world.by_guid_mut(guid) {
                        inst.properties.source = self.script_buffer.clone();
                        let name = inst.name.clone();
                        self.status_text = format!("Saved script: {name}");
                        self.log(
                            crate::state::LogLevel::Info,
                            format!("Saved changes to {name}"),
                        );
                    }
                }
            }

            if close_requested {
                is_open = false;
            }

            self.script_editor_open = is_open;
        }
    }
}
