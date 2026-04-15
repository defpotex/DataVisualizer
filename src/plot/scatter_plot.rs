use crate::data::schema::DataSchema;
use crate::data::source::DataSource;
use crate::plot::map_plot::snap_to_grid;
use crate::plot::plot_config::{AlphaConfig, AxisScale, ColorMode, Colormap, PlotConfig, ScatterPlotConfig, SizeConfig};
use crate::plot::styling::{
    apply_alpha, compute_alphas, compute_colors, compute_radii, ColorLegend, PlotLegendData,
};
use crate::plot::spatial_grid::SpatialGrid;
use crate::plot::sync::{CancelToken, ScatterSyncResult};
use crate::state::app_state::DataEvent;
use crate::theme::AppTheme;
use crossbeam_channel::Sender;
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
    /// Original row indices in the (filtered) DataFrame, aligned with `points`.
    row_indices: Arc<Vec<usize>>,
    /// For categorical mode: ordered (category_name, full-alpha color) from the legend.
    category_entries: Arc<Vec<(String, Color32)>>,
    /// Per-point category index into `category_entries` (empty for non-categorical modes).
    category_indices: Arc<Vec<Option<usize>>>,
    x_labels: Arc<Vec<String>>,
    y_labels: Arc<Vec<String>>,
    cached_schema: Option<DataSchema>,
    legend: Option<PlotLegendData>,

    configure_dialog: ScatterConfigDialog,
    context_menu_row: Option<usize>,
    context_menu_pos: Option<Pos2>,

    /// True while a background thread is computing plot data.
    computing: bool,
    /// True once the first sync result has arrived (suppresses overlay during playback).
    has_loaded: bool,
    /// Token to cancel a running background sync.
    cancel_token: CancelToken,
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
            row_indices: Arc::new(Vec::new()),
            category_entries: Arc::new(Vec::new()),
            category_indices: Arc::new(Vec::new()),
            x_labels: Arc::new(Vec::new()),
            y_labels: Arc::new(Vec::new()),
            cached_schema: None,
            legend: None,
            configure_dialog: ScatterConfigDialog::default(),
            context_menu_row: None,
            context_menu_pos: None,
            computing: false,
            has_loaded: false,
            cancel_token: CancelToken::new(),
        }
    }

    pub fn is_computing(&self) -> bool { self.computing }

    /// Kick off data computation on a background thread.  The UI remains
    /// responsive and shows a "Computing…" overlay until the result arrives
    /// via the event channel.
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
            let result = compute_scatter_data(plot_id, &config, &schema, &df, &token);
            match result {
                Some(r) => {
                    let _ = tx.send(DataEvent::PlotSyncReady(
                        crate::plot::sync::PlotSyncEvent::ScatterReady(r),
                    ));
                }
                None => {
                    // Cancelled.
                    let _ = tx.send(DataEvent::PlotSyncReady(
                        crate::plot::sync::PlotSyncEvent::Cancelled { plot_id },
                    ));
                }
            }
        });
    }

    /// Apply a completed sync result from the background thread.
    pub fn apply_sync_result(&mut self, result: ScatterSyncResult) {
        self.cached_schema = Some(result.schema);
        self.points = Arc::new(result.points);
        self.colors = Arc::new(result.colors);
        self.radii = Arc::new(result.radii);
        self.hover_labels = Arc::new(result.hover_labels);
        self.row_indices = Arc::new(result.row_indices);
        self.category_entries = Arc::new(result.category_entries);
        self.category_indices = Arc::new(result.category_indices);
        self.x_labels = Arc::new(result.x_labels);
        self.y_labels = Arc::new(result.y_labels);
        self.legend = Some(result.legend);
        self.computing = false;
        self.has_loaded = true;
    }

    /// Cancel any in-flight background sync and clear the computing flag.
    pub fn cancel_sync(&mut self) {
        self.cancel_token.cancel();
        self.computing = false;
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
        perf: &crate::state::perf_settings::PerformanceSettings,
        _selection: Option<&crate::state::selection::SelectionSet>,
    ) -> PlotWindowEvent {
        if !self.is_open { return PlotWindowEvent::Closed; }

        puffin::profile_function!();

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

        let show_overlay = self.computing && !self.has_loaded;
        let mut cancel_clicked = false;
        win.show(ctx, |ui| {
            ui.push_id(id, |ui| {
                gear_clicked = show_toolbar(ui, &self.config, self.points.len(), theme);
                ui.separator();
                if show_overlay {
                    show_computing_overlay(ui, theme, &mut cancel_clicked);
                } else {
                    show_scatter(
                        ui,
                        Arc::clone(&self.points),
                        display_colors,
                        Arc::clone(&self.radii),
                        Arc::clone(&self.hover_labels),
                        Arc::clone(&self.row_indices),
                        Arc::clone(&self.category_entries),
                        Arc::clone(&self.category_indices),
                        Arc::clone(&self.x_labels),
                        Arc::clone(&self.y_labels),
                        &self.config,
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
        // Request continuous repaint while computing so the spinner animates.
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
                event = PlotWindowEvent::ConfigChanged(PlotConfig::Scatter(new_config));
            }
        }

        // ── Handle selection interactions from show_scatter ───────────────────
        let interaction: Option<ScatterInteraction> = ctx.memory(|mem| {
            mem.data.get_temp(egui::Id::new(("scatter_interaction", id)))
        });
        if let Some(inter) = interaction {
            // Clear the temp so we don't re-process next frame.
            ctx.memory_mut(|mem| {
                mem.data.remove::<ScatterInteraction>(egui::Id::new(("scatter_interaction", id)));
            });

            use crate::state::selection::SelectionSet;
            let source_id = self.config.source_id;

            match inter {
                ScatterInteraction::Click { row, ctrl } => {
                    if ctrl {
                        // Toggle point in existing selection.
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
                        // Single click → select just this point.
                        event = PlotWindowEvent::SelectionChanged(
                            Some(SelectionSet::single(id, source_id, row))
                        );
                    }
                }
                ScatterInteraction::ClearSelection => {
                    if _selection.is_some() {
                        event = PlotWindowEvent::SelectionChanged(None);
                    }
                }
                ScatterInteraction::RightClick { row, screen_pos } => {
                    // Show context menu at the right-click position.
                    self.context_menu_row = Some(row);
                    self.context_menu_pos = Some(screen_pos);
                }
                ScatterInteraction::AreaSelect { rows, ctrl } => {
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

            let area_resp = egui::Area::new(egui::Id::new(("scatter_ctx_menu", id)))
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

// ── PlotWindowEvent ───────────────────────────────────────────────────────────

pub enum PlotWindowEvent {
    Open,
    Closed,
    ConfigChanged(PlotConfig),
    SelectionChanged(Option<crate::state::selection::SelectionSet>),
    FilterToSelection(crate::state::selection::SelectionSet),
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

// ── Computing overlay ────────────────────────────────────────────────────────

/// Shown inside a plot window while data is being computed on a background thread.
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

/// Pure function that does all the expensive data extraction + styling work.
/// Returns `None` if cancelled via `token`.
fn compute_scatter_data(
    plot_id: usize,
    config: &ScatterPlotConfig,
    schema: &DataSchema,
    df: &DataFrame,
    token: &CancelToken,
) -> Option<ScatterSyncResult> {
    puffin::profile_function!();
    let n = df.height();

    let (xs, x_labels_vec) = column_to_f64(df, &config.x_col, &config.x_scale);
    let (ys, y_labels_vec) = column_to_f64(df, &config.y_col, &config.y_scale);
    if token.is_cancelled() { return None; }

    let solid_color = Color32::from_rgb(100, 200, 255);
    let (mut all_colors, color_legend, all_cat_indices) = compute_colors(df, &config.color_mode, solid_color, n);
    if token.is_cancelled() { return None; }
    let (all_radii, size_legend) = compute_radii(df, config.size_config.as_ref(), 2.5, n);
    let base_alpha = 200.0 / 255.0;
    let (all_alphas, alpha_legend) = compute_alphas(df, config.alpha_config.as_ref(), base_alpha, n);
    apply_alpha(&mut all_colors, &all_alphas);
    if token.is_cancelled() { return None; }

    // Pre-extract hover field columns once.
    let hover_cols: Vec<(String, Vec<Option<String>>)> = config.hover_fields.iter()
        .filter_map(|field| {
            let series = df.column(field).ok()?.as_series()?.clone();
            let cast = series.cast(&DataType::String).ok()?;
            let ca = cast.str().ok()?.clone();
            let vals: Vec<Option<String>> = ca.into_iter().map(|v| v.map(|s| s.to_string())).collect();
            Some((field.clone(), vals))
        })
        .collect();

    // Read original row indices (injected by apply_filters_for_source).
    let orig_row_indices: Vec<usize> = df.column(crate::data::filter::ORIG_ROW_COL)
        .ok()
        .and_then(|c| c.as_series().map(|s| s.clone()))
        .and_then(|s| s.u64().ok().map(|ca| {
            ca.into_iter().map(|v| v.unwrap_or(0) as usize).collect()
        }))
        .unwrap_or_else(|| (0..n).collect());

    if token.is_cancelled() { return None; }

    let row_count = xs.len().min(ys.len());
    let mut pts: Vec<[f64; 2]> = Vec::new();
    let mut colors: Vec<Color32> = Vec::new();
    let mut radii: Vec<f32> = Vec::new();
    let mut hover_labels_vec: Vec<String> = Vec::new();
    let mut row_idx_vec: Vec<usize> = Vec::new();
    let mut cat_idx_vec: Vec<Option<usize>> = Vec::new();

    for i in 0..row_count {
        if i % 50_000 == 0 && token.is_cancelled() { return None; }
        if let (Some(x), Some(y)) = (xs[i], ys[i]) {
            pts.push([x, y]);
            colors.push(all_colors.get(i).copied().unwrap_or(solid_color));
            radii.push(all_radii.get(i).copied().unwrap_or(2.5));
            hover_labels_vec.push(build_hover_label(
                i, x, y,
                &x_labels_vec, &y_labels_vec,
                config,
                &hover_cols,
            ));
            row_idx_vec.push(orig_row_indices.get(i).copied().unwrap_or(i));
            cat_idx_vec.push(all_cat_indices.get(i).copied().flatten());
        }
    }

    let category_entries: Vec<(String, Color32)> = match &color_legend {
        ColorLegend::Categorical { entries, .. } => entries.clone(),
        _ => Vec::new(),
    };

    Some(ScatterSyncResult {
        plot_id,
        schema: schema.clone(),
        points: pts,
        colors,
        radii,
        hover_labels: hover_labels_vec,
        row_indices: row_idx_vec,
        category_entries,
        category_indices: cat_idx_vec,
        x_labels: x_labels_vec,
        y_labels: y_labels_vec,
        legend: PlotLegendData {
            plot_id,
            plot_title: config.title.clone(),
            color: color_legend,
            size: size_legend,
            alpha: alpha_legend,
        },
    })
}

// ── Scatter widget ────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn show_scatter(
    ui: &mut Ui,
    points: Arc<Vec<[f64; 2]>>,
    colors: Arc<Vec<Color32>>,
    radii: Arc<Vec<f32>>,
    hover_labels: Arc<Vec<String>>,
    row_indices: Arc<Vec<usize>>,
    category_entries: Arc<Vec<(String, Color32)>>,
    category_indices: Arc<Vec<Option<usize>>>,
    x_labels: Arc<Vec<String>>,
    y_labels: Arc<Vec<String>>,
    config: &ScatterPlotConfig,
    theme: &AppTheme,
    perf: &crate::state::perf_settings::PerformanceSettings,
    plot_id: usize,
    selection: Option<&crate::state::selection::SelectionSet>,
    context_menu_open: bool,
) {
    puffin::profile_function!();
    let n = points.len();
    if n == 0 {
        let c = &theme.colors;
        let s = &theme.spacing;
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No data").color(c.text_secondary).size(s.font_small).italics());
        });
        return;
    }

    let max_draw_points = perf.max_draw_points;
    let step = if max_draw_points > 0 && n > max_draw_points {
        (n / max_draw_points).max(1)
    } else {
        1
    };

    // Apply selection dimming: when a selection is active, unselected points get reduced alpha.
    // Cached by (plot_id, selection_version) to avoid re-allocating every frame.
    let has_selection = selection.map_or(false, |s| !s.is_empty());
    let colors = if has_selection {
        let sel = selection.unwrap();
        let sel_ver = sel.version();
        let cache_key = egui::Id::new(("scatter_dim_cache", plot_id));
        let cached: Option<(u64, Arc<Vec<Color32>>)> = ui.ctx().memory(|mem| mem.data.get_temp(cache_key));
        if let Some((ver, dimmed)) = cached {
            if ver == sel_ver && dimmed.len() == colors.len() {
                dimmed
            } else {
                let dimmed = Arc::new(compute_dimmed_colors(&colors, &row_indices, sel));
                ui.ctx().memory_mut(|mem| mem.data.insert_temp(cache_key, (sel_ver, Arc::clone(&dimmed))));
                dimmed
            }
        } else {
            let dimmed = Arc::new(compute_dimmed_colors(&colors, &row_indices, sel));
            ui.ctx().memory_mut(|mem| mem.data.insert_temp(cache_key, (sel_ver, Arc::clone(&dimmed))));
            dimmed
        }
    } else {
        colors
    };

    let is_categorical = !category_entries.is_empty();
    let is_continuous = matches!(config.color_mode, ColorMode::Continuous { .. });
    let use_painter = is_categorical || is_continuous;

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

    // label_formatter: used by Solid native series only.
    // Categorical and Continuous use painter-based manual hover.
    let hover_labels_fmt = Arc::clone(&hover_labels);
    let points_fmt = Arc::clone(&points);
    let step_fmt = step;
    let use_painter_fmt = use_painter;

    let mut plot = Plot::new(egui::Id::new(("scatter", plot_id)))
        .x_axis_label(&config.x_col as &str)
        .y_axis_label(&config.y_col as &str)
        .set_margin_fraction(egui::vec2(0.05, 0.05))
        .allow_zoom(true)
        .allow_drag(false)
        .allow_scroll(true)
        .allow_boxed_zoom(true)
        .auto_bounds(egui::Vec2b::new(!x_is_cat, !y_is_cat))
        .label_formatter(move |_name, val: &egui_plot::PlotPoint| {
            if use_painter_fmt || context_menu_open { return String::new(); }
            // Nearest-point lookup for Solid mode.
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

    // For painter mode (continuous + categorical): collect screen positions inside the closure.
    let sampled_count = if step > 1 { n / step } else { n };
    let mut screen_pts: Vec<(egui::Pos2, Color32, f32, usize)> = Vec::with_capacity(
        if use_painter { sampled_count } else { 0 }
    );

    // Read and consume any pending pan delta from shift+drag.
    let pan_key = egui::Id::new(("scatter_pan_delta", plot_id));
    let pending_pan: egui::Vec2 = ui.ctx().memory(|mem| mem.data.get_temp(pan_key).unwrap_or(egui::Vec2::ZERO));
    if pending_pan.length() > 0.001 {
        ui.ctx().memory_mut(|mem| { mem.data.remove::<egui::Vec2>(pan_key); });
    }

    let plot_response = plot.show(ui, |plot_ui: &mut egui_plot::PlotUi| {
        // Apply pending pan offset from shift+drag.
        if pending_pan.length() > 0.001 {
            let bounds = plot_ui.plot_bounds();
            let dx = pending_pan.x as f64;
            let dy = pending_pan.y as f64;
            plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                [bounds.min()[0] - dx, bounds.min()[1] - dy],
                [bounds.max()[0] - dx, bounds.max()[1] - dy],
            ));
        }

        if is_categorical || is_continuous {
            // ── Painter mode (categorical + continuous) ──────────────────────
            // Use a transparent bounds-only series for auto_bounds, then
            // collect screen positions for manual painting.  This is O(n)
            // regardless of how many categories exist.
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

    // ── Painter mode: paint circles and manual hover ───────────────────────────
    // Build spatial grid for O(1) nearest-point lookups (hover + click).
    let grid = if use_painter && !screen_pts.is_empty() {
        Some(SpatialGrid::build(&screen_pts, plot_response.response.rect, |p| p.0))
    } else {
        None
    };

    if use_painter && !screen_pts.is_empty() {
        let clip = plot_response.response.rect;
        let painter = ui.painter().with_clip_rect(clip);

        let use_batched = crate::plot::gpu_points::should_use_batched(
            perf.gpu_points_mode,
            perf.gpu_points_threshold,
            screen_pts.len(),
        );
        if use_batched {
            let mesh_shape = crate::plot::gpu_points::build_circle_mesh(
                screen_pts.iter().map(|&(pos, color, radius, _)| (pos, radius, color)),
                screen_pts.len(),
            );
            painter.add(mesh_shape);
        } else {
            for &(pos, color, radius, _) in &screen_pts {
                painter.circle_filled(pos, radius, color);
            }
        }

        // Manual hover tooltip (suppressed when context menu is open).
        if !context_menu_open && !ui.input(|i| i.pointer.secondary_down()) {
        if let Some(hpos) = plot_response.response.hover_pos() {
            let nearest = grid.as_ref().unwrap().find_nearest(hpos, &screen_pts, |p| p.0);
            if let Some((_, &(pos, _, radius, idx))) = nearest {
                if pos.distance(hpos) <= radius + 10.0 {
                    // Draw hover ring around nearest point (matching map plot style).
                    let clip = plot_response.response.rect;
                    let hover_painter = ui.painter().with_clip_rect(clip);
                    hover_painter.circle_stroke(pos, radius + 2.0, egui::Stroke::new(1.5, Color32::WHITE));

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

    // ── Selection highlight: draw rings around selected points ────────────────
    if has_selection {
        let sel = selection.unwrap();
        let clip = plot_response.response.rect;
        let painter = ui.painter().with_clip_rect(clip);

        if use_painter && !screen_pts.is_empty() {
            // We already have screen positions for continuous mode.
            for &(pos, _, radius, idx) in &screen_pts {
                let row = row_indices.get(idx).copied().unwrap_or(idx);
                if sel.contains(row) {
                    painter.circle_stroke(pos, radius + 2.0, egui::Stroke::new(1.5, Color32::WHITE));
                }
            }
        } else {
            // For solid/categorical: project selected points to screen space.
            let transform = plot_response.transform;
            for i in (0..n).step_by(step) {
                let row = row_indices.get(i).copied().unwrap_or(i);
                if sel.contains(row) {
                    let [x, y] = points[i];
                    let screen = transform.position_from_point(&PlotPoint::new(x, y));
                    let radius = radii.get(i).copied().unwrap_or(2.5);
                    painter.circle_stroke(screen, radius + 2.0, egui::Stroke::new(1.5, Color32::WHITE));
                }
            }
        }
    }

    // ── Click / Ctrl+click / Right-click interaction ─────────────────────────
    let response = &plot_response.response;
    let mut interaction: Option<ScatterInteraction> = None;

    // Find nearest point to pointer (for click and right-click).
    // In painter mode, reuse the spatial grid built during rendering.
    // In solid mode, fall back to linear scan (no screen_pts available).
    let find_nearest = |pos: egui::Pos2| -> Option<usize> {
        if let Some(ref g) = grid {
            // Use spatial grid for O(1) lookup.
            g.find_nearest(pos, &screen_pts, |p| p.0)
                .and_then(|(_, &(pt_pos, _, radius, idx))| {
                    if pt_pos.distance(pos) <= radius + 10.0 { Some(idx) } else { None }
                })
        } else {
            let transform = &plot_response.transform;
            let mut best_idx = None;
            let mut best_dist = f32::MAX;
            for i in (0..n).step_by(step) {
                let [x, y] = points[i];
                let screen = transform.position_from_point(&PlotPoint::new(x, y));
                let dist = screen.distance(pos);
                let radius = radii.get(i).copied().unwrap_or(2.5);
                if dist <= radius + 10.0 && dist < best_dist {
                    best_dist = dist;
                    best_idx = Some(i);
                }
            }
            best_idx
        }
    };

    // Primary click (left button released).
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if let Some(pt_idx) = find_nearest(pos) {
                let row = row_indices.get(pt_idx).copied().unwrap_or(pt_idx);
                let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                interaction = Some(ScatterInteraction::Click { row, ctrl });
            } else {
                // Clicked empty space → clear selection.
                interaction = Some(ScatterInteraction::ClearSelection);
            }
        }
    }

    // Secondary click (right-click).
    if response.secondary_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if let Some(pt_idx) = find_nearest(pos) {
                let row = row_indices.get(pt_idx).copied().unwrap_or(pt_idx);
                interaction = Some(ScatterInteraction::RightClick { row, screen_pos: pos });
            }
        }
    }

    // ── Drag interactions ──────────────────────────────────────────────────
    let drag_key = egui::Id::new(("scatter_drag_start", plot_id));
    let shift_held = ui.input(|i| i.modifiers.shift);

    // Shift+drag = pan (manual, since allow_drag is false).
    if shift_held && response.dragged_by(egui::PointerButton::Primary) {
        let delta = response.drag_delta();
        // Convert screen-space delta to plot-space delta using the transform.
        let transform = &plot_response.transform;
        let origin = transform.value_from_position(egui::pos2(0.0, 0.0));
        let shifted = transform.value_from_position(egui::pos2(delta.x, delta.y));
        let dx = shifted.x - origin.x;
        let dy = shifted.y - origin.y;
        // Store pan offset for next frame (accumulates across frames while dragging).
        let pan_key = egui::Id::new(("scatter_pan_delta", plot_id));
        let prev: egui::Vec2 = ui.ctx().memory(|mem| mem.data.get_temp(pan_key).unwrap_or(egui::Vec2::ZERO));
        ui.ctx().memory_mut(|mem| {
            mem.data.insert_temp(pan_key, prev + egui::vec2(dx as f32, dy as f32));
        });
    }

    // Plain drag (no shift) = area selection rectangle.
    // Record press position immediately on mouse-down so the rectangle starts
    // exactly where the user clicked (not after egui's drag-threshold delay).
    let press_key = egui::Id::new(("scatter_press_start", plot_id));
    if !shift_held && ui.input(|i| i.pointer.any_pressed()) {
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            if response.rect.contains(pos) {
                ui.ctx().memory_mut(|mem| {
                    mem.data.insert_temp::<Pos2>(press_key, pos);
                });
            }
        }
    }
    // Promote press to drag start once egui recognises the drag gesture.
    if !shift_held && response.dragged_by(egui::PointerButton::Primary) {
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
            // Draw selection rectangle.
            let sel_rect = Rect::from_two_pos(start, current);
            let painter = ui.painter().with_clip_rect(response.rect);
            painter.rect_filled(
                sel_rect,
                0.0,
                Color32::from_rgba_unmultiplied(100, 180, 255, 40),
            );
            painter.rect_stroke(
                sel_rect,
                0.0,
                egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 180, 255, 180)),
                egui::StrokeKind::Outside,
            );
        }

        // On release, collect all points inside the rectangle.
        if ui.input(|i| i.pointer.any_released()) {
            if let Some(end) = ui.input(|i| i.pointer.hover_pos()) {
                let sel_rect = Rect::from_two_pos(start, end);
                let transform = &plot_response.transform;
                let mut selected_rows: Vec<usize> = Vec::new();
                for i in (0..n).step_by(step) {
                    let [x, y] = points[i];
                    let screen = transform.position_from_point(&PlotPoint::new(x, y));
                    if sel_rect.contains(screen) {
                        let row = row_indices.get(i).copied().unwrap_or(i);
                        selected_rows.push(row);
                    }
                }
                let ctrl = ui.input(|i| i.modifiers.ctrl);
                interaction = Some(ScatterInteraction::AreaSelect { rows: selected_rows, ctrl });
            }
            // Clear drag start.
            ui.ctx().memory_mut(|mem| {
                mem.data.remove::<Pos2>(drag_key);
            });
        }
    }

    // Store interaction result for show_as_window to pick up.
    if let Some(inter) = interaction {
        ui.ctx().memory_mut(|mem| {
            mem.data.insert_temp(egui::Id::new(("scatter_interaction", plot_id)), inter);
        });
    }
}

/// Interaction events produced by show_scatter for show_as_window to consume.
#[derive(Clone, Debug)]
enum ScatterInteraction {
    Click { row: usize, ctrl: bool },
    ClearSelection,
    RightClick { row: usize, screen_pos: egui::Pos2 },
    /// Drag area selection completed. Contains row indices of all enclosed points.
    AreaSelect { rows: Vec<usize>, ctrl: bool },
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
    hover_cols: &[(String, Vec<Option<String>>)],
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
    let mut label_to_idx: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let values: Vec<Option<f64>> = ca.into_iter().map(|opt_s| {
        let s = opt_s?;
        let idx = if let Some(&pos) = label_to_idx.get(s) { pos }
        else { let pos = labels.len(); label_to_idx.insert(s.to_string(), pos); labels.push(s.to_string()); pos };
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

fn compute_dimmed_colors(
    colors: &[Color32],
    row_indices: &[usize],
    sel: &crate::state::selection::SelectionSet,
) -> Vec<Color32> {
    colors.iter().enumerate().map(|(i, &c)| {
        let row = row_indices.get(i).copied().unwrap_or(i);
        if sel.contains(row) {
            c
        } else {
            Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * 0.3) as u8)
        }
    }).collect()
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

