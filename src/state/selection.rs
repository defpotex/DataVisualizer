use crate::data::source::SourceId;
use std::collections::HashSet;

/// A set of selected data point indices, scoped to a specific source.
/// Only one selection is active at a time (global, like Tableau).
#[derive(Debug, Clone)]
pub struct SelectionSet {
    /// Which plot originated this selection.
    pub plot_id: usize,
    /// Which data source the indices refer to.
    pub source_id: SourceId,
    /// Row indices in the *original* (unfiltered) DataFrame.
    pub indices: HashSet<usize>,
    /// Monotonically increasing version — bumped on every mutation.
    version: u64,
}

impl SelectionSet {
    pub fn new(plot_id: usize, source_id: SourceId) -> Self {
        Self { plot_id, source_id, indices: HashSet::new(), version: 0 }
    }

    pub fn single(plot_id: usize, source_id: SourceId, idx: usize) -> Self {
        let mut s = Self::new(plot_id, source_id);
        s.indices.insert(idx);
        s.version = 1;
        s
    }

    pub fn from_indices(plot_id: usize, source_id: SourceId, indices: impl IntoIterator<Item = usize>) -> Self {
        let indices: HashSet<usize> = indices.into_iter().collect();
        Self { plot_id, source_id, indices, version: 1 }
    }

    pub fn toggle(&mut self, idx: usize) {
        if !self.indices.remove(&idx) {
            self.indices.insert(idx);
        }
        self.version += 1;
    }

    pub fn is_empty(&self) -> bool { self.indices.is_empty() }
    pub fn len(&self) -> usize { self.indices.len() }
    pub fn contains(&self, idx: usize) -> bool { self.indices.contains(&idx) }
    pub fn version(&self) -> u64 { self.version }
}
