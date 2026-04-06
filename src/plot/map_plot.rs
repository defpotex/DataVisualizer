use crate::data::schema::DataSchema;
use crate::data::source::DataSource;
use crate::plot::plot_config::{MapPlotConfig, PlotConfig, TileScheme};
use crate::plot::scatter_plot::PlotWindowEvent;
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

// ── Configure dialog state ────────────────────────────────────────────────────

struct MapConfigDialog {
    is_open: bool,
    draft_title: String,
    draft_lat_idx: usize,
    draft_lon_idx: usize,
    draft_tile_scheme: TileScheme,
}

impl Default for MapConfigDialog {
    fn default() -> Self {
        Self {
            is_open: false,
            draft_title: String::new(),
            draft_lat_idx: 0,
            draft_lon_idx: 0,
            draft_tile_scheme: TileScheme::CartoDark,
        }
    }
}

impl MapConfigDialog {
    fn open(&mut self, config: &MapPlotConfig, schema: &DataSchema) {
        self.is_open = true;
        self.draft_title = config.title.clone();
        self.draft_tile_scheme = config.tile_scheme.clone();

        // Find numeric-capable fields for lat/lon.
        let numeric_fields = numeric_field_names(schema);
        self.draft_lat_idx = numeric_fields.iter().position(|n| *n == config.lat_col).unwrap_or(0);
        self.draft_lon_idx = numeric_fields.iter().position(|n| *n == config.lon_col).unwrap_or(0);
    }

    /// Returns `Some(MapPlotConfig)` when user confirms.
    fn show(
        &mut self,
        ctx: &Context,
        config: &MapPlotConfig,
        schema: &DataSchema,
        theme: &AppTheme,
    ) -> Option<MapPlotConfig> {
        if !self.is_open { return None; }

        let c = &theme.colors;
        let s = &theme.spacing;
        let mut result = None;
        let mut close = false;

        let numeric = numeric_field_names(schema);
        if self.draft_lat_idx >= numeric.len() { self.draft_lat_idx = 0; }
        if self.draft_lon_idx >= numeric.len() { self.draft_lon_idx = 0; }

        egui::Window::new(
            RichText::new("Configure Map Plot")
                .color(c.text_primary)
                .size(s.font_body + 1.0)
                .strong(),
        )
        .id(egui::Id::new(("map_cfg_dlg", config.id)))
        .collapsible(false)
        .resizable(false)
        .min_width(340.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(egui::Frame {
            fill: c.bg_panel,
            stroke: egui::Stroke::new(1.0, c.border),
            corner_radius: egui::CornerRadius::from(6.0_f32),
            inner_margin: egui::Margin::from(16.0_f32),
            ..Default::default()
        })
        .show(ctx, |ui| {
            // ── Title ─────────────────────────────────────────────────────────
            ui.label(RichText::new("Title").color(c.text_secondary).size(s.font_small));
            ui.add_space(2.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.draft_title)
                    .desired_width(ui.available_width())
                    .text_color(c.text_primary)
                    .font(egui::FontSelection::FontId(egui::FontId::proportional(s.font_body))),
            );

            ui.add_space(12.0);

            // ── Lat/Lon ───────────────────────────────────────────────────────
            if numeric.is_empty() {
                ui.label(
                    RichText::new("No numeric columns available for lat/lon.")
                        .color(c.accent_warning)
                        .size(s.font_body),
                );
            } else {
                ui.label(RichText::new("Latitude Column").color(c.text_secondary).size(s.font_small));
                ui.add_space(2.0);
                name_combo(ui, "cfg_map_lat", &numeric, &mut self.draft_lat_idx, theme);

                ui.add_space(10.0);

                ui.label(RichText::new("Longitude Column").color(c.text_secondary).size(s.font_small));
                ui.add_space(2.0);
                name_combo(ui, "cfg_map_lon", &numeric, &mut self.draft_lon_idx, theme);
            }

            ui.add_space(10.0);

            // ── Tile scheme ───────────────────────────────────────────────────
            ui.label(RichText::new("Map Tiles").color(c.text_secondary).size(s.font_small));
            ui.add_space(2.0);
            egui::ComboBox::from_id_salt("cfg_map_tiles")
                .selected_text(
                    RichText::new(self.draft_tile_scheme.label())
                        .color(c.text_primary)
                        .size(s.font_body),
                )
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for scheme in TileScheme::all() {
                        ui.selectable_value(
                            &mut self.draft_tile_scheme,
                            scheme.clone(),
                            RichText::new(scheme.label()).color(c.text_primary).size(s.font_body),
                        );
                    }
                });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            let can_apply = !numeric.is_empty() && !self.draft_title.trim().is_empty();
            ui.horizontal(|ui| {
                let apply_btn = egui::Button::new(
                    RichText::new("Apply")
                        .color(if can_apply { c.bg_app } else { c.text_secondary })
                        .size(s.font_body)
                        .strong(),
                )
                .fill(if can_apply { c.accent_primary } else { c.widget_bg })
                .min_size(egui::vec2(90.0, 0.0));

                if ui.add_enabled(can_apply, apply_btn).clicked() {
                    let lat = numeric.get(self.draft_lat_idx).cloned().unwrap_or_default();
                    let lon = numeric.get(self.draft_lon_idx).cloned().unwrap_or_default();
                    result = Some(MapPlotConfig {
                        id: config.id,
                        title: self.draft_title.trim().to_string(),
                        source_id: config.source_id,
                        lat_col: lat,
                        lon_col: lon,
                        color_col: config.color_col.clone(),
                        tile_scheme: self.draft_tile_scheme.clone(),
                    });
                    close = true;
                }
                ui.add_space(8.0);
                if ui.button(RichText::new("Cancel").color(c.text_secondary).size(s.font_body)).clicked() {
                    close = true;
                }
            });
        });

        if close { self.is_open = false; }
        result
    }
}

// ── MapPlot ───────────────────────────────────────────────────────────────────

pub struct MapPlot {
    pub config: MapPlotConfig,
    is_open: bool,
    default_pos: Pos2,
    pending_snap: Option<Pos2>,

    tiles: Option<HttpTiles>,
    map_memory: MapMemory,

    /// Points stored behind Arc so per-frame clone is O(1).
    points: Arc<Vec<[f64; 2]>>,
    /// Schema cached at sync time so the configure dialog can list columns.
    cached_schema: Option<DataSchema>,

    configure_dialog: MapConfigDialog,
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
            cached_schema: None,
            configure_dialog: MapConfigDialog::default(),
        }
    }

    pub fn sync_data(&mut self, source: &DataSource) {
        self.cached_schema = Some(source.schema.clone());
        self.points = Arc::new(
            extract_lat_lon(&source.df, &self.config.lat_col, &self.config.lon_col)
        );
    }

    pub fn apply_config(&mut self, config: MapPlotConfig) {
        // If tile scheme changed, force tile re-init on next frame.
        if config.tile_scheme != self.config.tile_scheme {
            self.tiles = None;
        }
        self.config = config;
    }

    pub fn plot_id(&self) -> usize { self.config.id }

    pub fn window_id(&self) -> egui::Id {
        egui::Id::new(("map_plot_win", self.config.id))
    }

    pub fn set_pending_snap(&mut self, pos: Pos2) {
        self.pending_snap = Some(pos);
    }

    pub fn intended_rect(&self, ctx: &egui::Context) -> Option<egui::Rect> {
        let state = egui::AreaState::load(ctx, self.window_id())?;
        let size = state.size?;
        let pos = self.pending_snap.unwrap_or_else(|| state.left_top_pos());
        Some(egui::Rect::from_min_size(pos, size))
    }

    /// Render as a floating egui Window constrained to `central_rect`.
    /// Returns `PlotWindowEvent` indicating what happened this frame.
    pub fn show_as_window(
        &mut self,
        ctx: &Context,
        theme: &AppTheme,
        central_rect: Rect,
        grid_size: f32,
        max_draw_points: usize,
    ) -> PlotWindowEvent {
        if !self.is_open {
            return PlotWindowEvent::Closed;
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

        let snap = self.pending_snap.take();
        let mut is_open = self.is_open;
        let mut gear_clicked = false;

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
                gear_clicked = show_toolbar(ui, &self.config, self.points.len(), theme);
                ui.separator();
                show_map(
                    ui,
                    &mut self.tiles,
                    &mut self.map_memory,
                    &self.config,
                    Arc::clone(&self.points),
                    theme,
                    max_draw_points,
                );
            });
        });

        if gear_clicked {
            if let Some(schema) = &self.cached_schema {
                self.configure_dialog.open(&self.config, schema);
            }
        }

        let mut event = if is_open { PlotWindowEvent::Open } else { PlotWindowEvent::Closed };

        if let Some(schema) = &self.cached_schema.clone() {
            if let Some(new_config) = self.configure_dialog.show(ctx, &self.config, schema, theme) {
                event = PlotWindowEvent::ConfigChanged(PlotConfig::Map(new_config));
            }
        }

        self.is_open = is_open;

        // Snap to grid on pointer release.
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

        event
    }
}

// ── Toolbar ───────────────────────────────────────────────────────────────────

/// Returns `true` if the gear (⚙) button was clicked.
fn show_toolbar(ui: &mut Ui, config: &MapPlotConfig, point_count: usize, theme: &AppTheme) -> bool {
    let c = &theme.colors;
    let s = &theme.spacing;
    let mut gear_clicked = false;

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
                    let gear = egui::Button::new(
                        RichText::new("⚙").color(c.text_secondary).size(s.font_small),
                    ).frame(false);
                    if ui.add(gear).on_hover_text("Configure plot").clicked() {
                        gear_clicked = true;
                    }
                    ui.separator();
                    ui.label(
                        RichText::new(format!("lon: {}  lat: {}", config.lon_col, config.lat_col))
                            .color(c.text_secondary)
                            .size(s.font_small)
                            .monospace(),
                    );
                });
            });
        });

    gear_clicked
}

// ── Map widget ────────────────────────────────────────────────────────────────

fn show_map(
    ui: &mut Ui,
    tiles: &mut Option<HttpTiles>,
    map_memory: &mut MapMemory,
    config: &MapPlotConfig,
    points: Arc<Vec<[f64; 2]>>,
    theme: &AppTheme,
    max_draw_points: usize,
) {
    if tiles.is_none() {
        *tiles = Some(make_tiles(&config.tile_scheme, ui.ctx().clone()));
    }

    let center = compute_center(&points);
    let accent = theme.colors.accent_primary;

    let tile_ref: &mut dyn Tiles = tiles.as_mut().unwrap();

    ui.add(
        Map::new(Some(tile_ref), map_memory, center)
            .with_plugin(PointsPlugin { points, color: accent, max_draw_points }),
    );
}

// ── Plugin: draw data points ──────────────────────────────────────────────────

struct PointsPlugin {
    points: Arc<Vec<[f64; 2]>>,
    color: Color32,
    max_draw_points: usize,
}

const POINT_RADIUS: f32 = 3.0;

impl walkers::Plugin for PointsPlugin {
    fn run(
        self: Box<Self>,
        ui: &mut Ui,
        response: &egui::Response,
        projector: &Projector,
        _map_memory: &MapMemory,
    ) {
        let n = self.points.len();
        if n == 0 { return; }

        let step = (n / self.max_draw_points).max(1);
        let rect = response.rect;
        let expanded = rect.expand(POINT_RADIUS);
        let r = POINT_RADIUS;
        let color = self.color;

        let uv = egui::epaint::WHITE_UV;
        let mut mesh = egui::Mesh::default();

        let cap = (n / step).min(self.max_draw_points);
        mesh.vertices.reserve(cap * 4);
        mesh.indices.reserve(cap * 6);

        for [lat, lon] in self.points.iter().step_by(step) {
            let pos = walkers::lat_lon(*lat, *lon);
            let v = projector.project(pos);
            let center = Pos2::new(v.x, v.y);

            if !expanded.contains(center) { continue; }

            let base = mesh.vertices.len() as u32;
            mesh.vertices.extend_from_slice(&[
                egui::epaint::Vertex { pos: center + vec2(-r, -r), uv, color },
                egui::epaint::Vertex { pos: center + vec2( r, -r), uv, color },
                egui::epaint::Vertex { pos: center + vec2( r,  r), uv, color },
                egui::epaint::Vertex { pos: center + vec2(-r,  r), uv, color },
            ]);
            mesh.indices.extend_from_slice(&[
                base, base + 1, base + 2,
                base, base + 2, base + 3,
            ]);
        }

        if !mesh.is_empty() {
            let painter = ui.painter().with_clip_rect(rect);
            painter.add(egui::Shape::mesh(mesh));
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

// ── Dialog helpers ────────────────────────────────────────────────────────────

/// Combo box for picking a name from a list.
fn name_combo(ui: &mut Ui, id: &str, names: &[String], idx: &mut usize, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;
    let selected = names.get(*idx).map(|n| n.as_str()).unwrap_or("(none)");
    egui::ComboBox::from_id_salt(id)
        .selected_text(RichText::new(selected).color(c.text_data).size(s.font_body).monospace())
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (i, name) in names.iter().enumerate() {
                ui.selectable_value(
                    idx, i,
                    RichText::new(name).color(c.text_data).size(s.font_body).monospace(),
                );
            }
        });
}

/// Collect numeric-capable field names from a schema (usable as lat/lon).
fn numeric_field_names(schema: &DataSchema) -> Vec<String> {
    use crate::data::schema::FieldKind;
    schema.fields.iter()
        .filter(|f| matches!(
            f.kind,
            FieldKind::Latitude | FieldKind::Longitude | FieldKind::Altitude
            | FieldKind::Speed | FieldKind::Heading | FieldKind::Float | FieldKind::Integer
        ))
        .map(|f| f.name.clone())
        .collect()
}

// ── Data helpers ──────────────────────────────────────────────────────────────

fn extract_lat_lon(
    df: &polars::prelude::DataFrame,
    lat_col: &str,
    lon_col: &str,
) -> Vec<[f64; 2]> {
    let Some(lats) = get_f64_vec(df, lat_col) else { return vec![]; };
    let Some(lons) = get_f64_vec(df, lon_col) else { return vec![]; };
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
