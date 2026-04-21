use crate::data::filter::{apply_filters_for_source, Filter};
use crate::data::source::{DataSource, SourceId};
use crate::plot::map_plot::{snap_to_grid, MapPlot};
use crate::plot::plot_config::{MapPlotConfig, PlotConfig, ScatterPlotConfig, ScrollChartConfig};
use crate::plot::scatter_plot::{PlotWindowEvent, ScatterPlot};
use crate::plot::scroll_chart::ScrollChart;
use crate::plot::styling::PlotLegendData;
use crate::plot::sync::PlotSyncEvent;
use crate::state::app_state::{AppState, DataEvent};
use crate::state::selection::SelectionSet;
use crate::theme::AppTheme;
use crossbeam_channel::Sender;
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
    /// User changed the point selection (click, ctrl+click, area drag, or clear).
    SelectionChanged(Option<SelectionSet>),
    /// User chose "Filter to Selection" from the context menu.
    FilterToSelection(SelectionSet),
    /// Legend-style action from point context menu (filter to value, select all sharing).
    LegendAction(crate::ui::right_pane::LegendAction),
    /// Rebind a plot to a different source (from the plot's "source removed" overlay).
    RebindSource { plot_id: usize, new_source_id: crate::data::source::SourceId },
}

// ── ManagedPlot ───────────────────────────────────────────────────────────────

enum ManagedPlot {
    Map(MapPlot),
    Scatter(ScatterPlot),
    ScrollChart(ScrollChart),
}

impl ManagedPlot {
    fn plot_id(&self) -> usize {
        match self {
            Self::Map(p) => p.plot_id(),
            Self::Scatter(p) => p.plot_id(),
            Self::ScrollChart(p) => p.plot_id(),
        }
    }
    fn source_id(&self) -> SourceId {
        match self {
            Self::Map(p) => p.config.source_id,
            Self::Scatter(p) => p.config.source_id,
            Self::ScrollChart(p) => p.config.source_id,
        }
    }
    fn window_id(&self) -> egui::Id {
        match self {
            Self::Map(p) => p.window_id(),
            Self::Scatter(p) => p.window_id(),
            Self::ScrollChart(p) => p.window_id(),
        }
    }
    fn intended_rect(&self, ctx: &Context) -> Option<Rect> {
        match self {
            Self::Map(p) => p.intended_rect(ctx),
            Self::Scatter(p) => p.intended_rect(ctx),
            Self::ScrollChart(p) => p.intended_rect(ctx),
        }
    }
    fn set_pending_snap(&mut self, pos: egui::Pos2) {
        match self {
            Self::Map(p) => p.set_pending_snap(pos),
            Self::Scatter(p) => p.set_pending_snap(pos),
            Self::ScrollChart(p) => p.set_pending_snap(pos),
        }
    }
    fn show(&mut self, ctx: &Context, theme: &AppTheme, central_rect: Rect, grid_size: f32, perf: &crate::state::perf_settings::PerformanceSettings, selection: Option<&SelectionSet>) -> PlotWindowEvent {
        match self {
            Self::Map(p) => p.show_as_window(ctx, theme, central_rect, grid_size, perf, selection),
            Self::Scatter(p) => p.show_as_window(ctx, theme, central_rect, grid_size, perf, selection),
            Self::ScrollChart(p) => p.show_as_window(ctx, theme, central_rect, grid_size),
        }
    }
    fn is_computing(&self) -> bool {
        match self {
            Self::Map(p) => p.is_computing(),
            Self::Scatter(p) => p.is_computing(),
            Self::ScrollChart(p) => p.is_computing(),
        }
    }
    fn sync_data_async(&mut self, source: &DataSource, filters: &[Filter], tx: &Sender<DataEvent>) {
        // Scroll charts manage their own time windowing, so skip temporal
        // playback filters (TimeLe, TimeRange) to avoid truncating the
        // scroll window when playback trail is shorter.
        let effective_filters: Vec<Filter>;
        let filter_ref = match self {
            Self::ScrollChart(_) => {
                effective_filters = filters.iter()
                    .filter(|f| !matches!(f.op, crate::data::filter::FilterOp::TimeLe | crate::data::filter::FilterOp::TimeRange))
                    .cloned()
                    .collect();
                effective_filters.as_slice()
            }
            _ => filters,
        };
        let filtered_df = apply_filters_for_source(&source.df, filter_ref, Some(source.id));
        let mut tmp = source.clone();
        tmp.df = filtered_df;
        match self {
            Self::Map(p) => p.sync_data_async(&tmp, tx),
            Self::Scatter(p) => p.sync_data_async(&tmp, tx),
            Self::ScrollChart(p) => p.sync_data_async(&tmp, tx),
        }
    }
    fn legend_data(&self) -> Option<&PlotLegendData> {
        match self {
            Self::Map(p) => p.legend_data(),
            Self::Scatter(p) => p.legend_data(),
            Self::ScrollChart(_) => None, // Scroll charts don't have color legends
        }
    }

    fn apply_config(&mut self, new_config: PlotConfig) {
        match (self, new_config) {
            (Self::Map(p), PlotConfig::Map(c)) => p.apply_config(c),
            (Self::Scatter(p), PlotConfig::Scatter(c)) => p.apply_config(c),
            (Self::ScrollChart(p), PlotConfig::ScrollChart(c)) => p.apply_config(c),
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
            let filtered = apply_filters_for_source(&source.df, &state.filters, Some(source.id));
            let mut tmp = source.clone();
            tmp.df = filtered;
            plot.sync_data_async(&tmp, &state.event_tx);
        }
        self.plots.push(ManagedPlot::Map(plot));
    }

    pub fn add_scatter_plot(&mut self, config: ScatterPlotConfig, state: &AppState, central_rect: Rect) {
        let default_pos = tile_default_pos(self.plots.len(), central_rect);
        let mut plot = ScatterPlot::new(config, default_pos);
        if let Some(source) = state.sources.iter().find(|s| s.id == plot.config.source_id) {
            let filtered = apply_filters_for_source(&source.df, &state.filters, Some(source.id));
            let mut tmp = source.clone();
            tmp.df = filtered;
            plot.sync_data_async(&tmp, &state.event_tx);
        }
        self.plots.push(ManagedPlot::Scatter(plot));
    }

    pub fn add_scroll_chart(&mut self, config: ScrollChartConfig, state: &AppState, central_rect: Rect) {
        let default_pos = tile_default_pos(self.plots.len(), central_rect);
        let mut chart = ScrollChart::new(config, default_pos);
        if let Some(source) = state.sources.iter().find(|s| s.id == chart.config.source_id) {
            let filtered = apply_filters_for_source(&source.df, &state.filters, Some(source.id));
            let mut tmp = source.clone();
            tmp.df = filtered;
            chart.sync_data_async(&tmp, &state.event_tx);
        }
        self.plots.push(ManagedPlot::ScrollChart(chart));
    }

    /// Re-apply filters to all plots (call whenever filters change).
    pub fn sync_all_filters(&mut self, state: &AppState) {
        self.sync_all_filters_inner(state, false);
    }

    /// Re-apply filters, but skip plots that are already computing.
    /// Used by the playback engine to avoid cancelling in-flight syncs every frame.
    pub fn sync_all_filters_throttled(&mut self, state: &AppState) {
        self.sync_all_filters_inner(state, true);
    }

    fn sync_all_filters_inner(&mut self, state: &AppState, skip_computing: bool) {
        for plot in &mut self.plots {
            if skip_computing && plot.is_computing() { continue; }
            if let Some(source) = state.sources.iter().find(|s| s.id == plot.source_id()) {
                plot.sync_data_async(source, &state.filters, &state.event_tx);
            }
        }
    }

    /// Re-sync data for a single plot by ID (call after config change).
    pub fn sync_plot(&mut self, id: usize, state: &AppState) {
        if let Some(plot) = self.plots.iter_mut().find(|p| p.plot_id() == id) {
            if let Some(source) = state.sources.iter().find(|s| s.id == plot.source_id()) {
                plot.sync_data_async(source, &state.filters, &state.event_tx);
            }
        }
    }

    /// Re-sync a specific source's plots (call after source is loaded).
    pub fn sync_source(&mut self, source: &DataSource, filters: &[Filter], tx: &Sender<DataEvent>) {
        for plot in &mut self.plots {
            if plot.source_id() == source.id {
                plot.sync_data_async(source, filters, tx);
            }
        }
    }

    /// Apply a completed sync result from a background thread.
    pub fn apply_sync_event(&mut self, event: PlotSyncEvent) {
        match event {
            PlotSyncEvent::ScatterReady(result) => {
                let id = result.plot_id;
                if let Some(ManagedPlot::Scatter(p)) = self.plots.iter_mut().find(|p| p.plot_id() == id) {
                    p.apply_sync_result(result);
                }
            }
            PlotSyncEvent::MapReady(result) => {
                let id = result.plot_id;
                if let Some(ManagedPlot::Map(p)) = self.plots.iter_mut().find(|p| p.plot_id() == id) {
                    p.apply_sync_result(result);
                }
            }
            PlotSyncEvent::ScrollChartReady(result) => {
                let id = result.plot_id;
                if let Some(ManagedPlot::ScrollChart(p)) = self.plots.iter_mut().find(|p| p.plot_id() == id) {
                    p.apply_sync_result(result);
                }
            }
            PlotSyncEvent::Cancelled { plot_id } => {
                // Mark the plot as no longer computing (cancel already handled).
                if let Some(plot) = self.plots.iter_mut().find(|p| p.plot_id() == plot_id) {
                    match plot {
                        ManagedPlot::Map(p) => { p.cancel_sync(); }
                        ManagedPlot::Scatter(p) => { p.cancel_sync(); }
                        ManagedPlot::ScrollChart(p) => { p.cancel_sync(); }
                    }
                }
            }
        }
    }

    /// Update a plot's source_id in the live plot manager.
    pub fn rebind_plot_source(&mut self, plot_id: usize, new_source_id: SourceId) {
        if let Some(plot) = self.plots.iter_mut().find(|p| p.plot_id() == plot_id) {
            match plot {
                ManagedPlot::Map(p) => p.config.source_id = new_source_id,
                ManagedPlot::Scatter(p) => p.config.source_id = new_source_id,
                ManagedPlot::ScrollChart(p) => p.config.source_id = new_source_id,
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
        perf: &crate::state::perf_settings::PerformanceSettings,
        selection: Option<&SelectionSet>,
        available_sources: &[(SourceId, String)],
    ) -> Vec<PlotAction> {
        let mut actions: Vec<PlotAction> = Vec::new();
        let mut closed_ids: Vec<usize> = Vec::new();

        for plot in &mut self.plots {
            // Check if source still exists
            let source_exists = available_sources.iter().any(|(id, _)| *id == plot.source_id());
            if !source_exists {
                // Show detached overlay with rebind options.
                // Use the previous rect (captured before the source was removed)
                // so the window doesn't shrink or grow when it switches to the overlay.
                let prev_rect = self.prev_rects.get(&plot.plot_id()).copied();
                let evt = show_detached_overlay(ctx, theme, plot, central_rect, grid_size, available_sources, prev_rect);
                match evt {
                    PlotWindowEvent::Closed => {
                        closed_ids.push(plot.plot_id());
                    }
                    PlotWindowEvent::RebindSource { plot_id, new_source_id } => {
                        actions.push(PlotAction::RebindSource { plot_id, new_source_id });
                    }
                    _ => {}
                }
                continue;
            }

            match plot.show(ctx, theme, central_rect, grid_size, perf, selection) {
                PlotWindowEvent::Open => {}
                PlotWindowEvent::Closed => {
                    closed_ids.push(plot.plot_id());
                }
                PlotWindowEvent::ConfigChanged(new_config) => {
                    let id = new_config.id();
                    plot.apply_config(new_config.clone());
                    actions.push(PlotAction::ConfigChanged(new_config));
                    let _ = id;
                }
                PlotWindowEvent::SelectionChanged(sel) => {
                    actions.push(PlotAction::SelectionChanged(sel));
                }
                PlotWindowEvent::FilterToSelection(sel) => {
                    actions.push(PlotAction::FilterToSelection(sel));
                }
                PlotWindowEvent::LegendAction(la) => {
                    actions.push(PlotAction::LegendAction(la));
                }
                PlotWindowEvent::RebindSource { plot_id, new_source_id } => {
                    actions.push(PlotAction::RebindSource { plot_id, new_source_id });
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

/// Show a detached plot window (source removed) with rebind options.
fn show_detached_overlay(
    ctx: &Context,
    theme: &AppTheme,
    plot: &mut ManagedPlot,
    _central_rect: Rect,
    _grid_size: f32,
    available_sources: &[(SourceId, String)],
    prev_rect: Option<Rect>,
) -> PlotWindowEvent {
    use crate::plot::scatter_plot::PlotWindowEvent;
    let c = &theme.colors;
    let s = &theme.spacing;
    let mut event = PlotWindowEvent::Open;
    let mut open = true;

    let title = match plot {
        ManagedPlot::Map(p) => p.config.title.clone(),
        ManagedPlot::Scatter(p) => p.config.title.clone(),
        ManagedPlot::ScrollChart(p) => p.config.title.clone(),
    };
    let plot_id = plot.plot_id();
    let win_id = plot.window_id();

    // Pin the window to the rect it had before the source was removed so it
    // doesn't shrink to content or grow unboundedly. `prev_rect` comes from
    // PlotManager.prev_rects which was snapshotted prior to detachment.
    let pin_size = prev_rect.map(|r| r.size());
    let pin_pos = prev_rect.map(|r| r.min);

    let mut win = egui::Window::new(&title)
        .id(win_id)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .frame(egui::Frame {
            fill: c.bg_panel,
            stroke: egui::Stroke::new(2.0, c.accent_warning),
            corner_radius: egui::CornerRadius::same(s.rounding as u8),
            inner_margin: egui::Margin::from(16.0_f32),
            ..Default::default()
        });
    if let Some(size) = pin_size {
        win = win.fixed_size(size);
    }
    if let Some(pos) = pin_pos {
        win = win.current_pos(pos);
    }
    win.show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new("Source Removed")
                        .color(c.accent_warning)
                        .size(s.font_body + 2.0)
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("The data source for this plot has been removed.\nSelect a new source to rebind:")
                        .color(c.text_secondary)
                        .size(s.font_small),
                );
                ui.add_space(12.0);

                if available_sources.is_empty() {
                    ui.label(
                        egui::RichText::new("No sources available. Load a data source first.")
                            .color(c.text_secondary)
                            .size(s.font_small)
                            .italics(),
                    );
                } else {
                    for (source_id, label) in available_sources {
                        let btn = egui::Button::new(
                            egui::RichText::new(format!("▶  {}", label))
                                .color(c.accent_primary)
                                .size(s.font_body),
                        )
                        .min_size(egui::vec2(ui.available_width() * 0.8, 0.0));
                        if ui.add(btn).clicked() {
                            event = PlotWindowEvent::RebindSource {
                                plot_id,
                                new_source_id: *source_id,
                            };
                        }
                        ui.add_space(4.0);
                    }
                }
                ui.add_space(20.0);
            });
        });

    if !open {
        return PlotWindowEvent::Closed;
    }

    event
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
