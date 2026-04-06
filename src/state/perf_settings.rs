/// Performance tuning knobs exposed to the user via the Performance menu.
#[derive(Debug, Clone)]
pub struct PerformanceSettings {
    /// Maximum number of data points rendered per map plot per frame.
    /// Points are stride-sampled when the dataset exceeds this limit.
    /// Rendered as GPU quads (one draw call regardless of count), so this
    /// can be set much higher than the old CPU-painter limit.
    pub max_draw_points: usize,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            max_draw_points: 100_000,
        }
    }
}
