/// Controls whether batched mesh rendering is used for point plots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuPointsMode {
    /// Always use individual `circle_filled` calls (CPU tessellation).
    Off,
    /// Always use pre-tessellated mesh batching.
    On,
    /// Use mesh batching when point count exceeds the threshold.
    Auto,
}

/// Performance tuning knobs exposed to the user via the Performance menu.
#[derive(Debug, Clone)]
pub struct PerformanceSettings {
    /// Maximum number of data points rendered per map plot per frame.
    /// Points are stride-sampled when the dataset exceeds this limit.
    pub max_draw_points: usize,
    /// Show the puffin profiler window (flame graph).
    pub show_profiler: bool,
    /// Whether to use batched mesh rendering for point plots.
    pub gpu_points_mode: GpuPointsMode,
    /// Point count threshold for Auto mode. When the number of visible
    /// points exceeds this value, batched mesh rendering kicks in.
    pub gpu_points_threshold: usize,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            max_draw_points: 100_000,
            show_profiler: false,
            gpu_points_mode: GpuPointsMode::Auto,
            gpu_points_threshold: 5_000,
        }
    }
}
