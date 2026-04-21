use crate::data::schema::DataSchema;
use crate::data::source::{DataSource, SourceId};
use crate::plot::plot_config::{AlphaConfig, ColorMode, Colormap, PlotConfig, ScrollChartConfig, Threshold};
use crate::plot::styling::CATEGORICAL_PALETTE;
use crate::plot::sync::{CancelToken, ScrollChartSyncResult};
use crate::state::app_state::DataEvent;
use crate::theme::AppTheme;
use crossbeam_channel::Sender;
use egui::{Color32, Context, Pos2, Rect, RichText, vec2};
use egui_plot::{HLine, Line, Plot, PlotPoints, VLine};
use polars::prelude::{DataFrame, DataType};

// ── ScrollChart ──────────────────────────────────────────────────────────────

pub struct ScrollChart {
    pub config: ScrollChartConfig,
    default_pos: Pos2,
    pending_snap: Option<Pos2>,

    // Sync state
    computing: bool,
    has_loaded: bool,
    cancel_token: Option<CancelToken>,

    // Cached schema
    schema: Option<DataSchema>,

    // Synced data
    times: Vec<f64>,
    series: Vec<(String, Vec<f64>)>,

    // Configure dialog
    config_dialog: ScrollChartConfigDialog,
}

impl ScrollChart {
    pub fn new(config: ScrollChartConfig, default_pos: Pos2) -> Self {
        Self {
            config,
            default_pos,
            pending_snap: None,
            computing: false,
            has_loaded: false,
            cancel_token: None,
            schema: None,
            times: Vec::new(),
            series: Vec::new(),
            config_dialog: ScrollChartConfigDialog::default(),
        }
    }

    pub fn plot_id(&self) -> usize {
        self.config.id
    }

    pub fn window_id(&self) -> egui::Id {
        egui::Id::new(format!("scroll_chart_{}", self.config.id))
    }

    pub fn intended_rect(&self, ctx: &Context) -> Option<Rect> {
        ctx.memory(|mem| mem.area_rect(self.window_id()))
    }

    pub fn set_pending_snap(&mut self, pos: Pos2) {
        self.pending_snap = Some(pos);
    }

    pub fn is_computing(&self) -> bool {
        self.computing
    }

    pub fn apply_config(&mut self, config: ScrollChartConfig) {
        self.config = config;
    }

    pub fn cancel_sync(&mut self) {
        self.computing = false;
    }

    pub fn apply_sync_result(&mut self, result: ScrollChartSyncResult) {
        self.schema = Some(result.schema);
        self.times = result.times;
        self.series = result.series;
        self.computing = false;
        self.has_loaded = true;
    }

    /// Kick off async data extraction for this chart.
    pub fn sync_data_async(&mut self, source: &DataSource, tx: &Sender<DataEvent>) {
        // Cancel any in-flight sync.
        if let Some(token) = &self.cancel_token {
            token.cancel();
        }

        self.computing = true;
        let token = CancelToken::new();
        self.cancel_token = Some(token.clone());

        let plot_id = self.config.id;
        let time_col = self.config.time_col.clone();
        let y_cols = self.config.y_cols.clone();
        let df = source.df.clone();
        let schema = source.schema.clone();
        let tx = tx.clone();

        std::thread::spawn(move || {
            if token.is_cancelled() {
                let _ = tx.send(DataEvent::PlotSyncReady(
                    crate::plot::sync::PlotSyncEvent::Cancelled { plot_id },
                ));
                return;
            }

            let result = extract_scroll_data(plot_id, &df, &schema, &time_col, &y_cols);

            if token.is_cancelled() {
                let _ = tx.send(DataEvent::PlotSyncReady(
                    crate::plot::sync::PlotSyncEvent::Cancelled { plot_id },
                ));
                return;
            }

            let _ = tx.send(DataEvent::PlotSyncReady(
                crate::plot::sync::PlotSyncEvent::ScrollChartReady(result),
            ));
        });
    }

    /// Render as an egui window. Returns a PlotWindowEvent.
    pub fn show_as_window(
        &mut self,
        ctx: &Context,
        theme: &AppTheme,
        central_rect: Rect,
        grid_size: f32,
    ) -> super::scatter_plot::PlotWindowEvent {
        use super::scatter_plot::PlotWindowEvent;

        let c = &theme.colors;
        let s = &theme.spacing;
        let mut event = PlotWindowEvent::Open;
        let mut open = true;

        let default_size = if self.config.vertical {
            vec2(300.0, (central_rect.height() * 0.7).max(350.0))
        } else {
            vec2((central_rect.width() * 0.48).max(300.0), 250.0)
        };

        let mut win = egui::Window::new(&self.config.title)
            .id(self.window_id())
            .open(&mut open)
            .default_size(default_size)
            .min_size(vec2(250.0, 150.0))
            .resizable(true)
            .collapsible(false)
            .frame(egui::Frame {
                fill: c.bg_panel,
                stroke: egui::Stroke::new(1.0, c.border),
                corner_radius: egui::CornerRadius::same(s.rounding as u8),
                inner_margin: egui::Margin::from(0.0_f32),
                ..Default::default()
            });

        if let Some(snap) = self.pending_snap.take() {
            win = win.current_pos(snap);
        }

        win.show(ctx, |ui| {
            // ── Toolbar ─────────────────────────────────────────
            egui::Frame::default()
                .fill(c.bg_panel)
                .inner_margin(egui::Margin::from(egui::vec2(8.0, 4.0)))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("⏱")
                                .color(c.accent_primary)
                                .size(s.font_small),
                        );
                        ui.label(
                            RichText::new("Scroll Chart")
                                .color(c.text_secondary)
                                .size(s.font_small),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Configure button
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("⚙").color(c.text_secondary).size(s.font_body),
                                    )
                                    .frame(false),
                                )
                                .clicked()
                            {
                                if let Some(schema) = &self.schema {
                                    self.config_dialog.open(&self.config, schema);
                                }
                            }
                        });
                    });
                });

            ui.separator();

            // ── Computing overlay ────────────────────────────────
            if self.computing && !self.has_loaded {
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(160));
                ui.put(rect, |ui: &mut egui::Ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(rect.height() * 0.3);
                        ui.label(
                            RichText::new("Computing plot data…")
                                .color(c.text_primary)
                                .size(s.font_body),
                        );
                        ui.spinner();
                    })
                    .response
                });
                return;
            }

            if self.times.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new("No data")
                            .color(c.text_secondary)
                            .size(s.font_small)
                            .italics(),
                    );
                });
                return;
            }

            // ── Plot ─────────────────────────────────────────────
            let time_max = self.times.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let time_min_window = time_max - self.config.window_secs;

            // Compute Y data range for clamping threshold polygons.
            let (y_data_min, y_data_max) = {
                let mut lo = f64::INFINITY;
                let mut hi = f64::NEG_INFINITY;
                for (_name, vals) in &self.series {
                    for (&t, &v) in self.times.iter().zip(vals.iter()) {
                        if t >= time_min_window && v.is_finite() {
                            if v < lo { lo = v; }
                            if v > hi { hi = v; }
                        }
                    }
                }
                for th in &self.config.thresholds {
                    if th.value < lo { lo = th.value; }
                    if th.value > hi { hi = th.value; }
                }
                let margin = (hi - lo).abs() * 0.15;
                if margin == 0.0 { (lo - 1.0, hi + 1.0) } else { (lo - margin, hi + margin) }
            };

            let vertical = self.config.vertical;
            let time_label = &self.config.time_col;
            let has_fixed_value_range = self.config.y_range.is_some();

            let mut plot = Plot::new(format!("scroll_chart_plot_{}", self.config.id))
                .allow_drag(true)
                .allow_zoom(true)
                .allow_scroll(true)
                .legend(egui_plot::Legend::default())
                .show_axes([true, true])
                // Disable auto_bounds — we force bounds each frame to keep scrolling.
                .auto_bounds(egui::Vec2b::new(false, false));

            if vertical {
                plot = plot.y_axis_label(time_label);
            } else {
                plot = plot.x_axis_label(time_label);
            }

            let line_width = self.config.line_width;
            let fixed_value_range = self.config.y_range;

            plot.show(ui, |plot_ui| {
                // Force the time axis to follow the latest data each frame.
                // This is what makes the chart "scroll."
                let current = plot_ui.plot_bounds();
                if vertical {
                    let (x_lo, x_hi) = if let Some((lo, hi)) = fixed_value_range {
                        (lo, hi)
                    } else {
                        (current.min()[0], current.max()[0])
                    };
                    plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                        [x_lo, time_min_window],
                        [x_hi, time_max],
                    ));
                } else {
                    let (y_lo, y_hi) = if let Some((lo, hi)) = fixed_value_range {
                        (lo, hi)
                    } else {
                        (current.min()[1], current.max()[1])
                    };
                    plot_ui.set_plot_bounds(egui_plot::PlotBounds::from_min_max(
                        [time_min_window, y_lo],
                        [time_max, y_hi],
                    ));
                }
                // Draw threshold regions as colored horizontal/vertical bands
                for threshold in &self.config.thresholds {
                    let above = Color32::from_rgba_unmultiplied(
                        threshold.above_color[0], threshold.above_color[1],
                        threshold.above_color[2], threshold.above_color[3],
                    );
                    let below = Color32::from_rgba_unmultiplied(
                        threshold.below_color[0], threshold.below_color[1],
                        threshold.below_color[2], threshold.below_color[3],
                    );

                    if vertical {
                        // Vertical: threshold is on X axis (value axis)
                        let above_poly = egui_plot::Polygon::new(
                            format!("above_{}", threshold.label),
                            PlotPoints::new(vec![
                                [threshold.value, time_min_window],
                                [y_data_max, time_min_window],
                                [y_data_max, time_max],
                                [threshold.value, time_max],
                            ]),
                        ).fill_color(above).stroke(egui::Stroke::NONE);
                        plot_ui.polygon(above_poly);

                        let below_poly = egui_plot::Polygon::new(
                            format!("below_{}", threshold.label),
                            PlotPoints::new(vec![
                                [y_data_min, time_min_window],
                                [threshold.value, time_min_window],
                                [threshold.value, time_max],
                                [y_data_min, time_max],
                            ]),
                        ).fill_color(below).stroke(egui::Stroke::NONE);
                        plot_ui.polygon(below_poly);

                        let label = if threshold.label.is_empty() {
                            format!("Threshold: {:.1}", threshold.value)
                        } else { threshold.label.clone() };
                        let line_color = Color32::from_rgba_unmultiplied(
                            threshold.above_color[0], threshold.above_color[1],
                            threshold.above_color[2], 180,
                        );
                        plot_ui.vline(
                            VLine::new(&label, threshold.value)
                                .color(line_color).width(1.5),
                        );
                    } else {
                        // Horizontal: threshold on Y axis
                        let above_poly = egui_plot::Polygon::new(
                            format!("above_{}", threshold.label),
                            PlotPoints::new(vec![
                                [time_min_window, threshold.value],
                                [time_max, threshold.value],
                                [time_max, y_data_max],
                                [time_min_window, y_data_max],
                            ]),
                        ).fill_color(above).stroke(egui::Stroke::NONE);
                        plot_ui.polygon(above_poly);

                        let below_poly = egui_plot::Polygon::new(
                            format!("below_{}", threshold.label),
                            PlotPoints::new(vec![
                                [time_min_window, y_data_min],
                                [time_max, y_data_min],
                                [time_max, threshold.value],
                                [time_min_window, threshold.value],
                            ]),
                        ).fill_color(below).stroke(egui::Stroke::NONE);
                        plot_ui.polygon(below_poly);

                        let label = if threshold.label.is_empty() {
                            format!("Threshold: {:.1}", threshold.value)
                        } else { threshold.label.clone() };
                        let line_color = Color32::from_rgba_unmultiplied(
                            threshold.above_color[0], threshold.above_color[1],
                            threshold.above_color[2], 180,
                        );
                        plot_ui.hline(
                            HLine::new(&label, threshold.value)
                                .color(line_color).width(1.5),
                        );
                    }
                }

                // Draw each Y series as a line
                for (idx, (col_name, values)) in self.series.iter().enumerate() {
                    let color = CATEGORICAL_PALETTE[idx % CATEGORICAL_PALETTE.len()];
                    let points: Vec<[f64; 2]> = self
                        .times
                        .iter()
                        .zip(values.iter())
                        .filter(|(t, _)| **t >= time_min_window)
                        .map(|(&t, &v)| {
                            if vertical { [v, t] } else { [t, v] }
                        })
                        .collect();

                    let line = Line::new(col_name.clone(), PlotPoints::new(points))
                        .color(color)
                        .width(line_width);
                    plot_ui.line(line);
                }
            });
        });

        if !open {
            return super::scatter_plot::PlotWindowEvent::Closed;
        }

        // Handle configure dialog
        if let Some(new_config) = self.config_dialog.show(ctx, theme) {
            event = super::scatter_plot::PlotWindowEvent::ConfigChanged(
                PlotConfig::ScrollChart(new_config),
            );
        }

        event
    }
}

// ── Data extraction ──────────────────────────────────────────────────────────

fn extract_scroll_data(
    plot_id: usize,
    df: &DataFrame,
    schema: &DataSchema,
    time_col: &str,
    y_cols: &[String],
) -> ScrollChartSyncResult {
    let n = df.height();

    // Extract time values
    let times = get_f64_col(df, time_col).unwrap_or_else(|| vec![0.0; n]);

    // Extract each Y series
    let series: Vec<(String, Vec<f64>)> = y_cols
        .iter()
        .filter_map(|col| {
            get_f64_col(df, col).map(|vals| (col.clone(), vals))
        })
        .collect();

    ScrollChartSyncResult {
        plot_id,
        schema: schema.clone(),
        times,
        series,
    }
}

fn get_f64_col(df: &DataFrame, col: &str) -> Option<Vec<f64>> {
    let series = df.column(col).ok()?.as_series()?.clone();
    let cast = series.cast(&DataType::Float64).ok()?;
    let ca = cast.f64().ok()?;
    Some(ca.into_iter().map(|v| v.unwrap_or(f64::NAN)).collect())
}

// ── Configure Dialog ─────────────────────────────────────────────────────────

struct ScrollChartConfigDialog {
    is_open: bool,
    draft_title: String,
    draft_time_col_idx: usize,
    draft_y_col_selected: Vec<bool>,
    draft_window_secs: f64,
    draft_thresholds: Vec<Threshold>,
    draft_line_width: f32,
    draft_vertical: bool,
    draft_y_range_enabled: bool,
    draft_y_range_min: f64,
    draft_y_range_max: f64,
    field_names: Vec<String>,
    all_field_names: Vec<String>,
    plot_id: usize,
    source_id: SourceId,
    // Color
    draft_color_variant: usize,
    draft_color_col_idx: usize,
    draft_colormap: Colormap,
    draft_color_reverse: bool,
    // Alpha
    draft_alpha_enabled: bool,
    draft_alpha_col_idx: usize,
    draft_alpha_min: f32,
    draft_alpha_max: f32,
}

impl Default for ScrollChartConfigDialog {
    fn default() -> Self {
        Self {
            is_open: false,
            draft_title: String::new(),
            draft_time_col_idx: 0,
            draft_y_col_selected: Vec::new(),
            draft_window_secs: 60.0,
            draft_thresholds: Vec::new(),
            draft_line_width: 2.0,
            draft_vertical: false,
            draft_y_range_enabled: false,
            draft_y_range_min: 0.0,
            draft_y_range_max: 100.0,
            field_names: Vec::new(),
            all_field_names: Vec::new(),
            plot_id: 0,
            source_id: 0,
            draft_color_variant: 0,
            draft_color_col_idx: 0,
            draft_colormap: Colormap::Viridis,
            draft_color_reverse: false,
            draft_alpha_enabled: false,
            draft_alpha_col_idx: 0,
            draft_alpha_min: 0.2,
            draft_alpha_max: 1.0,
        }
    }
}

impl ScrollChartConfigDialog {
    fn open(&mut self, config: &ScrollChartConfig, schema: &DataSchema) {
        self.is_open = true;
        self.draft_title = config.title.clone();
        self.plot_id = config.id;
        self.source_id = config.source_id;
        self.draft_window_secs = config.window_secs;
        self.draft_thresholds = config.thresholds.clone();
        self.draft_line_width = config.line_width;
        self.draft_vertical = config.vertical;

        if let Some((lo, hi)) = config.y_range {
            self.draft_y_range_enabled = true;
            self.draft_y_range_min = lo;
            self.draft_y_range_max = hi;
        } else {
            self.draft_y_range_enabled = false;
        }

        // Build numeric field list
        self.field_names = schema
            .fields
            .iter()
            .filter(|f| f.kind.is_numeric())
            .map(|f| f.name.clone())
            .collect();

        // All fields (for color-by-category)
        self.all_field_names = schema
            .fields
            .iter()
            .map(|f| f.name.clone())
            .collect();

        self.draft_time_col_idx = self
            .field_names
            .iter()
            .position(|f| f == &config.time_col)
            .unwrap_or(0);

        // Y column checkboxes
        self.draft_y_col_selected = self
            .field_names
            .iter()
            .map(|f| config.y_cols.contains(f))
            .collect();

        // Color mode
        self.draft_color_variant = config.color_mode.variant_idx();
        let color_col = match &config.color_mode {
            ColorMode::Categorical { col } | ColorMode::Continuous { col, .. } => col.as_str(),
            ColorMode::Solid => "",
        };
        self.draft_color_col_idx = self.all_field_names.iter().position(|f| f == color_col).unwrap_or(0);
        if let ColorMode::Continuous { colormap, reverse, .. } = &config.color_mode {
            self.draft_colormap = colormap.clone();
            self.draft_color_reverse = *reverse;
        }

        // Alpha
        if let Some(al) = &config.alpha_config {
            self.draft_alpha_enabled = true;
            self.draft_alpha_col_idx = self.field_names.iter().position(|f| f == &al.col).unwrap_or(0);
            self.draft_alpha_min = al.min_alpha;
            self.draft_alpha_max = al.max_alpha;
        } else {
            self.draft_alpha_enabled = false;
        }
    }

    fn show(&mut self, ctx: &Context, theme: &AppTheme) -> Option<ScrollChartConfig> {
        if !self.is_open {
            return None;
        }

        let c = &theme.colors;
        let s = &theme.spacing;
        let mut result: Option<ScrollChartConfig> = None;

        let screen = ctx.screen_rect();
        let default_pos = egui::pos2(
            (screen.center().x - 200.0).max(screen.min.x),
            (screen.center().y - 250.0).max(screen.min.y),
        );

        egui::Window::new(
            RichText::new("Configure Scroll Chart")
                .color(c.text_primary)
                .size(s.font_body + 1.0)
                .strong(),
        )
        .id(egui::Id::new(format!("scroll_cfg_{}", self.plot_id)))
        .collapsible(false)
        .resizable(true)
        .default_pos(default_pos)
        .default_width(400.0)
        .order(egui::Order::Foreground)
        .frame(egui::Frame {
            fill: c.bg_panel,
            stroke: egui::Stroke::new(1.0, c.border),
            corner_radius: egui::CornerRadius::from(6.0_f32),
            inner_margin: egui::Margin::from(16.0_f32),
            ..Default::default()
        })
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                // ── Title ────────────────────────────────────────────
                ui.label(RichText::new("Title").color(c.text_secondary).size(s.font_small));
                ui.add_space(2.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.draft_title)
                        .desired_width(ui.available_width())
                        .text_color(c.text_primary)
                        .font(egui::FontSelection::FontId(egui::FontId::proportional(s.font_body))),
                );
                ui.add_space(8.0);

                // ── Time column ──────────────────────────────────────
                ui.label(RichText::new("Time Column").color(c.text_secondary).size(s.font_small));
                ui.add_space(2.0);
                egui::ComboBox::from_id_salt(format!("scroll_time_{}", self.plot_id))
                    .selected_text(
                        self.field_names
                            .get(self.draft_time_col_idx)
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for (i, name) in self.field_names.iter().enumerate() {
                            ui.selectable_value(&mut self.draft_time_col_idx, i, name);
                        }
                    });
                ui.add_space(8.0);

                // ── Y columns ────────────────────────────────────────
                ui.label(RichText::new("Y Columns").color(c.text_secondary).size(s.font_small));
                ui.add_space(2.0);
                egui::ScrollArea::vertical()
                    .id_salt("scroll_y_cols")
                    .max_height(100.0)
                    .show(ui, |ui| {
                        for (i, name) in self.field_names.iter().enumerate() {
                            if i < self.draft_y_col_selected.len() {
                                ui.checkbox(
                                    &mut self.draft_y_col_selected[i],
                                    RichText::new(name).size(s.font_small).monospace(),
                                );
                            }
                        }
                    });
                ui.add_space(8.0);

                // ── Window & Orientation ──────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Window").color(c.text_secondary).size(s.font_small));
                    ui.add(
                        egui::DragValue::new(&mut self.draft_window_secs)
                            .range(1.0..=100_000.0)
                            .speed(1.0)
                            .suffix(" s"),
                    );
                    ui.add_space(16.0);
                    ui.checkbox(
                        &mut self.draft_vertical,
                        RichText::new("Vertical (time on Y)").color(c.text_secondary).size(s.font_small),
                    );
                });
                ui.add_space(8.0);

                // ── Line width ──────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Line Width").color(c.text_secondary).size(s.font_small));
                    ui.add(
                        egui::DragValue::new(&mut self.draft_line_width)
                            .range(0.5..=10.0)
                            .speed(0.1)
                            .suffix(" px"),
                    );
                });
                ui.add_space(8.0);

                // ── Fixed Y range ────────────────────────────────────
                ui.checkbox(
                    &mut self.draft_y_range_enabled,
                    RichText::new("Fixed value axis range").color(c.text_secondary).size(s.font_small),
                );
                if self.draft_y_range_enabled {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Min").color(c.text_secondary).size(s.font_small));
                        ui.add(egui::DragValue::new(&mut self.draft_y_range_min).speed(0.1));
                        ui.add_space(8.0);
                        ui.label(RichText::new("Max").color(c.text_secondary).size(s.font_small));
                        ui.add(egui::DragValue::new(&mut self.draft_y_range_max).speed(0.1));
                    });
                    if self.draft_y_range_min >= self.draft_y_range_max {
                        self.draft_y_range_max = self.draft_y_range_min + 1.0;
                    }
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);

                // ── Thresholds ───────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("THRESHOLDS")
                            .color(c.text_secondary)
                            .size(s.font_small)
                            .strong(),
                    );
                    if ui
                        .add(egui::Button::new(
                            RichText::new("＋").color(c.accent_primary).size(s.font_body),
                        ))
                        .clicked()
                    {
                        self.draft_thresholds.push(Threshold::default());
                    }
                });
                ui.add_space(4.0);

                let mut remove_idx: Option<usize> = None;
                for (i, threshold) in self.draft_thresholds.iter_mut().enumerate() {
                    egui::Frame::default()
                        .fill(c.bg_app)
                        .stroke(egui::Stroke::new(1.0, c.border))
                        .corner_radius(egui::CornerRadius::from(4.0_f32))
                        .inner_margin(egui::Margin::from(6.0_f32))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("#{}", i + 1))
                                        .color(c.text_secondary)
                                        .size(s.font_small),
                                );
                                ui.label(RichText::new("Value").color(c.text_secondary).size(s.font_small));
                                ui.add(egui::DragValue::new(&mut threshold.value).speed(0.1));
                                ui.label(RichText::new("Label").color(c.text_secondary).size(s.font_small));
                                ui.add(
                                    egui::TextEdit::singleline(&mut threshold.label)
                                        .desired_width(60.0),
                                );
                                if ui
                                    .add(egui::Button::new(
                                        RichText::new("✕").color(c.accent_warning).size(s.font_small),
                                    ).frame(false))
                                    .clicked()
                                {
                                    remove_idx = Some(i);
                                }
                            });
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Above").color(c.text_secondary).size(s.font_small));
                                let mut above = Color32::from_rgba_unmultiplied(
                                    threshold.above_color[0], threshold.above_color[1],
                                    threshold.above_color[2], threshold.above_color[3],
                                );
                                if ui.color_edit_button_srgba(&mut above).changed() {
                                    threshold.above_color = [above.r(), above.g(), above.b(), above.a()];
                                }
                                ui.add_space(8.0);
                                ui.label(RichText::new("Below").color(c.text_secondary).size(s.font_small));
                                let mut below = Color32::from_rgba_unmultiplied(
                                    threshold.below_color[0], threshold.below_color[1],
                                    threshold.below_color[2], threshold.below_color[3],
                                );
                                if ui.color_edit_button_srgba(&mut below).changed() {
                                    threshold.below_color = [below.r(), below.g(), below.b(), below.a()];
                                }
                            });
                        });
                    ui.add_space(2.0);
                }
                if let Some(idx) = remove_idx {
                    self.draft_thresholds.remove(idx);
                }

            }); // end ScrollArea

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            {
                // ── OK / Apply / Cancel ───────────────────────────────
                let can_apply = !self.field_names.is_empty() && !self.draft_title.trim().is_empty();

                let build_config = |s: &ScrollChartConfigDialog| -> ScrollChartConfig {
                    let time_col = s.field_names
                        .get(s.draft_time_col_idx)
                        .cloned()
                        .unwrap_or_default();

                    let y_cols: Vec<String> = s.field_names
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| {
                            s.draft_y_col_selected
                                .get(*i)
                                .copied()
                                .unwrap_or(false)
                        })
                        .map(|(_, name)| name.clone())
                        .collect();

                    let color_mode = match s.draft_color_variant {
                        1 => ColorMode::Categorical {
                            col: s.all_field_names.get(s.draft_color_col_idx)
                                .cloned().unwrap_or_default(),
                        },
                        2 => ColorMode::Continuous {
                            col: s.all_field_names.get(s.draft_color_col_idx)
                                .cloned().unwrap_or_default(),
                            colormap: s.draft_colormap.clone(),
                            color_min: None,
                            color_max: None,
                            reverse: s.draft_color_reverse,
                        },
                        _ => ColorMode::Solid,
                    };

                    let alpha_config = if s.draft_alpha_enabled {
                        Some(AlphaConfig {
                            col: s.field_names.get(s.draft_alpha_col_idx)
                                .cloned().unwrap_or_default(),
                            min_alpha: s.draft_alpha_min,
                            max_alpha: s.draft_alpha_max,
                        })
                    } else { None };

                    let y_range = if s.draft_y_range_enabled {
                        Some((s.draft_y_range_min, s.draft_y_range_max))
                    } else { None };

                    ScrollChartConfig {
                        id: s.plot_id,
                        title: s.draft_title.clone(),
                        source_id: s.source_id,
                        time_col,
                        y_cols,
                        window_secs: s.draft_window_secs,
                        thresholds: s.draft_thresholds.clone(),
                        color_mode,
                        line_width: s.draft_line_width,
                        alpha_config,
                        vertical: s.draft_vertical,
                        y_range,
                    }
                };

                ui.horizontal(|ui| {
                    let ok_btn = egui::Button::new(
                        RichText::new("OK")
                            .color(if can_apply { c.bg_app } else { c.text_secondary })
                            .size(s.font_body)
                            .strong(),
                    )
                    .fill(if can_apply { c.accent_primary } else { c.widget_bg })
                    .min_size(egui::vec2(70.0, 0.0));
                    if ui.add_enabled(can_apply, ok_btn).clicked() {
                        result = Some(build_config(self));
                        self.is_open = false;
                    }

                    ui.add_space(4.0);

                    let apply_btn = egui::Button::new(
                        RichText::new("Apply")
                            .color(if can_apply { c.text_primary } else { c.text_secondary })
                            .size(s.font_body),
                    )
                    .min_size(egui::vec2(70.0, 0.0));
                    if ui.add_enabled(can_apply, apply_btn).clicked() {
                        result = Some(build_config(self));
                    }

                    ui.add_space(4.0);

                    if ui.button(RichText::new("Cancel").color(c.text_secondary).size(s.font_body)).clicked() {
                        self.is_open = false;
                    }
                });
            }
        });

        result
    }
}
