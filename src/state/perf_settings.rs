/// Performance tuning knobs exposed to the user via the Performance menu.
#[derive(Debug, Clone)]
pub struct PerformanceSettings {
    /// Maximum number of data points rendered per map plot per frame.
    /// Points are stride-sampled when the dataset exceeds this limit.
    pub max_draw_points: usize,
    /// Show the puffin profiler window (flame graph).
    pub show_profiler: bool,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            max_draw_points: 100_000,
            show_profiler: false,
        }
    }
}
