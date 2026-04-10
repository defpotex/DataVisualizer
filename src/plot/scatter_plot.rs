use crate::data::schema::DataSchema;
use crate::data::source::DataSource;
use crate::plot::map_plot::snap_to_grid;
use crate::plot::plot_config::{AlphaConfig, AxisScale, ColorMode, Colormap, PlotConfig, ScatterPlotConfig, SizeConfig};
use crate::plot::styling::{
    apply_alpha, compute_alphas, compute_colors, compute_radii, ColorLegend, PlotLegendData,
};
use crate::theme::AppTheme;
use egui::{Color32, Context, Pos2, Rect, RichText, Ui, vec2};
use egui_plot::{Plot, PlotBounds, PlotPoint, PlotPoints, Points};
use polars::prelude::{DataFrame, DataType};
use std::collections::HashSet;
use std::sync::Arc;

// ── Configure dialog ──────────────────────────────────────────────────────────

struct ScatterConfigDialog {
    is_open: bool,
    // Axis
    draft_title: String,
    draft_x_idx: usize,
    draft_y_idx: usize,
    draft_x_scale: AxisScale,
    draft_y_scale: AxisScale,
    // Color
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

impl Default for ScatterConfigDialog {
    fn default() -> Self {
        Self {
            is_open: false,
            draft_title: String::new(),
            draft_x_idx: 0,
            draft_y_idx: 0,
            draft_x_scale: AxisScale::Continuous,
            draft_y_scale: AxisScale::Continuous,
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

impl ScatterConfigDialog {
    fn open(&mut self, config: &ScatterPlotConfig, schema: &DataSchema) {
        self.is_open = true;
        self.draft_title = config.title.clone();
        let fields = &schema.fields;
        self.draft_x_idx = fields.iter().position(|f| f.name == config.x_col).unwrap_or(0);
        self.draft_y_idx = fields.iter().position(|f| f.name == config.y_col).unwrap_or(0);
        self.draft_x_scale = config.x_scale.clone();
        self.draft_y_scale = config.y_scale.clone();

        self.draft_color_variant = config.color_mode.variant_idx();
        let color_col = match &config.color_mode {
            ColorMode::Categorical { col } | ColorMode::Continuous { col, .. } => col.as_str(),
            ColorMode::Solid => "",
        };
        self.draft_color_col_idx = fields.iter().position(|f| f.name == color_col).unwrap_or(0);
        if let ColorMode::Continuous { colormap, .. } = &config.color_mode {
            self.draft_colormap = colormap.clone();
        }

        if let Some(sz) = &config.size_config {
            self.draft_size_enabled = true;
            self.draft_size_col_idx = fields.iter().position(|f| f.name == sz.col).unwrap_or(0);
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
            self.draft_alpha_col_idx = fields.iter().position(|f| f.name == al.col).unwrap_or(0);
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
        config: &ScatterPlotConfig,
        schema: &DataSchema,
        theme: &AppTheme,
    ) -> Option<ScatterPlotConfig> {
        if !self.is_open { return None; }

        let c = &theme.colors;
        let s = &theme.spacing;
        let mut result = None;
        let mut close = false;

        let fields = &schema.fields;
        let nf = fields.len();
        if self.draft_x_idx >= nf { self.draft_x_idx = 0; }
        if self.draft_y_idx >= nf { self.draft_y_idx = 0; }
        if self.draft_color_col_idx >= nf { self.draft_color_col_idx = 0; }
        if self.draft_size_col_idx >= nf { self.draft_size_col_idx = 0; }
        if self.draft_alpha_col_idx >= nf { self.draft_alpha_col_idx = 0; }

        egui::Window::new(
            RichText::new("Configure Scatter Plot")
                .color(c.text_primary)
                .size(s.font_body + 1.0)
                .strong(),
        )
        .id(egui::Id::new(("scatter_cfg_dlg", config.id)))
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

                // ── X axis ────────────────────────────────────────────────────
                ui.label(RichText::new("X Axis Column").color(c.text_secondary).size(s.font_small));
                ui.add_space(2.0);
                field_combo(ui, "cfg_x_col", fields, &mut self.draft_x_idx, theme);
                ui.add_space(4.0);
                let x_inferred = fields.get(self.draft_x_idx)
                    .map(|f| AxisScale::infer(&f.kind)).unwrap_or(AxisScale::Continuous);
                scale_toggle(ui, &mut self.draft_x_scale, &x_inferred, theme);

                ui.add_space(10.0);

                // ── Y axis ────────────────────────────────────────────────────
                ui.label(RichText::new("Y Axis Column").color(c.text_secondary).size(s.font_small));
                ui.add_space(2.0);
                field_combo(ui, "cfg_y_col", fields, &mut self.draft_y_idx, theme);
                ui.add_space(4.0);
                let y_inferred = fields.get(self.draft_y_idx)
                    .map(|f| AxisScale::infer(&f.kind)).unwrap_or(AxisScale::Continuous);
                scale_toggle(ui, &mut self.draft_y_scale, &y_inferred, theme);

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
                    field_combo(ui, "cfg_color_col", fields, &mut self.draft_color_col_idx, theme);
                }
                if self.draft_color_variant == 2 {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Colormap").color(c.text_secondary).size(s.font_small));
                    ui.add_space(2.0);
                    colormap_combo(ui, "cfg_colormap", &mut self.draft_colormap, theme);
                }

                ui.add_space(10.0);

                // ── Size ──────────────────────────────────────────────────────
                ui.checkbox(&mut self.draft_size_enabled,
                    RichText::new("SIZE  by column").color(c.text_secondary).size(s.font_small));
                if self.draft_size_enabled {
                    ui.add_space(4.0);
                    field_combo(ui, "cfg_size_col", fields, &mut self.draft_size_col_idx, theme);
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
                    field_combo(ui, "cfg_alpha_col", fields, &mut self.draft_alpha_col_idx, theme);
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
                    RichText::new("X and Y are always shown. Add extra columns:")
                        .color(c.text_secondary)
                        .size(s.font_small)
                        .italics(),
                );
                ui.add_space(4.0);

                let row_h = s.font_body + 8.0;
                let list_h = (row_h * fields.len().min(6) as f32).max(60.0);
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
                                for field in fields {
                                    // Skip x and y — they're always shown
                                    let x_name = fields.get(self.draft_x_idx).map(|f| f.name.as_str()).unwrap_or("");
                                    let y_name = fields.get(self.draft_y_idx).map(|f| f.name.as_str()).unwrap_or("");
                                    if field.name == x_name || field.name == y_name { continue; }

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

                let can_apply = !fields.is_empty() && !self.draft_title.trim().is_empty();
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
                        let x_col = fields.get(self.draft_x_idx).map(|f| f.name.clone()).unwrap_or_default();
                        let y_col = fields.get(self.draft_y_idx).map(|f| f.name.clone()).unwrap_or_default();

                        let color_mode = match self.draft_color_variant {
                            1 => ColorMode::Categorical {
                                col: fields.get(self.draft_color_col_idx).map(|f| f.name.clone()).unwrap_or_default(),
                            },
                            2 => ColorMode::Continuous {
                                col: fields.get(self.draft_color_col_idx).map(|f| f.name.clone()).unwrap_or_default(),
                                colormap: self.draft_colormap.clone(),
                            },
                            _ => ColorMode::Solid,
                        };

                        let size_config = if self.draft_size_enabled {
                            Some(SizeConfig {
                                col: fields.get(self.draft_size_col_idx).map(|f| f.name.clone()).unwrap_or_default(),
                                min_px: self.draft_size_min_px,
                                max_px: self.draft_size_max_px,
                            })
                        } else { None };

                        let alpha_config = if self.draft_alpha_enabled {
                            Some(AlphaConfig {
                                col: fields.get(self.draft_alpha_col_idx).map(|f| f.name.clone()).unwrap_or_default(),
                                min_alpha: self.draft_alpha_min,
                                max_alpha: self.draft_alpha_max,
                            })
                        } else { None };

                        let mut hover_fields: Vec<String> = self.draft_hover_fields.iter().cloned().collect();
                        hover_fields.sort_unstable();

                        result = Some(ScatterPlotConfig {
                            id: config.id,
                            title: self.draft_title.trim().to_string(),
                            source_id: config.source_id,
                            x_col,
                            y_col,
                            x_scale: self.draft_x_scale.clone(),
                            y_scale: self.draft_y_scale.clone(),
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

// ── ScatterPlot ───────────────────────────────────────────────────────────────

pub struct ScatterPlot {
    pub config: ScatterPlotConfig,
    is_open: bool,
    default_pos: Pos2,
    pending_snap: Option<Pos2>,

    points: Arc<Vec<[f64; 2]>>,
    colors: Arc<Vec<Color32>>,
    radii: Arc<Vec<f32>>,
    hover_labels: Arc<Vec<String>>,
    /// For categorical mode: ordered (category_name, full-alpha color) from the legend.
    category_entries: Arc<Vec<(String, Color32)>>,
    x_labels: Arc<Vec<String>>,
    y_labels: Arc<Vec<String>>,
    cached_schema: Option<DataSchema>,
    legend: Option<PlotLegendData>,

    configure_dialog: ScatterConfigDialog,
}

impl ScatterPlot {
    pub fn new(config: ScatterPlotConfig, default_pos: Pos2) -> Self {
        Self {
            config,
            is_open: true,
            default_pos,
            pending_snap: None,
            points: Arc::new(Vec::new()),
            colors: Arc::new(Vec::new()),
            radii: Arc::new(Vec::new()),
            hover_labels: Arc::new(Vec::new()),
            category_entries: Arc::new(Vec::new()),
            x_labels: Arc::new(Vec::new()),
            y_labels: Arc::new(Vec::new()),
            cached_schema: None,
            legend: None,
            configure_dialog: ScatterConfigDialog::default(),
        }
    }

    pub fn sync_data(&mut self, source: &DataSource) {
        self.cached_schema = Some(source.schema.clone());
        let df = &source.df;
        let n = df.height();

        let (xs, x_labels_vec) = column_to_f64(df, &self.config.x_col, &self.config.x_scale);
        let (ys, y_labels_vec) = column_to_f64(df, &self.config.y_col, &self.config.y_scale);

        let solid_color = Color32::from_rgb(100, 200, 255);
        let (mut all_colors, color_legend) = compute_colors(df, &self.config.color_mode, solid_color, n);
        let (all_radii, size_legend) = compute_radii(df, self.config.size_config.as_ref(), 2.5, n);
        let base_alpha = 200.0 / 255.0;
        let (all_alphas, alpha_legend) = compute_alphas(df, self.config.alpha_config.as_ref(), base_alpha, n);
        apply_alpha(&mut all_colors, &all_alphas);

        // Pre-extract hover field columns once (avoids per-row column lookup + cast).
        let hover_cols: Vec<(&str, Vec<Option<String>>)> = self.config.hover_fields.iter()
            .filter_map(|field| {
                let series = df.column(field).ok()?.as_series()?.clone();
                let cast = series.cast(&DataType::String).ok()?;
                let ca = cast.str().ok()?.clone();
                let vals: Vec<Option<String>> = ca.into_iter().map(|v| v.map(|s| s.to_string())).collect();
                Some((field.as_str(), vals))
            })
            .collect();

        let row_count = xs.len().min(ys.len());
        let mut pts: Vec<[f64; 2]> = Vec::new();
        let mut colors: Vec<Color32> = Vec::new();
        let mut radii: Vec<f32> = Vec::new();
        let mut hover_labels_vec: Vec<String> = Vec::new();

        for i in 0..row_count {
            if let (Some(x), Some(y)) = (xs[i], ys[i]) {
                pts.push([x, y]);
                colors.push(all_colors.get(i).copied().unwrap_or(solid_color));
                radii.push(all_radii.get(i).copied().unwrap_or(2.5));
                hover_labels_vec.push(build_hover_label(
                    i, x, y,
                    &x_labels_vec, &y_labels_vec,
                    &self.config,
                    &hover_cols,
                ));
            }
        }

        let category_entries: Vec<(String, Color32)> = match &color_legend {
            ColorLegend::Categorical { entries, .. } => entries.clone(),
            _ => Vec::new(),
        };

        self.points = Arc::new(pts);
        self.colors = Arc::new(colors);
        self.radii = Arc::new(radii);
        self.hover_labels = Arc::new(hover_labels_vec);
        self.category_entries = Arc::new(category_entries);
        self.x_labels = Arc::new(x_labels_vec);
        self.y_labels = Arc::new(y_labels_vec);

        self.legend = Some(PlotLegendData {
            plot_id: self.config.id,
            plot_title: self.config.title.clone(),
            color: color_legend,
            size: size_legend,
            alpha: alpha_legend,
        });
    }

    pub fn apply_config(&mut self, config: ScatterPlotConfig) { self.config = config; }
    pub fn plot_id(&self) -> usize { self.config.id }
    pub fn legend_data(&self) -> Option<&PlotLegendData> { self.legend.as_ref() }

    pub fn window_id(&self) -> egui::Id {
        egui::Id::new(("scatter_plot_win", self.config.id))
    }

    pub fn set_pending_snap(&mut self, pos: Pos2) { self.pending_snap = Some(pos); }

    pub fn intended_rect(&self, ctx: &Context) -> Option<Rect> {
        let state = egui::AreaState::load(ctx, self.window_id())?;
        let size = state.size?;
        let pos = self.pending_snap.unwrap_or_else(|| state.left_top_pos());
        Some(Rect::from_min_size(pos, size))
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
        let win_id = self.window_id();

        let default_w = (central_rect.width() * 0.5 - 12.0).max(320.0);
        let default_h = (central_rect.height() * 0.5 - 12.0).max(200.0);

        let window_frame = egui::Frame {
            fill: c.bg_panel,
            stroke: egui::Stroke::new(1.0, c.accent_secondary),
            corner_radius: egui::CornerRadius::from(s.rounding),
            inner_margin: egui::Margin::from(0.0_f32),
            ..Default::default()
        };

        let snap = self.pending_snap.take();
        let mut is_open = self.is_open;
        let mut gear_clicked = false;

        // For solid mode, replace placeholder colors with theme color.
        let solid_theme_color = Color32::from_rgba_unmultiplied(
            c.accent_secondary.r(), c.accent_secondary.g(), c.accent_secondary.b(), 200,
        );
        let display_colors = if matches!(self.config.color_mode, ColorMode::Solid) {
            Arc::new(vec![solid_theme_color; self.points.len()])
        } else {
            Arc::clone(&self.colors)
        };

        let mut win = egui::Window::new(
            RichText::new(&self.config.title).color(c.text_primary).size(s.font_body).strong(),
        )
        .id(win_id)
        .open(&mut is_open)
        .resizable(true)
        .collapsible(true)
        .default_pos(self.default_pos)
        .default_size([default_w, default_h])
        .min_size([280.0, 180.0])
        .constrain_to(central_rect)
        .frame(window_frame);

        if let Some(pos) = snap { win = win.current_pos(pos); }

        win.show(ctx, |ui| {
            ui.push_id(id, |ui| {
                gear_clicked = show_toolbar(ui, &self.config, self.points.len(), theme);
                ui.separator();
                show_scatter(
                    ui,
                    Arc::clone(&self.points),
                    display_colors,
                    Arc::clone(&self.radii),
                    Arc::clone(&self.hover_labels),
                    Arc::clone(&self.category_entries),
                    Arc::clone(&self.x_labels),
                    Arc::clone(&self.y_labels),
                    &self.config,
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
                event = PlotWindowEvent::ConfigChanged(PlotConfig::Scatter(new_config));
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

// ── PlotWindowEvent ───────────────────────────────────────────────────────────

pub enum PlotWindowEvent {
    Open,
    Closed,
    ConfigChanged(PlotConfig),
}

// ── Toolbar ───────────────────────────────────────────────────────────────────

fn show_toolbar(ui: &mut Ui, config: &ScatterPlotConfig, point_count: usize, theme: &AppTheme) -> bool {
    let c = &theme.colors;
    let s = &theme.spacing;
    let mut gear_clicked = false;

    egui::Frame::default()
        .fill(c.bg_app)
        .inner_margin(egui::Margin::from(vec2(8.0, 4.0)))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new("◉").color(c.accent_secondary).size(s.font_small));
                ui.label(RichText::new("Scatter").color(c.text_secondary).size(s.font_small));
                ui.separator();
                ui.label(
                    RichText::new(format!("{} pts", format_count(point_count)))
                        .color(c.accent_secondary).size(s.font_small),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(
                        RichText::new("⚙").color(c.text_secondary).size(s.font_small),
                    ).frame(false)).on_hover_text("Configure plot").clicked() {
                        gear_clicked = true;
                    }
                    ui.separator();
                    ui.label(
                        RichText::new(format!("y: {}  x: {}", config.y_col, config.x_col))
                            .color(c.text_secondary).size(s.font_small).monospace(),
                    );
                });
            });
        });

    gear_clicked
}

// ── Scatter widget ────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn show_scatter(
    ui: &mut Ui,
    points: Arc<Vec<[f64; 2]>>,
    colors: Arc<Vec<Color32>>,
    radii: Arc<Vec<f32>>,
    hover_labels: Arc<Vec<String>>,
    category_entries: Arc<Vec<(String, Color32)>>,
    x_labels: Arc<Vec<String>>,
    y_labels: Arc<Vec<String>>,
    config: &ScatterPlotConfig,
    theme: &AppTheme,
    max_draw_points: usize,
    plot_id: usize,
) {
    let n = points.len();
    if n == 0 {
        let c = &theme.colors;
        let s = &theme.spacing;
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No data").color(c.text_secondary).size(s.font_small).italics());
        });
        return;
    }

    let step = if max_draw_points > 0 && n > max_draw_points {
        (n / max_draw_points).max(1)
    } else {
        1
    };

    let is_categorical = !category_entries.is_empty();
    let is_continuous = matches!(config.color_mode, ColorMode::Continuous { .. });

    let x_is_cat = config.x_scale == AxisScale::Categorical && !x_labels.is_empty();
    let y_is_cat = config.y_scale == AxisScale::Categorical && !y_labels.is_empty();
    let x_len = x_labels.len();
    let y_len = y_labels.len();

    let xl_fmt = Arc::clone(&x_labels);
    let yl_fmt = Arc::clone(&y_labels);
    let xl_hover = Arc::clone(&x_labels);
    let yl_hover = Arc::clone(&y_labels);
    let x_is_cat2 = x_is_cat;
    let y_is_cat2 = y_is_cat;

    // label_formatter: used by Solid and Categorical native series.
    // For Continuous we use manual hover, so return empty.
    let hover_labels_fmt = Arc::clone(&hover_labels);
    let points_fmt = Arc::clone(&points);
    let step_fmt = step;
    let is_cont_fmt = is_continuous;

    let mut plot = Plot::new(egui::Id::new(("scatter", plot_id)))
        .x_axis_label(&config.x_col as &str)
        .y_axis_label(&config.y_col as &str)
        .set_margin_fraction(egui::vec2(0.05, 0.05))
        .allow_zoom(true)
        .allow_drag(true)
        .allow_scroll(true)
        .allow_boxed_zoom(true)
        .auto_bounds(egui::Vec2b::new(!x_is_cat, !y_is_cat))
        .label_formatter(move |_name, val: &egui_plot::PlotPoint| {
            if is_cont_fmt { return String::new(); }
            // Nearest-point lookup for Solid / Categorical.
            let best = (0..points_fmt.len())
                .step_by(step_fmt)
                .min_by(|&i, &j| {
                    let [xi, yi] = points_fmt[i];
                    let [xj, yj] = points_fmt[j];
                    let di = (xi - val.x) * (xi - val.x) + (yi - val.y) * (yi - val.y);
                    let dj = (xj - val.x) * (xj - val.x) + (yj - val.y) * (yj - val.y);
                    di.partial_cmp(&dj).unwrap_or(std::cmp::Ordering::Equal)
                });
            best.and_then(|i| hover_labels_fmt.get(i)).cloned().unwrap_or_default()
        })
        .coordinates_formatter(
            egui_plot::Corner::LeftBottom,
            egui_plot::CoordinatesFormatter::new(move |val, _bounds| {
                let x_str = if x_is_cat2 {
                    xl_hover.get(val.x.round() as usize).cloned()
                        .unwrap_or_else(|| format!("{:.2}", val.x))
                } else {
                    smart_fmt(val.x)
                };
                let y_str = if y_is_cat2 {
                    yl_hover.get(val.y.round() as usize).cloned()
                        .unwrap_or_else(|| format!("{:.2}", val.y))
                } else {
                    smart_fmt(val.y)
                };
                format!("x={x_str}  y={y_str}")
            }),
        );

    if x_is_cat {
        plot = plot
            .x_grid_spacer(egui_plot::uniform_grid_spacer(move |_| [1.0, 1.0, 1.0]))
            .x_axis_formatter(move |mark, _range| {
                xl_fmt.get(mark.value.round() as usize).cloned().unwrap_or_default()
            });
    } else {
        plot = plot.x_grid_spacer(egui_plot::log_grid_spacer(10));
    }
    if y_is_cat {
        plot = plot
            .y_grid_spacer(egui_plot::uniform_grid_spacer(move |_| [1.0, 1.0, 1.0]))
            .y_axis_formatter(move |mark, _range| {
                yl_fmt.get(mark.value.round() as usize).cloned().unwrap_or_default()
            });
    } else {
        plot = plot.y_grid_spacer(egui_plot::log_grid_spacer(10));
    }

    // For continuous painter mode: collect screen positions inside the closure.
    let mut screen_pts: Vec<(egui::Pos2, Color32, f32, usize)> = Vec::new();

    let plot_response = plot.show(ui, |plot_ui: &mut egui_plot::PlotUi| {
        if is_categorical {
            // ── Categorical: one Points series per category ────────────────────
            let first_radius = radii.first().copied().unwrap_or(2.5);
            for (cat_name, cat_color) in category_entries.iter() {
                let opaque_cat = Color32::from_rgb(cat_color.r(), cat_color.g(), cat_color.b());
                let cat_pts: PlotPoints = (0..n)
                    .step_by(step)
                    .filter_map(|i| {
                        let c = colors.get(i)?;
                        let opaque_c = Color32::from_rgb(c.r(), c.g(), c.b());
                        if opaque_c == opaque_cat { Some(points[i]) } else { None }
                    })
                    .collect();
                // Use alpha=200 to match the painter approach.
                let display_color = Color32::from_rgba_unmultiplied(
                    cat_color.r(), cat_color.g(), cat_color.b(), 200,
                );
                plot_ui.points(
                    Points::new(cat_name.as_str(), cat_pts)
                        .color(display_color)
                        .radius(first_radius)
                        .shape(egui_plot::MarkerShape::Circle),
                );
            }
        } else if is_continuous {
            // ── Continuous: bounds-only series + collect screen positions ──────
            // Use a nearly-zero-radius transparent series just for auto_bounds.
            let bounds_pts: PlotPoints = (0..n).step_by(step)
                .map(|i| points[i]).collect();
            plot_ui.points(
                Points::new("bounds", bounds_pts)
                    .radius(0.1)
                    .color(Color32::from_rgba_unmultiplied(0, 0, 0, 0)),
            );
            // Collect screen positions for manual painting and hover.
            for i in (0..n).step_by(step) {
                let [x, y] = points[i];
                let screen = plot_ui.screen_from_plot(PlotPoint::new(x, y));
                let color = colors.get(i).copied().unwrap_or(Color32::WHITE);
                let radius = radii.get(i).copied().unwrap_or(2.5);
                screen_pts.push((screen, color, radius, i));
            }
        } else {
            // ── Solid: single native Points series ────────────────────────────
            let first_color = colors.first().copied().unwrap_or(Color32::WHITE);
            let first_radius = radii.first().copied().unwrap_or(2.5);
            let sampled: PlotPoints = (0..n).step_by(step).map(|i| points[i]).collect();
            plot_ui.points(
                Points::new("data", sampled)
                    .color(first_color)
                    .radius(first_radius)
                    .shape(egui_plot::MarkerShape::Circle),
            );
        }

        // Fixed bounds for categorical axes.
        let current = plot_ui.plot_bounds();
        let xmin = if x_is_cat { -0.5 } else { current.min()[0] };
        let xmax = if x_is_cat { x_len as f64 - 0.5 } else { current.max()[0] };
        let ymin = if y_is_cat { -0.5 } else { current.min()[1] };
        let ymax = if y_is_cat { y_len as f64 - 0.5 } else { current.max()[1] };
        if x_is_cat || y_is_cat {
            plot_ui.set_plot_bounds(PlotBounds::from_min_max([xmin, ymin], [xmax, ymax]));
        }
    });

    // ── Continuous: paint circles and manual hover ─────────────────────────────
    if is_continuous && !screen_pts.is_empty() {
        let clip = plot_response.response.rect;
        let painter = ui.painter().with_clip_rect(clip);
        for &(pos, color, radius, _) in &screen_pts {
            painter.circle_filled(pos, radius, color);
        }

        // Manual hover tooltip.
        if let Some(hpos) = plot_response.response.hover_pos() {
            let nearest = screen_pts.iter()
                .min_by(|a, b| {
                    a.0.distance(hpos).partial_cmp(&b.0.distance(hpos))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            if let Some(&(pos, _, radius, idx)) = nearest {
                if pos.distance(hpos) <= radius + 10.0 {
                    let label = hover_labels.get(idx).cloned().unwrap_or_default();
                    if !label.is_empty() {
                        egui::show_tooltip_at_pointer(
                            ui.ctx(),
                            egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("scatter_tip_layer")),
                            egui::Id::new(("scatter_tip", plot_id)),
                            |ui: &mut egui::Ui| {
                                ui.label(RichText::new(&label).size(theme.spacing.font_body));
                            },
                        );
                    }
                }
            }
        }
    }
}

// ── Hover label construction ──────────────────────────────────────────────────

/// Build a formatted hover label for a single data point.
/// `hover_cols` contains pre-extracted string columns to avoid per-row column lookups.
fn build_hover_label(
    row: usize,
    x: f64,
    y: f64,
    x_labels: &[String],
    y_labels: &[String],
    config: &ScatterPlotConfig,
    hover_cols: &[(&str, Vec<Option<String>>)],
) -> String {
    let x_str = if config.x_scale == AxisScale::Categorical {
        x_labels.get(x.round() as usize).cloned()
            .unwrap_or_else(|| format!("{:.0}", x))
    } else {
        smart_fmt(x)
    };
    let y_str = if config.y_scale == AxisScale::Categorical {
        y_labels.get(y.round() as usize).cloned()
            .unwrap_or_else(|| format!("{:.0}", y))
    } else {
        smart_fmt(y)
    };

    let mut label = format!("{}: {}\n{}: {}", config.x_col, x_str, config.y_col, y_str);
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
    else if abs >= 0.001 { format!("{:.4}", v) }
    else { format!("{:.3e}", v) }
}

// ── Data extraction ───────────────────────────────────────────────────────────

fn column_to_f64(df: &DataFrame, col: &str, scale: &AxisScale) -> (Vec<Option<f64>>, Vec<String>) {
    match scale {
        AxisScale::Continuous => (get_f64_vec(df, col).unwrap_or_default(), Vec::new()),
        AxisScale::Categorical => column_to_categorical(df, col),
    }
}

fn get_f64_vec(df: &DataFrame, col: &str) -> Option<Vec<Option<f64>>> {
    let series = df.column(col).ok()?.as_series()?.clone();
    let cast = series.cast(&DataType::Float64).ok()?;
    let ca = cast.f64().ok()?;
    Some(ca.into_iter().collect())
}

fn column_to_categorical(df: &DataFrame, col: &str) -> (Vec<Option<f64>>, Vec<String>) {
    let series = match df.column(col).ok().and_then(|c| c.as_series()).map(|s| s.clone()) {
        Some(s) => s,
        None => return (Vec::new(), Vec::new()),
    };
    let cast = series.cast(&DataType::String).unwrap_or(series);
    let ca = match cast.str() {
        Ok(c) => c.clone(),
        Err(_) => return (Vec::new(), Vec::new()),
    };
    let mut labels: Vec<String> = Vec::new();
    let values: Vec<Option<f64>> = ca.into_iter().map(|opt_s| {
        let s = opt_s?;
        let idx = if let Some(pos) = labels.iter().position(|l| l == s) { pos }
        else { let pos = labels.len(); labels.push(s.to_string()); pos };
        Some(idx as f64)
    }).collect();
    (values, labels)
}

// ── Dialog helpers ────────────────────────────────────────────────────────────

fn field_combo(ui: &mut Ui, id: &str, fields: &[crate::data::schema::FieldMeta], idx: &mut usize, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;
    let selected = fields.get(*idx).map(|f| format!("{} {}", f.kind.icon(), f.name))
        .unwrap_or_else(|| "(none)".to_string());
    egui::ComboBox::from_id_salt(id)
        .selected_text(RichText::new(&selected).color(c.text_data).size(s.font_body).monospace())
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (i, field) in fields.iter().enumerate() {
                let label = format!("{} {}", field.kind.icon(), field.name);
                ui.selectable_value(idx, i, RichText::new(&label).color(c.text_data).size(s.font_body).monospace());
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

fn scale_toggle(ui: &mut Ui, scale: &mut AxisScale, inferred: &AxisScale, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Scale:").color(c.text_secondary).size(s.font_small));
        for variant in [AxisScale::Continuous, AxisScale::Categorical] {
            let is_selected = *scale == variant;
            let is_inferred = variant == *inferred;
            let label = if is_inferred { format!("{} (auto)", variant.label()) } else { variant.label().to_string() };
            let btn = egui::Button::new(
                RichText::new(&label)
                    .color(if is_selected { c.bg_app } else { c.text_secondary })
                    .size(s.font_small),
            )
            .fill(if is_selected { c.accent_secondary } else { c.widget_bg })
            .stroke(egui::Stroke::new(1.0, if is_selected { c.accent_secondary } else { c.border }));
            if ui.add(btn).clicked() { *scale = variant; }
        }
    });
}

// ── Misc helpers ──────────────────────────────────────────────────────────────

fn format_count(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { result.push(','); }
        result.push(ch);
    }
    result.chars().rev().collect()
}
