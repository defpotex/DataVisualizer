use crate::data::source::SourceId;
use crate::plot::map_plot::MapPlot;
use crate::plot::plot_config::MapPlotConfig;
use crate::state::app_state::AppState;
use crate::theme::AppTheme;
use egui::{Context, Rect, vec2};

/// Manages all live plot instances and renders each as a floating, resizable egui Window.
#[derive(Default)]
pub struct PlotManager {
    plots: Vec<MapPlot>,
}

impl PlotManager {
    /// Add a new map plot. Computes a non-overlapping default position within `central_rect`.
    pub fn add_map_plot(&mut self, config: MapPlotConfig, state: &AppState, central_rect: Rect) {
        let default_pos = tile_default_pos(self.plots.len(), central_rect);
        let mut plot = MapPlot::new(config, default_pos);
        if let Some(source) = state.sources.iter().find(|s| s.id == plot.config.source_id) {
            plot.sync_data(source);
        }
        self.plots.push(plot);
    }

    /// Remove a plot by plot ID.
    pub fn remove_plot(&mut self, id: usize) {
        self.plots.retain(|p| p.plot_id() != id);
    }

    /// Remove all plots whose source matches `source_id`.
    pub fn remove_plots_for_source(&mut self, source_id: SourceId) {
        self.plots.retain(|p| p.config.source_id != source_id);
    }

    pub fn is_empty(&self) -> bool {
        self.plots.is_empty()
    }

    pub fn len(&self) -> usize {
        self.plots.len()
    }

    /// Summary info for left-pane cards.
    pub fn plot_ids_and_titles(&self) -> Vec<(usize, String)> {
        self.plots.iter().map(|p| (p.plot_id(), p.config.title.clone())).collect()
    }

    /// Draw all plots as floating egui Windows constrained to `central_rect`.
    /// Returns IDs of any windows the user closed.
    pub fn show_windows(&mut self, ctx: &Context, theme: &AppTheme, central_rect: Rect) -> Vec<usize> {
        let mut closed = Vec::new();
        for plot in &mut self.plots {
            if !plot.show_as_window(ctx, theme, central_rect) {
                closed.push(plot.plot_id());
            }
        }
        self.plots.retain(|p| !closed.contains(&p.plot_id()));
        closed
    }
}

// ── Grid default positioning ──────────────────────────────────────────────────

/// Compute a non-overlapping starting position for the nth plot within `central_rect`.
/// Tiles 2 columns wide; each window starts at 50% of the central rect's dimensions.
fn tile_default_pos(idx: usize, r: Rect) -> egui::Pos2 {
    const COLS: usize = 2;
    let padding = 8.0_f32;
    let cell_w = (r.width() - padding * (COLS as f32 + 1.0)) / COLS as f32;
    let cell_h = r.height() * 0.5 - padding * 2.0;

    let col = idx % COLS;
    let row = idx / COLS;

    r.min + vec2(
        padding + col as f32 * (cell_w + padding),
        padding + row as f32 * (cell_h + padding),
    )
}
