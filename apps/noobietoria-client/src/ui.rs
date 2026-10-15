/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! egui rendering for the client.
//!
//! Three screens:
//!
//! * [`draw_connect`] – the login / connect form.
//! * [`draw_connecting`] – a spinner while the handshake is pending.
//! * [`draw_lobby`] – the main game HUD (world view, event console, log).

use eframe::egui::{self, Color32, RichText, Sense, Stroke, Vec2};

use crate::state::{ClientApp, LogKind, Screen};

// ── Palette ────────────────────────────────────────────────────────────────────

const BG: Color32 = Color32::from_rgb(22, 22, 30);
const PANEL: Color32 = Color32::from_rgb(30, 30, 42);
const ACCENT: Color32 = Color32::from_rgb(91, 163, 255);
const GREEN: Color32 = Color32::from_rgb(80, 220, 130);
const RED: Color32 = Color32::from_rgb(255, 100, 100);
const YELLOW: Color32 = Color32::from_rgb(255, 210, 80);
const DIM: Color32 = Color32::from_rgb(120, 120, 140);

// ── Entry point ────────────────────────────────────────────────────────────────

pub fn draw(app: &mut ClientApp, ctx: &egui::Context) {
    // Dark visuals.
    let mut style = (*ctx.style()).clone();
    style.visuals.window_fill = BG;
    style.visuals.panel_fill = PANEL;
    style.visuals.override_text_color = Some(Color32::from_rgb(220, 220, 235));
    ctx.set_style(style);

    match app.screen {
        Screen::Connect => draw_connect(app, ctx),
        Screen::Connecting => draw_connecting(app, ctx),
        Screen::Lobby => draw_lobby(app, ctx),
    }
}

// ── Connect screen ─────────────────────────────────────────────────────────────

fn draw_connect(app: &mut ClientApp, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::default().fill(BG))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);

                // Logo text.
                ui.label(
                    RichText::new("🟦 NOOBIETORIA")
                        .size(36.0)
                        .color(ACCENT)
                        .strong(),
                );
                ui.label(RichText::new("Client").size(16.0).color(DIM));

                ui.add_space(32.0);

                // Form card.
                egui::Frame::default()
                    .fill(PANEL)
                    .corner_radius(12.0)
                    .inner_margin(egui::Margin::same(24))
                    .stroke(Stroke::new(1.0, ACCENT.linear_multiply(0.3)))
                    .show(ui, |ui| {
                        ui.set_min_width(340.0);

                        field(ui, "Username", &mut app.username);
                        ui.add_space(6.0);
                        field(ui, "Host", &mut app.host);
                        ui.add_space(6.0);
                        field(ui, "Port", &mut app.port_str);

                        ui.add_space(16.0);

                        let btn = egui::Button::new(
                            RichText::new("  Connect  ")
                                .size(15.0)
                                .color(Color32::WHITE),
                        )
                        .fill(ACCENT)
                        .min_size(Vec2::new(ui.available_width(), 38.0));

                        if ui.add(btn).clicked() {
                            app.do_connect();
                        }

                        if let Some(err) = &app.connect_error.clone() {
                            ui.add_space(8.0);
                            ui.label(RichText::new(format!("⚠ {err}")).color(RED).size(12.0));
                        }
                    });
            });
        });
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(RichText::new(label).size(11.0).color(DIM));
    ui.add(
        egui::TextEdit::singleline(value)
            .min_size(Vec2::new(ui.available_width(), 30.0))
            .frame(true),
    );
}

// ── Connecting spinner ─────────────────────────────────────────────────────────

fn draw_connecting(app: &mut ClientApp, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(egui::Frame::default().fill(BG))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(160.0);
                ui.label(RichText::new("🔄 Connecting…").size(22.0).color(ACCENT));
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("{}:{}", app.host, app.port_str))
                        .size(13.0)
                        .color(DIM),
                );

                // Animated spinner built from a rotating ellipse.
                ui.add_space(24.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
                let t = ctx.input(|i| i.time) as f32;
                let painter = ui.painter();
                for i in 0..8 {
                    let angle = i as f32 * std::f32::consts::TAU / 8.0 + t * 3.0;
                    let r = 14.0f32;
                    let cx = rect.center().x + angle.cos() * r;
                    let cy = rect.center().y + angle.sin() * r;
                    let alpha = ((i as f32 / 8.0) * 255.0) as u8;
                    painter.circle_filled(
                        egui::pos2(cx, cy),
                        3.0,
                        ACCENT.linear_multiply(alpha as f32 / 255.0),
                    );
                }
            });
        });
    ctx.request_repaint(); // Animate the spinner.
}

// ── Lobby ──────────────────────────────────────────────────────────────────────

fn draw_lobby(app: &mut ClientApp, ctx: &egui::Context) {
    // ── Top bar ───────────────────────────────────────────────────────────────
    egui::TopBottomPanel::top("topbar")
        .frame(
            egui::Frame::default()
                .fill(Color32::from_rgb(18, 18, 26))
                .inner_margin(egui::Margin::symmetric(12, 6)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🟦 NOOBIETORIA").color(ACCENT).strong());
                ui.separator();
                ui.label(RichText::new(&app.server_name).color(GREEN));
                ui.separator();

                let ccu_label = format!("👤 {}", {
                    // we don't have live CCU on client, show "connected"
                    "online"
                });
                ui.label(RichText::new(ccu_label).color(DIM));

                if let Some(ms) = app.ping_ms {
                    ui.separator();
                    let color = if ms < 100 {
                        GREEN
                    } else if ms < 300 {
                        YELLOW
                    } else {
                        RED
                    };
                    ui.label(RichText::new(format!("🏓 {ms} ms")).color(color));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("Disconnect").color(RED)).clicked() {
                        app.do_disconnect();
                    }
                    if ui.button("Ping").clicked() {
                        app.do_ping();
                    }
                });
            });
        });

    // ── Left panel: World Explorer ────────────────────────────────────────────
    egui::SidePanel::left("explorer")
        .default_width(260.0)
        .min_width(180.0)
        .frame(
            egui::Frame::default()
                .fill(PANEL)
                .inner_margin(egui::Margin::same(8)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🌍 World Explorer").color(ACCENT).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("tick {}", app.snapshot.tick))
                            .size(10.0)
                            .color(DIM),
                    );
                });
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                if app.snapshot.instances.is_empty() {
                    ui.label(
                        RichText::new("(waiting for snapshot…)")
                            .color(DIM)
                            .italics(),
                    );
                } else {
                    for inst in &app.snapshot.instances.clone() {
                        let name = inst.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                        let kind = inst.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                        let container =
                            inst.get("container").and_then(|v| v.as_str()).unwrap_or("");

                        ui.horizontal(|ui| {
                            ui.label(RichText::new(instance_icon(kind)).size(13.0));
                            ui.label(RichText::new(name).size(13.0));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(RichText::new(container).size(10.0).color(DIM));
                                },
                            );
                        });
                    }
                }
            });
        });

    // ── Right panel: Event Console ────────────────────────────────────────────
    egui::SidePanel::right("events")
        .default_width(280.0)
        .min_width(200.0)
        .frame(
            egui::Frame::default()
                .fill(PANEL)
                .inner_margin(egui::Margin::same(8)),
        )
        .show(ctx, |ui| {
            ui.label(RichText::new("📡 Event Console").color(ACCENT).strong());
            ui.separator();

            ui.label(RichText::new("Event name:").size(11.0).color(DIM));
            ui.add(
                egui::TextEdit::singleline(&mut app.event_name)
                    .hint_text("e.g. Jump")
                    .min_size(Vec2::new(ui.available_width(), 26.0)),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Args (JSON, comma-separated):")
                    .size(11.0)
                    .color(DIM),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.event_args)
                    .hint_text(r#""Alice", 42"#)
                    .min_size(Vec2::new(ui.available_width(), 26.0)),
            );
            ui.add_space(6.0);

            let fire_btn = egui::Button::new(RichText::new("🔥 Fire Event").color(Color32::WHITE))
                .fill(Color32::from_rgb(180, 90, 30))
                .min_size(Vec2::new(ui.available_width(), 32.0));

            if ui.add(fire_btn).clicked() {
                app.do_fire();
            }
        });

    // ── Central panel: Log ────────────────────────────────────────────────────
    egui::CentralPanel::default()
        .frame(
            egui::Frame::default()
                .fill(BG)
                .inner_margin(egui::Margin::same(8)),
        )
        .show(ctx, |ui| {
            ui.label(RichText::new("📋 Log").color(ACCENT).strong());
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for entry in &app.log {
                        let (icon, color) = match entry.kind {
                            LogKind::System => ("·", DIM),
                            LogKind::Event => ("↔", ACCENT),
                            LogKind::Error => ("✗", RED),
                        };
                        ui.label(
                            RichText::new(format!("{icon} {}", entry.text))
                                .size(12.0)
                                .color(color),
                        );
                    }
                });
        });
}

fn instance_icon(kind: &str) -> &'static str {
    match kind {
        "Part" | "UnionOperation" | "MeshPart" | "SpecialMesh" => "⬜",
        "Script" | "LocalScript" | "ModuleScript" => "📜",
        "Model" => "🧩",
        "Folder" => "📁",
        "SpawnLocation" => "🏁",
        "Sound" | "Audio" => "🔊",
        "NetworkEvent" => "📡",
        _ => "●",
    }
}
