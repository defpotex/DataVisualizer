use crate::data::schema::DataSchema;
use crate::data::source::{DataSource, SourceId};
use crate::plot::plot_config::{PlotConfig, ScrollChartConfig, Threshold};
use crate::plot::styling::CATEGORICAL_PALETTE;
use crate::plot::sync::{CancelToken, ScrollChartSyncResult};
use crate::state::app_state::DataEvent;
use crate::theme::AppTheme;
use crossbeam_channel::Sender;
use egui::{Color32, Context, Pos2, Rect, RichText, vec2};
use egui_plot::{HLine, Line, Plot, PlotPoints};
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

        let default_size = vec2(
            (central_rect.width() * 0.48).max(300.0),
            250.0,
        );

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

            // Compute Y data range for clamping threshold polygons (avoids ±infinity feedback loop).
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
                // Include threshold values in range so they're visible.
                for th in &self.config.thresholds {
                    if th.value < lo { lo = th.value; }
                    if th.value > hi { hi = th.value; }
                }
                let margin = (hi - lo).abs() * 0.15;
                if margin == 0.0 { (lo - 1.0, hi + 1.0) } else { (lo - margin, hi + margin) }
            };

            let plot = Plot::new(format!("scroll_chart_plot_{}", self.config.id))
                .x_axis_label(&self.config.time_col)
                .allow_drag(true)
                .allow_zoom(true)
                .allow_scroll(true)
                .legend(egui_plot::Legend::default())
                .show_axes([true, true])
                .include_x(time_min_window)
                .include_x(time_max);

            plot.show(ui, |plot_ui| {
                // Draw threshold regions as colored horizontal bands
                for threshold in &self.config.thresholds {
                    // Above threshold — draw filled polygon
                    let above = Color32::from_rgba_unmultiplied(
                        threshold.above_color[0],
                        threshold.above_color[1],
                        threshold.above_color[2],
                        threshold.above_color[3],
                    );
                    let below = Color32::from_rgba_unmultiplied(
                        threshold.below_color[0],
                        threshold.below_color[1],
                        threshold.below_color[2],
                        threshold.below_color[3],
                    );

                    // Above region: threshold → top of data range (clamped)
                    let above_poly = egui_plot::Polygon::new(
                        format!("above_{}", threshold.label),
                        PlotPoints::new(vec![
                            [time_min_window, threshold.value],
                            [time_max, threshold.value],
                            [time_max, y_data_max],
                            [time_min_window, y_data_max],
                        ]),
                    )
                    .fill_color(above)
                    .stroke(egui::Stroke::NONE);
                    plot_ui.polygon(above_poly);

                    // Below region: bottom of data range → threshold (clamped)
                    let below_poly = egui_plot::Polygon::new(
                        format!("below_{}", threshold.label),
                        PlotPoints::new(vec![
                            [time_min_window, y_data_min],
                            [time_max, y_data_min],
                            [time_max, threshold.value],
                            [time_min_window, threshold.value],
                        ]),
                    )
                    .fill_color(below)
                    .stroke(egui::Stroke::NONE);
                    plot_ui.polygon(below_poly);

                    // Threshold line
                    let line_color = Color32::from_rgba_unmultiplied(
                        threshold.above_color[0],
                        threshold.above_color[1],
                        threshold.above_color[2],
                        180,
                    );
                    let label = if threshold.label.is_empty() {
                        format!("Threshold: {:.1}", threshold.value)
                    } else {
                        threshold.label.clone()
                    };
                    plot_ui.hline(
                        HLine::new(&label, threshold.value)
                            .color(line_color)
                            .width(1.5),
                    );
                }

                // Draw each Y series as a line
                for (idx, (col_name, values)) in self.series.iter().enumerate() {
                    let color = CATEGORICAL_PALETTE[idx % CATEGORICAL_PALETTE.len()];
                    let points: Vec<[f64; 2]> = self
                        .times
                        .iter()
                        .zip(values.iter())
                        .filter(|(t, _)| **t >= time_min_window)
                        .map(|(&t, &v)| [t, v])
                        .collect();

                    let line = Line::new(col_name.clone(), PlotPoints::new(points))
                        .color(color)
                        .width(2.0);
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
    field_names: Vec<String>,
    plot_id: usize,
    source_id: SourceId,
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
            field_names: Vec::new(),
            plot_id: 0,
            source_id: 0,
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

        // Build numeric field list
        self.field_names = schema
            .fields
            .iter()
            .filter(|f| f.kind.is_numeric())
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
    }

    fn show(&mut self, ctx: &Context, theme: &AppTheme) -> Option<ScrollChartConfig> {
        if !self.is_open {
            return None;
        }

        let c = &theme.colors;
        let s = &theme.spacing;
        let mut result: Option<ScrollChartConfig> = None;

        egui::Window::new("Configure Scroll Chart")
            .id(egui::Id::new(format!("scroll_cfg_{}", self.plot_id)))
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                // Title
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Title").color(c.text_primary).size(s.font_body));
                    ui.text_edit_singleline(&mut self.draft_title);
                });
                ui.add_space(6.0);

                // Time column
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Time Column").color(c.text_primary).size(s.font_body));
                    egui::ComboBox::from_id_salt(format!("scroll_time_{}", self.plot_id))
                        .selected_text(
                            self.field_names
                                .get(self.draft_time_col_idx)
                                .cloned()
                                .unwrap_or_default(),
                        )
                        .show_ui(ui, |ui| {
                            for (i, name) in self.field_names.iter().enumerate() {
                                ui.selectable_value(&mut self.draft_time_col_idx, i, name);
                            }
                        });
                });
                ui.add_space(6.0);

                // Y columns
                ui.label(RichText::new("Y Columns").color(c.text_primary).size(s.font_body));
                egui::ScrollArea::vertical()
                    .max_height(120.0)
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
                ui.add_space(6.0);

                // Window size
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Window").color(c.text_primary).size(s.font_body));
                    ui.add(
                        egui::DragValue::new(&mut self.draft_window_secs)
                            .range(1.0..=100_000.0)
                            .speed(1.0)
                            .suffix(" s"),
                    );
                });
                ui.add_space(6.0);

                // Thresholds
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Thresholds")
                            .color(c.text_primary)
                            .size(s.font_body)
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
                }
                if let Some(idx) = remove_idx {
                    self.draft_thresholds.remove(idx);
                }

                ui.add_space(8.0);

                // Buttons
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(
                            RichText::new("Apply").color(c.accent_primary).size(s.font_body),
                        ))
                        .clicked()
                    {
                        let time_col = self
                            .field_names
                            .get(self.draft_time_col_idx)
                            .cloned()
                            .unwrap_or_default();

                        let y_cols: Vec<String> = self
                            .field_names
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| {
                                self.draft_y_col_selected
                                    .get(*i)
                                    .copied()
                                    .unwrap_or(false)
                            })
                            .map(|(_, name)| name.clone())
                            .collect();

                        result = Some(ScrollChartConfig {
                            id: self.plot_id,
                            title: self.draft_title.clone(),
                            source_id: self.source_id,
                            time_col,
                            y_cols,
                            window_secs: self.draft_window_secs,
                            thresholds: self.draft_thresholds.clone(),
                        });
                        self.is_open = false;
                    }
                    if ui
                        .add(egui::Button::new(
                            RichText::new("Cancel").color(c.text_secondary).size(s.font_body),
                        ))
                        .clicked()
                    {
                        self.is_open = false;
                    }
                });
            });

        result
    }
}
