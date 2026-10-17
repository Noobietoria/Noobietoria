/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Noobietoria 3D Viewport Renderer.
//!
//! Provides 3D projection, camera orbit/pan/zoom, world geometry rendering,
//! character physics, and blocky Avatar rendering (with body colors, clothes, and faces).

use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use nb_types::{AvatarConfig, Vector3};

// ── 3D Camera ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Camera3D {
    pub target: Vector3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            target: Vector3::new(0.0, 2.5, 0.0),
            distance: 22.0,
            yaw: 0.785,  // 45 degrees
            pitch: 0.35, // ~20 degrees down
        }
    }
}

impl Camera3D {
    pub fn new(target: Vector3, distance: f32, yaw: f32, pitch: f32) -> Self {
        Self {
            target,
            distance,
            yaw,
            pitch,
        }
    }

    /// Rotates camera horizontally (yaw) and vertically (pitch).
    pub fn orbit(&mut self, delta: Vec2) {
        self.yaw -= delta.x * 0.008;
        self.pitch = (self.pitch + delta.y * 0.008).clamp(-1.45, 1.45);
    }

    /// Zooms camera in or out smoothly.
    pub fn zoom(&mut self, scroll: f32) {
        self.distance = (self.distance - scroll * 0.12).clamp(3.0, 300.0);
    }

    /// Pans camera focus relative to view direction.
    pub fn pan(&mut self, delta: Vec2) {
        let forward_x = -self.yaw.sin();
        let forward_z = -self.yaw.cos();
        let right_x = self.yaw.cos();
        let right_z = -self.yaw.sin();

        let pan_speed = (self.distance as f64) * 0.0015;
        self.target.x -= (right_x as f64) * (delta.x as f64) * pan_speed;
        self.target.z -= (right_z as f64) * (delta.x as f64) * pan_speed;
        self.target.x += (forward_x as f64) * (delta.y as f64) * pan_speed;
        self.target.z += (forward_z as f64) * (delta.y as f64) * pan_speed;
    }

    /// Projects a 3D world coordinate into 2D screen space. Returns (Pos2, depth_z).
    pub fn project(&self, v: Vector3, rect: Rect) -> Option<(Pos2, f32)> {
        let center = rect.center();
        let cos_yaw = self.yaw.cos();
        let sin_yaw = self.yaw.sin();
        let cos_pitch = self.pitch.cos();
        let sin_pitch = self.pitch.sin();
        let dist = self.distance.max(1.0);
        let scale = (rect.height() * 0.85) / dist;

        // Vector relative to camera target
        let dx = (v.x - self.target.x) as f32;
        let dy = (v.y - self.target.y) as f32;
        let dz = (v.z - self.target.z) as f32;

        // Rotate yaw
        let x1 = dx * cos_yaw - dz * sin_yaw;
        let z1 = dx * sin_yaw + dz * cos_yaw;

        // Rotate pitch
        let y2 = dy * cos_pitch - z1 * sin_pitch;
        let z2 = dy * sin_pitch + z1 * cos_pitch + dist;

        if z2 <= 0.1 {
            return None; // Clipped behind view plane
        }

        let proj_x = center.x + (x1 / z2) * scale * 25.0;
        let proj_y = center.y - (y2 / z2) * scale * 25.0;
        Some((Pos2::new(proj_x, proj_y), z2))
    }
}

// ── 3D Box & Cube Rendering ───────────────────────────────────────────────────

/// Draws a shaded 3D axis-aligned box with top face highlighting and edges.
pub fn draw_box_3d(
    painter: &egui::Painter,
    cam: &Camera3D,
    rect: Rect,
    pos: Vector3,
    size: Vector3,
    base_color: Color32,
    border_color: Option<Color32>,
    selected: bool,
) {
    let hx = (size.x * 0.5) as f64;
    let hy = (size.y * 0.5) as f64;
    let hz = (size.z * 0.5) as f64;

    let corners = [
        Vector3::new(pos.x - hx, pos.y - hy, pos.z - hz), // 0: Bottom Front-Left
        Vector3::new(pos.x + hx, pos.y - hy, pos.z - hz), // 1: Bottom Front-Right
        Vector3::new(pos.x + hx, pos.y + hy, pos.z - hz), // 2: Top Front-Right
        Vector3::new(pos.x - hx, pos.y + hy, pos.z - hz), // 3: Top Front-Left
        Vector3::new(pos.x - hx, pos.y - hy, pos.z + hz), // 4: Bottom Back-Left
        Vector3::new(pos.x + hx, pos.y - hy, pos.z + hz), // 5: Bottom Back-Right
        Vector3::new(pos.x + hx, pos.y + hy, pos.z + hz), // 6: Top Back-Right
        Vector3::new(pos.x - hx, pos.y + hy, pos.z + hz), // 7: Top Back-Left
    ];

    let projected: Vec<Option<Pos2>> = corners
        .iter()
        .map(|&c| cam.project(c, rect).map(|(p, _)| p))
        .collect();

    // Top Face: 3, 2, 6, 7 (illuminated by overhead sky)
    if let (Some(p3), Some(p2), Some(p6), Some(p7)) =
        (projected[3], projected[2], projected[6], projected[7])
    {
        let top_color = Color32::from_rgba_unmultiplied(
            base_color.r().saturating_add(35),
            base_color.g().saturating_add(35),
            base_color.b().saturating_add(35),
            base_color.a(),
        );
        painter.add(egui::Shape::convex_polygon(
            vec![p3, p2, p6, p7],
            top_color,
            Stroke::NONE,
        ));
    }

    // Front Face: 0, 1, 2, 3
    if let (Some(p0), Some(p1), Some(p2), Some(p3)) =
        (projected[0], projected[1], projected[2], projected[3])
    {
        painter.add(egui::Shape::convex_polygon(
            vec![p0, p1, p2, p3],
            base_color,
            Stroke::NONE,
        ));
    }

    // Back Face: 4, 5, 6, 7
    if let (Some(p4), Some(p5), Some(p6), Some(p7)) =
        (projected[4], projected[5], projected[6], projected[7])
    {
        let shaded = Color32::from_rgba_unmultiplied(
            base_color.r().saturating_sub(25),
            base_color.g().saturating_sub(25),
            base_color.b().saturating_sub(25),
            base_color.a(),
        );
        painter.add(egui::Shape::convex_polygon(
            vec![p4, p5, p6, p7],
            shaded,
            Stroke::NONE,
        ));
    }

    // Edges
    let edge_color = border_color.unwrap_or_else(|| {
        if selected {
            Color32::from_rgb(0, 240, 255)
        } else {
            Color32::from_rgba_unmultiplied(
                base_color.r().saturating_add(50),
                base_color.g().saturating_add(50),
                base_color.b().saturating_add(50),
                200,
            )
        }
    });

    let stroke = Stroke::new(if selected { 2.2 } else { 1.0 }, edge_color);
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    for &(i1, i2) in &edges {
        if let (Some(p1), Some(p2)) = (projected[i1], projected[i2]) {
            painter.line_segment([p1, p2], stroke);
        }
    }
}

/// Draws an oriented 3D box rotated around the Y axis by yaw radians.
pub fn draw_oriented_box_3d(
    painter: &egui::Painter,
    cam: &Camera3D,
    rect: Rect,
    center: Vector3,
    size: Vector3,
    yaw: f32,
    base_color: Color32,
) {
    let hx = size.x * 0.5;
    let hy = size.y * 0.5;
    let hz = size.z * 0.5;

    let cos_y = yaw.cos() as f64;
    let sin_y = yaw.sin() as f64;

    let local_corners = [
        (-hx, -hy, -hz),
        (hx, -hy, -hz),
        (hx, hy, -hz),
        (-hx, hy, -hz),
        (-hx, -hy, hz),
        (hx, -hy, hz),
        (hx, hy, hz),
        (-hx, hy, hz),
    ];

    let corners: Vec<Vector3> = local_corners
        .iter()
        .map(|&(lx, ly, lz)| {
            let rx = lx * cos_y - lz * sin_y;
            let rz = lx * sin_y + lz * cos_y;
            Vector3::new(center.x + rx, center.y + ly, center.z + rz)
        })
        .collect();

    let projected: Vec<Option<Pos2>> = corners
        .iter()
        .map(|&c| cam.project(c, rect).map(|(p, _)| p))
        .collect();

    // Top face
    if let (Some(p3), Some(p2), Some(p6), Some(p7)) =
        (projected[3], projected[2], projected[6], projected[7])
    {
        let top_color = Color32::from_rgba_unmultiplied(
            base_color.r().saturating_add(30),
            base_color.g().saturating_add(30),
            base_color.b().saturating_add(30),
            base_color.a(),
        );
        painter.add(egui::Shape::convex_polygon(
            vec![p3, p2, p6, p7],
            top_color,
            Stroke::NONE,
        ));
    }

    // Side faces fill
    if let (Some(p0), Some(p1), Some(p2), Some(p3)) =
        (projected[0], projected[1], projected[2], projected[3])
    {
        painter.add(egui::Shape::convex_polygon(
            vec![p0, p1, p2, p3],
            base_color,
            Stroke::NONE,
        ));
    }

    let stroke = Stroke::new(
        1.0,
        Color32::from_rgba_unmultiplied(
            base_color.r().saturating_add(40),
            base_color.g().saturating_add(40),
            base_color.b().saturating_add(40),
            180,
        ),
    );
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    for &(i1, i2) in &edges {
        if let (Some(p1), Some(p2)) = (projected[i1], projected[i2]) {
            painter.line_segment([p1, p2], stroke);
        }
    }
}

// ── Ground Baseplate & Grid ────────────────────────────────────────────────────

pub fn draw_ground_grid(painter: &egui::Painter, cam: &Camera3D, rect: Rect, size_half: f64) {
    let step = 8.0;
    let grid_color = Color32::from_rgba_unmultiplied(75, 85, 105, 50);
    let center_color = Color32::from_rgba_unmultiplied(120, 140, 180, 100);

    let mut z = -size_half;
    while z <= size_half {
        if let (Some((p1, _)), Some((p2, _))) = (
            cam.project(Vector3::new(-size_half, 0.0, z), rect),
            cam.project(Vector3::new(size_half, 0.0, z), rect),
        ) {
            let col = if z.abs() < 0.01 {
                center_color
            } else {
                grid_color
            };
            painter.line_segment([p1, p2], Stroke::new(1.0, col));
        }
        z += step;
    }

    let mut x = -size_half;
    while x <= size_half {
        if let (Some((p1, _)), Some((p2, _))) = (
            cam.project(Vector3::new(x, 0.0, -size_half), rect),
            cam.project(Vector3::new(x, 0.0, size_half), rect),
        ) {
            let col = if x.abs() < 0.01 {
                center_color
            } else {
                grid_color
            };
            painter.line_segment([p1, p2], Stroke::new(1.0, col));
        }
        x += step;
    }
}

// ── Avatar Renderer (Classic Blocky R6) ────────────────────────────────────────

/// Draws the player's 3D avatar with body colors, clothes patterns, faces, and walk animation.
pub fn draw_avatar_3d(
    painter: &egui::Painter,
    cam: &Camera3D,
    rect: Rect,
    position: Vector3,
    yaw: f32,
    walk_cycle: f32,
    is_moving: bool,
    avatar: &AvatarConfig,
    username: &str,
    health: f32,
    is_local_player: bool,
) {
    let limb_swing = if is_moving {
        walk_cycle.sin() * 0.55
    } else {
        0.0
    };

    let to_c32 = |r: &nb_types::Rgba| -> Color32 {
        Color32::from_rgba_unmultiplied(r.r as u8, r.g as u8, r.b as u8, 255)
    };

    let head_c = to_c32(&avatar.head_color);
    let torso_c = to_c32(&avatar.torso_color);
    let left_arm_c = to_c32(&avatar.left_arm_color);
    let right_arm_c = to_c32(&avatar.right_arm_color);
    let left_leg_c = to_c32(&avatar.left_leg_color);
    let right_leg_c = to_c32(&avatar.right_leg_color);

    // Apply clothes color tint / patterns
    let (shirt_tint, torso_detail) = match avatar.shirt.as_str() {
        "tuxedo" => (Color32::from_rgb(25, 25, 30), "tuxedo"),
        "hoodie" => (Color32::from_rgb(180, 45, 45), "hoodie"),
        "noobietoria_tee" => (Color32::from_rgb(205, 122, 123), "logo"),
        "flannel" => (Color32::from_rgb(160, 30, 30), "flannel"),
        "cyber" => (Color32::from_rgb(20, 25, 35), "cyber"),
        "striped" => (Color32::from_rgb(220, 180, 50), "striped"),
        _ => (torso_c, "plain"),
    };

    let (pants_tint, _pants_detail) = match avatar.pants.as_str() {
        "jeans" => (Some(Color32::from_rgb(35, 65, 120)), "jeans"),
        "cargo" => (Some(Color32::from_rgb(45, 50, 40)), "cargo"),
        "sweatpants" => (Some(Color32::from_rgb(225, 225, 230)), "sweatpants"),
        "slacks" => (Some(Color32::from_rgb(30, 30, 35)), "slacks"),
        "shorts" => (Some(Color32::from_rgb(180, 140, 70)), "shorts"),
        _ => (None, "plain"),
    };

    let left_pant_color = pants_tint.unwrap_or(left_leg_c);
    let right_pant_color = pants_tint.unwrap_or(right_leg_c);

    let cos_y = yaw.cos() as f64;
    let sin_y = yaw.sin() as f64;

    // Helper to rotate local character-relative offset to world coordinates
    let rot = |lx: f64, ly: f64, lz: f64| -> Vector3 {
        Vector3::new(
            position.x + lx * cos_y - lz * sin_y,
            position.y + ly,
            position.z + lx * sin_y + lz * cos_y,
        )
    };

    // ── 1. Legs ───────────────────────────────────────────────────────────────
    // Left leg
    let left_leg_offset_z = (-limb_swing as f64) * 0.4;
    let left_leg_pos = rot(-0.5, 1.0, left_leg_offset_z);
    draw_oriented_box_3d(
        painter,
        cam,
        rect,
        left_leg_pos,
        Vector3::new(1.0, 2.0, 1.0),
        yaw,
        left_pant_color,
    );

    // Right leg
    let right_leg_offset_z = (limb_swing as f64) * 0.4;
    let right_leg_pos = rot(0.5, 1.0, right_leg_offset_z);
    draw_oriented_box_3d(
        painter,
        cam,
        rect,
        right_leg_pos,
        Vector3::new(1.0, 2.0, 1.0),
        yaw,
        right_pant_color,
    );

    // ── 2. Torso ──────────────────────────────────────────────────────────────
    let torso_pos = rot(0.0, 3.0, 0.0);
    draw_oriented_box_3d(
        painter,
        cam,
        rect,
        torso_pos,
        Vector3::new(2.0, 2.0, 1.0),
        yaw,
        shirt_tint,
    );

    // Front torso detail (shirt logo / tie / zipper)
    let front_center = rot(0.0, 3.1, -0.52);
    if let Some((proj_front, _)) = cam.project(front_center, rect) {
        match torso_detail {
            "tuxedo" => {
                // White shirt collar + red bowtie
                painter.circle_filled(proj_front, 4.0, Color32::WHITE);
                painter.rect_filled(
                    Rect::from_center_size(proj_front, Vec2::new(7.0, 4.0)),
                    1.0,
                    Color32::from_rgb(200, 20, 20),
                );
            }
            "logo" => {
                // Noobietoria rose square badge
                painter.rect_filled(
                    Rect::from_center_size(proj_front, Vec2::new(8.0, 8.0)),
                    2.0,
                    Color32::from_rgb(205, 122, 123),
                );
            }
            "cyber" => {
                // Cyan circuit neon
                painter.circle_filled(proj_front, 3.5, Color32::from_rgb(0, 255, 255));
            }
            "hoodie" => {
                // Zipper line
                painter.line_segment(
                    [
                        Pos2::new(proj_front.x, proj_front.y - 7.0),
                        Pos2::new(proj_front.x, proj_front.y + 7.0),
                    ],
                    Stroke::new(1.5, Color32::LIGHT_GRAY),
                );
            }
            _ => {}
        }
    }

    // ── 3. Arms ───────────────────────────────────────────────────────────────
    // Left arm (swings forward when right leg swings backward)
    let left_arm_offset_z = (limb_swing as f64) * 0.45;
    let left_arm_pos = rot(-1.5, 3.0, left_arm_offset_z);
    draw_oriented_box_3d(
        painter,
        cam,
        rect,
        left_arm_pos,
        Vector3::new(1.0, 2.0, 1.0),
        yaw,
        left_arm_c,
    );

    // Right arm
    let right_arm_offset_z = (-limb_swing as f64) * 0.45;
    let right_arm_pos = rot(1.5, 3.0, right_arm_offset_z);
    draw_oriented_box_3d(
        painter,
        cam,
        rect,
        right_arm_pos,
        Vector3::new(1.0, 2.0, 1.0),
        yaw,
        right_arm_c,
    );

    // ── 4. Head & Face Decals ─────────────────────────────────────────────────
    let head_pos = rot(0.0, 4.6, 0.0);
    draw_oriented_box_3d(
        painter,
        cam,
        rect,
        head_pos,
        Vector3::new(1.25, 1.25, 1.25),
        yaw,
        head_c,
    );

    // Face rendering on head front
    let face_center = rot(0.0, 4.65, -0.65);
    if let Some((face_p, _)) = cam.project(face_center, rect) {
        draw_avatar_face(painter, face_p, avatar.face.as_str());
    }

    // ── 5. Overhead Name Tag & Health Bar ─────────────────────────────────────
    let overhead_pos = rot(0.0, 5.8, 0.0);
    if let Some((tag_p, _)) = cam.project(overhead_pos, rect) {
        let display_name = if is_local_player {
            format!("★ {username}")
        } else {
            username.to_owned()
        };

        // Dark pill background
        let name_font = egui::FontId::proportional(11.0);
        let text_size = painter
            .layout_no_wrap(display_name.clone(), name_font.clone(), Color32::WHITE)
            .size();
        let pill_rect = Rect::from_center_size(tag_p, Vec2::new(text_size.x + 14.0, 18.0));
        painter.rect_filled(pill_rect, 9.0, Color32::from_black_alpha(170));
        painter.text(
            tag_p,
            egui::Align2::CENTER_CENTER,
            display_name,
            name_font,
            if is_local_player {
                Color32::from_rgb(255, 220, 100)
            } else {
                Color32::WHITE
            },
        );

        // Health Bar below name
        let hp_pct = (health / 100.0).clamp(0.0, 1.0);
        let bar_width = 36.0;
        let bar_rect = Rect::from_center_size(
            Pos2::new(tag_p.x, tag_p.y + 12.0),
            Vec2::new(bar_width, 4.0),
        );
        painter.rect_filled(bar_rect, 2.0, Color32::from_rgb(60, 20, 20));
        if hp_pct > 0.0 {
            let filled_rect = Rect::from_min_size(bar_rect.min, Vec2::new(bar_width * hp_pct, 4.0));
            painter.rect_filled(filled_rect, 2.0, Color32::from_rgb(50, 220, 70));
        }
    }
}

/// Draws 2D expressive decals on the front face of the head.
fn draw_avatar_face(painter: &egui::Painter, p: Pos2, face_id: &str) {
    let eye_col = Color32::from_rgb(20, 20, 20);
    let eye_radius = 1.6;

    match face_id {
        "wink" => {
            // Left eye open, right eye winking line
            painter.circle_filled(Pos2::new(p.x - 3.5, p.y - 2.0), eye_radius, eye_col);
            painter.line_segment(
                [
                    Pos2::new(p.x + 2.0, p.y - 2.0),
                    Pos2::new(p.x + 5.0, p.y - 1.0),
                ],
                Stroke::new(1.8, eye_col),
            );
            // Smile
            draw_smile(painter, Pos2::new(p.x, p.y + 2.5), 3.5);
        }
        "chill" => {
            // Sunglasses
            let glasses_rect =
                Rect::from_center_size(Pos2::new(p.x, p.y - 2.0), Vec2::new(12.0, 5.0));
            painter.rect_filled(glasses_rect, 2.0, Color32::from_rgb(15, 15, 20));
            painter.line_segment(
                [
                    Pos2::new(p.x - 6.0, p.y - 2.0),
                    Pos2::new(p.x + 6.0, p.y - 2.0),
                ],
                Stroke::new(1.5, Color32::from_rgb(80, 80, 90)),
            );
            // Chill smirk
            painter.line_segment(
                [
                    Pos2::new(p.x - 2.0, p.y + 2.5),
                    Pos2::new(p.x + 3.0, p.y + 1.8),
                ],
                Stroke::new(1.4, eye_col),
            );
        }
        "joy" => {
            // Happy squinting curves ^ ^
            painter.line_segment(
                [
                    Pos2::new(p.x - 5.0, p.y - 1.5),
                    Pos2::new(p.x - 3.5, p.y - 3.0),
                ],
                Stroke::new(1.6, eye_col),
            );
            painter.line_segment(
                [
                    Pos2::new(p.x - 3.5, p.y - 3.0),
                    Pos2::new(p.x - 2.0, p.y - 1.5),
                ],
                Stroke::new(1.6, eye_col),
            );
            painter.line_segment(
                [
                    Pos2::new(p.x + 2.0, p.y - 1.5),
                    Pos2::new(p.x + 3.5, p.y - 3.0),
                ],
                Stroke::new(1.6, eye_col),
            );
            painter.line_segment(
                [
                    Pos2::new(p.x + 3.5, p.y - 3.0),
                    Pos2::new(p.x + 5.0, p.y - 1.5),
                ],
                Stroke::new(1.6, eye_col),
            );
            // Big open grin
            draw_smile(painter, Pos2::new(p.x, p.y + 2.5), 4.5);
        }
        "silly" => {
            // Wink eye + Tongue sticking out
            painter.circle_filled(Pos2::new(p.x - 3.5, p.y - 2.0), eye_radius, eye_col);
            painter.line_segment(
                [
                    Pos2::new(p.x + 2.0, p.y - 2.0),
                    Pos2::new(p.x + 5.0, p.y - 1.0),
                ],
                Stroke::new(1.8, eye_col),
            );
            // Tongue
            painter.rect_filled(
                Rect::from_center_size(Pos2::new(p.x + 1.0, p.y + 4.0), Vec2::new(3.5, 4.0)),
                1.5,
                Color32::from_rgb(240, 70, 90),
            );
            draw_smile(painter, Pos2::new(p.x, p.y + 2.0), 3.5);
        }
        "surprised" => {
            // Wide circular eyes + open O mouth
            painter.circle_filled(Pos2::new(p.x - 3.5, p.y - 2.0), 2.0, eye_col);
            painter.circle_filled(Pos2::new(p.x + 3.5, p.y - 2.0), 2.0, eye_col);
            painter.circle_filled(
                Pos2::new(p.x, p.y + 3.0),
                2.2,
                Color32::from_rgb(30, 20, 20),
            );
        }
        "ninja" => {
            // Mask covering lower face + intense squinting eyes
            let mask_rect =
                Rect::from_min_max(Pos2::new(p.x - 6.5, p.y), Pos2::new(p.x + 6.5, p.y + 6.0));
            painter.rect_filled(mask_rect, 1.0, Color32::from_rgb(25, 25, 30));
            painter.circle_filled(
                Pos2::new(p.x - 3.5, p.y - 2.0),
                1.5,
                Color32::from_rgb(240, 240, 240),
            );
            painter.circle_filled(
                Pos2::new(p.x + 3.5, p.y - 2.0),
                1.5,
                Color32::from_rgb(240, 240, 240),
            );
        }
        "smirk" => {
            painter.circle_filled(Pos2::new(p.x - 3.5, p.y - 2.0), eye_radius, eye_col);
            painter.circle_filled(Pos2::new(p.x + 3.5, p.y - 2.0), eye_radius, eye_col);
            painter.line_segment(
                [
                    Pos2::new(p.x - 2.0, p.y + 3.0),
                    Pos2::new(p.x + 3.5, p.y + 1.5),
                ],
                Stroke::new(1.6, eye_col),
            );
        }
        _ => {
            // Classic Roblox smile :-)
            painter.circle_filled(Pos2::new(p.x - 3.5, p.y - 2.0), eye_radius, eye_col);
            painter.circle_filled(Pos2::new(p.x + 3.5, p.y - 2.0), eye_radius, eye_col);
            draw_smile(painter, Pos2::new(p.x, p.y + 2.5), 3.5);
        }
    }
}

fn draw_smile(painter: &egui::Painter, center: Pos2, radius: f32) {
    let stroke = Stroke::new(1.6, Color32::from_rgb(20, 20, 20));
    painter.line_segment(
        [
            Pos2::new(center.x - radius, center.y - 0.5),
            Pos2::new(center.x - radius * 0.4, center.y + 1.2),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(center.x - radius * 0.4, center.y + 1.2),
            Pos2::new(center.x + radius * 0.4, center.y + 1.2),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(center.x + radius * 0.4, center.y + 1.2),
            Pos2::new(center.x + radius, center.y - 0.5),
        ],
        stroke,
    );
}

// ── Character Movement & Physics ───────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CharacterState {
    pub position: Vector3,
    pub velocity_y: f32,
    pub yaw: f32,
    pub is_grounded: bool,
    pub walk_cycle: f32,
    pub is_moving: bool,
    pub health: f32,
    pub max_health: f32,
    pub avatar: AvatarConfig,
}

impl Default for CharacterState {
    fn default() -> Self {
        Self {
            position: Vector3::new(0.0, 0.0, 0.0),
            velocity_y: 0.0,
            yaw: 0.0,
            is_grounded: true,
            walk_cycle: 0.0,
            is_moving: false,
            health: 100.0,
            max_health: 100.0,
            avatar: AvatarConfig::classic_noob(),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct PlayerInput {
    pub move_forward: bool,
    pub move_backward: bool,
    pub move_left: bool,
    pub move_right: bool,
    pub jump: bool,
    pub sprint: bool,
}

impl CharacterState {
    /// Advances physics simulation and applies user inputs relative to camera yaw.
    pub fn update(&mut self, dt: f32, input: &PlayerInput, cam_yaw: f32, gravity: f32) {
        let speed = if input.sprint { 24.0 } else { 16.0 };
        let mut move_x = 0.0f32;
        let mut move_z = 0.0f32;

        if input.move_forward {
            move_z -= 1.0;
        }
        if input.move_backward {
            move_z += 1.0;
        }
        if input.move_left {
            move_x -= 1.0;
        }
        if input.move_right {
            move_x += 1.0;
        }

        let len = (move_x * move_x + move_z * move_z).sqrt();
        if len > 0.001 {
            move_x /= len;
            move_z /= len;

            // Rotate input direction by camera yaw
            let cos_c = cam_yaw.cos();
            let sin_c = cam_yaw.sin();
            let world_dx = (move_x * cos_c - move_z * sin_c) as f64;
            let world_dz = (move_x * sin_c + move_z * cos_c) as f64;

            self.position.x += world_dx * (speed as f64) * (dt as f64);
            self.position.z += world_dz * (speed as f64) * (dt as f64);

            // Orient avatar towards movement direction
            self.yaw = world_dx.atan2(world_dz) as f32;
            self.walk_cycle += dt * (if input.sprint { 16.0 } else { 11.0 });
            self.is_moving = true;
        } else {
            self.is_moving = false;
        }

        // Jump
        if input.jump && self.is_grounded {
            self.velocity_y = 22.0; // Jump impulse
            self.is_grounded = false;
        }

        // Apply gravity
        self.velocity_y -= gravity * dt;
        self.position.y += (self.velocity_y as f64) * (dt as f64);

        // Ground floor collision (Baseplate at Y = 0)
        if self.position.y <= 0.0 {
            self.position.y = 0.0;
            self.velocity_y = 0.0;
            self.is_grounded = true;
        }
    }
}
