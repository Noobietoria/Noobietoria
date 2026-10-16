/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! egui rendering for the client — the Noobietoria PC app shell.
//!
//! Layout follows the PC app mockup: a full-height icon rail on the left, the
//! "Noobietoria." lockup in the top-left of the content area, and a responsive
//! grid of place tiles below it.
//!
//! Screens:
//!
//! * [`draw_app_shell`] – the PC app shell (rail + header + content). Home shows
//!   the Discover grid; the other rail sections are placeholders for now.
//! * [`draw_lobby`] – the in-game HUD (world view, event console, log).
//!
//! Every palette value and metric below is measured from the mockup, so the
//! result matches it instead of approximating it. Note that the mockup's pink is
//! not arbitrary: `#CD7A7B` is the fill colour of the repo's own `logo.svg`, so
//! the rose palette *is* the brand.
//!
//! There is deliberately no host/port input anywhere: every endpoint the client
//! dials comes from the website API.

use eframe::egui::{self, Color32, Pos2, RichText, Sense, Stroke, StrokeKind, Vec2};

use crate::api::{ServerInfo, WorldInfo};
use crate::state::{ClientApp, LogKind, NavSection, Screen};

// ── Brand artwork ──────────────────────────────────────────────────────────────

/// The Noobietoria mark and light lockup, derived from the design owner's
/// artwork by `scripts/make-logo-assets.py`.
const LOGO_MARK_PNG: &[u8] = include_bytes!("../assets/logo-mark.png");
const LOGO_WORDMARK_PNG: &[u8] = include_bytes!("../assets/logo-wordmark.png");
const LOGO_LOCKUP_PNG: &[u8] = include_bytes!("../assets/logo-lockup.png");

/// The brand artwork, in the order its textures are uploaded. Both the app and
/// the dev-only `ui-shot` tool read this list, so texture ids stay in step.
pub const BRAND_TEXTURES: [(&str, &[u8]); 3] = [
    ("noobietoria-mark", LOGO_MARK_PNG),
    ("noobietoria-wordmark", LOGO_WORDMARK_PNG),
    ("noobietoria-lockup", LOGO_LOCKUP_PNG),
];

/// Handles to the brand textures, uploaded once and reused every frame.
pub struct Logos {
    pub mark: egui::TextureHandle,
    pub wordmark: egui::TextureHandle,
    pub lockup: egui::TextureHandle,
}

impl Logos {
    /// Decodes the embedded PNGs and uploads them to the GPU.
    pub fn load(ctx: &egui::Context) -> Self {
        let decode = |bytes: &[u8], name: &str| -> egui::ColorImage {
            let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                .unwrap_or_else(|e| panic!("decode embedded {name}: {e}"))
                .to_rgba8();
            let size = [image.width() as usize, image.height() as usize];
            let pixels = image
                .pixels()
                .map(|p| {
                    // PNGs are straight (non-premultiplied) sRGBA.
                    Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3])
                })
                .collect();
            egui::ColorImage::new(size, pixels)
        };

        let [mark, wordmark, lockup] = BRAND_TEXTURES.map(|(name, bytes)| {
            ctx.load_texture(name, decode(bytes, name), egui::TextureOptions::LINEAR)
        });

        Self {
            mark,
            wordmark,
            lockup,
        }
    }

    /// Aspect ratio (width / height) of a texture handle.
    fn aspect(handle: &egui::TextureHandle) -> f32 {
        let size = handle.size_vec2();
        size.x / size.y
    }

    /// Aspect ratio of the wordmark artwork.
    fn wordmark_aspect(&self) -> f32 {
        Self::aspect(&self.wordmark)
    }

    /// Aspect ratio (width / height) of the mark artwork.
    fn mark_aspect(&self) -> f32 {
        Self::aspect(&self.mark)
    }
}

/// Uploads the brand textures on the first frame and returns them.
fn ensure_logos<'a>(app: &'a mut ClientApp, ctx: &egui::Context) -> &'a Logos {
    if app.logos.is_none() {
        app.logos = Some(Logos::load(ctx));
    }
    app.logos.as_ref().expect("logos just loaded")
}

// ── Palette (measured from the mockup) ─────────────────────────────────────────

/// Rail background — the brand rose, identical to `logo.svg`'s fill.
const RAIL: Color32 = Color32::from_rgb(205, 122, 123); // #CD7A7B
/// The thin near-black strip on the window's left edge.
const RAIL_EDGE: Color32 = Color32::from_rgb(30, 30, 30); // #1E1E1E
/// Content background.
const CANVAS: Color32 = Color32::from_rgb(218, 165, 165); // #DAA5A5
/// Place tile fill.
const TILE: Color32 = Color32::from_rgb(217, 191, 191); // #D9BFBF
/// Ink used by the icons and the wordmark.
const INK: Color32 = Color32::from_rgb(27, 23, 23); // #1B1717
/// Secondary text: the ink at reduced strength, so it stays in-palette.
const INK_SOFT: Color32 = Color32::from_rgb(120, 78, 79);

/// In-game surfaces, derived from the same brand rose.
const GAME_BG: Color32 = Color32::from_rgb(36, 27, 27);
const GAME_PANEL: Color32 = Color32::from_rgb(48, 36, 36);
const GAME_INK: Color32 = Color32::from_rgb(240, 228, 228);
const GAME_GO: Color32 = Color32::from_rgb(0, 150, 60);
const GAME_WARN: Color32 = Color32::from_rgb(235, 180, 90);
const GAME_BAD: Color32 = Color32::from_rgb(226, 110, 110);

// ── Metrics (measured from the mockup, 1176×675) ───────────────────────────────

const RAIL_W: f32 = 87.0;
const RAIL_EDGE_W: f32 = 5.0;
/// Centre of each rail icon, measured down from the top of the window.
const RAIL_ICON_Y: [f32; 7] = [20.0, 119.5, 219.0, 317.5, 417.5, 516.5, 616.0];
/// Rail icons sit ~43px in from the window's left edge.
const RAIL_ICON_X: f32 = 43.0;

/// Tile (thumbnail) size; the mockup's tiles are 122×134.
const TILE_W: f32 = 122.0;
const TILE_H: f32 = 134.0;
/// Corner radius of a place tile. Measured off the mockup: the tile's top edge
/// reaches its full width 11px in from the left edge.
const TILE_RADIUS: u8 = 11;
/// Distance between the tops of two neighbouring grid columns (measured step
/// between tile left edges in the mockup: 155.66px).
const COL_STEP: f32 = 155.66;
/// Distance between the tops of two neighbouring grid rows (measured step
/// between tile tops in the mockup: 199px).
const ROW_STEP: f32 = 199.0;
/// Gap between a tile's bottom and the first caption line.
const CAP_GAP: f32 = 10.0;
const CAP1_H: f32 = 12.0;
const CAP2_GAP: f32 = 11.0;
const CAP2_H: f32 = 12.0;
/// Left inset of the grid inside the content area. The mockup's first tile
/// starts 1px after the rail, i.e. at x=89 with an 87px rail and a 1px border.
const GRID_PAD: f32 = 2.0;
/// Height reserved for the header strip (logo row). Together with the 22px
/// status line and the 2px grid inset this puts the first tile row's top at
/// y=85, matching the mockup.
const HEADER_H: f32 = 60.0;

/// The seven rail sections, in mockup order.
const RAIL_ITEMS: [NavSection; 7] = [
    NavSection::Home,
    NavSection::Search,
    NavSection::Explore,
    NavSection::Shop,
    NavSection::Create,
    NavSection::Chat,
    NavSection::Profile,
];

// ── Entry point ────────────────────────────────────────────────────────────────

pub fn draw(app: &mut ClientApp, ctx: &egui::Context) {
    match app.screen {
        Screen::Lobby => {
            apply_game_style(ctx);
            draw_lobby(app, ctx);
        }
        _ => {
            apply_browse_style(ctx);
            draw_app_shell(app, ctx);
        }
    }
}

fn apply_browse_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.window_fill = CANVAS;
    style.visuals.panel_fill = CANVAS;
    style.visuals.extreme_bg_color = TILE;
    style.visuals.override_text_color = Some(INK);
    // Widgets are painted by hand here, so keep egui's chrome out of the way.
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, INK);
    style.visuals.widgets.inactive.bg_fill = TILE;
    style.visuals.widgets.inactive.weak_bg_fill = TILE;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(230, 208, 208);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(236, 216, 216);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, INK);
    style.visuals.selection.bg_fill = INK;
    style.visuals.selection.stroke = Stroke::new(1.0, TILE);
    ctx.set_style(style);
}

fn apply_game_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.window_fill = GAME_BG;
    style.visuals.panel_fill = GAME_BG;
    style.visuals.override_text_color = Some(GAME_INK);
    style.visuals.widgets.inactive.bg_fill = GAME_PANEL;
    style.visuals.widgets.inactive.weak_bg_fill = GAME_PANEL;
    style.visuals.widgets.hovered.bg_fill = RAIL;
    style.visuals.widgets.active.bg_fill = INK;
    style.visuals.selection.bg_fill = RAIL;
    ctx.set_style(style);
}

// ── App shell: rail + header + content ─────────────────────────────────────────

fn draw_app_shell(app: &mut ClientApp, ctx: &egui::Context) {
    draw_rail(app, ctx);

    egui::CentralPanel::default()
        .frame(
            egui::Frame::default()
                .fill(CANVAS)
                .inner_margin(egui::Margin::ZERO),
        )
        .show(ctx, |ui| {
            draw_header(app, ctx, ui);
            draw_content(app, ctx, ui);
        });
}

/// The left icon rail: a 5px near-black edge, then the brand rose column.
fn draw_rail(app: &mut ClientApp, ctx: &egui::Context) {
    egui::SidePanel::left("rail")
        .exact_width(RAIL_W)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::default()
                .fill(RAIL)
                .inner_margin(egui::Margin::ZERO),
        )
        .show(ctx, |ui| {
            let h = ui.available_height();
            let (rect, _) = ui.allocate_exact_size(Vec2::new(RAIL_W, h), Sense::hover());
            let painter = ui.painter();

            // The dark strip hugging the window edge.
            painter.rect_filled(
                egui::Rect::from_min_size(rect.min, Vec2::new(RAIL_EDGE_W, rect.height())),
                0.0,
                RAIL_EDGE,
            );

            for (i, section) in RAIL_ITEMS.iter().enumerate() {
                let center = Pos2::new(rect.left() + RAIL_ICON_X, rect.top() + RAIL_ICON_Y[i]);
                let hit = egui::Rect::from_center_size(center, Vec2::splat(46.0));
                let resp = ui
                    .interact(hit, ui.id().with(("rail", i)), Sense::click())
                    .on_hover_text(section.label());

                let active = app.nav == *section;
                let alpha = if active { 1.0 } else { 0.72 };
                let ink = Color32::from_rgba_unmultiplied(
                    INK.r(),
                    INK.g(),
                    INK.b(),
                    (255.0 * alpha) as u8,
                );
                draw_nav_icon(painter, *section, center, 17.0, ink, 2.0);

                if resp.clicked() {
                    app.nav = *section;
                    app.screen = Screen::Home;
                }
            }
        });
}

/// The lockup row: the gem mark plus the "Noobietoria." wordmark.
fn draw_header(app: &mut ClientApp, ctx: &egui::Context, ui: &mut egui::Ui) {
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), HEADER_H), Sense::hover());

    // On the Home section the header *is* the brand lockup: mark + wordmark,
    // taken from the real artwork.
    let home_lockup = app.screen == Screen::Home && app.nav == NavSection::Home;
    if home_lockup {
        let logos = ensure_logos(app, ctx);
        let mark_h = 26.0;
        let mark_rect = egui::Rect::from_center_size(
            Pos2::new(rect.left() + 27.0, rect.top() + 35.0),
            Vec2::new(mark_h * logos.mark_aspect(), mark_h),
        );
        ui.painter()
            .image(logos.mark.id(), mark_rect, UV_FULL, Color32::WHITE);

        // Wordmark from its own asset, so the typeface is the brand one.
        let word_h = 17.0;
        let word_rect = egui::Rect::from_min_size(
            Pos2::new(mark_rect.right() + 8.0, rect.top() + 35.0 - word_h / 2.0),
            Vec2::new(word_h * logos.wordmark_aspect(), word_h),
        );
        ui.painter()
            .image(logos.wordmark.id(), word_rect, UV_FULL, Color32::WHITE);
        return;
    }

    let painter = ui.painter();

    // Elsewhere the header is the section title, with a small brand mark.
    let logos = ensure_logos(app, ctx);
    let mark_h = 22.0;
    let mark_rect = egui::Rect::from_center_size(
        Pos2::new(rect.left() + 24.0, rect.top() + 35.0),
        Vec2::new(mark_h * logos.mark_aspect(), mark_h),
    );
    painter.image(logos.mark.id(), mark_rect, UV_FULL, Color32::WHITE);

    // Section title doubles as the wordmark, exactly like the mockup.
    let title = match app.screen {
        Screen::Place | Screen::Connecting => app
            .selected_place
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Noobietoria".to_owned()),
        _ => app.nav.label().to_owned(),
    };

    if app.screen == Screen::Place || app.screen == Screen::Connecting {
        // On a place, the back affordance comes first, then the place's name.
        let back = ui.interact(
            egui::Rect::from_min_size(
                Pos2::new(rect.left() + 46.0, rect.top() + 22.0),
                Vec2::new(56.0, 26.0),
            ),
            ui.id().with("back-to-home"),
            Sense::click(),
        );
        painter.text(
            Pos2::new(rect.left() + 52.0, rect.top() + 35.0),
            egui::Align2::LEFT_CENTER,
            "‹ Back",
            egui::FontId::proportional(13.0),
            if back.hovered() { INK } else { INK_SOFT },
        );
        if back.clicked() {
            app.go_home();
        }

        painter.text(
            Pos2::new(rect.left() + 118.0, rect.top() + 35.0),
            egui::Align2::LEFT_CENTER,
            truncate(&title, 46),
            egui::FontId::proportional(16.0),
            INK,
        );
    } else {
        painter.text(
            Pos2::new(rect.left() + 46.0, rect.top() + 35.0),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(16.0),
            INK,
        );
    }

    // A hairline under the header keeps the grid visually anchored.
    painter.line_segment(
        [
            Pos2::new(rect.left(), rect.bottom() - 0.5),
            Pos2::new(rect.right(), rect.bottom() - 0.5),
        ],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(27, 23, 23, 24)),
    );
}

fn draw_content(app: &mut ClientApp, ctx: &egui::Context, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            // All spacing is placed explicitly, so egui's defaults (which would
            // add 3px per row) are switched off to match the mockup's pitch.
            ui.spacing_mut().item_spacing.y = 0.0;
            match app.screen {
                Screen::Place => draw_place(app, ctx, ui),
                Screen::Connecting => draw_connecting(app, ui),
                _ => match app.nav {
                    other if other.is_implemented() => draw_home(app, ctx, ui),
                    other => draw_placeholder(ui, other),
                },
            }
        });
}

/// Sections the PC app will grow into; Home is the only complete one so far.
fn draw_placeholder(ui: &mut egui::Ui, section: NavSection) {
    ui.add_space(40.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 120.0), Sense::hover());
    let painter = ui.painter();
    draw_nav_icon(
        painter,
        section,
        Pos2::new(rect.center().x, rect.top() + 26.0),
        20.0,
        INK_SOFT,
        2.0,
    );
    painter.text(
        Pos2::new(rect.center().x, rect.top() + 70.0),
        egui::Align2::CENTER_CENTER,
        section.label(),
        egui::FontId::proportional(20.0),
        INK,
    );
    painter.text(
        Pos2::new(rect.center().x, rect.top() + 96.0),
        egui::Align2::CENTER_CENTER,
        "Coming in a later build — Home is the finished section today.",
        egui::FontId::proportional(12.0),
        INK_SOFT,
    );
}

// ── Home (Discover grid) ───────────────────────────────────────────────────────

pub fn draw_home(app: &mut ClientApp, ctx: &egui::Context, ui: &mut egui::Ui) {
    // A quiet status line so refresh state and API errors stay visible.
    draw_status_line(app, ui);

    if app.worlds_loading {
        loading_row(ui, "Loading places…");
        return;
    }
    if let Some(err) = app.worlds_error.clone() {
        error_row(ui, &err, "Try again", || 0);
        return;
    }

    let visible: Vec<WorldInfo> = app.visible_worlds().into_iter().cloned().collect();
    if visible.is_empty() {
        ui.add_space(24.0);
        ui.label(
            RichText::new("No places match your search.")
                .size(13.0)
                .color(INK_SOFT)
                .italics(),
        );
        return;
    }

    // Responsive columns: as many 122px tiles as fit with a 35px gutter, which
    // reproduces the mockup's 7 columns at its 1176px width.
    let avail = ui.available_width() - GRID_PAD * 2.0;
    let gap = COL_STEP - TILE_W;
    let cols = (((avail + gap) / COL_STEP).floor() as usize).max(1);
    let mut to_open: Option<WorldInfo> = None;

    for row_start in (0..visible.len()).step_by(cols) {
        ui.horizontal(|ui| {
            // The gutter is placed explicitly, so egui must not add its own
            // spacing on top of it.
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(GRID_PAD);
            for world in visible.iter().skip(row_start).take(cols) {
                if tile(ui, ensure_logos(app, ctx), world) {
                    to_open = Some(world.clone());
                }
            }
        });
        ui.add_space(ROW_STEP - (TILE_H + CAP_GAP + CAP1_H + CAP2_GAP + CAP2_H));
    }

    if let Some(place) = to_open {
        app.open_place(place);
    }
}

fn draw_status_line(app: &mut ClientApp, ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::hover());
    let painter = ui.painter();
    let busy = app.worlds_loading || app.servers_loading;
    let text = if busy {
        "Loading…".to_owned()
    } else {
        format!("{} places  ·  {}", app.worlds.len(), app.api_base)
    };
    painter.text(
        Pos2::new(rect.left() + GRID_PAD, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(11.0),
        INK_SOFT,
    );

    // Refresh control, right-aligned on the same line.
    let refresh = ui.interact(
        egui::Rect::from_min_size(
            Pos2::new(rect.right() - 74.0 - GRID_PAD, rect.top()),
            Vec2::new(74.0, rect.height()),
        ),
        ui.id().with("refresh"),
        Sense::click(),
    );
    painter.text(
        Pos2::new(rect.right() - GRID_PAD - 37.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        "⟳ Refresh",
        egui::FontId::proportional(11.0),
        if refresh.hovered() { INK } else { INK_SOFT },
    );
    if refresh.clicked() && !busy {
        match app.screen {
            Screen::Place => app.refresh_servers(),
            _ => app.refresh_worlds(),
        }
    }
}

/// One place tile: 122×134 rounded thumbnail plus two caption lines, at the
/// exact mockup metrics. Returns true when clicked.
fn tile(ui: &mut egui::Ui, logos: &Logos, world: &WorldInfo) -> bool {
    let unit = Vec2::new(TILE_W, TILE_H + CAP_GAP + CAP1_H + CAP2_GAP + CAP2_H);
    let (rect, resp) = ui.allocate_exact_size(unit, Sense::click());
    let hovered = resp.hovered();
    let painter = ui.painter();

    let thumb = egui::Rect::from_min_size(rect.min, Vec2::new(TILE_W, TILE_H));
    let fill = if hovered {
        Color32::from_rgb(232, 212, 212)
    } else {
        TILE
    };
    painter.rect_filled(thumb, TILE_RADIUS, fill);

    // The brand mark stands in for artwork until real thumbnails exist.
    draw_mark(logos, painter, thumb.center(), 30.0);

    let cap1 = egui::Rect::from_min_size(
        Pos2::new(rect.left(), thumb.bottom() + CAP_GAP),
        Vec2::new(TILE_W, CAP1_H),
    );
    painter.text(
        Pos2::new(cap1.left(), cap1.center().y),
        egui::Align2::LEFT_CENTER,
        truncate(&world.name, 20),
        egui::FontId::proportional(12.0),
        INK,
    );

    let cap2 = egui::Rect::from_min_size(
        Pos2::new(rect.left(), cap1.bottom() + CAP2_GAP),
        Vec2::new(TILE_W, CAP2_H),
    );
    let byline = format!(
        "{} · 👤 {}",
        truncate(&world.creator_name, 10),
        world.playing()
    );
    painter.text(
        Pos2::new(cap2.left(), cap2.center().y),
        egui::Align2::LEFT_CENTER,
        byline,
        egui::FontId::proportional(11.0),
        INK_SOFT,
    );

    // Leave the 35px gutter that the next column's tile will start after.
    ui.add_space(COL_STEP - TILE_W);
    resp.clicked()
}

// ── Place (server list) ────────────────────────────────────────────────────────

fn draw_place(app: &mut ClientApp, ctx: &egui::Context, ui: &mut egui::Ui) {
    let Some(place) = app.selected_place.clone() else {
        ui.add_space(24.0);
        ui.label(RichText::new("No place selected.").color(INK_SOFT));
        return;
    };

    draw_status_line(app, ui);

    // Place summary card.
    let card_w = (ui.available_width() - GRID_PAD * 2.0).min(720.0);
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(card_w + GRID_PAD * 2.0, 118.0), Sense::hover());
    let painter = ui.painter();
    let card = egui::Rect::from_min_size(
        Pos2::new(rect.left() + GRID_PAD, rect.top()),
        Vec2::new(card_w, 104.0),
    );
    painter.rect_filled(card, 12, TILE);

    let thumb = egui::Rect::from_min_size(
        Pos2::new(card.left() + 14.0, card.top() + 14.0),
        Vec2::new(76.0, 76.0),
    );
    painter.rect_filled(thumb, 12, Color32::from_rgb(230, 210, 210));
    draw_mark(ensure_logos(app, ctx), painter, thumb.center(), 42.0);

    let text_x = thumb.right() + 16.0;
    painter.text(
        Pos2::new(text_x, card.top() + 26.0),
        egui::Align2::LEFT_CENTER,
        truncate(&place.name, 44),
        egui::FontId::proportional(18.0),
        INK,
    );
    painter.text(
        Pos2::new(text_x, card.top() + 48.0),
        egui::Align2::LEFT_CENTER,
        format!(
            "By {} · 👍 {}% · 👁 {}",
            truncate(&place.creator_name, 22),
            place.like_pct(),
            place.visits
        ),
        egui::FontId::proportional(11.0),
        INK_SOFT,
    );
    painter.text(
        Pos2::new(text_x, card.top() + 70.0),
        egui::Align2::LEFT_CENTER,
        truncate(&place.description, 74),
        egui::FontId::proportional(11.0),
        INK_SOFT,
    );

    ui.add_space(14.0);

    if app.servers_loading {
        loading_row(ui, "Loading servers…");
        return;
    }
    if let Some(err) = app.servers_error.clone() {
        error_row(ui, &err, "Try again", || 0);
        return;
    }
    if app.servers.is_empty() {
        ui.add_space(8.0);
        ui.label(
            RichText::new("No server is running this place right now.")
                .size(13.0)
                .color(INK_SOFT)
                .italics(),
        );
        return;
    }

    let servers: Vec<ServerInfo> = app.servers.clone();
    let mut to_join: Option<ServerInfo> = None;
    for server in &servers {
        if server_card(ui, server) {
            to_join = Some(server.clone());
        }
    }
    if let Some(server) = to_join {
        let username = app.username.clone();
        app.join_server(server, username);
    }
}

/// One server row: region, occupancy, ping and a Join button. Returns true when
/// Join was pressed on a joinable server.
fn server_card(ui: &mut egui::Ui, server: &ServerInfo) -> bool {
    let width = (ui.available_width() - GRID_PAD * 2.0).min(720.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width + GRID_PAD * 2.0, 60.0), Sense::hover());
    let painter = ui.painter();

    let card = egui::Rect::from_min_size(
        Pos2::new(rect.left() + GRID_PAD, rect.top()),
        Vec2::new(width, 52.0),
    );
    painter.rect_filled(card, 12, TILE);

    let region = if server.region.is_empty() {
        "Unknown region"
    } else {
        server.region.as_str()
    };
    painter.text(
        Pos2::new(card.left() + 14.0, card.center().y - 8.0),
        egui::Align2::LEFT_CENTER,
        region,
        egui::FontId::proportional(13.0),
        INK,
    );
    painter.text(
        Pos2::new(card.left() + 14.0, card.center().y + 9.0),
        egui::Align2::LEFT_CENTER,
        format!(
            "👤 {}   🏓 {} ms   {}",
            server.occupancy(),
            server.ping_ms,
            match server.status.as_str() {
                "full" => "Full",
                "offline" => "Offline",
                _ => "Online",
            }
        ),
        egui::FontId::proportional(11.0),
        INK_SOFT,
    );
    painter.text(
        Pos2::new(card.right() - 108.0, card.center().y),
        egui::Align2::RIGHT_CENTER,
        server.endpoint(),
        egui::FontId::proportional(10.0),
        INK_SOFT,
    );

    let join_rect = egui::Rect::from_min_size(
        Pos2::new(card.right() - 96.0, card.center().y - 15.0),
        Vec2::new(84.0, 30.0),
    );
    let joinable = server.is_joinable();
    let resp = if joinable {
        ui.interact(
            join_rect,
            ui.id().with(("join", &server.server_id)),
            Sense::click(),
        )
    } else {
        // Keep the layout stable but refuse interaction on full/offline servers.
        ui.interact(
            join_rect,
            ui.id().with(("join", &server.server_id)),
            Sense::hover(),
        )
    };
    painter.rect_filled(
        join_rect,
        8,
        if !joinable {
            Color32::from_rgb(200, 178, 178)
        } else if resp.hovered() {
            Color32::from_rgb(180, 100, 101)
        } else {
            RAIL
        },
    );
    painter.text(
        join_rect.center(),
        egui::Align2::CENTER_CENTER,
        if joinable { "▶ Join" } else { "Full" },
        egui::FontId::proportional(12.0),
        if joinable { GAME_INK } else { INK_SOFT },
    );

    ui.add_space(8.0);
    joinable && resp.clicked()
}

// ── Connecting ─────────────────────────────────────────────────────────────────

fn draw_connecting(app: &mut ClientApp, ui: &mut egui::Ui) {
    ui.add_space(60.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 170.0), Sense::hover());
    let painter = ui.painter();

    spinner(
        painter,
        Pos2::new(rect.center().x, rect.top() + 24.0),
        18.0,
        ui.ctx(),
    );

    painter.text(
        Pos2::new(rect.center().x, rect.top() + 76.0),
        egui::Align2::CENTER_CENTER,
        "Joining",
        egui::FontId::proportional(13.0),
        INK_SOFT,
    );
    let place = app
        .selected_place
        .as_ref()
        .map(|p| p.name.clone())
        .or_else(|| {
            app.joining_server
                .as_ref()
                .map(|s| s.place_name.clone())
                .filter(|n| !n.is_empty())
        })
        .unwrap_or_else(|| "Place".to_owned());
    painter.text(
        Pos2::new(rect.center().x, rect.top() + 102.0),
        egui::Align2::CENTER_CENTER,
        truncate(&place, 48),
        egui::FontId::proportional(20.0),
        INK,
    );
    if let Some(server) = app.joining_server.as_ref() {
        painter.text(
            Pos2::new(rect.center().x, rect.top() + 128.0),
            egui::Align2::CENTER_CENTER,
            format!("{}  ·  {}", server.server_id, server.endpoint()),
            egui::FontId::proportional(11.0),
            INK_SOFT,
        );
    }

    let cancel = egui::Rect::from_center_size(
        Pos2::new(rect.center().x, rect.top() + 156.0),
        Vec2::new(96.0, 28.0),
    );
    let resp = ui.interact(cancel, ui.id().with("cancel-join"), Sense::click());
    painter.rect_filled(
        cancel,
        8,
        if resp.hovered() {
            Color32::from_rgb(180, 100, 101)
        } else {
            RAIL
        },
    );
    painter.text(
        cancel.center(),
        egui::Align2::CENTER_CENTER,
        "Cancel",
        egui::FontId::proportional(12.0),
        GAME_INK,
    );
    if resp.clicked() {
        app.do_disconnect();
    }

    ui.ctx().request_repaint(); // Keep the spinner animating.
}

// ── Lobby (in-game HUD) ────────────────────────────────────────────────────────

fn draw_lobby(app: &mut ClientApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("gamebar")
        .frame(
            egui::Frame::default()
                .fill(GAME_PANEL)
                .inner_margin(egui::Margin::symmetric(12, 6)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("▣ Noobietoria").color(RAIL).strong());
                ui.separator();
                ui.label(RichText::new(&app.server_name).color(GAME_INK).strong());

                if let Some(place) = &app.selected_place {
                    ui.separator();
                    ui.label(RichText::new(&place.name).color(GAME_INK));
                }

                if let Some(ms) = app.ping_ms {
                    ui.separator();
                    let color = if ms < 100 {
                        GAME_GO
                    } else if ms < 300 {
                        GAME_WARN
                    } else {
                        GAME_BAD
                    };
                    ui.label(RichText::new(format!("🏓 {ms} ms")).color(color));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Leave").color(GAME_INK))
                                .fill(GAME_BAD),
                        )
                        .clicked()
                    {
                        app.do_disconnect();
                    }
                    if ui.button("Ping").clicked() {
                        app.do_ping();
                    }
                    ui.label(
                        RichText::new(format!("🧩 {} instances", app.snapshot.instances.len()))
                            .size(11.0)
                            .color(GAME_WARN),
                    );
                });
            });
        });

    egui::SidePanel::left("explorer")
        .default_width(260.0)
        .min_width(180.0)
        .frame(
            egui::Frame::default()
                .fill(GAME_PANEL)
                .inner_margin(egui::Margin::same(8)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🌍 World Explorer").color(RAIL).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("tick {}", app.snapshot.tick))
                            .size(10.0)
                            .color(GAME_WARN),
                    );
                });
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                if app.snapshot.instances.is_empty() {
                    ui.label(
                        RichText::new("(waiting for snapshot…)")
                            .color(GAME_WARN)
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
                            ui.label(RichText::new(name).size(13.0).color(GAME_INK));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(RichText::new(container).size(10.0).color(GAME_WARN));
                                },
                            );
                        });
                    }
                }
            });
        });

    egui::SidePanel::right("events")
        .default_width(280.0)
        .min_width(200.0)
        .frame(
            egui::Frame::default()
                .fill(GAME_PANEL)
                .inner_margin(egui::Margin::same(8)),
        )
        .show(ctx, |ui| {
            ui.label(RichText::new("📡 Event Console").color(RAIL).strong());
            ui.separator();

            ui.label(RichText::new("Event name:").size(11.0).color(GAME_WARN));
            ui.add(
                egui::TextEdit::singleline(&mut app.event_name)
                    .hint_text("e.g. Jump")
                    .min_size(Vec2::new(ui.available_width(), 26.0)),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Args (JSON, comma-separated):")
                    .size(11.0)
                    .color(GAME_WARN),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.event_args)
                    .hint_text(r#""Alice", 42"#)
                    .min_size(Vec2::new(ui.available_width(), 26.0)),
            );
            ui.add_space(6.0);

            if ui
                .add(
                    egui::Button::new(RichText::new("🔥 Fire Event").color(GAME_INK))
                        .fill(RAIL)
                        .min_size(Vec2::new(ui.available_width(), 32.0)),
                )
                .clicked()
            {
                app.do_fire();
            }

            ui.add_space(14.0);
            ui.separator();
            ui.label(RichText::new("Session").size(11.0).color(GAME_WARN));
            ui.label(
                RichText::new(format!("endpoint  {}", app.connected_endpoint))
                    .size(10.0)
                    .color(GAME_WARN),
            );
            ui.label(
                RichText::new(format!("server    {}", app.server_name))
                    .size(10.0)
                    .color(GAME_WARN),
            );
        });

    egui::CentralPanel::default()
        .frame(
            egui::Frame::default()
                .fill(GAME_BG)
                .inner_margin(egui::Margin::same(8)),
        )
        .show(ctx, |ui| {
            ui.label(RichText::new("📋 Log").color(RAIL).strong());
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for entry in &app.log {
                        let (icon, color) = match entry.kind {
                            LogKind::System => ("·", GAME_WARN),
                            LogKind::Event => ("↔", RAIL),
                            LogKind::Error => ("✗", GAME_BAD),
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

// ── Drawing helpers ────────────────────────────────────────────────────────────

/// Paints the brand mark centred on `center` at the requested height.
fn draw_mark(logos: &Logos, painter: &egui::Painter, center: Pos2, height: f32) {
    let rect =
        egui::Rect::from_center_size(center, Vec2::new(height * logos.mark_aspect(), height));
    painter.image(logos.mark.id(), rect, UV_FULL, Color32::WHITE);
}

/// Draw one rail glyph in the Roblox-style outline set.
fn draw_nav_icon(
    painter: &egui::Painter,
    section: NavSection,
    center: Pos2,
    size: f32,
    color: Color32,
    width: f32,
) {
    let s = size;
    let p = |x: f32, y: f32| Pos2::new(center.x + x * s, center.y + y * s);
    let line = |a: Pos2, b: Pos2| painter.line_segment([a, b], Stroke::new(width, color));

    match section {
        NavSection::Home => {
            // Roof, then the walls and floor.
            line(p(-0.95, 0.05), p(0.0, -0.85));
            line(p(0.0, -0.85), p(0.95, 0.05));
            line(p(-0.72, 0.0), p(-0.72, 0.80));
            line(p(-0.72, 0.80), p(0.72, 0.80));
            line(p(0.72, 0.80), p(0.72, 0.0));
        }
        NavSection::Search => {
            painter.circle_stroke(p(-0.15, -0.15), 0.62 * s, Stroke::new(width, color));
            line(p(0.32, 0.32), p(0.88, 0.88));
        }
        NavSection::Explore => {
            painter.circle_stroke(center, 0.90 * s, Stroke::new(width, color));
            // Needle: a diamond outline with its north-east half filled.
            painter.add(egui::Shape::convex_polygon(
                vec![p(0.0, -0.62), p(0.44, 0.0), p(0.0, 0.62), p(-0.44, 0.0)],
                Color32::TRANSPARENT,
                Stroke::new(width, color),
            ));
            painter.add(egui::Shape::convex_polygon(
                vec![p(0.0, -0.62), p(0.44, 0.0), p(0.0, 0.0)],
                color,
                Stroke::NONE,
            ));
        }
        NavSection::Shop => {
            line(p(-0.90, -0.72), p(-0.58, -0.72));
            line(p(-0.58, -0.72), p(-0.28, 0.12));
            // Basket.
            line(p(-0.46, 0.12), p(0.72, 0.12));
            line(p(-0.28, -0.34), p(0.88, -0.34));
            line(p(-0.46, 0.12), p(-0.28, -0.34));
            line(p(0.72, 0.12), p(0.88, -0.34));
            // Wheels.
            painter.circle_filled(p(-0.16, 0.66), 0.13 * s, color);
            painter.circle_filled(p(0.62, 0.66), 0.13 * s, color);
        }
        NavSection::Create => {
            painter.circle_stroke(center, 0.90 * s, Stroke::new(width, color));
            line(p(-0.42, 0.0), p(0.42, 0.0));
            line(p(0.0, -0.42), p(0.0, 0.42));
        }
        NavSection::Chat => {
            let bubble = egui::Rect::from_min_max(p(-0.86, -0.70), p(0.86, 0.42));
            painter.rect_stroke(bubble, 6, Stroke::new(width, color), StrokeKind::Middle);
            // Tail.
            painter.add(egui::Shape::convex_polygon(
                vec![p(-0.34, 0.42), p(-0.52, 0.86), p(-0.06, 0.42)],
                color,
                Stroke::NONE,
            ));
            for x in [-0.44, -0.14, 0.16, 0.46] {
                painter.circle_filled(p(x, -0.14), 0.07 * s, color);
            }
        }
        NavSection::Profile => {
            painter.circle_stroke(center, 0.90 * s, Stroke::new(width, color));
            painter.circle_filled(p(0.0, -0.24), 0.28 * s, color);
            painter.add(egui::Shape::convex_polygon(
                vec![p(-0.54, 0.60), p(-0.26, 0.16), p(0.26, 0.16), p(0.54, 0.60)],
                color,
                Stroke::NONE,
            ));
        }
    }
}

fn loading_row(ui: &mut egui::Ui, label: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::hover());
    let painter = ui.painter();
    spinner(
        painter,
        Pos2::new(rect.left() + GRID_PAD + 9.0, rect.center().y),
        9.0,
        ui.ctx(),
    );
    painter.text(
        Pos2::new(rect.left() + GRID_PAD + 26.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        INK_SOFT,
    );
}

fn error_row(ui: &mut egui::Ui, message: &str, action: &str, _on_action: impl FnOnce() -> i32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::hover());
    let painter = ui.painter();
    painter.text(
        Pos2::new(rect.left() + GRID_PAD, rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("⚠ {message}"),
        egui::FontId::proportional(12.0),
        INK,
    );
    painter.text(
        Pos2::new(rect.left() + GRID_PAD + 420.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        action,
        egui::FontId::proportional(11.0),
        INK_SOFT,
    );
}

/// Unit UV: the whole texture.
const UV_FULL: egui::Rect = egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));

/// Animated ring of dots for the loading states.
fn spinner(painter: &egui::Painter, center: Pos2, radius: f32, ctx: &egui::Context) {
    let t = ctx.input(|i| i.time) as f32;
    for i in 0..8 {
        let angle = i as f32 * std::f32::consts::TAU / 8.0 + t * 3.0;
        let alpha = 0.15 + 0.85 * (i as f32 / 8.0);
        let color =
            Color32::from_rgba_unmultiplied(INK.r(), INK.g(), INK.b(), (255.0 * alpha) as u8);
        painter.circle_filled(
            Pos2::new(
                center.x + angle.cos() * radius * 0.6,
                center.y + angle.sin() * radius * 0.6,
            ),
            2.0,
            color,
        );
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn instance_icon(kind: &str) -> &'static str {
    match kind {
        "Part" | "TrussPart" | "UnionOperation" | "MeshPart" | "SpecialMesh" => "⬜",
        "ServerScript" | "ClientScript" | "Script" | "LocalScript" | "ModuleScript" => "📜",
        "Model" => "🧩",
        "Folder" => "📁",
        "SpawnLocation" => "🏁",
        "Attachment" | "Trigger" => "📍",
        "Sound" | "Audio" => "🔊",
        "NetworkEvent" => "📡",
        "ScreenGui" | "Frame" | "ScrollingFrame" => "🖼",
        "TextLabel" | "TextButton" | "TextBox" => "🔤",
        "ImageLabel" | "ImageButton" => "🎨",
        _ => "●",
    }
}
