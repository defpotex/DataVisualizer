//! Background sync machinery for plot data computation.
//!
//! When a plot's config or filters change, the expensive data extraction and
//! color/size/alpha computation is offloaded to a background thread.  The UI
//! thread shows a "Computing…" overlay and remains responsive.  A cancel token
//! lets the user abort long-running computations.

use crate::data::schema::DataSchema;
use crate::plot::styling::PlotLegendData;
use egui::Color32;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ── Cancel token ─────────────────────────────────────────────────────────────

/// Shared flag that the UI thread sets to `true` to request cancellation.
#[derive(Clone)]
pub struct CancelToken(pub Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

// ── Sync results ─────────────────────────────────────────────────────────────

/// The precomputed data produced by a background sync for a scatter plot.
pub struct ScatterSyncResult {
    pub plot_id: usize,
    pub schema: DataSchema,
    pub points: Vec<[f64; 2]>,
    pub colors: Vec<Color32>,
    pub radii: Vec<f32>,
    pub hover_labels: Vec<String>,
    pub row_indices: Vec<usize>,
    pub category_entries: Vec<(String, Color32)>,
    /// Per-point category index into `category_entries`. `None` for non-categorical modes.
    pub category_indices: Vec<Option<usize>>,
    pub x_labels: Vec<String>,
    pub y_labels: Vec<String>,
    pub legend: PlotLegendData,
}

/// The precomputed data produced by a background sync for a map plot.
pub struct MapSyncResult {
    pub plot_id: usize,
    pub schema: DataSchema,
    pub points: Vec<[f64; 2]>,
    pub colors: Vec<Color32>,
    pub radii: Vec<f32>,
    pub hover_labels: Vec<String>,
    pub row_indices: Vec<usize>,
    pub legend: PlotLegendData,
}

/// The precomputed data produced by a background sync for a scroll chart.
pub struct ScrollChartSyncResult {
    pub plot_id: usize,
    pub schema: DataSchema,
    /// Time values for each row.
    pub times: Vec<f64>,
    /// One series per Y column: (col_name, values).
    pub series: Vec<(String, Vec<f64>)>,
}

/// Wrapper that goes through the event channel.
pub enum PlotSyncEvent {
    ScatterReady(ScatterSyncResult),
    MapReady(MapSyncResult),
    ScrollChartReady(ScrollChartSyncResult),
    /// The background computation was cancelled (no data to apply).
    Cancelled { plot_id: usize },
}
