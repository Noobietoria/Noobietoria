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
        // Playtest simulation tick
        if self.playtest != PlaytestState::Stopped {
            let dt = ctx.input(|i| i.stable_dt as f64).min(0.1);
            self.simulation_time += dt;
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
    }
}
