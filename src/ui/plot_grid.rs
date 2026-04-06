use crate::data::source::SourceId;
use crate::plot::map_plot::{snap_to_grid, MapPlot};
use crate::plot::plot_config::MapPlotConfig;
use crate::state::app_state::AppState;
use crate::theme::AppTheme;
use egui::{Context, Rect, vec2};
use std::collections::HashMap;

/// Manages all live plot instances and renders each as a floating, resizable egui Window.
#[derive(Default)]
pub struct PlotManager {
    plots: Vec<MapPlot>,
    /// Cached rects from the previous frame, keyed by plot ID.
    /// Used to detect which window moved so we know what to push on collision.
    prev_rects: HashMap<usize, Rect>,
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
        self.prev_rects.remove(&id);
    }

    /// Remove all plots whose source matches `source_id`.
    pub fn remove_plots_for_source(&mut self, source_id: SourceId) {
        let removed: Vec<usize> = self.plots.iter()
            .filter(|p| p.config.source_id == source_id)
            .map(|p| p.plot_id())
            .collect();
        self.plots.retain(|p| p.config.source_id != source_id);
        for id in removed { self.prev_rects.remove(&id); }
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
    pub fn show_windows(&mut self, ctx: &Context, theme: &AppTheme, central_rect: Rect, grid_size: f32) -> Vec<usize> {
        let mut closed = Vec::new();
        for plot in &mut self.plots {
            if !plot.show_as_window(ctx, theme, central_rect, grid_size) {
                closed.push(plot.plot_id());
            }
        }
        self.plots.retain(|p| !closed.contains(&p.plot_id()));

        // Resolve inter-window collisions after all windows are shown.
        self.resolve_collisions(ctx, central_rect, grid_size);

        closed
    }

    // ── Collision resolution ──────────────────────────────────────────────────

    /// On pointer release: push each moved window away from all others iteratively
    /// until every pair is non-overlapping (or max iterations are exhausted).
    fn resolve_collisions(&mut self, ctx: &Context, central_rect: Rect, grid_size: f32) {
        // Always maintain a fresh snapshot of rects so we can detect movers.
        let current: Vec<(usize, Rect)> = self.plots.iter()
            .filter_map(|p| p.intended_rect(ctx).map(|r| (p.plot_id(), r)))
            .collect();

        if !ctx.input(|i| i.pointer.any_released()) {
            for &(id, rect) in &current {
                self.prev_rects.insert(id, rect);
            }
            return;
        }

        if current.len() < 2 {
            for &(id, rect) in &current {
                self.prev_rects.insert(id, rect);
            }
            return;
        }

        // Identify which windows moved or were resized since last frame.
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

        // For every moved window, iteratively push it away from all other windows.
        // We work on a mutable position vector so each push is visible to the next check.
        let mut resolved: Vec<(usize, Rect)> = current.clone();

        for &mover_id in &moved {
            let Some(mover_idx) = resolved.iter().position(|(id, _)| *id == mover_id) else { continue };

            let original_rect = resolved[mover_idx].1;
            let mut pos = original_rect.min;
            let size  = original_rect.size();

            // Collect the current rects of all OTHER windows (their resolved positions).
            // These are treated as static obstacles for this mover.
            'outer: for _iter in 0..32 {
                let mut any_overlap = false;
                for k in 0..resolved.len() {
                    if resolved[k].0 == mover_id { continue; }
                    let obstacle = resolved[k].1;
                    let mover_rect = Rect::from_min_size(pos, size);
                    if rects_overlap(mover_rect, obstacle) {
                        let sep = min_separation_vec(mover_rect, obstacle);
                        pos += sep;
                        any_overlap = true;
                    }
                }
                if !any_overlap { break 'outer; }
            }

            // Grid-snap and clamp to central panel.
            let snapped = snap_to_grid(pos, grid_size);
            let clamped = egui::pos2(
                snapped.x.max(central_rect.min.x)
                    .min((central_rect.max.x - size.x).max(central_rect.min.x)),
                snapped.y.max(central_rect.min.y)
                    .min((central_rect.max.y - size.y).max(central_rect.min.y)),
            );

            resolved[mover_idx].1 = Rect::from_min_size(clamped, size);

            // Write the snap if it actually moved.
            if let Some(plot) = self.plots.iter_mut().find(|p| p.plot_id() == mover_id) {
                if (clamped - original_rect.min).length() > 0.5 {
                    plot.set_pending_snap(clamped);
                }
            }
        }

        // Update snapshot.
        for &(id, rect) in &current {
            self.prev_rects.insert(id, rect);
        }
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

// ── Collision helpers ─────────────────────────────────────────────────────────

fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.min.x < b.max.x && a.max.x > b.min.x
        && a.min.y < b.max.y && a.max.y > b.min.y
}

/// Minimum displacement vector to move `moving` out of overlap with `other`.
/// Chooses the direction (horizontal or vertical) with the smallest magnitude.
fn min_separation_vec(moving: Rect, other: Rect) -> egui::Vec2 {
    // How far to move in each direction to just touch (not overlap).
    let dx_left  = other.min.x - moving.max.x; // negative → move left
    let dx_right = other.max.x - moving.min.x; // positive → move right
    let dy_up    = other.min.y - moving.max.y; // negative → move up
    let dy_down  = other.max.y - moving.min.y; // positive → move down

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

