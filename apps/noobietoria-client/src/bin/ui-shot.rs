/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Dev-only screenshot tool for the client UI.
//!
//! Renders the egui output of [`noobietoria_client::ui`] into a PNG so the
//! layout can be compared against the PC app mockup without opening a window.
//! It is a plain software rasterizer over the tessellated meshes: enough to
//! check geometry and colour, and it exercises the exact same `ui::draw` the
//! real app calls.
//!
//! Usage:
//!
//! ```text
//! cargo run -p noobietoria-client --bin ui-shot -- <out.png> [scene] [width] [height]
//! ```

use eframe::egui::{self, epaint};
use noobietoria_client::state::{ClientApp, Screen};
/// One RGBA canvas the rasterizer draws into.
struct Canvas {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(w: usize, h: usize, clear: [f32; 4]) -> Self {
        Self {
            w,
            h,
            px: vec![clear; w * h],
        }
    }

    /// Composites a premultiplied source colour over the canvas, honouring the
    /// clip rectangle. egui delivers vertex colours and font texels as
    /// premultiplied sRGBA, so the source RGB is already scaled by its alpha.
    fn blend(&mut self, x: i32, y: i32, color: [f32; 4], clip: [f32; 4]) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let cx = x as f32 + 0.5;
        let cy = y as f32 + 0.5;
        if cx < clip[0] || cy < clip[1] || cx > clip[2] || cy > clip[3] {
            return;
        }
        let dst = &mut self.px[y as usize * self.w + x as usize];
        let a = color[3].clamp(0.0, 1.0);
        let inv = 1.0 - a;
        for i in 0..3 {
            dst[i] = (color[i] + dst[i] * inv).clamp(0.0, 1.0);
        }
        // Once anything opaque has been composited the canvas is opaque.
        dst[3] = (a + dst[3] * inv).clamp(0.0, 1.0);
    }

    fn to_png(&self, path: &str) -> Result<(), String> {
        let mut bytes = Vec::with_capacity(self.w * self.h * 4);
        for p in &self.px {
            for c in p {
                bytes.push((c.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
        write_png(path, self.w, self.h, &bytes)
    }
}

/// Minimal PNG writer: truecolour with alpha, no interlacing, one IDAT.
fn write_png(path: &str, w: usize, h: usize, rgba: &[u8]) -> Result<(), String> {
    fn crc32(data: &[u8]) -> u32 {
        let mut table = [0u32; 256];
        for (i, entry) in table.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *entry = c;
        }
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
        }
        crc ^ 0xFFFF_FFFF
    }

    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() + 12);
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut crc_input = Vec::from(&kind[..]);
        crc_input.extend_from_slice(data);
        out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
        out
    }

    // Raw scanlines each prefixed with a filter byte.
    let mut raw = Vec::with_capacity(h * (1 + w * 4));
    for y in 0..h {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * w * 4..(y + 1) * w * 4]);
    }

    // zlib stream, stored (uncompressed) deflate blocks — no compressor needed.
    let mut zlib = vec![0x78, 0x01];
    for block in raw.chunks(65535) {
        let last = block.len() < 65535;
        zlib.push(if last { 1 } else { 0 });
        let len = block.len() as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    // Adler-32 of the uncompressed data.
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in &raw {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit, RGBA, deflate, no filter, no interlace
    out.extend_from_slice(&chunk(b"IHDR", &ihdr));
    out.extend_from_slice(&chunk(b"IDAT", &zlib));
    out.extend_from_slice(&chunk(b"IEND", &[]));
    std::fs::write(path, out).map_err(|e| e.to_string())
}

/// A texture the rasterizer can sample: either the font atlas or one of the
/// brand images uploaded by the app.
struct Texture {
    w: usize,
    h: usize,
    pixels: Vec<[f32; 4]>,
}

impl Texture {
    fn from_color_image(image: &epaint::ColorImage) -> Self {
        Self {
            w: image.size[0],
            h: image.size[1],
            pixels: image
                .pixels
                .iter()
                .map(|c| {
                    [
                        c.r() as f32 / 255.0,
                        c.g() as f32 / 255.0,
                        c.b() as f32 / 255.0,
                        c.a() as f32 / 255.0,
                    ]
                })
                .collect(),
        }
    }

    fn from_png(path: &str) -> Result<Self, String> {
        let img = image::open(path)
            .map_err(|e| format!("open {path}: {e}"))?
            .to_rgba8();
        let (w, h) = (img.width() as usize, img.height() as usize);
        let pixels = img
            .pixels()
            .map(|p| {
                [
                    p[0] as f32 / 255.0,
                    p[1] as f32 / 255.0,
                    p[2] as f32 / 255.0,
                    p[3] as f32 / 255.0,
                ]
            })
            .collect();
        Ok(Self { w, h, pixels })
    }

    fn texel(&self, x: usize, y: usize) -> [f32; 4] {
        self.pixels[y.min(self.h - 1) * self.w + x.min(self.w - 1)]
    }

    /// Bilinear sample in normalized uv space; the texture does not wrap.
    fn sample(&self, uv: egui::Pos2) -> [f32; 4] {
        let fx = uv.x * self.w as f32 - 0.5;
        let fy = uv.y * self.h as f32 - 0.5;
        let x0 = fx.floor();
        let y0 = fy.floor();
        let tx = fx - x0;
        let ty = fy - y0;

        let clamp_x = |v: f32| v.clamp(0.0, self.w as f32 - 1.0) as usize;
        let clamp_y = |v: f32| v.clamp(0.0, self.h as f32 - 1.0) as usize;
        let (cx0, cx1) = (clamp_x(x0), clamp_x(x0 + 1.0));
        let (cy0, cy1) = (clamp_y(y0), clamp_y(y0 + 1.0));

        let a = self.texel(cx0, cy0);
        let b = self.texel(cx1, cy0);
        let c = self.texel(cx0, cy1);
        let d = self.texel(cx1, cy1);

        let mut out = [0.0f32; 4];
        for i in 0..4 {
            let top = a[i] + (b[i] - a[i]) * tx;
            let bot = c[i] + (d[i] - c[i]) * tx;
            out[i] = top + (bot - top) * ty;
        }
        out
    }
}

/// Draws one tessellated mesh. The UV is interpolated across each triangle and
/// the texture is then sampled bilinearly, because sampling only at the triangle
/// corners would miss glyph interiors entirely (all four corners of a text quad
/// are transparent).
fn draw_mesh(canvas: &mut Canvas, mesh: &epaint::Mesh, texture: &Texture, clip: [f32; 4]) {
    for tri in mesh.indices.chunks_exact(3) {
        let v = [
            mesh.vertices[tri[0] as usize],
            mesh.vertices[tri[1] as usize],
            mesh.vertices[tri[2] as usize],
        ];
        let (x0, y0) = (v[0].pos.x, v[0].pos.y);
        let (x1, y1) = (v[1].pos.x, v[1].pos.y);
        let (x2, y2) = (v[2].pos.x, v[2].pos.y);

        let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
        if area.abs() < 1e-6 {
            continue;
        }

        let min_x = x0.min(x1).min(x2).floor().max(clip[0]) as i32;
        let max_x = x0.max(x1).max(x2).ceil().min(clip[2]) as i32;
        let min_y = y0.min(y1).min(y2).floor().max(clip[1]) as i32;
        let max_y = y0.max(y1).max(y2).ceil().min(clip[3]) as i32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let w0 = ((x1 - px) * (y2 - py) - (x2 - px) * (y1 - py)) / area;
                let w1 = ((x2 - px) * (y0 - py) - (x0 - px) * (y2 - py)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < -0.002 || w1 < -0.002 || w2 < -0.002 {
                    continue;
                }

                let uv = egui::Pos2::new(
                    w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x,
                    w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y,
                );
                let tex = texture.sample(uv);

                // Vertex colours and the sampled texel are both premultiplied,
                // so they combine by multiplication.
                let mut color = [0.0f32; 4];
                for (weight, vertex) in [(w0, v[0]), (w1, v[1]), (w2, v[2])] {
                    let c = vertex.color;
                    color[0] += (c.r() as f32 / 255.0) * weight;
                    color[1] += (c.g() as f32 / 255.0) * weight;
                    color[2] += (c.b() as f32 / 255.0) * weight;
                    color[3] += (c.a() as f32 / 255.0) * weight;
                }

                let final_color = [
                    color[0] * tex[0],
                    color[1] * tex[1],
                    color[2] * tex[2],
                    color[3] * tex[3],
                ];
                canvas.blend(x, y, final_color, clip);
            }
        }
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let out = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "shot.png".to_owned());
    let scene = args.get(2).cloned().unwrap_or_else(|| "home".to_owned());
    let w: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1176);
    let h: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(675);

    let ctx = egui::Context::default();
    let mut app = ClientApp::new_headless("http://127.0.0.1:3000".to_owned(), "PlayerOne");
    app.worlds_loading = false;
    app.worlds = scene_worlds();

    match scene.as_str() {
        "place" => {
            app.open_place(app.worlds[0].clone());
            app.servers_loading = false;
            app.servers = scene_servers();
        }
        "place-empty" => {
            app.open_place(app.worlds[0].clone());
            app.servers_loading = false;
        }
        "connecting" => {
            app.open_place(app.worlds[0].clone());
            app.servers_loading = false;
            app.servers = scene_servers();
            let server = app.servers[0].clone();
            app.join_server(server, "PlayerOne".to_owned());
        }
        "lobby" => {
            app.screen = Screen::Lobby;
            app.server_name = "Classic Baseplate #1".to_owned();
            app.max_players = 20;
            app.connected_endpoint = "127.0.0.1:53640".to_owned();
            app.ping_ms = Some(42);
            app.snapshot = noobietoria_client::state::Snapshot {
                tick: 2845,
                instances: serde_json::from_str(
                    r#"[
                      {"name":"Baseplate","kind":"Part","container":"Workspace"},
                      {"name":"SpawnLocation","kind":"SpawnLocation","container":"Workspace"},
                      {"name":"AmbientAudio","kind":"Audio","container":"Workspace"}
                    ]"#,
                )
                .unwrap_or_default(),
            };
        }
        "search" => {
            app.nav = noobietoria_client::state::NavSection::Search;
        }
        "profile" => {
            app.nav = noobietoria_client::state::NavSection::Profile;
        }
        _ => {}
    }

    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(w as f32, h as f32),
        )),
        ..Default::default()
    };

    let output = ctx.run(raw, |ctx| {
        noobietoria_client::ui::draw(&mut app, ctx);
    });
    let shapes = ctx.tessellate(output.shapes, 1.0);

    let mut canvas = Canvas::new(w, h, [0.0, 0.0, 0.0, 1.0]);
    let font_atlas = Texture::from_color_image(&ctx.fonts(|f| f.image()));

    // The app uploads its brand PNGs; the files on disk are the same bytes.
    // The order follows `ui::BRAND_TEXTURES` so texture ids stay in step.
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let brand_files = ["logo-mark.png", "logo-wordmark.png", "logo-lockup.png"];
    debug_assert_eq!(
        brand_files.len(),
        noobietoria_client::ui::BRAND_TEXTURES.len()
    );
    let brand: Vec<Texture> = brand_files
        .iter()
        .map(|name| {
            let path = assets.join(name);
            Texture::from_png(&path.to_string_lossy())
                .unwrap_or_else(|e| panic!("load brand texture {name}: {e}"))
        })
        .collect();

    let mut meshes = 0usize;
    for prim in &shapes {
        if let epaint::Primitive::Mesh(mesh) = &prim.primitive {
            meshes += 1;
            // Managed(0) is the font atlas; brand images are uploaded after it,
            // in the order the app asked for them.
            let texture = match mesh.texture_id {
                epaint::TextureId::Managed(0) => &font_atlas,
                epaint::TextureId::Managed(n) => brand.get((n - 1) as usize).unwrap_or(&font_atlas),
                _ => &font_atlas,
            };
            draw_mesh(
                &mut canvas,
                mesh,
                texture,
                [
                    prim.clip_rect.min.x,
                    prim.clip_rect.min.y,
                    prim.clip_rect.max.x,
                    prim.clip_rect.max.y,
                ],
            );
        }
    }

    // `--probe` reports how the font atlas maps onto the tessellated text, so a
    // wrong uv interpretation is caught instead of guessed at.
    if std::env::args().any(|a| a == "--probe") {
        let (aw, ah) = (font_atlas.w, font_atlas.h);
        println!("atlas {}x{}", aw, ah);
        let mut shown = 0;
        for prim in &shapes {
            if let epaint::Primitive::Mesh(mesh) = &prim.primitive {
                for vertex in &mesh.vertices {
                    if vertex.uv == egui::Pos2::ZERO {
                        continue;
                    }
                    let x = ((vertex.uv.x * aw as f32) as usize).min(aw - 1);
                    let y = ((vertex.uv.y * ah as f32) as usize).min(ah - 1);
                    println!(
                        "  uv=({:.4},{:.4}) texid={:?} vtx_rgba=({},{},{},{}) tex_a={:.3}",
                        vertex.uv.x,
                        vertex.uv.y,
                        mesh.texture_id,
                        vertex.color.r(),
                        vertex.color.g(),
                        vertex.color.b(),
                        vertex.color.a(),
                        font_atlas.texel(x, y)[3],
                    );
                    shown += 1;
                    if shown >= 3 {
                        break;
                    }
                }
            }
            if shown >= 3 {
                break;
            }
        }
    }

    canvas.to_png(&out)?;
    println!("wrote {out} ({w}x{h}) scene={scene} meshes={meshes}");
    Ok(())
}

fn scene_worlds() -> Vec<noobietoria_client::api::WorldInfo> {
    let names = [
        "Classic Baseplate",
        "Mega Easy Obby [100+ Stages]",
        "Natural Disaster Survival",
        "Prop Hunt",
        "Rainy Day Cafe",
        "Siege [v0.11 ALPHA]",
        "Guboigle",
        "Call of Noobietoria",
        "Trick or Retreat!",
        "My Dreamscape [2.0]",
        "No Poly",
        "Build a Boat",
        "Tower of Hell",
        "Adopt Me Classic",
        "Parkour Rush",
        "Speed Run Simulator",
        "Hide and Seek",
        "Pizza Place",
        "Sword Fight Arena",
        "Zombie Rush",
    ];
    let creators = [
        "Noobietoria",
        "ObbyKing2026",
        "SurvivalCraft",
        "cya",
        "rompork",
    ];
    names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            serde_json::from_value(serde_json::json!({
                "place_id": format!("place_{i}"),
                "name": name,
                "description": "A place built in Noobietoria Studio.",
                "creator_name": creators[i % creators.len()],
                "visits": 124_500 - i as u64 * 4_321,
                "upvotes": 8_900,
                "downvotes": 120,
                "max_players": 50,
            }))
            .expect("world")
        })
        .collect()
}

fn scene_servers() -> Vec<noobietoria_client::api::ServerInfo> {
    [("Singapore", 0u32), ("Tokyo", 30), ("US East", 42)]
        .iter()
        .enumerate()
        .map(|(i, (region, players))| {
            serde_json::from_value(serde_json::json!({
                "server_id": format!("place_0:{}", i + 1),
                "place_id": "place_0",
                "place_name": "Classic Baseplate",
                "host": "127.0.0.1",
                "port": 53640 + i as u16,
                "players": players,
                "max_players": 50,
                "region": region,
                "ping_ms": 30 + i as u32 * 26,
                "status": if i == 2 { "full" } else { "online" },
            }))
            .expect("server")
        })
        .collect()
}
