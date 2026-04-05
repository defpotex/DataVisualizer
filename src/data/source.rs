use crate::data::schema::DataSchema;
use polars::prelude::DataFrame;
use std::path::PathBuf;

/// Unique identifier for a data source within a session.
pub type SourceId = usize;

/// A loaded dataset — file metadata + schema + the actual data.
#[derive(Debug)]
pub struct DataSource {
    pub id: SourceId,
    /// Display label (filename stem)
    pub label: String,
    /// Full path to the original file (None for streams)
    #[allow(dead_code)] // used by session persistence in Phase 13
    pub path: Option<PathBuf>,
    /// Detected schema (field names + kinds)
    pub schema: DataSchema,
    /// The actual data
    pub df: DataFrame,
}

impl DataSource {
    pub fn row_count(&self) -> usize {
        self.df.height()
    }

    pub fn field_count(&self) -> usize {
        self.schema.field_count()
    }
}
