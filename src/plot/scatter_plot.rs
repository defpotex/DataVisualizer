use crate::data::source::DataSource;
use crate::plot::map_plot::snap_to_grid;
use crate::plot::plot_config::ScatterPlotConfig;
use crate::theme::AppTheme;
use egui::{Color32, Context, Pos2, Rect, RichText, Ui, vec2};
use egui_plot::{Plot, PlotBounds, PlotPoints, Points};
use polars::prelude::DataType;
use std::sync::Arc;

pub struct ScatterPlot {
    pub config: ScatterPlotConfig,
    is_open: bool,
    default_pos: Pos2,
    pending_snap: Option<Pos2>,
    /// Cached screen-ready points (x, y pairs).
    points: Arc<Vec<[f64; 2]>>,
}

impl ScatterPlot {
    pub fn new(config: ScatterPlotConfig, default_pos: Pos2) -> Self {
        Self {
            config,
            is_open: true,
            default_pos,
            pending_snap: None,
            points: Arc::new(Vec::new()),
        }
    }

    pub fn sync_data(&mut self, source: &DataSource) {
        self.points = Arc::new(
            extract_xy(&source.df, &self.config.x_col, &self.config.y_col)
        );
    }

    pub fn plot_id(&self) -> usize {
        self.config.id
    }

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

    /// Render as a floating egui Window. Returns false if the user closed it.
    pub fn show_as_window(
        &mut self,
        ctx: &Context,
        theme: &AppTheme,
        central_rect: Rect,
        grid_size: f32,
        max_draw_points: usize,
    ) -> bool {
        if !self.is_open { return false; }

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
                show_toolbar(ui, &self.config, self.points.len(), theme);
                ui.separator();
                show_scatter(ui, Arc::clone(&self.points), &self.config, theme, max_draw_points);
            });
        });

        self.is_open = is_open;

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

        self.is_open
    }
}

// ── Toolbar ───────────────────────────────────────────────────────────────────

fn show_toolbar(ui: &mut Ui, config: &ScatterPlotConfig, point_count: usize, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;

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
                    ui.label(
                        RichText::new(format!("y: {}  x: {}", config.y_col, config.x_col))
                            .color(c.text_secondary)
                            .size(s.font_small)
                            .monospace(),
                    );
                });
            });
        });
}

// ── Scatter widget ────────────────────────────────────────────────────────────

fn show_scatter(
    ui: &mut Ui,
    points: Arc<Vec<[f64; 2]>>,
    config: &ScatterPlotConfig,
    theme: &AppTheme,
    max_draw_points: usize,
) {
    let c = &theme.colors;
    let s = &theme.spacing;

    let n = points.len();
    let step = if max_draw_points > 0 { (n / max_draw_points).max(1) } else { 1 };

    // Build PlotPoints from strided sample.
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

    // Style the plot frame to match the dark theme.
    let plot_bg = c.bg_plot;
    let grid_color = Color32::from_rgba_unmultiplied(
        c.border.r(), c.border.g(), c.border.b(), 120,
    );

    Plot::new(egui::Id::new(("scatter", config.id)))
        .x_axis_label(RichText::new(&config.x_col).color(c.text_secondary).size(s.font_small))
        .y_axis_label(RichText::new(&config.y_col).color(c.text_secondary).size(s.font_small))
        .set_margin_fraction(egui::vec2(0.05, 0.05))
        .allow_zoom(true)
        .allow_drag(true)
        .allow_scroll(true)
        .allow_boxed_zoom(true)
        .auto_bounds(egui::Vec2b::new(true, true))
        .background_color(plot_bg)
        .x_grid_spacer(egui_plot::log_grid_spacer(10))
        .y_grid_spacer(egui_plot::log_grid_spacer(10))
        .x_axis_color(c.text_secondary)
        .y_axis_color(c.text_secondary)
        .grid_stroke(egui::Stroke::new(0.5, grid_color))
        .label_formatter(move |_name, val| {
            format!("x: {:.4}\ny: {:.4}", val.x, val.y)
        })
        .show(ui, |plot_ui| {
            plot_ui.points(scatter_points);
        });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn extract_xy(df: &polars::prelude::DataFrame, x_col: &str, y_col: &str) -> Vec<[f64; 2]> {
    let Some(xs) = get_f64_vec(df, x_col) else { return vec![]; };
    let Some(ys) = get_f64_vec(df, y_col) else { return vec![]; };
    xs.into_iter()
        .zip(ys)
        .filter_map(|(x, y)| Some([x?, y?]))
        .collect()
}

fn get_f64_vec(df: &polars::prelude::DataFrame, col: &str) -> Option<Vec<Option<f64>>> {
    let series = df.column(col).ok()?.as_series()?.clone();
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

// Suppress unused import warning — PlotBounds used for future zoom-link feature
#[allow(dead_code)]
fn _use_plot_bounds(_: PlotBounds) {}
