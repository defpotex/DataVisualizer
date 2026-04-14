use egui::Pos2;

/// A simple 2D grid that buckets screen-space points for O(1) average-case
/// nearest-neighbor queries. Cell size is chosen so the average cell has ~4
/// points, giving fast lookups regardless of dataset size.
pub struct SpatialGrid {
    cell_size: f32,
    origin: Pos2,
    cols: usize,
    rows: usize,
    /// Each cell stores indices into the source points slice.
    cells: Vec<Vec<usize>>,
}

impl SpatialGrid {
    /// Build a spatial grid from a slice of items. The `pos_fn` closure
    /// extracts the screen position from each item.
    pub fn build<T>(items: &[T], clip: egui::Rect, pos_fn: impl Fn(&T) -> Pos2) -> Self {
        // Target ~4 points per cell; clamp cell size to [10, 80] px.
        let n = items.len().max(1);
        let area = clip.width() * clip.height();
        let cell_size = (area / (n as f32 / 4.0)).sqrt().clamp(10.0, 80.0);
        let cols = ((clip.width() / cell_size).ceil() as usize).max(1);
        let rows = ((clip.height() / cell_size).ceil() as usize).max(1);
        let origin = clip.min;
        let mut cells = vec![Vec::new(); cols * rows];

        for (i, item) in items.iter().enumerate() {
            let pos = pos_fn(item);
            let c = ((pos.x - origin.x) / cell_size) as usize;
            let r = ((pos.y - origin.y) / cell_size) as usize;
            if c < cols && r < rows {
                cells[r * cols + c].push(i);
            }
        }

        Self { cell_size, origin, cols, rows, cells }
    }

    /// Find the nearest item to `query` by checking the cell it falls in
    /// plus all 8 neighbors. Returns the index and reference into `items`.
    pub fn find_nearest<'a, T>(
        &self,
        query: Pos2,
        items: &'a [T],
        pos_fn: impl Fn(&T) -> Pos2,
    ) -> Option<(usize, &'a T)> {
        let gc = ((query.x - self.origin.x) / self.cell_size) as isize;
        let gr = ((query.y - self.origin.y) / self.cell_size) as isize;

        let mut best_idx = None;
        let mut best_dist = f32::MAX;

        for dr in -1..=1 {
            for dc in -1..=1 {
                let r = gr + dr;
                let c = gc + dc;
                if r < 0 || c < 0 || r >= self.rows as isize || c >= self.cols as isize {
                    continue;
                }
                for &pt_idx in &self.cells[r as usize * self.cols + c as usize] {
                    let d = pos_fn(&items[pt_idx]).distance(query);
                    if d < best_dist {
                        best_dist = d;
                        best_idx = Some(pt_idx);
                    }
                }
            }
        }

        best_idx.map(|i| (i, &items[i]))
    }
}
