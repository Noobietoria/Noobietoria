/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! 3D Interactive Viewport with camera orbit/pan/zoom, 3D grid and part rendering.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use nb_types::{Container, Vector3};

use crate::state::{PlaytestState, StudioApp};

pub fn render_viewport(app: &mut StudioApp, ui: &mut Ui) {
    let available_size = ui.available_size();
    let (response, painter) = ui.allocate_painter(available_size, Sense::click_and_drag());
    let rect = response.rect;

    // --- Input handling for 3D Camera Controls ---
    if response.dragged_by(egui::PointerButton::Secondary) {
        let delta = response.drag_delta();
        app.camera.yaw -= delta.x * 0.008;
        app.camera.pitch = (app.camera.pitch + delta.y * 0.008).clamp(-1.45, 1.45);
    }
    if response.dragged_by(egui::PointerButton::Middle) {
        let delta = response.drag_delta();
        let forward_x = -app.camera.yaw.sin();
        let forward_z = -app.camera.yaw.cos();
        let right_x = app.camera.yaw.cos();
        let right_z = -app.camera.yaw.sin();

        let pan_speed = (app.camera.distance as f64) * 0.0015;
        app.camera.target.x -= (right_x as f64) * (delta.x as f64) * pan_speed;
        app.camera.target.z -= (right_z as f64) * (delta.x as f64) * pan_speed;
        app.camera.target.x += (forward_x as f64) * (delta.y as f64) * pan_speed;
        app.camera.target.z += (forward_z as f64) * (delta.y as f64) * pan_speed;
    }

    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    if response.hovered() && scroll_delta.abs() > 0.01 {
        app.camera.distance = (app.camera.distance - scroll_delta * 0.08).clamp(2.0, 500.0);
    }

    // --- Background (Sky / Horizon) ---
    painter.rect_filled(rect, 0.0, Color32::from_rgb(30, 33, 40));

    // --- 3D Projection Helpers ---
    let center = rect.center();
    let cam = &app.camera;
    let cos_yaw = cam.yaw.cos();
    let sin_yaw = cam.yaw.sin();
    let cos_pitch = cam.pitch.cos();
    let sin_pitch = cam.pitch.sin();
    let dist = cam.distance.max(1.0);
    let scale = (rect.height() * 0.8) / dist;

    let project = |v: Vector3| -> Option<Pos2> {
        // Relative to camera target
        let dx = (v.x - cam.target.x) as f32;
        let dy = (v.y - cam.target.y) as f32;
        let dz = (v.z - cam.target.z) as f32;

        // Rotate yaw
        let x1 = dx * cos_yaw - dz * sin_yaw;
        let z1 = dx * sin_yaw + dz * cos_yaw;

        // Rotate pitch
        let y2 = dy * cos_pitch - z1 * sin_pitch;
        let z2 = dy * sin_pitch + z1 * cos_pitch + dist;

        if z2 <= 0.1 {
            return None; // Behind camera plane
        }

        let proj_x = center.x + (x1 / z2) * scale * 25.0;
        let proj_y = center.y - (y2 / z2) * scale * 25.0;
        Some(Pos2::new(proj_x, proj_y))
    };

    // --- Draw Ground Grid (Y = 0) ---
    let grid_half = 64.0;
    let grid_step = 8.0;
    let grid_color = Color32::from_rgba_unmultiplied(80, 90, 110, 60);

    let mut z = -grid_half;
    while z <= grid_half {
        if let (Some(p1), Some(p2)) = (
            project(Vector3::new(-grid_half, 0.0, z)),
            project(Vector3::new(grid_half, 0.0, z)),
        ) {
            painter.line_segment([p1, p2], Stroke::new(1.0, grid_color));
        }
        z += grid_step;
    }

    let mut x = -grid_half;
    while x <= grid_half {
        if let (Some(p1), Some(p2)) = (
            project(Vector3::new(x, 0.0, -grid_half)),
            project(Vector3::new(x, 0.0, grid_half)),
        ) {
            painter.line_segment([p1, p2], Stroke::new(1.0, grid_color));
        }
        x += grid_step;
    }

    // --- Draw Instances in Workspace ---
    let instances = app.world.iter_container(Container::Workspace);
    for inst in instances {
        if !inst.properties.visible {
            continue;
        }

        let pos = inst.properties.position;
        let size = inst.properties.size;
        let hx = (size.x * 0.5).max(0.2);
        let hy = (size.y * 0.5).max(0.2);
        let hz = (size.z * 0.5).max(0.2);

        // 8 Bounding Box Vertices
        let corners = [
            Vector3::new(pos.x - hx, pos.y - hy, pos.z - hz),
            Vector3::new(pos.x + hx, pos.y - hy, pos.z - hz),
            Vector3::new(pos.x + hx, pos.y + hy, pos.z - hz),
            Vector3::new(pos.x - hx, pos.y + hy, pos.z - hz),
            Vector3::new(pos.x - hx, pos.y - hy, pos.z + hz),
            Vector3::new(pos.x + hx, pos.y - hy, pos.z + hz),
            Vector3::new(pos.x + hx, pos.y + hy, pos.z + hz),
            Vector3::new(pos.x - hx, pos.y + hy, pos.z + hz),
        ];

        let proj_corners: Vec<Option<Pos2>> = corners.iter().map(|&c| project(c)).collect();

        let is_selected = app.selected_guid.as_deref() == Some(inst.id.as_str());

        let base_col = Color32::from_rgba_unmultiplied(
            inst.properties.color.r as u8,
            inst.properties.color.g as u8,
            inst.properties.color.b as u8,
            if is_selected { 220 } else { 180 },
        );

        let edges = [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0), // Front face
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4), // Back face
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7), // Side connections
        ];

        let stroke_color = if is_selected {
            Color32::from_rgb(0, 255, 255) // Cyan highlight
        } else {
            base_col
        };
        let stroke_width = if is_selected { 2.5 } else { 1.2 };

        for &(i1, i2) in &edges {
            if let (Some(p1), Some(p2)) = (proj_corners[i1], proj_corners[i2]) {
                painter.line_segment([p1, p2], Stroke::new(stroke_width, stroke_color));
            }
        }

        // Top face fill if all projected
        if let (Some(p3), Some(p2), Some(p6), Some(p7)) = (
            proj_corners[3],
            proj_corners[2],
            proj_corners[6],
            proj_corners[7],
        ) {
            let top_color = Color32::from_rgba_unmultiplied(
                (inst.properties.color.r + 30.0).min(255.0) as u8,
                (inst.properties.color.g + 30.0).min(255.0) as u8,
                (inst.properties.color.b + 30.0).min(255.0) as u8,
                90,
            );
            painter.add(egui::Shape::convex_polygon(
                vec![p3, p2, p6, p7],
                top_color,
                Stroke::NONE,
            ));
        }

        // Draw Transform Gizmo if selected
        if is_selected {
            if let Some(center_pos) = project(pos) {
                // X Axis (Red)
                if let Some(px) = project(Vector3::new(pos.x + 3.0, pos.y, pos.z)) {
                    painter.line_segment([center_pos, px], Stroke::new(3.0, Color32::RED));
                }
                // Y Axis (Green)
                if let Some(py) = project(Vector3::new(pos.x, pos.y + 3.0, pos.z)) {
                    painter.line_segment([center_pos, py], Stroke::new(3.0, Color32::GREEN));
                }
                // Z Axis (Blue)
                if let Some(pz) = project(Vector3::new(pos.x, pos.y, pos.z + 3.0)) {
                    painter.line_segment([center_pos, pz], Stroke::new(3.0, Color32::BLUE));
                }
            }
        }
    }

    // --- Draw Playtest Avatar (Inner Server Simulation) ---
    let cam_3d = nb_render::Camera3D::new(
        app.camera.target,
        app.camera.distance,
        app.camera.yaw,
        app.camera.pitch,
    );

    if app.playtest == PlaytestState::PlayingSolo {
        if let Some(ref char_state) = app.playtest_character {
            nb_render::draw_avatar_3d(
                &painter,
                &cam_3d,
                rect,
                char_state.position,
                char_state.yaw,
                char_state.walk_cycle,
                char_state.is_moving,
                &char_state.avatar,
                "You (Playtest)",
                char_state.health,
                true,
            );
        }
    }

    // --- Overlay HUD (Top-right of viewport) ---
    let is_in_playtest = app.playtest != PlaytestState::Stopped;
    let hud_width = if is_in_playtest { 260.0 } else { 210.0 };
    let hud_height = if is_in_playtest { 130.0 } else { 75.0 };

    let hud_rect = Rect::from_min_size(
        Pos2::new(rect.max.x - (hud_width + 10.0), rect.min.y + 10.0),
        Vec2::new(hud_width, hud_height),
    );
    painter.rect_filled(hud_rect, 6.0, Color32::from_black_alpha(190));

    let status_label = match app.playtest {
        PlaytestState::Stopped => "MODE: Edit Mode",
        PlaytestState::PlayingSolo => "MODE: Playtest (Inner Server Solo)",
        PlaytestState::RunningServer => "MODE: Running (Inner Server Simulation)",
    };

    let mut stop_clicked = false;
    let mut respawn_clicked = false;

    ui.put(hud_rect, |ui: &mut Ui| {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(status_label)
                    .strong()
                    .color(if is_in_playtest { Color32::from_rgb(100, 255, 120) } else { Color32::WHITE }),
            );
            ui.label(
                egui::RichText::new(format!("Instances: {}", app.world.len()))
                    .size(11.0)
                    .color(Color32::LIGHT_GRAY),
            );

            if let Some(ref char_state) = app.playtest_character {
                ui.label(
                    egui::RichText::new(format!(
                        "Player: ({:.1}, {:.1}, {:.1}) | HP: {:.0}%",
                        char_state.position.x, char_state.position.y, char_state.position.z, char_state.health
                    ))
                    .size(11.0)
                    .color(Color32::from_rgb(255, 220, 100)),
                );
                ui.label(
                    egui::RichText::new("WASD: Move | Space: Jump | Shift: Run | RMB: Rotate View")
                        .size(9.5)
                        .color(Color32::GRAY),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!(
                        "Cam Target: ({:.1}, {:.1}, {:.1})",
                        app.camera.target.x, app.camera.target.y, app.camera.target.z
                    ))
                    .size(11.0)
                    .color(Color32::GRAY),
                );
            }

            if is_in_playtest {
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new("⏹ Stop").color(Color32::from_rgb(255, 100, 100)).small()).clicked() {
                        stop_clicked = true;
                    }
                    if app.playtest == PlaytestState::PlayingSolo {
                        if ui.button(egui::RichText::new("🔄 Respawn").color(Color32::LIGHT_BLUE).small()).clicked() {
                            respawn_clicked = true;
                        }
                    }
                });
            }
        })
        .response
    });

    if stop_clicked {
        app.stop_playtest();
    }
    if respawn_clicked {
        app.respawn_character();
    }
}
