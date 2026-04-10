use crate::data::filter::{apply_filters, Filter};
use crate::data::source::{DataSource, SourceId};
use crate::plot::map_plot::{snap_to_grid, MapPlot};
use crate::plot::plot_config::{MapPlotConfig, PlotConfig, ScatterPlotConfig};
use crate::plot::scatter_plot::{PlotWindowEvent, ScatterPlot};
use crate::plot::styling::PlotLegendData;
use crate::state::app_state::AppState;
use crate::theme::AppTheme;
use egui::{Context, Rect, vec2};
use std::collections::HashMap;

// ── PlotAction ────────────────────────────────────────────────────────────────

/// Actions produced by `PlotManager::show_windows` that the caller (app.rs) needs to handle.
pub enum PlotAction {
    /// User closed a plot window. The live instance has already been removed from PlotManager.
    Closed(usize),
    /// User confirmed a config change. The live instance has already been updated.
    /// Caller should sync app_state.plots and call sync_plot to re-extract data.
    ConfigChanged(PlotConfig),
}

// ── ManagedPlot ───────────────────────────────────────────────────────────────

enum ManagedPlot {
    Map(MapPlot),
    Scatter(ScatterPlot),
}

impl ManagedPlot {
    fn plot_id(&self) -> usize {
        match self { Self::Map(p) => p.plot_id(), Self::Scatter(p) => p.plot_id() }
    }
    fn source_id(&self) -> SourceId {
        match self {
            Self::Map(p) => p.config.source_id,
            Self::Scatter(p) => p.config.source_id,
        }
    }
    fn window_id(&self) -> egui::Id {
        match self { Self::Map(p) => p.window_id(), Self::Scatter(p) => p.window_id() }
    }
    fn intended_rect(&self, ctx: &Context) -> Option<Rect> {
        match self { Self::Map(p) => p.intended_rect(ctx), Self::Scatter(p) => p.intended_rect(ctx) }
    }
    fn set_pending_snap(&mut self, pos: egui::Pos2) {
        match self { Self::Map(p) => p.set_pending_snap(pos), Self::Scatter(p) => p.set_pending_snap(pos) }
    }
    fn show(&mut self, ctx: &Context, theme: &AppTheme, central_rect: Rect, grid_size: f32, max_pts: usize) -> PlotWindowEvent {
        match self {
            Self::Map(p) => p.show_as_window(ctx, theme, central_rect, grid_size, max_pts),
            Self::Scatter(p) => p.show_as_window(ctx, theme, central_rect, grid_size, max_pts),
        }
    }
    fn sync_data(&mut self, source: &DataSource, filters: &[Filter]) {
        let filtered_df = apply_filters(&source.df, filters);
        let mut tmp = source.clone();
        tmp.df = filtered_df;
        match self {
            Self::Map(p) => p.sync_data(&tmp),
            Self::Scatter(p) => p.sync_data(&tmp),
        }
    }
    fn legend_data(&self) -> Option<&PlotLegendData> {
        match self {
            Self::Map(p) => p.legend_data(),
            Self::Scatter(p) => p.legend_data(),
        }
    }

    fn apply_config(&mut self, new_config: PlotConfig) {
        match (self, new_config) {
            (Self::Map(p), PlotConfig::Map(c)) => p.apply_config(c),
            (Self::Scatter(p), PlotConfig::Scatter(c)) => p.apply_config(c),
            _ => {} // type mismatch — shouldn't happen
        }
    }
}

// ── PlotManager ───────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct PlotManager {
    plots: Vec<ManagedPlot>,
    prev_rects: HashMap<usize, Rect>,
}

impl PlotManager {
    pub fn add_map_plot(&mut self, config: MapPlotConfig, state: &AppState, central_rect: Rect) {
        let default_pos = tile_default_pos(self.plots.len(), central_rect);
        let mut plot = MapPlot::new(config, default_pos);
        if let Some(source) = state.sources.iter().find(|s| s.id == plot.config.source_id) {
            plot.sync_data(source);
        }
        self.plots.push(ManagedPlot::Map(plot));
    }

    pub fn add_scatter_plot(&mut self, config: ScatterPlotConfig, state: &AppState, central_rect: Rect) {
        let default_pos = tile_default_pos(self.plots.len(), central_rect);
        let mut plot = ScatterPlot::new(config, default_pos);
        if let Some(source) = state.sources.iter().find(|s| s.id == plot.config.source_id) {
            let filtered = apply_filters(&source.df, &state.filters);
            let mut tmp = source.clone();
            tmp.df = filtered;
            plot.sync_data(&tmp);
        }
        self.plots.push(ManagedPlot::Scatter(plot));
    }

    /// Re-apply filters to all plots (call whenever filters change).
    pub fn sync_all_filters(&mut self, state: &AppState) {
        for plot in &mut self.plots {
            if let Some(source) = state.sources.iter().find(|s| s.id == plot.source_id()) {
                plot.sync_data(source, &state.filters);
            }
        }
    }

    /// Re-sync data for a single plot by ID (call after config change).
    pub fn sync_plot(&mut self, id: usize, state: &AppState) {
        if let Some(plot) = self.plots.iter_mut().find(|p| p.plot_id() == id) {
            if let Some(source) = state.sources.iter().find(|s| s.id == plot.source_id()) {
                plot.sync_data(source, &state.filters);
            }
        }
    }

    /// Re-sync a specific source's plots (call after source is loaded).
    pub fn sync_source(&mut self, source: &DataSource, filters: &[Filter]) {
        for plot in &mut self.plots {
            if plot.source_id() == source.id {
                plot.sync_data(source, filters);
            }
        }
    }

    pub fn remove_plot(&mut self, id: usize) {
        self.plots.retain(|p| p.plot_id() != id);
        self.prev_rects.remove(&id);
    }

    pub fn remove_plots_for_source(&mut self, source_id: SourceId) {
        let removed: Vec<usize> = self.plots.iter()
            .filter(|p| p.source_id() == source_id)
            .map(|p| p.plot_id())
            .collect();
        self.plots.retain(|p| p.source_id() != source_id);
        for id in removed { self.prev_rects.remove(&id); }
    }

    pub fn is_empty(&self) -> bool { self.plots.is_empty() }
    pub fn len(&self) -> usize { self.plots.len() }

    /// Collect legend data from all plots (plots without synced data are skipped).
    pub fn legend_data(&self) -> Vec<PlotLegendData> {
        self.plots.iter().filter_map(|p| p.legend_data().cloned()).collect()
    }

    /// Draw all plot windows. Returns actions for the caller to process.
    pub fn show_windows(
        &mut self,
        ctx: &Context,
        theme: &AppTheme,
        central_rect: Rect,
        grid_size: f32,
        max_draw_points: usize,
    ) -> Vec<PlotAction> {
        let mut actions: Vec<PlotAction> = Vec::new();
        let mut closed_ids: Vec<usize> = Vec::new();

        for plot in &mut self.plots {
            match plot.show(ctx, theme, central_rect, grid_size, max_draw_points) {
                PlotWindowEvent::Open => {}
                PlotWindowEvent::Closed => {
                    closed_ids.push(plot.plot_id());
                }
                PlotWindowEvent::ConfigChanged(new_config) => {
                    let id = new_config.id();
                    // Update the live widget's config in-place.
                    plot.apply_config(new_config.clone());
                    actions.push(PlotAction::ConfigChanged(new_config));
                    // Note: data re-sync is handled by the caller (app.rs) via sync_plot,
                    // because the caller needs AppState to look up the source and filters.
                    let _ = id;
                }
            }
        }

        for id in &closed_ids {
            self.plots.retain(|p| p.plot_id() != *id);
            self.prev_rects.remove(id);
            actions.push(PlotAction::Closed(*id));
        }

        self.resolve_collisions(ctx, central_rect, grid_size);
        actions
    }

    // ── Collision resolution ──────────────────────────────────────────────────

    fn resolve_collisions(&mut self, ctx: &Context, central_rect: Rect, grid_size: f32) {
        let current: Vec<(usize, Rect)> = self.plots.iter()
            .filter_map(|p| p.intended_rect(ctx).map(|r| (p.plot_id(), r)))
            .collect();

        if !ctx.input(|i| i.pointer.any_released()) {
            for &(id, rect) in &current { self.prev_rects.insert(id, rect); }
            return;
        }
        if current.len() < 2 {
            for &(id, rect) in &current { self.prev_rects.insert(id, rect); }
            return;
        }

        let moved: std::collections::HashSet<usize> = current.iter()
            .filter(|(id, rect)| {
                match self.prev_rects.get(id) {
                    Some(prev) => (prev.min - rect.min).length() > 1.0
                               || (prev.size() - rect.size()).length() > 1.0,
                    None => true,
                }
            })
            .map(|(id, _)| *id)
            .collect();

        let mut resolved: Vec<(usize, Rect)> = current.clone();

        for &mover_id in &moved {
            let Some(mover_idx) = resolved.iter().position(|(id, _)| *id == mover_id) else { continue };
            let original_rect = resolved[mover_idx].1;
            let mut pos = original_rect.min;
            let size = original_rect.size();

            'outer: for _iter in 0..32 {
                let mut any_overlap = false;
                for k in 0..resolved.len() {
                    if resolved[k].0 == mover_id { continue; }
                    let obstacle = resolved[k].1;
                    let mover_rect = Rect::from_min_size(pos, size);
                    if rects_overlap(mover_rect, obstacle) {
                        pos += min_separation_vec(mover_rect, obstacle);
                        any_overlap = true;
                    }
                }
                if !any_overlap { break 'outer; }
            }

            let snapped = snap_to_grid(pos, grid_size);
            let clamped = egui::pos2(
                snapped.x.max(central_rect.min.x)
                    .min((central_rect.max.x - size.x).max(central_rect.min.x)),
                snapped.y.max(central_rect.min.y)
                    .min((central_rect.max.y - size.y).max(central_rect.min.y)),
            );

            resolved[mover_idx].1 = Rect::from_min_size(clamped, size);

            if let Some(plot) = self.plots.iter_mut().find(|p| p.plot_id() == mover_id) {
                if (clamped - original_rect.min).length() > 0.5 {
                    plot.set_pending_snap(clamped);
                }
            }
        }

        for &(id, rect) in &current { self.prev_rects.insert(id, rect); }
    }
}

// ── Grid positioning ──────────────────────────────────────────────────────────

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

// ── Collision helpers ─────────────────────────────────────────────────────────

fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.min.x < b.max.x && a.max.x > b.min.x
        && a.min.y < b.max.y && a.max.y > b.min.y
}

fn min_separation_vec(moving: Rect, other: Rect) -> egui::Vec2 {
    let dx_left  = other.min.x - moving.max.x;
    let dx_right = other.max.x - moving.min.x;
    let dy_up    = other.min.y - moving.max.y;
    let dy_down  = other.max.y - moving.min.y;
    let options = [
        egui::vec2(dx_left,  0.0),
        egui::vec2(dx_right, 0.0),
        egui::vec2(0.0, dy_up),
        egui::vec2(0.0, dy_down),
    ];
    options.into_iter()
        .min_by(|a, b| a.length().partial_cmp(&b.length()).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or_default()
}
