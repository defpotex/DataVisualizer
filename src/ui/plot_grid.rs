use crate::data::source::SourceId;
use crate::plot::map_plot::MapPlot;
use crate::plot::plot_config::MapPlotConfig;
use crate::state::app_state::AppState;
use crate::theme::AppTheme;
use egui::Context;

/// Manages all live plot instances and renders each as a floating, resizable egui Window.
#[derive(Default)]
pub struct PlotManager {
    plots: Vec<MapPlot>,
}

impl PlotManager {
    /// Add a new map plot and sync its data from the matching source.
    pub fn add_map_plot(&mut self, config: MapPlotConfig, state: &AppState) {
        let mut plot = MapPlot::new(config);
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

    /// Summary info for left-pane cards: (plot_id, title, source_label).
    /// Caller must resolve source_label from AppState.
    pub fn plot_ids_and_titles(&self) -> Vec<(usize, String)> {
        self.plots.iter().map(|p| (p.plot_id(), p.config.title.clone())).collect()
    }

    /// Draw all plots as floating egui Windows. Returns IDs of any windows the user closed.
    pub fn show_windows(&mut self, ctx: &Context, theme: &AppTheme) -> Vec<usize> {
        let mut closed = Vec::new();
        for plot in &mut self.plots {
            if !plot.show_as_window(ctx, theme) {
                closed.push(plot.plot_id());
            }
        }
        // Purge closed windows immediately so next frame is clean.
        self.plots.retain(|p| !closed.contains(&p.plot_id()));
        closed
    }
}
