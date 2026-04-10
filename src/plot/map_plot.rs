use crate::data::schema::DataSchema;
use crate::data::source::DataSource;
use crate::plot::plot_config::{
    AlphaConfig, ColorMode, Colormap, MapPlotConfig, PlotConfig, SizeConfig, TileScheme,
};
use crate::plot::scatter_plot::PlotWindowEvent;
use crate::plot::styling::{
    apply_alpha, compute_alphas, compute_colors, compute_radii, PlotLegendData,
};
use crate::theme::AppTheme;
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
        if let ColorMode::Continuous { colormap, .. } = &config.color_mode {
            self.draft_colormap = colormap.clone();
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
                    colormap_combo(ui, "cfg_map_colormap", &mut self.draft_colormap, theme);
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
    /// Cached schema for the configure dialog.
    cached_schema: Option<DataSchema>,
    /// Legend metadata.
    legend: Option<PlotLegendData>,

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
            colors: Arc::new(Vec::new()),
            radii: Arc::new(Vec::new()),
            hover_labels: Arc::new(Vec::new()),
            cached_schema: None,
            legend: None,
            configure_dialog: MapConfigDialog::default(),
        }
    }

    pub fn sync_data(&mut self, source: &DataSource) {
        self.cached_schema = Some(source.schema.clone());
        let df = &source.df;
        let n = df.height();

        // Compute per-row colors (before filtering invalid lat/lon).
        let solid_color = Color32::from_rgb(100, 180, 255); // placeholder; overridden at render if Solid
        let (mut all_colors, color_legend) = compute_colors(df, &self.config.color_mode, solid_color, n);

        let base_radius = 3.0_f32;
        let (all_radii, size_legend) = compute_radii(df, self.config.size_config.as_ref(), base_radius, n);
        let base_alpha = 200.0 / 255.0;
        let (all_alphas, alpha_legend) = compute_alphas(df, self.config.alpha_config.as_ref(), base_alpha, n);
        apply_alpha(&mut all_colors, &all_alphas);

        // Pre-extract hover field columns once.
        let hover_cols: Vec<(&str, Vec<Option<String>>)> = self.config.hover_fields.iter()
            .filter_map(|field| {
                let series = df.column(field).ok()?.as_series()?.clone();
                let cast = series.cast(&DataType::String).ok()?;
                let ca = cast.str().ok()?.clone();
                let vals: Vec<Option<String>> = ca.into_iter().map(|v| v.map(|s| s.to_string())).collect();
                Some((field.as_str(), vals))
            })
            .collect();

        // Extract lat/lon and filter invalid rows, keeping colors/radii/labels aligned.
        let all_lats = get_f64_vec(df, &self.config.lat_col).unwrap_or_else(|| vec![None; n]);
        let all_lons = get_f64_vec(df, &self.config.lon_col).unwrap_or_else(|| vec![None; n]);

        let mut pts: Vec<[f64; 2]> = Vec::new();
        let mut colors: Vec<Color32> = Vec::new();
        let mut radii: Vec<f32> = Vec::new();
        let mut hover_labels_vec: Vec<String> = Vec::new();

        for i in 0..n {
            if let (Some(lat), Some(lon)) = (all_lats.get(i).copied().flatten(), all_lons.get(i).copied().flatten()) {
                pts.push([lat, lon]);
                colors.push(all_colors.get(i).copied().unwrap_or(solid_color));
                radii.push(all_radii.get(i).copied().unwrap_or(base_radius));
                hover_labels_vec.push(build_hover_label(
                    i, lat, lon,
                    &self.config,
                    &hover_cols,
                ));
            }
        }

        self.points = Arc::new(pts);
        self.colors = Arc::new(colors);
        self.radii = Arc::new(radii);
        self.hover_labels = Arc::new(hover_labels_vec);
        self.legend = Some(PlotLegendData {
            plot_id: self.config.id,
            plot_title: self.config.title.clone(),
            color: color_legend,
            size: size_legend,
            alpha: alpha_legend,
        });
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
        max_draw_points: usize,
    ) -> PlotWindowEvent {
        if !self.is_open { return PlotWindowEvent::Closed; }

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
        let display_colors = if matches!(self.config.color_mode, ColorMode::Solid) {
            Arc::new(vec![solid_theme_color; self.points.len()])
        } else {
            Arc::clone(&self.colors)
        };

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
                    display_colors,
                    Arc::clone(&self.radii),
                    Arc::clone(&self.hover_labels),
                    theme,
                    max_draw_points,
                    id,
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
    theme: &AppTheme,
    max_draw_points: usize,
    plot_id: usize,
) {
    if tiles.is_none() {
        *tiles = Some(make_tiles(&config.tile_scheme, ui.ctx().clone()));
    }

    let center = compute_center(&points);
    let tile_ref: &mut dyn Tiles = tiles.as_mut().unwrap();

    let map_response = ui.add(
        Map::new(Some(tile_ref), map_memory, center)
            .with_plugin(PointsPlugin {
                points: Arc::clone(&points),
                colors,
                radii: Arc::clone(&radii),
                max_draw_points,
                plot_id,
            }),
    );

    // Hover tooltip: find nearest point to cursor within the map area.
    if let Some(hover_pos) = map_response.hover_pos() {
        // We need to project points to screen space to find nearest.
        // Re-create a projector from the map memory and response rect.
        // Since walkers doesn't expose the projector outside the plugin,
        // we store screen positions in the plugin and retrieve them via egui's memory.
        let screen_pts: Vec<(Pos2, f32, usize)> = ui.ctx().memory(|mem| {
            mem.data.get_temp(egui::Id::new(("map_screen_pts", plot_id)))
                .unwrap_or_default()
        });

        if !screen_pts.is_empty() {
            let nearest = screen_pts.iter()
                .min_by(|a, b| {
                    a.0.distance(hover_pos).partial_cmp(&b.0.distance(hover_pos))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            if let Some(&(pos, radius, idx)) = nearest {
                if pos.distance(hover_pos) <= radius + 10.0 {
                    if let Some(label) = hover_labels.get(idx) {
                        if !label.is_empty() {
                            // Highlight the hovered point.
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

// ── Plugin: draw data points ──────────────────────────────────────────────────

struct PointsPlugin {
    points: Arc<Vec<[f64; 2]>>,
    colors: Arc<Vec<Color32>>,
    radii: Arc<Vec<f32>>,
    max_draw_points: usize,
    plot_id: usize,
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
        let mut screen_pts: Vec<(Pos2, f32, usize)> = Vec::new();

        for (i, [lat, lon]) in self.points.iter().enumerate().step_by(step) {
            let r = self.radii.get(i).copied().unwrap_or(default_radius);
            let color = self.colors.get(i).copied().unwrap_or(Color32::WHITE);
            let pos = walkers::lat_lon(*lat, *lon);
            let v = projector.project(pos);
            let center = Pos2::new(v.x, v.y);

            let expanded = rect.expand(r);
            if !expanded.contains(center) { continue; }

            painter.circle_filled(center, r, color);
            screen_pts.push((center, r, i));
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
    hover_cols: &[(&str, Vec<Option<String>>)],
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

fn colormap_combo(ui: &mut Ui, id: &str, colormap: &mut Colormap, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;
    egui::ComboBox::from_id_salt(id)
        .selected_text(RichText::new(colormap.label()).color(c.text_primary).size(s.font_body))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for cm in Colormap::all() {
                ui.selectable_value(colormap, cm.clone(),
                    RichText::new(cm.label()).color(c.text_primary).size(s.font_body));
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
