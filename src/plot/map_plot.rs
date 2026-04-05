use crate::data::source::DataSource;
use crate::plot::plot_config::{MapPlotConfig, TileScheme};
use crate::theme::AppTheme;
use egui::{Color32, Pos2, Ui};
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
    tiles: Option<HttpTiles>,
    map_memory: MapMemory,
    points: Vec<[f64; 2]>,
}

impl MapPlot {
    pub fn new(config: MapPlotConfig) -> Self {
        Self {
            config,
            tiles: None,
            map_memory: MapMemory::default(),
            points: Vec::new(),
        }
    }

    /// Extract lat/lon from the source and cache them. Call once after load.
    pub fn sync_data(&mut self, source: &DataSource) {
        self.points = extract_lat_lon(&source.df, &self.config.lat_col, &self.config.lon_col);
    }

    pub fn show(&mut self, ui: &mut Ui, theme: &AppTheme) {
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

    pub fn plot_id(&self) -> usize {
        self.config.id
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
            // project() returns a Vec2 that is already in screen-space (includes clip_rect center)
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
