use crate::plot::plot_config::{MapPlotConfig, ScatterPlotConfig, ScrollChartConfig};
use crate::plot::styling::PlotLegendData;
use crate::plot::sync::PlotSyncEvent;
use crate::state::app_state::AppState;
use crate::state::selection::SelectionSet;
use crate::theme::AppTheme;
use crate::ui::plot_grid::{PlotAction, PlotManager};
use egui::{Align, Color32, Context, Layout, RichText, Ui};

#[derive(Default)]
pub struct PlotArea {
    plot_manager: PlotManager,
}

impl PlotArea {
    pub fn add_map_plot(&mut self, config: MapPlotConfig, state: &AppState, central_rect: egui::Rect) {
        self.plot_manager.add_map_plot(config, state, central_rect);
    }

    pub fn add_scatter_plot(&mut self, config: ScatterPlotConfig, state: &AppState, central_rect: egui::Rect) {
        self.plot_manager.add_scatter_plot(config, state, central_rect);
    }

    pub fn add_scroll_chart(&mut self, config: ScrollChartConfig, state: &AppState, central_rect: egui::Rect) {
        self.plot_manager.add_scroll_chart(config, state, central_rect);
    }

    pub fn remove_plot(&mut self, id: usize) {
        self.plot_manager.remove_plot(id);
    }

    pub fn remove_plots_for_source(&mut self, source_id: usize) {
        self.plot_manager.remove_plots_for_source(source_id);
    }

    /// Re-sync all plot data after filters change.
    pub fn sync_all_filters(&mut self, state: &AppState) {
        self.plot_manager.sync_all_filters(state);
    }

    /// Re-sync all plot data, skipping plots that are already computing.
    /// Used by the playback engine to avoid cancelling in-flight syncs every frame.
    pub fn sync_all_filters_throttled(&mut self, state: &AppState) {
        self.plot_manager.sync_all_filters_throttled(state);
    }

    /// Re-sync data for a single plot after its config changes.
    pub fn sync_plot(&mut self, id: usize, state: &AppState) {
        self.plot_manager.sync_plot(id, state);
    }

    /// Apply a completed sync result from a background thread.
    pub fn apply_sync_event(&mut self, event: PlotSyncEvent) {
        self.plot_manager.apply_sync_event(event);
    }

    /// Collect current legend data from all plots.
    pub fn legend_data(&self) -> Vec<PlotLegendData> {
        self.plot_manager.legend_data()
    }

    pub fn show_windows(
        &mut self,
        ctx: &Context,
        theme: &AppTheme,
        central_rect: egui::Rect,
        grid_size: f32,
        perf: &crate::state::perf_settings::PerformanceSettings,
        selection: Option<&SelectionSet>,
    ) -> Vec<PlotAction> {
        self.plot_manager.show_windows(ctx, theme, central_rect, grid_size, perf, selection)
    }

    pub fn show(&mut self, ui: &mut Ui, theme: &AppTheme, state: &AppState, grid_size: f32) {
        if !self.plot_manager.is_empty() {
            self.show_has_plots(ui, theme, grid_size);
        } else if state.has_sources() {
            self.show_has_sources(ui, theme, state);
        } else {
            self.show_empty(ui, theme);
        }
    }

    fn show_empty(&self, ui: &mut Ui, theme: &AppTheme) {
        let c = &theme.colors;
        let s = &theme.spacing;
        let panel_width = ui.available_width();

        ui.add_space(ui.available_height() * 0.28);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("◈").color(c.accent_primary).size(52.0));
            ui.add_space(10.0);
            ui.label(
                RichText::new("No data sources loaded.")
                    .color(c.text_primary)
                    .size(s.font_body + 2.0)
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Use  Data Sources → Add Source  or the panel on the left")
                    .color(c.text_secondary)
                    .size(s.font_body),
            );
            ui.label(
                RichText::new("to load a CSV, Parquet file, or connect to a UDP stream.")
                    .color(c.text_secondary)
                    .size(s.font_body),
            );
            ui.add_space(24.0);
            let group_width = 388.0_f32;
            let offset = (panel_width - group_width).max(0.0) / 2.0;
            ui.horizontal(|ui| {
                ui.add_space(offset);
                quick_hint(ui, "CSV", "Load flat file data", theme);
                ui.add_space(8.0);
                quick_hint(ui, "Parquet", "Load columnar data", theme);
                ui.add_space(8.0);
                quick_hint(ui, "UDP", "Connect live stream", theme);
            });
        });
    }

    fn show_has_sources(&self, ui: &mut Ui, theme: &AppTheme, state: &AppState) {
        let c = &theme.colors;
        let s = &theme.spacing;
        ui.add_space(ui.available_height() * 0.28);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("◈").color(c.accent_primary).size(40.0));
            ui.add_space(10.0);
            let src_count = state.sources.len();
            let total_rows: usize = state.sources.iter().map(|s| s.row_count()).sum();
            ui.label(
                RichText::new(format!(
                    "{} source{} loaded  ·  {} rows",
                    src_count,
                    if src_count == 1 { "" } else { "s" },
                    format_count(total_rows),
                ))
                .color(c.accent_secondary)
                .size(s.font_body + 1.0)
                .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Use  Add Plot  in the left panel to create a visualization.")
                    .color(c.text_secondary)
                    .size(s.font_body),
            );
        });
    }

    fn show_has_plots(&self, ui: &mut Ui, theme: &AppTheme, grid_size: f32) {
        let c = &theme.colors;
        let s = &theme.spacing;

        // Dot grid background.
        let rect = ui.max_rect();
        let interacting = ui.ctx().input(|i| i.pointer.is_decidedly_dragging() || i.pointer.any_down());
        let grid_alpha: u8 = if interacting { 55 } else { 25 };
        let dot_color = Color32::from_rgba_unmultiplied(
            c.accent_primary.r(), c.accent_primary.g(), c.accent_primary.b(), grid_alpha,
        );
        let painter = ui.painter().with_clip_rect(rect);
        let mut x = rect.min.x + grid_size;
        while x < rect.max.x {
            let mut y = rect.min.y + grid_size;
            while y < rect.max.y {
                painter.circle_filled(egui::pos2(x, y), 1.0, dot_color);
                y += grid_size;
            }
            x += grid_size;
        }

        ui.add_space(ui.available_height() * 0.35);
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} plot window{} open",
                    self.plot_manager.len(),
                    if self.plot_manager.len() == 1 { "" } else { "s" }
                ))
                .color(c.text_secondary)
                .size(s.font_small)
                .italics(),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Drag windows to reposition · Resize from edges")
                    .color(c.text_secondary)
                    .size(s.font_small),
            );
        });
    }
}

fn quick_hint(ui: &mut Ui, label: &str, desc: &str, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;
    egui::Frame::default()
        .fill(c.bg_panel)
        .stroke(egui::Stroke::new(1.0, c.border))
        .corner_radius(s.rounding)
        .inner_margin(egui::Margin::from(egui::vec2(12.0, 8.0)))
        .show(ui, |ui| {
            ui.set_width(100.0);
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.label(RichText::new(label).color(c.accent_primary).size(s.font_body).strong());
                ui.label(RichText::new(desc).color(c.text_secondary).size(s.font_small));
            });
        });
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
