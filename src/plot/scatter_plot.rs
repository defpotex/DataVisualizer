use crate::data::schema::DataSchema;
use crate::data::source::DataSource;
use crate::plot::map_plot::snap_to_grid;
use crate::plot::plot_config::{AxisScale, PlotConfig, ScatterPlotConfig};
use crate::theme::AppTheme;
use egui::{Color32, Context, Pos2, Rect, RichText, Ui, vec2};
use egui_plot::{Plot, PlotBounds, PlotPoints, Points};
use polars::prelude::{DataFrame, DataType};
use std::sync::Arc;

// ── Configure dialog state ────────────────────────────────────────────────────

struct ScatterConfigDialog {
    is_open: bool,
    draft_title: String,
    draft_x_idx: usize,
    draft_y_idx: usize,
    draft_x_scale: AxisScale,
    draft_y_scale: AxisScale,
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
    }

    /// Returns `Some(ScatterPlotConfig)` when user confirms. `None` = still open or cancelled.
    fn show(
        &mut self,
        ctx: &Context,
        config: &ScatterPlotConfig,
        schema: &DataSchema,
        theme: &AppTheme,
    ) -> Option<ScatterPlotConfig> {
        if !self.is_open {
            return None;
        }

        let c = &theme.colors;
        let s = &theme.spacing;
        let mut result = None;
        let mut close = false;

        let fields = &schema.fields;
        if self.draft_x_idx >= fields.len() { self.draft_x_idx = 0; }
        if self.draft_y_idx >= fields.len() { self.draft_y_idx = 0; }

        egui::Window::new(
            RichText::new("Configure Scatter Plot")
                .color(c.text_primary)
                .size(s.font_body + 1.0)
                .strong(),
        )
        .id(egui::Id::new(("scatter_cfg_dlg", config.id)))
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

            // ── X axis ────────────────────────────────────────────────────────
            ui.label(RichText::new("X Axis Column").color(c.text_secondary).size(s.font_small));
            ui.add_space(2.0);
            field_combo(ui, "cfg_x_col", fields, &mut self.draft_x_idx, theme);
            ui.add_space(4.0);
            // Auto-inferred scale for selected X field
            let x_inferred = fields.get(self.draft_x_idx)
                .map(|f| AxisScale::infer(&f.kind))
                .unwrap_or(AxisScale::Continuous);
            scale_toggle(ui, "cfg_x_scale", &mut self.draft_x_scale, &x_inferred, theme);

            ui.add_space(10.0);

            // ── Y axis ────────────────────────────────────────────────────────
            ui.label(RichText::new("Y Axis Column").color(c.text_secondary).size(s.font_small));
            ui.add_space(2.0);
            field_combo(ui, "cfg_y_col", fields, &mut self.draft_y_idx, theme);
            ui.add_space(4.0);
            let y_inferred = fields.get(self.draft_y_idx)
                .map(|f| AxisScale::infer(&f.kind))
                .unwrap_or(AxisScale::Continuous);
            scale_toggle(ui, "cfg_y_scale", &mut self.draft_y_scale, &y_inferred, theme);

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
                    result = Some(ScatterPlotConfig {
                        id: config.id,
                        title: self.draft_title.trim().to_string(),
                        source_id: config.source_id,
                        x_col,
                        y_col,
                        color_col: config.color_col.clone(),
                        x_scale: self.draft_x_scale.clone(),
                        y_scale: self.draft_y_scale.clone(),
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

// ── ScatterPlot ───────────────────────────────────────────────────────────────

pub struct ScatterPlot {
    pub config: ScatterPlotConfig,
    is_open: bool,
    default_pos: Pos2,
    pending_snap: Option<Pos2>,

    /// Cached (x, y) pairs — categoricals encoded as integer indices.
    points: Arc<Vec<[f64; 2]>>,
    /// Labels for categorical X axis (empty if continuous).
    x_labels: Arc<Vec<String>>,
    /// Labels for categorical Y axis (empty if continuous).
    y_labels: Arc<Vec<String>>,
    /// Schema cached at sync time so the configure dialog can list columns.
    cached_schema: Option<DataSchema>,

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
            x_labels: Arc::new(Vec::new()),
            y_labels: Arc::new(Vec::new()),
            cached_schema: None,
            configure_dialog: ScatterConfigDialog::default(),
        }
    }

    pub fn sync_data(&mut self, source: &DataSource) {
        self.cached_schema = Some(source.schema.clone());
        let (pts, xl, yl) = extract_xy_aware(
            &source.df,
            &self.config.x_col,
            &self.config.y_col,
            &self.config.x_scale,
            &self.config.y_scale,
        );
        self.points = Arc::new(pts);
        self.x_labels = Arc::new(xl);
        self.y_labels = Arc::new(yl);
    }

    pub fn apply_config(&mut self, config: ScatterPlotConfig) {
        self.config = config;
    }

    pub fn plot_id(&self) -> usize { self.config.id }

    pub fn window_id(&self) -> egui::Id {
        egui::Id::new(("scatter_plot_win", self.config.id))
    }

    pub fn set_pending_snap(&mut self, pos: Pos2) {
        self.pending_snap = Some(pos);
    }

    pub fn intended_rect(&self, ctx: &Context) -> Option<Rect> {
        let state = egui::AreaState::load(ctx, self.window_id())?;
        let size = state.size?;
        let pos = self.pending_snap.unwrap_or_else(|| state.left_top_pos());
        Some(Rect::from_min_size(pos, size))
    }

    /// Render as a floating egui Window.
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
        .min_size([280.0, 180.0])
        .constrain_to(central_rect)
        .frame(window_frame);

        if let Some(pos) = snap {
            win = win.current_pos(pos);
        }

        win.show(ctx, |ui| {
            ui.push_id(id, |ui| {
                gear_clicked = show_toolbar(ui, &self.config, self.points.len(), theme);
                ui.separator();
                show_scatter(
                    ui,
                    Arc::clone(&self.points),
                    Arc::clone(&self.x_labels),
                    Arc::clone(&self.y_labels),
                    &self.config,
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

        // Show configure dialog and handle result.
        let mut event = if is_open { PlotWindowEvent::Open } else { PlotWindowEvent::Closed };

        if let Some(schema) = &self.cached_schema.clone() {
            if let Some(new_config) = self.configure_dialog.show(ctx, &self.config, schema, theme) {
                event = PlotWindowEvent::ConfigChanged(PlotConfig::Scatter(new_config));
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

// ── Return event ──────────────────────────────────────────────────────────────

/// What happened during one frame of a plot window render.
pub enum PlotWindowEvent {
    Open,
    Closed,
    ConfigChanged(PlotConfig),
}

// ── Toolbar ───────────────────────────────────────────────────────────────────

/// Returns `true` if the gear (⚙) button was clicked.
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
                        RichText::new(format!("y: {}  x: {}", config.y_col, config.x_col))
                            .color(c.text_secondary)
                            .size(s.font_small)
                            .monospace(),
                    );
                });
            });
        });

    gear_clicked
}

// ── Scatter widget ────────────────────────────────────────────────────────────

fn show_scatter(
    ui: &mut Ui,
    points: Arc<Vec<[f64; 2]>>,
    x_labels: Arc<Vec<String>>,
    y_labels: Arc<Vec<String>>,
    config: &ScatterPlotConfig,
    theme: &AppTheme,
    max_draw_points: usize,
) {
    let c = &theme.colors;

    let n = points.len();
    let step = if max_draw_points > 0 && n > max_draw_points {
        (n / max_draw_points).max(1)
    } else {
        1
    };

    let sampled: PlotPoints = points
        .iter()
        .step_by(step)
        .map(|[x, y]| [*x, *y])
        .collect();

    let dot_color = Color32::from_rgba_unmultiplied(
        c.accent_secondary.r(),
        c.accent_secondary.g(),
        c.accent_secondary.b(),
        200,
    );

    let scatter_points = Points::new("data", sampled)
        .color(dot_color)
        .radius(2.5)
        .shape(egui_plot::MarkerShape::Circle);

    // Build axis formatters for categorical axes.
    let xl_fmt = Arc::clone(&x_labels);
    let yl_fmt = Arc::clone(&y_labels);
    let x_is_cat = config.x_scale == AxisScale::Categorical && !x_labels.is_empty();
    let y_is_cat = config.y_scale == AxisScale::Categorical && !y_labels.is_empty();

    let x_len = x_labels.len();
    let y_len = y_labels.len();

    let xl_hover = Arc::clone(&x_labels);
    let yl_hover = Arc::clone(&y_labels);
    let x_is_cat2 = x_is_cat;
    let y_is_cat2 = y_is_cat;

    let mut plot = Plot::new(egui::Id::new(("scatter", config.id)))
        .x_axis_label(&config.x_col as &str)
        .y_axis_label(&config.y_col as &str)
        .set_margin_fraction(egui::vec2(0.05, 0.05))
        .allow_zoom(true)
        .allow_drag(true)
        .allow_scroll(true)
        .allow_boxed_zoom(true)
        .auto_bounds(egui::Vec2b::new(!x_is_cat, !y_is_cat))
        .label_formatter(move |_name, val| {
            let x_str = if x_is_cat2 {
                xl_hover.get(val.x.round() as usize)
                    .cloned()
                    .unwrap_or_else(|| format!("{:.0}", val.x))
            } else {
                format!("{:.4}", val.x)
            };
            let y_str = if y_is_cat2 {
                yl_hover.get(val.y.round() as usize)
                    .cloned()
                    .unwrap_or_else(|| format!("{:.0}", val.y))
            } else {
                format!("{:.4}", val.y)
            };
            format!("x: {x_str}\ny: {y_str}")
        });

    // Categorical X: integer-spaced ticks with string labels, fixed bounds.
    if x_is_cat {
        plot = plot
            .x_grid_spacer(egui_plot::uniform_grid_spacer(move |_| [1.0, 1.0, 1.0]))
            .x_axis_formatter(move |mark, _range| {
                let idx = mark.value.round() as usize;
                xl_fmt.get(idx).cloned().unwrap_or_default()
            });
    } else {
        plot = plot.x_grid_spacer(egui_plot::log_grid_spacer(10));
    }

    // Categorical Y: integer-spaced ticks with string labels, fixed bounds.
    if y_is_cat {
        plot = plot
            .y_grid_spacer(egui_plot::uniform_grid_spacer(move |_| [1.0, 1.0, 1.0]))
            .y_axis_formatter(move |mark, _range| {
                let idx = mark.value.round() as usize;
                yl_fmt.get(idx).cloned().unwrap_or_default()
            });
    } else {
        plot = plot.y_grid_spacer(egui_plot::log_grid_spacer(10));
    }

    plot.show(ui, |plot_ui: &mut egui_plot::PlotUi| {
        plot_ui.points(scatter_points);

        // Set explicit bounds for categorical axes so they span all labels.
        let current = plot_ui.plot_bounds();
        let xmin = if x_is_cat { -0.5 } else { current.min()[0] };
        let xmax = if x_is_cat { x_len as f64 - 0.5 } else { current.max()[0] };
        let ymin = if y_is_cat { -0.5 } else { current.min()[1] };
        let ymax = if y_is_cat { y_len as f64 - 0.5 } else { current.max()[1] };
        if x_is_cat || y_is_cat {
            plot_ui.set_plot_bounds(PlotBounds::from_min_max([xmin, ymin], [xmax, ymax]));
        }
    });
}

// ── Data extraction ───────────────────────────────────────────────────────────

/// Extract (x, y) point pairs from a DataFrame, handling both continuous and categorical columns.
/// Returns (points, x_labels, y_labels) — labels are empty for continuous axes.
fn extract_xy_aware(
    df: &DataFrame,
    x_col: &str,
    y_col: &str,
    x_scale: &AxisScale,
    y_scale: &AxisScale,
) -> (Vec<[f64; 2]>, Vec<String>, Vec<String>) {
    let (xs, x_labels) = column_to_f64(df, x_col, x_scale);
    let (ys, y_labels) = column_to_f64(df, y_col, y_scale);

    let pts = xs.into_iter()
        .zip(ys)
        .filter_map(|(x, y)| Some([x?, y?]))
        .collect();

    (pts, x_labels, y_labels)
}

/// Convert a column to `Vec<Option<f64>>`.
/// For categorical columns, encodes values as integer indices and returns the label list.
/// For continuous columns, casts to f64 directly.
fn column_to_f64(df: &DataFrame, col: &str, scale: &AxisScale) -> (Vec<Option<f64>>, Vec<String>) {
    match scale {
        AxisScale::Continuous => {
            let vals = get_f64_vec(df, col).unwrap_or_default();
            (vals, Vec::new())
        }
        AxisScale::Categorical => column_to_categorical(df, col),
    }
}

fn get_f64_vec(df: &DataFrame, col: &str) -> Option<Vec<Option<f64>>> {
    let series = df.column(col).ok()?.as_series()?.clone();
    let cast = series.cast(&DataType::Float64).ok()?;
    let ca = cast.f64().ok()?;
    Some(ca.into_iter().collect())
}

/// Encode a column as ordered categorical indices.
/// Returns (index_values, label_list) where `label_list[i]` is the string for index i.
fn column_to_categorical(df: &DataFrame, col: &str) -> (Vec<Option<f64>>, Vec<String>) {
    let series = match df.column(col).ok().and_then(|c| c.as_series()).map(|s| s.clone()) {
        Some(s) => s,
        None => return (Vec::new(), Vec::new()),
    };

    // Cast everything to string for uniform handling (covers bool, int, float, text).
    let cast = series.cast(&DataType::String).unwrap_or(series);
    let ca = match cast.str() {
        Ok(c) => c.clone(),
        Err(_) => return (Vec::new(), Vec::new()),
    };

    let mut labels: Vec<String> = Vec::new();
    let values: Vec<Option<f64>> = ca.into_iter().map(|opt_s| {
        let s = opt_s?;
        let idx = if let Some(pos) = labels.iter().position(|l| l == s) {
            pos
        } else {
            let pos = labels.len();
            labels.push(s.to_string());
            pos
        };
        Some(idx as f64)
    }).collect();

    (values, labels)
}

// ── Dialog helpers ────────────────────────────────────────────────────────────

/// Combo box for picking a field by index from a schema.
fn field_combo(
    ui: &mut Ui,
    id: &str,
    fields: &[crate::data::schema::FieldMeta],
    idx: &mut usize,
    theme: &AppTheme,
) {
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
                ui.selectable_value(
                    idx, i,
                    RichText::new(&label).color(c.text_data).size(s.font_body).monospace(),
                );
            }
        });
}

/// Two-button toggle: Continuous / Categorical, with inferred default shown.
fn scale_toggle(
    ui: &mut Ui,
    id: &str,
    scale: &mut AxisScale,
    inferred: &AxisScale,
    theme: &AppTheme,
) {
    let c = &theme.colors;
    let s = &theme.spacing;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Scale:").color(c.text_secondary).size(s.font_small));
        for variant in [AxisScale::Continuous, AxisScale::Categorical] {
            let is_selected = *scale == variant;
            let is_inferred = variant == *inferred;
            let label = if is_inferred {
                format!("{} (auto)", variant.label())
            } else {
                variant.label().to_string()
            };
            let btn = egui::Button::new(
                RichText::new(&label)
                    .color(if is_selected { c.bg_app } else { c.text_secondary })
                    .size(s.font_small),
            )
            .fill(if is_selected { c.accent_secondary } else { c.widget_bg })
            .stroke(egui::Stroke::new(1.0, if is_selected { c.accent_secondary } else { c.border }))
            .min_size(egui::vec2(0.0, 0.0));

            let response = ui.add(btn);
            // Use unique ID per call to avoid collision detection across multiple toggles.
            let _ = id; // id is embedded in the enclosing ComboBox call; not needed here
            if response.clicked() {
                *scale = variant;
            }
        }
    });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn format_count(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { result.push(','); }
        result.push(ch);
    }
    result.chars().rev().collect()
}
