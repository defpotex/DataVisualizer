use crate::data::source::DataSource;
use crate::plot::plot_config::{MapPlotConfig, TileScheme};
use crate::theme::AppTheme;
use egui::{Color32, Context, Pos2, Rect, RichText, Ui, vec2};
use polars::prelude::DataType;
use std::sync::Arc;
use walkers::sources::{Attribution, OpenStreetMap, TileSource};
use walkers::{HttpTiles, Map, MapMemory, Position, Projector, Tiles};

// ── Custom tile source: Carto Dark Matter ─────────────────────────────────────

struct CartoDark;

impl TileSource for CartoDark {
    fn tile_url(&self, tile_id: walkers::TileId) -> String {
        format!(
            "https://cartodb-basemaps-a.global.ssl.fastly.net/dark_all/{}/{}/{}.png",
            tile_id.zoom, tile_id.x, tile_id.y
        )
    }

    fn attribution(&self) -> Attribution {
        Attribution {
            text: "© OpenStreetMap contributors, © CARTO",
            url: "https://carto.com/attributions",
            logo_light: None,
            logo_dark: None,
        }
    }
}

// ── MapPlot ───────────────────────────────────────────────────────────────────

pub struct MapPlot {
    pub config: MapPlotConfig,
    is_open: bool,

    /// Starting grid-aligned position.  egui remembers user moves after first show.
    default_pos: Pos2,

    /// When Some, force the window to this position for one frame (post-drag snap).
    pending_snap: Option<Pos2>,

    tiles: Option<HttpTiles>,
    map_memory: MapMemory,

    /// Points stored behind Arc so per-frame clone is O(1).
    points: Arc<Vec<[f64; 2]>>,
}

impl MapPlot {
    pub fn new(config: MapPlotConfig, default_pos: Pos2) -> Self {
        Self {
            config,
            is_open: true,
            default_pos,
            pending_snap: None,
            tiles: None,
            map_memory: MapMemory::default(),
            points: Arc::new(Vec::new()),
        }
    }

    pub fn sync_data(&mut self, source: &DataSource) {
        self.points = Arc::new(
            extract_lat_lon(&source.df, &self.config.lat_col, &self.config.lon_col)
        );
    }

    pub fn plot_id(&self) -> usize {
        self.config.id
    }

    /// The egui Area ID this window uses — needed externally for collision detection.
    pub fn window_id(&self) -> egui::Id {
        egui::Id::new(("map_plot_win", self.config.id))
    }

    /// Override the pending snap position (e.g. after collision resolution).
    pub fn set_pending_snap(&mut self, pos: Pos2) {
        self.pending_snap = Some(pos);
    }

    /// Returns the rect this window intends to occupy next frame:
    /// pending_snap position (if set) or current AreaState position, plus current size.
    pub fn intended_rect(&self, ctx: &egui::Context) -> Option<egui::Rect> {
        let state = egui::AreaState::load(ctx, self.window_id())?;
        let size = state.size?;
        let pos = self.pending_snap.unwrap_or_else(|| state.left_top_pos());
        Some(egui::Rect::from_min_size(pos, size))
    }

    /// Render as a floating egui Window constrained to `central_rect`.
    /// Returns `false` if the user closed the window.
    pub fn show_as_window(
        &mut self,
        ctx: &Context,
        theme: &AppTheme,
        central_rect: Rect,
        grid_size: f32,
    ) -> bool {
        if !self.is_open {
            return false;
        }

        let c = &theme.colors;
        let s = &theme.spacing;
        let id = self.config.id;
        let win_id = egui::Id::new(("map_plot_win", id));

        let default_w = (central_rect.width() * 0.5 - 12.0).max(320.0);
        let default_h = (central_rect.height() * 0.5 - 12.0).max(200.0);

        let window_frame = egui::Frame {
            fill: c.bg_panel,
            stroke: egui::Stroke::new(1.0, c.accent_primary),
            corner_radius: egui::CornerRadius::from(s.rounding),
            inner_margin: egui::Margin::from(0.0_f32),
            ..Default::default()
        };

        // Apply a pending snap by using current_pos for this frame only.
        let snap = self.pending_snap.take();

        let mut is_open = self.is_open;

        let mut win = egui::Window::new(
            RichText::new(&self.config.title)
                .color(c.text_primary)
                .size(s.font_body)
                .strong(),
        )
        .id(win_id)
        .open(&mut is_open)
        .resizable(true)
        .collapsible(true)
        .default_pos(self.default_pos)
        .default_size([default_w, default_h])
        .min_size([320.0, 200.0])
        .constrain_to(central_rect)
        .frame(window_frame);

        if let Some(snapped_pos) = snap {
            win = win.current_pos(snapped_pos);
        }

        win.show(ctx, |ui| {
            ui.push_id(id, |ui| {
                show_toolbar(ui, &self.config, self.points.len(), theme);
                ui.separator();
                show_map(
                    ui,
                    &mut self.tiles,
                    &mut self.map_memory,
                    &self.config,
                    Arc::clone(&self.points),
                    theme,
                );
            });
        });

        self.is_open = is_open;

        // Snap to grid on pointer release (end of drag).
        let pointer_released = ctx.input(|i| i.pointer.any_released());
        if pointer_released {
            if let Some(state) = egui::AreaState::load(ctx, win_id) {
                let current = state.left_top_pos();
                let snapped = snap_to_grid(current, grid_size);
                if (snapped - current).length() > 0.5 {
                    self.pending_snap = Some(snapped);
                }
            }
        }

        self.is_open
    }
}

// ── Toolbar ───────────────────────────────────────────────────────────────────

fn show_toolbar(ui: &mut Ui, config: &MapPlotConfig, point_count: usize, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;

    egui::Frame::default()
        .fill(c.bg_app)
        .inner_margin(egui::Margin::from(vec2(8.0, 4.0)))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("◈").color(c.accent_primary).size(s.font_small));
                ui.label(RichText::new("Map").color(c.text_secondary).size(s.font_small));
                ui.separator();
                ui.label(
                    RichText::new(config.tile_scheme.label())
                        .color(c.text_secondary)
                        .size(s.font_small),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("{} pts", format_count(point_count)))
                        .color(c.accent_secondary)
                        .size(s.font_small),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "lon: {}  lat: {}",
                            config.lon_col, config.lat_col
                        ))
                        .color(c.text_secondary)
                        .size(s.font_small)
                        .monospace(),
                    );
                });
            });
        });
}

// ── Map widget ────────────────────────────────────────────────────────────────

fn show_map(
    ui: &mut Ui,
    tiles: &mut Option<HttpTiles>,
    map_memory: &mut MapMemory,
    config: &MapPlotConfig,
    points: Arc<Vec<[f64; 2]>>,
    theme: &AppTheme,
) {
    if tiles.is_none() {
        *tiles = Some(make_tiles(&config.tile_scheme, ui.ctx().clone()));
    }

    let center = compute_center(&points);
    let accent = theme.colors.accent_primary;
    let tile_ref: &mut dyn Tiles = tiles.as_mut().unwrap();

    ui.add(
        Map::new(Some(tile_ref), map_memory, center)
            .with_plugin(PointsPlugin { points, color: accent }),
    );
}

// ── Plugin: draw data points ──────────────────────────────────────────────────

struct PointsPlugin {
    /// Arc clone — O(1), no data copy.
    points: Arc<Vec<[f64; 2]>>,
    color: Color32,
}

/// Maximum points rendered per frame regardless of dataset size.
/// Above this threshold we subsample evenly so draw-call cost is bounded.
const MAX_DRAW_POINTS: usize = 8_000;

impl walkers::Plugin for PointsPlugin {
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        response: &egui::Response,
        projector: &Projector,
        _map_memory: &MapMemory,
    ) {
        let painter = ui.painter().with_clip_rect(response.rect);
        let n = self.points.len();
        if n == 0 { return; }

        // Adaptive stride: draw every Nth point so we never exceed MAX_DRAW_POINTS.
        let step = (n / MAX_DRAW_POINTS).max(1);

        let rect = response.rect;
        for [lat, lon] in self.points.iter().step_by(step) {
            let pos = walkers::lat_lon(*lat, *lon);
            let v = projector.project(pos);
            let screen = Pos2::new(v.x, v.y);
            // Skip points outside the visible rect (viewport culling).
            if !rect.contains(screen) { continue; }
            painter.circle_filled(screen, 3.0, self.color);
        }
    }
}

// ── Grid snap ─────────────────────────────────────────────────────────────────

pub fn snap_to_grid(pos: Pos2, grid: f32) -> Pos2 {
    Pos2::new(
        (pos.x / grid).round() * grid,
        (pos.y / grid).round() * grid,
    )
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_tiles(scheme: &TileScheme, ctx: egui::Context) -> HttpTiles {
    match scheme {
        TileScheme::OpenStreetMap => HttpTiles::new(OpenStreetMap, ctx),
        TileScheme::CartoDark => HttpTiles::new(CartoDark, ctx),
    }
}

fn compute_center(points: &[[f64; 2]]) -> Position {
    if points.is_empty() {
        return walkers::lat_lon(39.5, -98.35);
    }
    let lat = points.iter().map(|p| p[0]).sum::<f64>() / points.len() as f64;
    let lon = points.iter().map(|p| p[1]).sum::<f64>() / points.len() as f64;
    walkers::lat_lon(lat, lon)
}

fn extract_lat_lon(
    df: &polars::prelude::DataFrame,
    lat_col: &str,
    lon_col: &str,
) -> Vec<[f64; 2]> {
    let Some(lats) = get_f64_vec(df, lat_col) else {
        return vec![];
    };
    let Some(lons) = get_f64_vec(df, lon_col) else {
        return vec![];
    };
    lats.into_iter()
        .zip(lons)
        .filter_map(|(lat, lon)| Some([lat?, lon?]))
        .collect()
}

fn get_f64_vec(
    df: &polars::prelude::DataFrame,
    col_name: &str,
) -> Option<Vec<Option<f64>>> {
    let col = df.column(col_name).ok()?;
    let series = col.as_series()?;
    let cast = series.cast(&DataType::Float64).ok()?;
    let ca = cast.f64().ok()?;
    Some(ca.into_iter().collect())
}

fn format_count(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}
