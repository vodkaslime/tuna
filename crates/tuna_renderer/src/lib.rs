pub mod prelude {
    pub use crate::TerminalRendererPlugin;
    pub use crate::TerminalSurfaceConfig;
}

use std::collections::HashMap;
use std::path::Path;

use ab_glyph::{point, Font, FontArc, PxScale, ScaleFont};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use tuna_terminal_core::screen::{CellColor, ScreenSnapshot};

pub struct TerminalRendererPlugin;

pub fn measure_cell_size(font_path: &str, font_size_px: f32) -> Option<(u32, u32)> {
    let font = try_load_font_arc(font_path)?;
    let scale = PxScale::from(font_size_px.max(1.0));
    let scaled = font.as_scaled(scale);

    // Use a representative glyph for monospace advance width.
    let advance = scaled.h_advance(font.glyph_id('M'));
    // Ignore line gap by default to avoid extra vertical padding between rows.
    let line_height = scaled.ascent() - scaled.descent();

    let cell_w = advance.round().max(1.0) as u32;
    let cell_h = line_height.round().max(1.0) as u32;

    Some((cell_w.max(1), cell_h.max(1)))
}

impl Plugin for TerminalRendererPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerminalSurfaceConfig>()
            .add_systems(Startup, setup_renderer)
            .add_systems(Update, (sync_surfaces_to_config, render_terminal_surfaces).chain());
    }
}

#[derive(Resource, Debug, Clone)]
pub struct TerminalSurfaceConfig {
    pub cols: u16,
    pub rows: u16,
    pub cell_w: u32,
    pub cell_h: u32,
    pub font_path: Option<String>,
    pub font_size_px: f32,
    pub fg: Color,
    pub bg: Color,
    pub show_fps: bool,
}

impl Default for TerminalSurfaceConfig {
    fn default() -> Self {
        Self {
            cols: 120,
            rows: 40,
            cell_w: 12,
            cell_h: 20,
            font_path: None,
            font_size_px: 18.0,
            fg: Color::srgb(0.85, 0.85, 0.85),
            bg: Color::srgb(0.05, 0.05, 0.06),
            show_fps: false,
        }
    }
}

#[derive(Component)]
pub struct TerminalSurface {
    pub image: Handle<Image>,
    pub width: u32,
    pub height: u32,
    pub cell_w: u32,
    pub cell_h: u32,
}

#[derive(Component, Default)]
struct TerminalRenderState {
    last_generation: u64,
}

#[derive(Debug, Clone)]
pub struct SpawnedTerminalSurface {
    pub entity: Entity,
    pub image: Handle<Image>,
    pub width: u32,
    pub height: u32,
}

#[derive(Resource)]
struct TerminalFont {
    font_path: String,
    font: FontArc,
    scale: PxScale,
    baseline_y: f32,
    glyphs: HashMap<char, RasterizedGlyph>,
}

#[derive(Debug, Clone)]
struct RasterizedGlyph {
    min_x: i32,
    min_y: i32,
    width: u32,
    height: u32,
    coverage: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct TerminalViewport {
    pub origin_col: u16,
    pub origin_row: u16,
    pub focused: bool,
    pub snapshot: ScreenSnapshot,
    pub highlight_mask: Option<Vec<u8>>,
}

/// Composite terminal screen for a window: a list of viewports (split panes) plus a monotonic
/// `generation` counter.
///
/// The app is expected to insert/update this component when the terminal state changes.
#[derive(Component, Debug, Clone)]
pub struct TerminalScreen {
    pub generation: u64,
    pub cols: u16,
    pub rows: u16,
    pub viewports: Vec<TerminalViewport>,
    pub overlay_text: Option<String>,
}

impl TerminalFont {
    fn sync_to_config(&mut self, config: &TerminalSurfaceConfig) -> bool {
        let target_font_path = resolve_font_path(config);
        if self.font_path != target_font_path {
            *self = load_terminal_font(config);
            return true;
        }

        let target_scale = PxScale::from(config.font_size_px.max(1.0));
        let cell_h = config.cell_h.max(1);
        let target_baseline_y = compute_baseline_y(&self.font, target_scale, cell_h);

        let mut changed = false;
        if self.scale != target_scale {
            self.scale = target_scale;
            changed = true;
        }
        if (self.baseline_y - target_baseline_y).abs() > f32::EPSILON {
            self.baseline_y = target_baseline_y;
            changed = true;
        }

        if changed {
            self.glyphs.clear();
        }

        changed
    }

    fn glyph(&mut self, ch: char) -> Option<&RasterizedGlyph> {
        let ch = if ch == '\t' { ' ' } else { ch };
        if ch == ' ' {
            return None;
        }
        if self.glyphs.contains_key(&ch) {
            return self.glyphs.get(&ch);
        }

        let raster = rasterize_glyph(&self.font, self.scale, self.baseline_y, ch)
            .or_else(|| rasterize_glyph(&self.font, self.scale, self.baseline_y, '?'))?;

        self.glyphs.insert(ch, raster);
        self.glyphs.get(&ch)
    }
}

fn load_terminal_font(config: &TerminalSurfaceConfig) -> TerminalFont {
    let font_path = resolve_font_path(config);
    let font_bytes = std::fs::read(&font_path)
        .unwrap_or_else(|err| panic!("failed to read font at {font_path}: {err}"));
    let font =
        FontArc::try_from_vec(font_bytes).unwrap_or_else(|err| panic!("invalid font {font_path}: {err}"));

    let scale = PxScale::from(config.font_size_px.max(1.0));
    let cell_h = config.cell_h.max(1);
    let baseline_y = compute_baseline_y(&font, scale, cell_h);

    TerminalFont {
        font_path,
        font,
        scale,
        baseline_y,
        glyphs: HashMap::new(),
    }
}

fn resolve_font_path(config: &TerminalSurfaceConfig) -> String {
    let mut candidates: Vec<String> = Vec::new();

    if let Some(path) = config.font_path.as_ref().map(|p| p.trim()).filter(|p| !p.is_empty()) {
        let path = path.to_string();
        if try_load_font_arc(&path).is_some() {
            return path;
        }
        panic!("invalid font at {path}; set TerminalSurfaceConfig.font_path to a valid .ttf/.otf");
    }

    if let Ok(path) = std::env::var("tuna_FONT_PATH") {
        let path = path.trim().to_string();
        if !path.is_empty() {
            candidates.push(path);
        }
    }

    #[cfg(target_os = "linux")]
    candidates.extend(
        [
            "/usr/share/fonts/liberation-mono-fonts/LiberationMono-Regular.ttf",
            "/usr/share/fonts/adobe-source-code-pro-fonts/SourceCodePro-Regular.otf",
            "/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf",
            "/usr/share/fonts/google-noto-vf/NotoSansMono[wght].ttf",
        ]
        .into_iter()
        .map(str::to_string),
    );

    #[cfg(target_os = "macos")]
    candidates.extend(
        [
            "/System/Library/Fonts/Monaco.ttf",
            "/System/Library/Fonts/Supplemental/Andale Mono.ttf",
            "/System/Library/Fonts/Supplemental/Courier New.ttf",
        ]
        .into_iter()
        .map(str::to_string),
    );

    #[cfg(target_os = "windows")]
    candidates.extend(
        [
            r"C:\Windows\Fonts\consola.ttf",
            r"C:\Windows\Fonts\lucon.ttf",
            r"C:\Windows\Fonts\cour.ttf",
        ]
        .into_iter()
        .map(str::to_string),
    );

    for candidate in candidates {
        if try_load_font_arc(&candidate).is_some() {
            return candidate;
        }
    }

    panic!("no usable font found; set TerminalSurfaceConfig.font_path or tuna_FONT_PATH to a .ttf/.otf");
}

fn compute_baseline_y(font: &FontArc, scale: PxScale, cell_h: u32) -> f32 {
    let scaled = font.as_scaled(scale);
    let line_height = scaled.ascent() - scaled.descent() + scaled.line_gap();
    let cell_h = cell_h as f32;
    let top = ((cell_h - line_height).max(0.0)) / 2.0;
    top + scaled.ascent()
}

fn try_load_font_arc(path: &str) -> Option<FontArc> {
    if path.trim().is_empty() {
        return None;
    }
    if !Path::new(path).exists() {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    FontArc::try_from_vec(bytes).ok()
}

fn rasterize_glyph(font: &FontArc, scale: PxScale, baseline_y: f32, ch: char) -> Option<RasterizedGlyph> {
    if font.glyph_id(ch).0 == 0 {
        return None;
    }

    let glyph = font
        .glyph_id(ch)
        .with_scale_and_position(scale, point(0.0, baseline_y));
    let outlined = font.outline_glyph(glyph)?;
    let bounds = outlined.px_bounds();
    let width = bounds.width() as u32;
    let height = bounds.height() as u32;

    if width == 0 || height == 0 {
        return Some(RasterizedGlyph {
            min_x: bounds.min.x as i32,
            min_y: bounds.min.y as i32,
            width,
            height,
            coverage: Vec::new(),
        });
    }

    let mut coverage = vec![0u8; (width * height) as usize];
    outlined.draw(|x, y, c| {
        let idx = (y * width + x) as usize;
        let alpha = (c * 255.0).clamp(0.0, 255.0) as u8;
        coverage[idx] = alpha;
    });

    Some(RasterizedGlyph {
        min_x: bounds.min.x as i32,
        min_y: bounds.min.y as i32,
        width,
        height,
        coverage,
    })
}

fn setup_renderer(mut commands: Commands, config: Res<TerminalSurfaceConfig>) {
    commands.insert_resource(load_terminal_font(config.as_ref()));
}

pub fn spawn_terminal_surface(
    commands: &mut Commands,
    config: &TerminalSurfaceConfig,
    images: &mut Assets<Image>,
) -> SpawnedTerminalSurface {
    let cell_w = config.cell_w.max(1);
    let cell_h = config.cell_h.max(1);
    let width = u32::from(config.cols) * cell_w;
    let height = u32::from(config.rows) * cell_h;

    let mut image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::render::texture::ImageSampler::nearest();

    let image_handle = images.add(image);

    let entity = commands
        .spawn((
            TerminalRenderState::default(),
            TerminalSurface {
                image: image_handle.clone(),
                width,
                height,
                cell_w,
                cell_h,
            },
        ))
        .id();

    SpawnedTerminalSurface {
        entity,
        image: image_handle,
        width,
        height,
    }
}

fn render_terminal_surfaces(
    config: Res<TerminalSurfaceConfig>,
    mut font: ResMut<TerminalFont>,
    mut images: ResMut<Assets<Image>>,
    mut surfaces: Query<(&TerminalSurface, &TerminalScreen, &mut TerminalRenderState)>,
) {
    for (surface, screen, mut state) in &mut surfaces {
        if screen.generation == state.last_generation {
            continue;
        }

        let Some(image) = images.get_mut(&surface.image) else {
            continue;
        };

        draw_composite_screen(
            screen,
            config.bg,
            config.fg,
            font.as_mut(),
            surface.width,
            surface.height,
            surface.cell_w,
            surface.cell_h,
            &mut image.data,
        );

        state.last_generation = screen.generation;
    }
}

fn sync_surfaces_to_config(
    config: Res<TerminalSurfaceConfig>,
    mut font: ResMut<TerminalFont>,
    mut images: ResMut<Assets<Image>>,
    mut surfaces: Query<(&mut TerminalSurface, &mut TerminalRenderState)>,
) {
    if !config.is_changed() {
        return;
    }

    let _font_changed = font.sync_to_config(config.as_ref());

    let cell_w = config.cell_w.max(1);
    let cell_h = config.cell_h.max(1);
    let width = u32::from(config.cols) * cell_w;
    let height = u32::from(config.rows) * cell_h;

    for (mut surface, mut state) in &mut surfaces {
        let surface_changed = width != surface.width
            || height != surface.height
            || cell_w != surface.cell_w
            || cell_h != surface.cell_h;
        if surface_changed {
            if let Some(image) = images.get_mut(&surface.image) {
                image.resize(Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                });
                image.sampler = bevy::render::texture::ImageSampler::nearest();
            } else {
                let mut image = Image::new_fill(
                    Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    &[0, 0, 0, 255],
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::default(),
                );
                image.sampler = bevy::render::texture::ImageSampler::nearest();
                surface.image = images.add(image);
            }
            surface.width = width;
            surface.height = height;
            surface.cell_w = cell_w;
            surface.cell_h = cell_h;
        }

        // Force rerender on config changes (colors, font, etc), even if the terminal generation didn't change.
        state.last_generation = 0;
    }
}

fn draw_composite_screen(
    screen: &TerminalScreen,
    bg: Color,
    fg: Color,
    font: &mut TerminalFont,
    width: u32,
    height: u32,
    cell_w: u32,
    cell_h: u32,
    pixels: &mut [u8],
) {
    let bg = bg.to_srgba();
    let fg = fg.to_srgba();

    let bg_rgba = [
        (bg.red * 255.0) as u8,
        (bg.green * 255.0) as u8,
        (bg.blue * 255.0) as u8,
        (bg.alpha * 255.0) as u8,
    ];
    let fg_rgba = [
        (fg.red * 255.0) as u8,
        (fg.green * 255.0) as u8,
        (fg.blue * 255.0) as u8,
        (fg.alpha * 255.0) as u8,
    ];

    // Clear background.
    for px in pixels.chunks_exact_mut(4) {
        px.copy_from_slice(&bg_rgba);
    }

    if screen.cols == 0 || screen.rows == 0 {
        return;
    }

    for viewport in &screen.viewports {
        draw_snapshot(
            &viewport.snapshot,
            viewport.origin_col,
            viewport.origin_row,
            viewport.focused,
            viewport.highlight_mask.as_deref(),
            fg_rgba,
            bg_rgba,
            font,
            width,
            height,
            cell_w,
            cell_h,
            pixels,
        );
    }

    if let Some(text) = &screen.overlay_text {
        draw_overlay(text, fg_rgba, bg_rgba, font, width, height, cell_w, cell_h, pixels);
    }
}

fn draw_overlay(
    text: &str,
    fg_rgba: [u8; 4],
    bg_rgba: [u8; 4],
    font: &mut TerminalFont,
    width: u32,
    height: u32,
    cell_w: u32,
    cell_h: u32,
    pixels: &mut [u8],
) {
    if height < cell_h {
        return;
    }

    // Darken the top row.
    let mut bar_bg = bg_rgba;
    bar_bg[0] = bar_bg[0].saturating_add(10);
    bar_bg[1] = bar_bg[1].saturating_add(10);
    bar_bg[2] = bar_bg[2].saturating_add(12);

    for y in 0..cell_h {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            if idx + 4 <= pixels.len() {
                pixels[idx..idx + 4].copy_from_slice(&bar_bg);
            }
        }
    }

    // Draw overlay text at (0, 0) cell coords.
    let cols = width / cell_w;
    for (i, ch) in text.chars().take(cols as usize).enumerate() {
        let x0 = i as u32 * cell_w;
        let y0 = 0;
        draw_char(font, ch, fg_rgba, x0, y0, cell_w, cell_h, width, height, pixels);
    }
}

fn fill_rect(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    x0: u32,
    y0: u32,
    rect_w: u32,
    rect_h: u32,
    rgba: [u8; 4],
) {
    let x1 = x0.saturating_add(rect_w).min(width);
    let y1 = y0.saturating_add(rect_h).min(height);
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    for y in y0..y1 {
        let start = ((y * width + x0) * 4) as usize;
        let end = ((y * width + x1) * 4) as usize;
        for px in pixels[start..end].chunks_exact_mut(4) {
            px.copy_from_slice(&rgba);
        }
    }
}

fn draw_char(
    font: &mut TerminalFont,
    ch: char,
    fg_rgba: [u8; 4],
    cell_x0: u32,
    cell_y0: u32,
    cell_w: u32,
    cell_h: u32,
    width: u32,
    height: u32,
    pixels: &mut [u8],
) {
    let Some(glyph) = font.glyph(ch) else {
        return;
    };
    if glyph.width == 0 || glyph.height == 0 {
        return;
    }

    let cell_x0_i32 = cell_x0 as i32;
    let cell_y0_i32 = cell_y0 as i32;
    let cell_x1_i32 = cell_x0_i32 + cell_w as i32;
    let cell_y1_i32 = cell_y0_i32 + cell_h as i32;

    let min_x = glyph.min_x;
    let min_y = glyph.min_y;
    for gy in 0..glyph.height {
        let y = cell_y0_i32 + min_y + gy as i32;
        if y < cell_y0_i32 || y >= cell_y1_i32 {
            continue;
        }
        if y < 0 || y >= height as i32 {
            continue;
        }

        for gx in 0..glyph.width {
            let x = cell_x0_i32 + min_x + gx as i32;
            if x < cell_x0_i32 || x >= cell_x1_i32 {
                continue;
            }
            if x < 0 || x >= width as i32 {
                continue;
            }

            let idx = (gy * glyph.width + gx) as usize;
            let alpha = glyph.coverage.get(idx).copied().unwrap_or(0);
            if alpha == 0 {
                continue;
            }

            let px_idx = ((y as u32 * width + x as u32) * 4) as usize;
            if px_idx + 4 > pixels.len() {
                continue;
            }
            blend_over(&mut pixels[px_idx..px_idx + 4], fg_rgba, alpha);
        }
    }
}

fn blend_over(dst: &mut [u8], src: [u8; 4], alpha: u8) {
    let inv = 255_u16 - u16::from(alpha);
    let alpha = u16::from(alpha);

    dst[0] = ((u16::from(src[0]) * alpha + u16::from(dst[0]) * inv + 127) / 255) as u8;
    dst[1] = ((u16::from(src[1]) * alpha + u16::from(dst[1]) * inv + 127) / 255) as u8;
    dst[2] = ((u16::from(src[2]) * alpha + u16::from(dst[2]) * inv + 127) / 255) as u8;
    dst[3] = 255;
}

fn draw_snapshot(
    snapshot: &ScreenSnapshot,
    origin_col: u16,
    origin_row: u16,
    focused: bool,
    highlight_mask: Option<&[u8]>,
    fg_rgba: [u8; 4],
    bg_rgba: [u8; 4],
    font: &mut TerminalFont,
    width: u32,
    height: u32,
    cell_w: u32,
    cell_h: u32,
    pixels: &mut [u8],
) {
    let cols = u32::from(snapshot.cols);
    let rows = u32::from(snapshot.rows);
    if cols == 0 || rows == 0 {
        return;
    }

    let mut default_fg_rgba = fg_rgba;
    let mut default_bg_rgba = bg_rgba;
    if let CellColor::Rgb { r, g, b } = snapshot.default_fg {
        default_fg_rgba = [r, g, b, 255];
    }
    if let CellColor::Rgb { r, g, b } = snapshot.default_bg {
        default_bg_rgba = [r, g, b, 255];
    }

    let cursor_col = if focused && snapshot.cursor_visible {
        u32::from(snapshot.cursor_col).min(cols.saturating_sub(1))
    } else {
        u32::MAX
    };
    let cursor_row = if focused && snapshot.cursor_visible {
        u32::from(snapshot.cursor_row).min(rows.saturating_sub(1))
    } else {
        u32::MAX
    };

    let origin_col = u32::from(origin_col);
    let origin_row = u32::from(origin_row);

    if default_bg_rgba != bg_rgba {
        fill_rect(
            pixels,
            width,
            height,
            origin_col * cell_w,
            origin_row * cell_h,
            cols * cell_w,
            rows * cell_h,
            default_bg_rgba,
        );
    }

    for row in 0..rows {
        for col in 0..cols {
            let cell = snapshot.cell(col as u16, row as u16);
            let is_cursor = col == cursor_col && row == cursor_row;
            let mut fg = rgba_for_cell_color(cell.fg, default_fg_rgba, default_bg_rgba);
            let mut bg = rgba_for_cell_color(cell.bg, default_fg_rgba, default_bg_rgba);

            if cell.selected {
                std::mem::swap(&mut fg, &mut bg);
            }

            if let Some(mask) = highlight_mask {
                let idx = (row * cols + col) as usize;
                if idx < mask.len() && mask[idx] != 0 {
                    std::mem::swap(&mut fg, &mut bg);
                }
            }
            if is_cursor {
                std::mem::swap(&mut fg, &mut bg);
            }

            let x0 = (origin_col + col) * cell_w;
            let y0 = (origin_row + row) * cell_h;

            if bg != default_bg_rgba {
                fill_rect(pixels, width, height, x0, y0, cell_w, cell_h, bg);
            }

            if cell.ch != ' ' {
                draw_char(font, cell.ch, fg, x0, y0, cell_w, cell_h, width, height, pixels);
            }
        }
    }
}

fn rgba_for_cell_color(color: CellColor, default_fg: [u8; 4], default_bg: [u8; 4]) -> [u8; 4] {
    match color {
        CellColor::DefaultForeground => default_fg,
        CellColor::DefaultBackground => default_bg,
        CellColor::Rgb { r, g, b } => [r, g, b, 255],
        CellColor::Ansi256(index) => {
            let (r, g, b) = xterm_256_rgb(index);
            [r, g, b, 255]
        }
    }
}

fn xterm_256_rgb(index: u8) -> (u8, u8, u8) {
    match index {
        0 => (0x00, 0x00, 0x00),
        1 => (0xcd, 0x00, 0x00),
        2 => (0x00, 0xcd, 0x00),
        3 => (0xcd, 0xcd, 0x00),
        4 => (0x00, 0x00, 0xee),
        5 => (0xcd, 0x00, 0xcd),
        6 => (0x00, 0xcd, 0xcd),
        7 => (0xe5, 0xe5, 0xe5),
        8 => (0x7f, 0x7f, 0x7f),
        9 => (0xff, 0x00, 0x00),
        10 => (0x00, 0xff, 0x00),
        11 => (0xff, 0xff, 0x00),
        12 => (0x5c, 0x5c, 0xff),
        13 => (0xff, 0x00, 0xff),
        14 => (0x00, 0xff, 0xff),
        15 => (0xff, 0xff, 0xff),
        16..=231 => {
            let idx = index - 16;
            let r = idx / 36;
            let g = (idx % 36) / 6;
            let b = idx % 6;
            (cube_component(r), cube_component(g), cube_component(b))
        }
        232..=255 => {
            let gray = 8 + 10 * (index - 232);
            (gray, gray, gray)
        }
    }
}

fn cube_component(value: u8) -> u8 {
    match value {
        0 => 0,
        1 => 95,
        2 => 135,
        3 => 175,
        4 => 215,
        5 => 255,
        _ => 0,
    }
}
