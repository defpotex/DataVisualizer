use crate::plot::map_plot::MapPlot;
use crate::plot::plot_config::MapPlotConfig;
use crate::state::app_state::AppState;
use crate::theme::AppTheme;
use egui::{RichText, Ui};

/// Manages a collection of live plot instances and renders them in a responsive grid.
#[derive(Default)]
pub struct PlotGrid {
    plots: Vec<MapPlot>,
}

impl PlotGrid {
    /// Add a new map plot. Immediately syncs data from the matching source.
    pub fn add_map_plot(&mut self, config: MapPlotConfig, state: &AppState) {
        let mut plot = MapPlot::new(config);
        if let Some(source) = state.sources.iter().find(|s| s.id == plot.config.source_id) {
            plot.sync_data(source);
        }
        self.plots.push(plot);
    }

    /// Remove a plot by ID.
    pub fn remove_plot(&mut self, id: usize) {
        self.plots.retain(|p| p.plot_id() != id);
    }

    pub fn is_empty(&self) -> bool {
        self.plots.is_empty()
    }

    pub fn len(&self) -> usize {
        self.plots.len()
    }

    /// Render all plots into the available space.
    /// Layout: 1 plot → full area; 2+ plots → 2-column grid with equal-height rows.
    pub fn show(&mut self, ui: &mut Ui, theme: &AppTheme) {
        if self.plots.is_empty() {
            return;
        }

        let n = self.plots.len();
        let cols = if n == 1 { 1 } else { 2 };
        let rows = (n + cols - 1) / cols;

        let available = ui.available_rect_before_wrap();
        let spacing = ui.spacing().item_spacing;
        let cell_w = (available.width() - spacing.x * (cols as f32 - 1.0)) / cols as f32;
        let cell_h = (available.height() - spacing.y * (rows as f32 - 1.0)) / rows as f32;

        let mut plot_iter = self.plots.iter_mut().peekable();

        for _row in 0..rows {
            if plot_iter.peek().is_none() {
                break;
            }

            ui.horizontal(|ui| {
                for _col in 0..cols {
                    if let Some(plot) = plot_iter.next() {
                        plot_cell(ui, plot, theme, cell_w, cell_h);
                    }
                }
            });
        }
    }
}

// ── Plot cell ─────────────────────────────────────────────────────────────────

fn plot_cell(ui: &mut Ui, plot: &mut MapPlot, theme: &AppTheme, width: f32, height: f32) {
    let c = &theme.colors;
    let s = &theme.spacing;

    let title_bar_height = s.font_body + 10.0;
    let map_height = (height - title_bar_height - 2.0).max(60.0);

    egui::Frame::default()
        .fill(c.bg_panel)
        .stroke(egui::Stroke::new(1.0, c.border))
        .corner_radius(s.rounding)
        .inner_margin(egui::Margin::from(0.0_f32))
        .show(ui, |ui| {
            ui.set_min_size(egui::vec2(width, height));
            ui.set_max_size(egui::vec2(width, height));

            // Title bar
            egui::Frame::default()
                .fill(c.bg_app)
                .inner_margin(egui::Margin::from(egui::vec2(8.0, 4.0)))
                .corner_radius(egui::CornerRadius { nw: s.rounding as u8, ne: s.rounding as u8, sw: 0, se: 0 })
                .show(ui, |ui| {
                    ui.set_min_width(width);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("◈")
                                .color(c.accent_primary)
                                .size(s.font_small),
                        );
                        ui.label(
                            RichText::new(&plot.config.title)
                                .color(c.text_primary)
                                .size(s.font_body)
                                .strong(),
                        );
                    });
                });

            // Map content area
            let (map_rect, _) = ui.allocate_exact_size(
                egui::vec2(width, map_height),
                egui::Sense::hover(),
            );

            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(map_rect));
            plot.show(&mut child, theme);
        });
}
