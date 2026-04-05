use crate::data::source::DataSource;
use crate::plot::plot_config::{MapPlotConfig, TileScheme};
use crate::theme::AppTheme;
use egui::{Color32, Context, Pos2, RichText, Ui};
use polars::prelude::DataType;
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

/// Runtime state for a single geographic map plot.
pub struct MapPlot {
    pub config: MapPlotConfig,
    /// Drives the egui::Window open state (false = user clicked X).
    is_open: bool,
    tiles: Option<HttpTiles>,
    map_memory: MapMemory,
    points: Vec<[f64; 2]>,
}

impl MapPlot {
    pub fn new(config: MapPlotConfig) -> Self {
        Self {
            config,
            is_open: true,
            tiles: None,
            map_memory: MapMemory::default(),
            points: Vec::new(),
        }
    }

    /// Extract lat/lon from the source and cache them. Call once after load.
    pub fn sync_data(&mut self, source: &DataSource) {
        self.points = extract_lat_lon(&source.df, &self.config.lat_col, &self.config.lon_col);
    }

    pub fn plot_id(&self) -> usize {
        self.config.id
    }

    /// Show this plot as a floating, resizable, draggable egui Window.
    /// Returns `false` if the user closed the window (caller should remove this plot).
    pub fn show_as_window(&mut self, ctx: &Context, theme: &AppTheme) -> bool {
        if !self.is_open {
            return false;
        }

        let c = &theme.colors;
        let s = &theme.spacing;
        let id = self.config.id;

        // Style the window frame to match our dark theme.
        let window_frame = egui::Frame {
            fill: c.bg_panel,
            stroke: egui::Stroke::new(1.0, c.accent_primary),
            corner_radius: egui::CornerRadius::from(s.rounding),
            inner_margin: egui::Margin::from(0.0_f32),
            ..Default::default()
        };

        // Extract `is_open` before the closure to avoid self-borrow conflict with `.open()`.
        let mut is_open = self.is_open;

        egui::Window::new(RichText::new(&self.config.title).color(c.text_primary).size(s.font_body).strong())
            .id(egui::Id::new(("map_plot", id)))
            .open(&mut is_open)
            .resizable(true)
            .default_size([700.0, 500.0])
            .min_size([320.0, 200.0])
            .frame(window_frame)
            .show(ctx, |ui| {
                self.show_toolbar(ui, theme);
                ui.separator();
                self.show_map(ui, theme);
            });

        self.is_open = is_open;
        self.is_open
    }

    /// Thin toolbar row: icon, tile scheme label, point count.
    fn show_toolbar(&mut self, ui: &mut Ui, theme: &AppTheme) {
        let c = &theme.colors;
        let s = &theme.spacing;

        egui::Frame::default()
            .fill(c.bg_app)
            .inner_margin(egui::Margin::from(egui::vec2(8.0, 4.0)))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new("◈").color(c.accent_primary).size(s.font_small));
                    ui.label(
                        RichText::new("Map")
                            .color(c.text_secondary)
                            .size(s.font_small),
                    );
                    ui.separator();
                    ui.label(
                        RichText::new(self.config.tile_scheme.label())
                            .color(c.text_secondary)
                            .size(s.font_small),
                    );
                    ui.separator();
                    ui.label(
                        RichText::new(format!("{} pts", format_count(self.points.len())))
                            .color(c.accent_secondary)
                            .size(s.font_small),
                    );

                    // Right-aligned: lat/lon column labels
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("lon: {}  lat: {}", self.config.lon_col, self.config.lat_col))
                                .color(c.text_secondary)
                                .size(s.font_small)
                                .monospace(),
                        );
                    });
                });
            });
    }

    /// The actual map widget — fills remaining space in the window.
    fn show_map(&mut self, ui: &mut Ui, theme: &AppTheme) {
        // Lazy-init tiles on first render.
        if self.tiles.is_none() {
            self.tiles = Some(make_tiles(&self.config.tile_scheme, ui.ctx().clone()));
        }

        let center = compute_center(&self.points);
        let points = self.points.clone();
        let accent = theme.colors.accent_primary;

        let tiles: &mut dyn Tiles = self.tiles.as_mut().unwrap();

        ui.add(
            Map::new(Some(tiles), &mut self.map_memory, center)
                .with_plugin(PointsPlugin { points, color: accent }),
        );
    }
}

// ── Plugin: draw data points ──────────────────────────────────────────────────

struct PointsPlugin {
    points: Vec<[f64; 2]>,
    color: Color32,
}

impl walkers::Plugin for PointsPlugin {
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        _response: &egui::Response,
        projector: &Projector,
        _map_memory: &MapMemory,
    ) {
        let painter = ui.painter();
        for [lat, lon] in &self.points {
            let pos = walkers::lat_lon(*lat, *lon);
            // project() returns absolute screen Vec2 (includes clip_rect.center offset).
            let v = projector.project(pos);
            painter.circle_filled(Pos2::new(v.x, v.y), 3.0, self.color);
        }
    }
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
        return walkers::lat_lon(39.5, -98.35); // Geographic center of USA
    }
    let lat = points.iter().map(|p| p[0]).sum::<f64>() / points.len() as f64;
    let lon = points.iter().map(|p| p[1]).sum::<f64>() / points.len() as f64;
    walkers::lat_lon(lat, lon)
}

fn extract_lat_lon(df: &polars::prelude::DataFrame, lat_col: &str, lon_col: &str) -> Vec<[f64; 2]> {
    let Some(lats) = get_f64_vec(df, lat_col) else { return vec![] };
    let Some(lons) = get_f64_vec(df, lon_col) else { return vec![] };
    lats.into_iter()
        .zip(lons)
        .filter_map(|(lat, lon)| Some([lat?, lon?]))
        .collect()
}

fn get_f64_vec(df: &polars::prelude::DataFrame, col_name: &str) -> Option<Vec<Option<f64>>> {
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
        if i > 0 && i % 3 == 0 { result.push(','); }
        result.push(ch);
    }
    result.chars().rev().collect()
}
