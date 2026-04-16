use crate::data::schema::DataSchema;
use crate::data::source::DataSource;
use crate::plot::plot_config::{
    AlphaConfig, ColorMode, Colormap, MapPlotConfig, PlotConfig, SizeConfig, TileScheme,
};
use crate::plot::scatter_plot::PlotWindowEvent;
use crate::plot::styling::{
    apply_alpha, compute_alphas, compute_colors, compute_radii, PlotLegendData,
};
use crate::plot::spatial_grid::SpatialGrid;
use crate::plot::sync::{CancelToken, MapSyncResult};
use crate::state::app_state::DataEvent;
use crate::theme::AppTheme;
use crossbeam_channel::Sender;
use egui::{Color32, Context, Pos2, Rect, RichText, Ui, vec2};
use polars::prelude::DataType;
use std::collections::HashSet;
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

// ── Configure dialog ──────────────────────────────────────────────────────────

struct MapConfigDialog {
    is_open: bool,
    draft_title: String,
    draft_lat_idx: usize,
    draft_lon_idx: usize,
    draft_tile_scheme: TileScheme,
    // Color mode (0=Solid, 1=Categorical, 2=Continuous)
    draft_color_variant: usize,
    draft_color_col_idx: usize,
    draft_colormap: Colormap,
    draft_color_min_enabled: bool,
    draft_color_min: f64,
    draft_color_max_enabled: bool,
    draft_color_max: f64,
    draft_color_reverse: bool,
    // Size
    draft_size_enabled: bool,
    draft_size_col_idx: usize,
    draft_size_min_px: f32,
    draft_size_max_px: f32,
    // Alpha
    draft_alpha_enabled: bool,
    draft_alpha_col_idx: usize,
    draft_alpha_min: f32,
    draft_alpha_max: f32,
    // Hover fields
    draft_hover_fields: HashSet<String>,
}

impl Default for MapConfigDialog {
    fn default() -> Self {
        Self {
            is_open: false,
            draft_title: String::new(),
            draft_lat_idx: 0,
            draft_lon_idx: 0,
            draft_tile_scheme: TileScheme::CartoDark,
            draft_color_variant: 0,
            draft_color_col_idx: 0,
            draft_colormap: Colormap::Viridis,
            draft_color_min_enabled: false,
            draft_color_min: 0.0,
            draft_color_max_enabled: false,
            draft_color_max: 1.0,
            draft_color_reverse: false,
            draft_size_enabled: false,
            draft_size_col_idx: 0,
            draft_size_min_px: 2.0,
            draft_size_max_px: 10.0,
            draft_alpha_enabled: false,
            draft_alpha_col_idx: 0,
            draft_alpha_min: 0.2,
            draft_alpha_max: 1.0,
            draft_hover_fields: HashSet::new(),
        }
    }
}

impl MapConfigDialog {
    fn open(&mut self, config: &MapPlotConfig, schema: &DataSchema) {
        self.is_open = true;
        self.draft_title = config.title.clone();
        self.draft_tile_scheme = config.tile_scheme.clone();

        let numeric = numeric_field_names(schema);
        self.draft_lat_idx = numeric.iter().position(|n| *n == config.lat_col).unwrap_or(0);
        self.draft_lon_idx = numeric.iter().position(|n| *n == config.lon_col).unwrap_or(0);

        self.draft_color_variant = config.color_mode.variant_idx();
        let color_col = match &config.color_mode {
            ColorMode::Categorical { col } | ColorMode::Continuous { col, .. } => col.as_str(),
            ColorMode::Solid => "",
        };
        let all_fields = &schema.fields;
        self.draft_color_col_idx = all_fields.iter().position(|f| f.name == color_col).unwrap_or(0);
        if let ColorMode::Continuous { colormap, color_min, color_max, reverse, .. } = &config.color_mode {
            self.draft_colormap = colormap.clone();
            self.draft_color_min_enabled = color_min.is_some();
            self.draft_color_min = color_min.unwrap_or(0.0);
            self.draft_color_max_enabled = color_max.is_some();
            self.draft_color_max = color_max.unwrap_or(1.0);
            self.draft_color_reverse = *reverse;
        } else {
            self.draft_color_min_enabled = false;
            self.draft_color_min = 0.0;
            self.draft_color_max_enabled = false;
            self.draft_color_max = 1.0;
            self.draft_color_reverse = false;
        }

        if let Some(sz) = &config.size_config {
            self.draft_size_enabled = true;
            self.draft_size_col_idx = all_fields.iter().position(|f| f.name == sz.col).unwrap_or(0);
            self.draft_size_min_px = sz.min_px;
            self.draft_size_max_px = sz.max_px;
        } else {
            self.draft_size_enabled = false;
            self.draft_size_col_idx = 0;
            self.draft_size_min_px = 2.0;
            self.draft_size_max_px = 10.0;
        }

        if let Some(al) = &config.alpha_config {
            self.draft_alpha_enabled = true;
            self.draft_alpha_col_idx = all_fields.iter().position(|f| f.name == al.col).unwrap_or(0);
            self.draft_alpha_min = al.min_alpha;
            self.draft_alpha_max = al.max_alpha;
        } else {
            self.draft_alpha_enabled = false;
            self.draft_alpha_col_idx = 0;
            self.draft_alpha_min = 0.2;
            self.draft_alpha_max = 1.0;
        }

        self.draft_hover_fields = config.hover_fields.iter().cloned().collect();
    }

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
        let all_fields = &schema.fields;
        if self.draft_lat_idx >= numeric.len() { self.draft_lat_idx = 0; }
        if self.draft_lon_idx >= numeric.len() { self.draft_lon_idx = 0; }
        if self.draft_color_col_idx >= all_fields.len() { self.draft_color_col_idx = 0; }
        if self.draft_size_col_idx >= all_fields.len() { self.draft_size_col_idx = 0; }
        if self.draft_alpha_col_idx >= all_fields.len() { self.draft_alpha_col_idx = 0; }

        egui::Window::new(
            RichText::new("Configure Map Plot")
                .color(c.text_primary)
                .size(s.font_body + 1.0)
                .strong(),
        )
        .id(egui::Id::new(("map_cfg_dlg", config.id)))
        .collapsible(false)
        .resizable(true)
        .min_width(360.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(egui::Frame {
            fill: c.bg_panel,
            stroke: egui::Stroke::new(1.0, c.border),
            corner_radius: egui::CornerRadius::from(6.0_f32),
            inner_margin: egui::Margin::from(16.0_f32),
            ..Default::default()
        })
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                // ── Title ─────────────────────────────────────────────────────
                ui.label(RichText::new("Title").color(c.text_secondary).size(s.font_small));
                ui.add_space(2.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.draft_title)
                        .desired_width(ui.available_width())
                        .text_color(c.text_primary)
                        .font(egui::FontSelection::FontId(egui::FontId::proportional(s.font_body))),
                );

                ui.add_space(12.0);

                // ── Lat/Lon ───────────────────────────────────────────────────
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

                // ── Tile scheme ───────────────────────────────────────────────
                ui.label(RichText::new("Map Tiles").color(c.text_secondary).size(s.font_small));
                ui.add_space(2.0);
                egui::ComboBox::from_id_salt("cfg_map_tiles")
                    .selected_text(RichText::new(self.draft_tile_scheme.label()).color(c.text_primary).size(s.font_body))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for scheme in TileScheme::all() {
                            ui.selectable_value(
                                &mut self.draft_tile_scheme, scheme.clone(),
                                RichText::new(scheme.label()).color(c.text_primary).size(s.font_body),
                            );
                        }
                    });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // ── Color mode ────────────────────────────────────────────────
                ui.label(RichText::new("COLOR").color(c.text_secondary).size(s.font_small));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for (idx, label) in ["Solid", "By Category", "By Value"].iter().enumerate() {
                        let selected = self.draft_color_variant == idx;
                        let btn = egui::Button::new(
                            RichText::new(*label)
                                .color(if selected { c.bg_app } else { c.text_secondary })
                                .size(s.font_small),
                        )
                        .fill(if selected { c.accent_secondary } else { c.widget_bg })
                        .stroke(egui::Stroke::new(1.0, if selected { c.accent_secondary } else { c.border }));
                        if ui.add(btn).clicked() { self.draft_color_variant = idx; }
                    }
                });

                if self.draft_color_variant == 1 || self.draft_color_variant == 2 {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Color Column").color(c.text_secondary).size(s.font_small));
                    ui.add_space(2.0);
                    field_combo(ui, "cfg_map_color_col", all_fields, &mut self.draft_color_col_idx, theme);
                }
                if self.draft_color_variant == 2 {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Colormap").color(c.text_secondary).size(s.font_small));
                    ui.add_space(2.0);
                    colormap_combo(ui, "cfg_map_colormap", &mut self.draft_colormap, self.draft_color_reverse, theme);

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.draft_color_min_enabled,
                            RichText::new("Min").color(c.text_secondary).size(s.font_small));
                        if self.draft_color_min_enabled {
                            ui.add(egui::DragValue::new(&mut self.draft_color_min).speed(0.1));
                        }
                        ui.add_space(12.0);
                        ui.checkbox(&mut self.draft_color_max_enabled,
                            RichText::new("Max").color(c.text_secondary).size(s.font_small));
                        if self.draft_color_max_enabled {
                            ui.add(egui::DragValue::new(&mut self.draft_color_max).speed(0.1));
                        }
                    });
                    ui.add_space(4.0);
                    ui.checkbox(&mut self.draft_color_reverse,
                        RichText::new("Reverse colormap").color(c.text_secondary).size(s.font_small));
                }

                ui.add_space(10.0);

                // ── Size ──────────────────────────────────────────────────────
                ui.checkbox(&mut self.draft_size_enabled,
                    RichText::new("SIZE  by column").color(c.text_secondary).size(s.font_small));
                if self.draft_size_enabled {
                    ui.add_space(4.0);
                    field_combo(ui, "cfg_map_size_col", all_fields, &mut self.draft_size_col_idx, theme);
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Min px").color(c.text_secondary).size(s.font_small));
                        ui.add(egui::DragValue::new(&mut self.draft_size_min_px).range(1.0..=20.0).speed(0.5));
                        ui.add_space(8.0);
                        ui.label(RichText::new("Max px").color(c.text_secondary).size(s.font_small));
                        ui.add(egui::DragValue::new(&mut self.draft_size_max_px).range(1.0..=40.0).speed(0.5));
                    });
                    if self.draft_size_min_px > self.draft_size_max_px { self.draft_size_max_px = self.draft_size_min_px; }
                }

                ui.add_space(8.0);

                // ── Alpha ─────────────────────────────────────────────────────
                ui.checkbox(&mut self.draft_alpha_enabled,
                    RichText::new("OPACITY  by column").color(c.text_secondary).size(s.font_small));
                if self.draft_alpha_enabled {
                    ui.add_space(4.0);
                    field_combo(ui, "cfg_map_alpha_col", all_fields, &mut self.draft_alpha_col_idx, theme);
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Min α").color(c.text_secondary).size(s.font_small));
                        ui.add(egui::Slider::new(&mut self.draft_alpha_min, 0.0..=1.0).fixed_decimals(2));
                        ui.add_space(8.0);
                        ui.label(RichText::new("Max α").color(c.text_secondary).size(s.font_small));
                        ui.add(egui::Slider::new(&mut self.draft_alpha_max, 0.0..=1.0).fixed_decimals(2));
                    });
                    if self.draft_alpha_min > self.draft_alpha_max { self.draft_alpha_max = self.draft_alpha_min; }
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // ── Hover fields ──────────────────────────────────────────────
                ui.label(RichText::new("HOVER TOOLTIP").color(c.text_secondary).size(s.font_small));
                ui.add_space(2.0);
                ui.label(
                    RichText::new("Lat and Lon are always shown. Add extra columns:")
                        .color(c.text_secondary)
                        .size(s.font_small)
                        .italics(),
                );
                ui.add_space(4.0);

                let lat_name = numeric.get(self.draft_lat_idx).map(|n| n.as_str()).unwrap_or("");
                let lon_name = numeric.get(self.draft_lon_idx).map(|n| n.as_str()).unwrap_or("");

                let row_h = s.font_body + 8.0;
                let list_h = (row_h * all_fields.len().min(6) as f32).max(60.0);
                egui::Frame::default()
                    .fill(c.bg_app)
                    .stroke(egui::Stroke::new(1.0, c.border))
                    .corner_radius(egui::CornerRadius::from(4.0_f32))
                    .inner_margin(egui::Margin::from(egui::vec2(6.0, 4.0)))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(list_h)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                for field in all_fields {
                                    if field.name == lat_name || field.name == lon_name { continue; }
                                    let mut checked = self.draft_hover_fields.contains(&field.name);
                                    let label = format!("{} {}", field.kind.icon(), field.name);
                                    if ui.checkbox(&mut checked,
                                        RichText::new(&label).color(c.text_data).size(s.font_body).monospace(),
                                    ).changed() {
                                        if checked {
                                            self.draft_hover_fields.insert(field.name.clone());
                                        } else {
                                            self.draft_hover_fields.remove(&field.name);
                                        }
                                    }
                                }
                            });
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

                        let color_mode = match self.draft_color_variant {
                            1 => ColorMode::Categorical {
                                col: all_fields.get(self.draft_color_col_idx).map(|f| f.name.clone()).unwrap_or_default(),
                            },
                            2 => ColorMode::Continuous {
                                col: all_fields.get(self.draft_color_col_idx).map(|f| f.name.clone()).unwrap_or_default(),
                                colormap: self.draft_colormap.clone(),
                                color_min: if self.draft_color_min_enabled { Some(self.draft_color_min) } else { None },
                                color_max: if self.draft_color_max_enabled { Some(self.draft_color_max) } else { None },
                                reverse: self.draft_color_reverse,
                            },
                            _ => ColorMode::Solid,
                        };

                        let size_config = if self.draft_size_enabled {
                            Some(SizeConfig {
                                col: all_fields.get(self.draft_size_col_idx).map(|f| f.name.clone()).unwrap_or_default(),
                                min_px: self.draft_size_min_px,
                                max_px: self.draft_size_max_px,
                            })
                        } else { None };

                        let alpha_config = if self.draft_alpha_enabled {
                            Some(AlphaConfig {
                                col: all_fields.get(self.draft_alpha_col_idx).map(|f| f.name.clone()).unwrap_or_default(),
                                min_alpha: self.draft_alpha_min,
                                max_alpha: self.draft_alpha_max,
                            })
                        } else { None };

                        let mut hover_fields: Vec<String> = self.draft_hover_fields.iter().cloned().collect();
                        hover_fields.sort_unstable();

                        result = Some(MapPlotConfig {
                            id: config.id,
                            title: self.draft_title.trim().to_string(),
                            source_id: config.source_id,
                            lat_col: lat,
                            lon_col: lon,
                            tile_scheme: self.draft_tile_scheme.clone(),
                            color_mode,
                            size_config,
                            alpha_config,
                            hover_fields,
                        });
                        close = true;
                    }
                    ui.add_space(8.0);
                    if ui.button(RichText::new("Cancel").color(c.text_secondary).size(s.font_body)).clicked() {
                        close = true;
                    }
                });
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

    /// Filtered lat/lon pairs.
    points: Arc<Vec<[f64; 2]>>,
    /// Per-point colors aligned with `points`.
    colors: Arc<Vec<Color32>>,
    /// Per-point radii aligned with `points`.
    radii: Arc<Vec<f32>>,
    /// Per-point hover labels aligned with `points`.
    hover_labels: Arc<Vec<String>>,
    /// Maps filtered point index → original DataFrame row index.
    row_indices: Arc<Vec<usize>>,
    /// Cached schema for the configure dialog.
    cached_schema: Option<DataSchema>,
    /// Legend metadata.
    legend: Option<PlotLegendData>,

    configure_dialog: MapConfigDialog,

    /// Context menu state (right-click).
    context_menu_row: Option<usize>,
    context_menu_pos: Option<Pos2>,

    /// True while a background thread is computing plot data.
    computing: bool,
    /// True once the first sync result has arrived (suppresses overlay during playback).
    has_loaded: bool,
    /// Token to cancel a running background sync.
    cancel_token: CancelToken,
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
            colors: Arc::new(Vec::new()),
            radii: Arc::new(Vec::new()),
            hover_labels: Arc::new(Vec::new()),
            row_indices: Arc::new(Vec::new()),
            cached_schema: None,
            legend: None,
            configure_dialog: MapConfigDialog::default(),
            context_menu_row: None,
            context_menu_pos: None,
            computing: false,
            has_loaded: false,
            cancel_token: CancelToken::new(),
        }
    }

    pub fn is_computing(&self) -> bool { self.computing }

    /// Kick off data computation on a background thread.
    pub fn sync_data_async(&mut self, source: &DataSource, tx: &Sender<DataEvent>) {
        self.cached_schema = Some(source.schema.clone());

        // Cancel any previous in-flight computation.
        self.cancel_token.cancel();
        let token = CancelToken::new();
        self.cancel_token = token.clone();
        self.computing = true;

        let plot_id = self.config.id;
        let config = self.config.clone();
        let schema = source.schema.clone();
        let df = source.df.clone();
        let tx = tx.clone();

        rayon::spawn(move || {
            let result = compute_map_data(plot_id, &config, &schema, &df, &token);
            match result {
                Some(r) => {
                    let _ = tx.send(DataEvent::PlotSyncReady(
                        crate::plot::sync::PlotSyncEvent::MapReady(r),
                    ));
                }
                None => {
                    let _ = tx.send(DataEvent::PlotSyncReady(
                        crate::plot::sync::PlotSyncEvent::Cancelled { plot_id },
                    ));
                }
            }
        });
    }

    /// Apply a completed sync result from the background thread.
    pub fn apply_sync_result(&mut self, result: MapSyncResult) {
        self.cached_schema = Some(result.schema);
        self.points = Arc::new(result.points);
        self.colors = Arc::new(result.colors);
        self.radii = Arc::new(result.radii);
        self.hover_labels = Arc::new(result.hover_labels);
        self.row_indices = Arc::new(result.row_indices);
        self.legend = Some(result.legend);
        self.computing = false;
        self.has_loaded = true;
    }

    /// Cancel any in-flight background sync.
    pub fn cancel_sync(&mut self) {
        self.cancel_token.cancel();
        self.computing = false;
    }

    pub fn apply_config(&mut self, config: MapPlotConfig) {
        if config.tile_scheme != self.config.tile_scheme {
            self.tiles = None;
        }
        self.config = config;
    }

    pub fn plot_id(&self) -> usize { self.config.id }
    pub fn legend_data(&self) -> Option<&PlotLegendData> { self.legend.as_ref() }

    pub fn window_id(&self) -> egui::Id {
        egui::Id::new(("map_plot_win", self.config.id))
    }

    pub fn set_pending_snap(&mut self, pos: Pos2) { self.pending_snap = Some(pos); }

    pub fn intended_rect(&self, ctx: &egui::Context) -> Option<egui::Rect> {
        let state = egui::AreaState::load(ctx, self.window_id())?;
        let size = state.size?;
        let pos = self.pending_snap.unwrap_or_else(|| state.left_top_pos());
        Some(egui::Rect::from_min_size(pos, size))
    }

    pub fn show_as_window(
        &mut self,
        ctx: &Context,
        theme: &AppTheme,
        central_rect: Rect,
        grid_size: f32,
        perf: &crate::state::perf_settings::PerformanceSettings,
        _selection: Option<&crate::state::selection::SelectionSet>,
    ) -> PlotWindowEvent {
        if !self.is_open { return PlotWindowEvent::Closed; }
        puffin::profile_function!();

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

        // For Solid mode, build a color vec using the theme accent.
        let solid_theme_color = c.accent_primary;
        let mut display_colors = if matches!(self.config.color_mode, ColorMode::Solid) {
            Arc::new(vec![solid_theme_color; self.points.len()])
        } else {
            Arc::clone(&self.colors)
        };

        // Apply selection dimming: unselected points get 30% alpha.
        // Cached by (plot_id, selection_version) to avoid re-allocating every frame.
        let has_selection = _selection.map_or(false, |s| !s.is_empty());
        if has_selection {
            let sel = _selection.unwrap();
            let sel_ver = sel.version();
            let cache_key = egui::Id::new(("map_dim_cache", self.plot_id()));
            let cached: Option<(u64, Arc<Vec<Color32>>)> = ctx.memory(|mem| mem.data.get_temp(cache_key));
            let reuse = cached.as_ref().map_or(false, |(ver, d)| *ver == sel_ver && d.len() == display_colors.len());
            if reuse {
                display_colors = cached.unwrap().1;
            } else {
                let row_indices = &self.row_indices;
                let dimmed: Vec<Color32> = display_colors.iter().enumerate().map(|(i, &c)| {
                    let row = row_indices.get(i).copied().unwrap_or(i);
                    if sel.contains(row) {
                        c
                    } else {
                        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * 0.3) as u8)
                    }
                }).collect();
                display_colors = Arc::new(dimmed);
                ctx.memory_mut(|mem| mem.data.insert_temp(cache_key, (sel_ver, Arc::clone(&display_colors))));
            }
        }

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

        if let Some(snapped_pos) = snap { win = win.current_pos(snapped_pos); }

        let show_overlay = self.computing && !self.has_loaded;
        let mut cancel_clicked = false;
        win.show(ctx, |ui| {
            ui.push_id(id, |ui| {
                gear_clicked = show_toolbar(ui, &self.config, self.points.len(), theme);
                ui.separator();
                if show_overlay {
                    show_computing_overlay(ui, theme, &mut cancel_clicked);
                } else {
                    show_map(
                        ui,
                        &mut self.tiles,
                        &mut self.map_memory,
                        &self.config,
                        Arc::clone(&self.points),
                        display_colors,
                        Arc::clone(&self.radii),
                        Arc::clone(&self.hover_labels),
                        Arc::clone(&self.row_indices),
                        theme,
                        perf,
                        id,
                        _selection,
                        self.context_menu_row.is_some(),
                    );
                }
            });
        });
        if cancel_clicked {
            self.cancel_sync();
        }
        if self.computing {
            ctx.request_repaint();
        }

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

        // ── Handle selection interactions from show_map ──────────────────────
        let interaction: Option<MapInteraction> = ctx.memory(|mem| {
            mem.data.get_temp(egui::Id::new(("map_interaction", id)))
        });
        if let Some(inter) = interaction {
            ctx.memory_mut(|mem| {
                mem.data.remove::<MapInteraction>(egui::Id::new(("map_interaction", id)));
            });

            use crate::state::selection::SelectionSet;
            let source_id = self.config.source_id;

            match inter {
                MapInteraction::Click { row, ctrl } => {
                    if ctrl {
                        let mut sel = _selection.cloned()
                            .unwrap_or_else(|| SelectionSet::new(id, source_id));
                        sel.plot_id = id;
                        sel.source_id = source_id;
                        sel.toggle(row);
                        if sel.is_empty() {
                            event = PlotWindowEvent::SelectionChanged(None);
                        } else {
                            event = PlotWindowEvent::SelectionChanged(Some(sel));
                        }
                    } else {
                        event = PlotWindowEvent::SelectionChanged(
                            Some(SelectionSet::single(id, source_id, row))
                        );
                    }
                }
                MapInteraction::ClearSelection => {
                    if _selection.is_some() {
                        event = PlotWindowEvent::SelectionChanged(None);
                    }
                }
                MapInteraction::RightClick { row, screen_pos } => {
                    self.context_menu_row = Some(row);
                    self.context_menu_pos = Some(screen_pos);
                }
                MapInteraction::AreaSelect { rows, ctrl } => {
                    if rows.is_empty() && !ctrl {
                        event = PlotWindowEvent::SelectionChanged(None);
                    } else if ctrl {
                        // Ctrl+drag: add to existing selection.
                        let mut sel = _selection.cloned()
                            .unwrap_or_else(|| SelectionSet::new(id, source_id));
                        sel.plot_id = id;
                        sel.source_id = source_id;
                        for row in rows { sel.indices.insert(row); }
                        if sel.is_empty() {
                            event = PlotWindowEvent::SelectionChanged(None);
                        } else {
                            event = PlotWindowEvent::SelectionChanged(Some(sel));
                        }
                    } else {
                        event = PlotWindowEvent::SelectionChanged(
                            Some(SelectionSet::from_indices(id, source_id, rows))
                        );
                    }
                }
            }
        }

        // ── Context menu ─────────────────────────────────────────────────────
        if let Some(row) = self.context_menu_row {
            let mut close_menu = false;
            let menu_pos = self.context_menu_pos.unwrap_or(egui::pos2(100.0, 100.0));

            let area_resp = egui::Area::new(egui::Id::new(("map_ctx_menu", id)))
                .fixed_pos(menu_pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::default()
                        .fill(c.bg_panel)
                        .stroke(egui::Stroke::new(1.0, c.border))
                        .corner_radius(egui::CornerRadius::from(4.0_f32))
                        .inner_margin(egui::Margin::from(6.0_f32))
                        .show(ui, |ui| {
                            ui.set_min_width(160.0);
                            ui.label(
                                RichText::new(format!("Row {}", row))
                                    .color(c.text_secondary)
                                    .size(s.font_small),
                            );
                            ui.separator();

                            if ui.button(RichText::new("Select Point").color(c.text_primary).size(s.font_body)).clicked() {
                                use crate::state::selection::SelectionSet;
                                event = PlotWindowEvent::SelectionChanged(
                                    Some(SelectionSet::single(id, self.config.source_id, row))
                                );
                                close_menu = true;
                            }

                            if let Some(sel) = _selection {
                                if !sel.is_empty() {
                                    if ui.button(RichText::new(format!("Filter to Selection ({} pts)", sel.len())).color(c.text_primary).size(s.font_body)).clicked() {
                                        event = PlotWindowEvent::FilterToSelection(sel.clone());
                                        close_menu = true;
                                    }
                                }
                            }

                            if ui.button(RichText::new("Clear Selection").color(c.text_primary).size(s.font_body)).clicked() {
                                event = PlotWindowEvent::SelectionChanged(None);
                                close_menu = true;
                            }

                            if ui.button(RichText::new("Cancel").color(c.text_secondary).size(s.font_body)).clicked() {
                                close_menu = true;
                            }
                        });
                });

            // Close on button action, or on click outside the menu area.
            let clicked_outside = ctx.input(|i| i.pointer.any_pressed())
                && !area_resp.response.rect.contains(ctx.input(|i| i.pointer.hover_pos().unwrap_or(egui::pos2(-1.0, -1.0))));
            if close_menu || clicked_outside {
                self.context_menu_row = None;
                self.context_menu_pos = None;
            }
        }

        self.is_open = is_open;

        if ctx.input(|i| i.pointer.any_released()) {
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
                ui.label(RichText::new(config.tile_scheme.label()).color(c.text_secondary).size(s.font_small));
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

// ── Computing overlay ────────────────────────────────────────────────────────

fn show_computing_overlay(ui: &mut Ui, theme: &AppTheme, cancel_clicked: &mut bool) {
    let c = &theme.colors;
    let s = &theme.spacing;
    let available = ui.available_size();
    // Claim the full available space so the window doesn't shrink.
    let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
    let center = rect.center();
    ui.allocate_ui_at_rect(
        egui::Rect::from_center_size(center, egui::vec2(200.0, 100.0)),
        |ui| {
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Computing plot data...")
                        .color(c.text_secondary)
                        .size(s.font_body),
                );
                ui.add_space(12.0);
                let btn = egui::Button::new(
                    RichText::new("Cancel").color(c.text_primary).size(s.font_small),
                )
                .fill(c.widget_bg)
                .stroke(egui::Stroke::new(1.0, c.border));
                if ui.add(btn).clicked() {
                    *cancel_clicked = true;
                }
            });
        },
    );
}

// ── Background computation ───────────────────────────────────────────────────

/// Pure function that does all the expensive data extraction + styling work
/// for a map plot.  Returns `None` if cancelled via `token`.
fn compute_map_data(
    plot_id: usize,
    config: &MapPlotConfig,
    schema: &DataSchema,
    df: &polars::prelude::DataFrame,
    token: &CancelToken,
) -> Option<MapSyncResult> {
    puffin::profile_function!();
    let n = df.height();

    let solid_color = Color32::from_rgb(100, 180, 255);
    let (mut all_colors, color_legend, _cat_indices) = compute_colors(df, &config.color_mode, solid_color, n);
    if token.is_cancelled() { return None; }

    let base_radius = 3.0_f32;
    let (all_radii, size_legend) = compute_radii(df, config.size_config.as_ref(), base_radius, n);
    let base_alpha = 200.0 / 255.0;
    let (all_alphas, alpha_legend) = compute_alphas(df, config.alpha_config.as_ref(), base_alpha, n);
    apply_alpha(&mut all_colors, &all_alphas);
    if token.is_cancelled() { return None; }

    let hover_cols: Vec<(String, Vec<Option<String>>)> = config.hover_fields.iter()
        .filter_map(|field| {
            let series = df.column(field).ok()?.as_series()?.clone();
            let cast = series.cast(&DataType::String).ok()?;
            let ca = cast.str().ok()?.clone();
            let vals: Vec<Option<String>> = ca.into_iter().map(|v| v.map(|s| s.to_string())).collect();
            Some((field.clone(), vals))
        })
        .collect();

    let orig_row_indices: Vec<usize> = df.column(crate::data::filter::ORIG_ROW_COL)
        .ok()
        .and_then(|c| c.as_series().map(|s| s.clone()))
        .and_then(|s| s.u64().ok().map(|ca| {
            ca.into_iter().map(|v| v.unwrap_or(0) as usize).collect()
        }))
        .unwrap_or_else(|| (0..n).collect());

    if token.is_cancelled() { return None; }

    let all_lats = get_f64_vec(df, &config.lat_col).unwrap_or_else(|| vec![None; n]);
    let all_lons = get_f64_vec(df, &config.lon_col).unwrap_or_else(|| vec![None; n]);

    let mut pts: Vec<[f64; 2]> = Vec::new();
    let mut colors: Vec<Color32> = Vec::new();
    let mut radii: Vec<f32> = Vec::new();
    let mut hover_labels_vec: Vec<String> = Vec::new();
    let mut row_idx_vec: Vec<usize> = Vec::new();

    for i in 0..n {
        if i % 50_000 == 0 && token.is_cancelled() { return None; }
        if let (Some(lat), Some(lon)) = (all_lats.get(i).copied().flatten(), all_lons.get(i).copied().flatten()) {
            pts.push([lat, lon]);
            colors.push(all_colors.get(i).copied().unwrap_or(solid_color));
            radii.push(all_radii.get(i).copied().unwrap_or(base_radius));
            hover_labels_vec.push(build_hover_label(
                i, lat, lon,
                config,
                &hover_cols,
            ));
            row_idx_vec.push(orig_row_indices.get(i).copied().unwrap_or(i));
        }
    }

    Some(MapSyncResult {
        plot_id,
        schema: schema.clone(),
        points: pts,
        colors,
        radii,
        hover_labels: hover_labels_vec,
        row_indices: row_idx_vec,
        legend: PlotLegendData {
            plot_id,
            plot_title: config.title.clone(),
            color: color_legend,
            size: size_legend,
            alpha: alpha_legend,
        },
    })
}

// ── Map widget ────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn show_map(
    ui: &mut Ui,
    tiles: &mut Option<HttpTiles>,
    map_memory: &mut MapMemory,
    config: &MapPlotConfig,
    points: Arc<Vec<[f64; 2]>>,
    colors: Arc<Vec<Color32>>,
    radii: Arc<Vec<f32>>,
    hover_labels: Arc<Vec<String>>,
    row_indices: Arc<Vec<usize>>,
    theme: &AppTheme,
    perf: &crate::state::perf_settings::PerformanceSettings,
    plot_id: usize,
    selection: Option<&crate::state::selection::SelectionSet>,
    context_menu_open: bool,
) {
    if tiles.is_none() {
        *tiles = Some(make_tiles(&config.tile_scheme, ui.ctx().clone()));
    }

    let center = compute_center(&points);
    let tile_ref: &mut dyn Tiles = tiles.as_mut().unwrap();

    // Disable default left-drag panning so we can use plain drag for area selection.
    // Shift+drag panning is handled manually after the map renders.
    let map_result = Map::new(Some(tile_ref), map_memory, center)
        .drag_pan_buttons(egui::DragPanButtons::EXTRA_2)
        .with_plugin(PointsPlugin {
            points: Arc::clone(&points),
            colors,
            radii: Arc::clone(&radii),
            max_draw_points: perf.max_draw_points,
            plot_id,
            gpu_points_mode: perf.gpu_points_mode,
            gpu_points_threshold: perf.gpu_points_threshold,
        })
        .show(ui, |_ui, _response, _projector, _mem| {});

    let map_response = &map_result.response;
    let map_rect = map_response.rect;

    // Retrieve screen positions stored by the plugin.
    let screen_pts: Vec<(Pos2, f32, usize)> = ui.ctx().memory(|mem| {
        mem.data.get_temp(egui::Id::new(("map_screen_pts", plot_id)))
            .unwrap_or_default()
    });

    // ── Selection highlight: draw white rings around selected points ─────
    let has_selection = selection.map_or(false, |s| !s.is_empty());
    if has_selection && !screen_pts.is_empty() {
        let sel = selection.unwrap();
        let painter = ui.painter().with_clip_rect(map_response.rect);
        for &(pos, radius, idx) in &screen_pts {
            let row = row_indices.get(idx).copied().unwrap_or(idx);
            if sel.contains(row) {
                painter.circle_stroke(pos, radius + 2.0, egui::Stroke::new(1.5, Color32::WHITE));
            }
        }
    }

    // ── Spatial grid for O(1) nearest-point lookups ────────────────────
    let grid = if !screen_pts.is_empty() {
        Some(SpatialGrid::build(&screen_pts, map_response.rect, |p| p.0))
    } else {
        None
    };
    let find_nearest = |pos: Pos2| -> Option<usize> {
        grid.as_ref()?.find_nearest(pos, &screen_pts, |p| p.0)
            .and_then(|(_, &(pt_pos, radius, idx))| {
                if pt_pos.distance(pos) <= radius + 10.0 { Some(idx) } else { None }
            })
    };

    let mut interaction: Option<MapInteraction> = None;

    // Hover tooltip (suppressed when context menu is open).
    if !context_menu_open && !ui.input(|i| i.pointer.secondary_down()) {
    if let Some(hover_pos) = map_response.hover_pos() {
        if let Some(ref g) = grid {
            if let Some((_, &(pos, radius, idx))) = g.find_nearest(hover_pos, &screen_pts, |p| p.0) {
                if pos.distance(hover_pos) <= radius + 10.0 {
                    if let Some(label) = hover_labels.get(idx) {
                        if !label.is_empty() {
                            let painter = ui.painter().with_clip_rect(map_response.rect);
                            painter.circle_stroke(pos, radius + 2.0, egui::Stroke::new(1.5, Color32::WHITE));

                            egui::show_tooltip_at_pointer(
                                ui.ctx(),
                                egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("map_tip_layer")),
                                egui::Id::new(("map_tip", plot_id)),
                                |ui: &mut egui::Ui| {
                                    ui.label(RichText::new(label).size(theme.spacing.font_body));
                                },
                            );
                        }
                    }
                }
            }
        }
    }
    }

    // ── Click / Ctrl+click / Right-click interaction ─────────────────────
    if map_response.clicked() {
        if let Some(pos) = map_response.interact_pointer_pos() {
            if let Some(pt_idx) = find_nearest(pos) {
                let row = row_indices.get(pt_idx).copied().unwrap_or(pt_idx);
                let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                interaction = Some(MapInteraction::Click { row, ctrl });
            } else {
                interaction = Some(MapInteraction::ClearSelection);
            }
        }
    }

    if map_response.secondary_clicked() {
        if let Some(pos) = map_response.interact_pointer_pos() {
            if let Some(pt_idx) = find_nearest(pos) {
                let row = row_indices.get(pt_idx).copied().unwrap_or(pt_idx);
                interaction = Some(MapInteraction::RightClick { row, screen_pos: pos });
            }
        }
    }

    // ── Drag interactions ────────────────────────────────────────────────
    let drag_key = egui::Id::new(("map_drag_start", plot_id));
    let shift_held = ui.input(|i| i.modifiers.shift);

    // Shift+drag = pan (manually, since we disabled walkers' left-drag pan).
    if shift_held && map_response.dragged_by(egui::PointerButton::Primary) {
        let delta = map_response.drag_delta();
        if delta.length() > 0.5 {
            // Create a projector to convert screen delta to geo coordinates.
            let projector = walkers::Projector::new(map_rect, map_memory, center);
            let rect_center = map_rect.center();
            // Unproject the center, and the center minus the drag delta.
            let offset_screen = egui::vec2(rect_center.x - delta.x, rect_center.y - delta.y);
            let new_geo = projector.unproject(offset_screen);
            map_memory.center_at(new_geo);
        }
    }

    // Plain drag (no shift) = area selection rectangle.
    // Record press position immediately on mouse-down so the rectangle starts
    // exactly where the user clicked (not after egui's drag-threshold delay).
    let press_key = egui::Id::new(("map_press_start", plot_id));
    if !shift_held && ui.input(|i| i.pointer.any_pressed()) {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            if map_response.rect.contains(pos) {
                ui.ctx().memory_mut(|mem| {
                    mem.data.insert_temp::<Pos2>(press_key, pos);
                });
            }
        }
    }
    // Promote press to drag start once egui recognises the drag gesture.
    if !shift_held && map_response.dragged_by(egui::PointerButton::Primary) {
        let have_drag: bool = ui.ctx().memory(|mem| mem.data.get_temp::<Pos2>(drag_key).is_some());
        if !have_drag {
            let origin: Option<Pos2> = ui.ctx().memory(|mem| mem.data.get_temp(press_key));
            if let Some(pos) = origin {
                ui.ctx().memory_mut(|mem| {
                    mem.data.insert_temp::<Pos2>(drag_key, pos);
                });
            }
        }
    }
    // Clean up press position on release.
    if ui.input(|i| i.pointer.any_released()) {
        ui.ctx().memory_mut(|mem| { mem.data.remove::<Pos2>(press_key); });
    }

    let drag_start: Option<Pos2> = ui.ctx().memory(|mem| mem.data.get_temp(drag_key));
    if let Some(start) = drag_start {
        if let Some(current) = ui.input(|i| i.pointer.hover_pos()) {
            let sel_rect = Rect::from_two_pos(start, current);
            let painter = ui.painter().with_clip_rect(map_response.rect);
            painter.rect_filled(
                sel_rect, 0.0,
                Color32::from_rgba_unmultiplied(100, 180, 255, 40),
            );
            painter.rect_stroke(
                sel_rect, 0.0,
                egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 180, 255, 180)),
                egui::StrokeKind::Outside,
            );
        }

        if ui.input(|i| i.pointer.any_released()) {
            if let Some(end) = ui.input(|i| i.pointer.hover_pos()) {
                let sel_rect = Rect::from_two_pos(start, end);
                let mut selected_rows: Vec<usize> = Vec::new();
                for &(pos, _, idx) in &screen_pts {
                    if sel_rect.contains(pos) {
                        let row = row_indices.get(idx).copied().unwrap_or(idx);
                        selected_rows.push(row);
                    }
                }
                let ctrl = ui.input(|i| i.modifiers.ctrl);
                interaction = Some(MapInteraction::AreaSelect { rows: selected_rows, ctrl });
            }
            ui.ctx().memory_mut(|mem| {
                mem.data.remove::<Pos2>(drag_key);
            });
        }
    }

    // Store interaction for show_as_window to pick up.
    if let Some(inter) = interaction {
        ui.ctx().memory_mut(|mem| {
            mem.data.insert_temp(egui::Id::new(("map_interaction", plot_id)), inter);
        });
    }
}

/// Interaction events produced by show_map for show_as_window to consume.
#[derive(Clone, Debug)]
enum MapInteraction {
    Click { row: usize, ctrl: bool },
    ClearSelection,
    RightClick { row: usize, screen_pos: Pos2 },
    AreaSelect { rows: Vec<usize>, ctrl: bool },
}

// ── Plugin: draw data points ──────────────────────────────────────────────────

struct PointsPlugin {
    points: Arc<Vec<[f64; 2]>>,
    colors: Arc<Vec<Color32>>,
    radii: Arc<Vec<f32>>,
    max_draw_points: usize,
    plot_id: usize,
    gpu_points_mode: crate::state::perf_settings::GpuPointsMode,
    gpu_points_threshold: usize,
}

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
        let default_radius = 3.0_f32;

        let painter = ui.painter().with_clip_rect(rect);

        // Collect screen positions for hover detection (stored in egui memory).
        let sampled_count = n / step;
        let mut screen_pts: Vec<(Pos2, f32, usize)> = Vec::with_capacity(sampled_count);

        // First pass: project all visible points to screen space.
        let mut draw_data: Vec<(Pos2, f32, Color32, usize)> = Vec::with_capacity(sampled_count);
        for (i, [lat, lon]) in self.points.iter().enumerate().step_by(step) {
            let r = self.radii.get(i).copied().unwrap_or(default_radius);
            let color = self.colors.get(i).copied().unwrap_or(Color32::WHITE);
            let pos = walkers::lat_lon(*lat, *lon);
            let v = projector.project(pos);
            let center = Pos2::new(v.x, v.y);

            let expanded = rect.expand(r);
            if !expanded.contains(center) { continue; }

            draw_data.push((center, r, color, i));
            screen_pts.push((center, r, i));
        }

        // Second pass: render using batched mesh or individual circles.
        let use_batched = crate::plot::gpu_points::should_use_batched(
            self.gpu_points_mode,
            self.gpu_points_threshold,
            draw_data.len(),
        );
        if use_batched {
            let mesh_shape = crate::plot::gpu_points::build_circle_mesh(
                draw_data.iter().map(|&(pos, r, color, _)| (pos, r, color)),
                draw_data.len(),
            );
            painter.add(mesh_shape);
        } else {
            for &(center, r, color, _) in &draw_data {
                painter.circle_filled(center, r, color);
            }
        }

        // Store screen positions for hover lookup by show_map.
        let key = egui::Id::new(("map_screen_pts", self.plot_id));
        ui.ctx().memory_mut(|mem| {
            mem.data.insert_temp(key, screen_pts);
        });
    }
}

// ── Grid snap ─────────────────────────────────────────────────────────────────

pub fn snap_to_grid(pos: Pos2, grid: f32) -> Pos2 {
    Pos2::new((pos.x / grid).round() * grid, (pos.y / grid).round() * grid)
}

// ── Hover label construction ─────────────────────────────────────────────────

fn build_hover_label(
    row: usize,
    lat: f64,
    lon: f64,
    config: &MapPlotConfig,
    hover_cols: &[(String, Vec<Option<String>>)],
) -> String {
    let mut label = format!(
        "{}: {}\n{}: {}",
        config.lat_col, smart_fmt(lat),
        config.lon_col, smart_fmt(lon),
    );
    for (field_name, vals) in hover_cols {
        if let Some(Some(val)) = vals.get(row) {
            label.push_str(&format!("\n{}: {}", field_name, val));
        }
    }
    label
}

fn smart_fmt(v: f64) -> String {
    if v == 0.0 { return "0".to_string(); }
    let abs = v.abs();
    if abs >= 1_000_000.0 { format!("{:.3e}", v) }
    else if abs >= 1.0 && v.fract().abs() < 1e-9 { format!("{:.0}", v) }
    else if abs >= 0.001 { format!("{:.6}", v) }
    else { format!("{:.3e}", v) }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn field_combo(ui: &mut Ui, id: &str, fields: &[crate::data::schema::FieldMeta], idx: &mut usize, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;
    let selected_text = fields.get(*idx)
        .map(|f| format!("{} {}", f.kind.icon(), f.name))
        .unwrap_or_else(|| "(none)".to_string());
    egui::ComboBox::from_id_salt(id)
        .selected_text(RichText::new(&selected_text).color(c.text_data).size(s.font_body).monospace())
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (i, field) in fields.iter().enumerate() {
                let entry = format!("{} {}", field.kind.icon(), field.name);
                ui.selectable_value(idx, i, RichText::new(&entry).color(c.text_data).size(s.font_body).monospace());
            }
        });
}

fn colormap_combo(ui: &mut Ui, id: &str, colormap: &mut Colormap, reverse: bool, theme: &AppTheme) {
    use crate::plot::colormap::sample_gradient;

    let c = &theme.colors;
    let s = &theme.spacing;
    let swatch_w = 60.0_f32;
    let swatch_h = s.font_body;

    egui::ComboBox::from_id_salt(id)
        .selected_text(RichText::new(colormap.label()).color(c.text_primary).size(s.font_body))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for cm in Colormap::all() {
                let is_selected = *colormap == *cm;
                let resp = ui.horizontal(|ui| {
                    let text_resp = ui.selectable_label(is_selected,
                        RichText::new(cm.label()).color(c.text_primary).size(s.font_body));
                    let (rect, _) = ui.allocate_exact_size(vec2(swatch_w, swatch_h), egui::Sense::hover());
                    let samples = sample_gradient(cm, 32);
                    let step_w = rect.width() / 32.0;
                    let painter = ui.painter();
                    for (i, color) in samples.iter().enumerate() {
                        let idx = if reverse { 31 - i } else { i };
                        let x = rect.min.x + idx as f32 * step_w;
                        let strip = Rect::from_min_size(egui::pos2(x, rect.min.y), vec2(step_w.ceil() + 0.5, rect.height()));
                        painter.rect_filled(strip, 0.0, *color);
                    }
                    painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, Color32::from_gray(60)), egui::StrokeKind::Outside);
                    text_resp
                });
                if resp.inner.clicked() {
                    *colormap = cm.clone();
                }
            }
        });
}

fn name_combo(ui: &mut Ui, id: &str, names: &[String], idx: &mut usize, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;
    let selected = names.get(*idx).map(|n| n.as_str()).unwrap_or("(none)");
    egui::ComboBox::from_id_salt(id)
        .selected_text(RichText::new(selected).color(c.text_data).size(s.font_body).monospace())
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (i, name) in names.iter().enumerate() {
                ui.selectable_value(idx, i, RichText::new(name).color(c.text_data).size(s.font_body).monospace());
            }
        });
}

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

fn get_f64_vec(df: &polars::prelude::DataFrame, col_name: &str) -> Option<Vec<Option<f64>>> {
    let col = df.column(col_name).ok()?;
    let series = col.as_series()?;
    let cast = series.cast(&DataType::Float64).ok()?;
    let ca = cast.f64().ok()?;
    Some(ca.into_iter().collect())
}

fn make_tiles(scheme: &TileScheme, ctx: egui::Context) -> HttpTiles {
    match scheme {
        TileScheme::OpenStreetMap => HttpTiles::new(OpenStreetMap, ctx),
        TileScheme::CartoDark => HttpTiles::new(CartoDark, ctx),
    }
}

fn compute_center(points: &[[f64; 2]]) -> Position {
    if points.is_empty() { return walkers::lat_lon(39.5, -98.35); }
    let lat = points.iter().map(|p| p[0]).sum::<f64>() / points.len() as f64;
    let lon = points.iter().map(|p| p[1]).sum::<f64>() / points.len() as f64;
    walkers::lat_lon(lat, lon)
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
