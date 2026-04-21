use crate::data::schema::DataSchema;
use polars::prelude::DataFrame;
use std::collections::HashMap;
use std::path::PathBuf;

/// Unique identifier for a data source within a session.
pub type SourceId = usize;

/// A loaded dataset — file metadata + schema + the actual data.
#[derive(Debug, Clone)]
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
    /// User-assigned column aliases: original_name → display_name.
    /// When set, the alias is shown in all column pickers and legends.
    #[allow(dead_code)]
    pub column_aliases: HashMap<String, String>,
}

impl DataSource {
    pub fn row_count(&self) -> usize {
        self.df.height()
    }

    pub fn field_count(&self) -> usize {
        self.schema.field_count()
    }

    /// Get the display name for a column (alias if set, otherwise original name).
    pub fn display_name<'a>(&'a self, original: &'a str) -> &'a str {
        self.column_aliases.get(original).map(|s| s.as_str()).unwrap_or(original)
    }

    /// Get the original column name from a display name (reverse alias lookup).
    pub fn original_name<'a>(&'a self, display: &'a str) -> &'a str {
        for (orig, alias) in &self.column_aliases {
            if alias == display {
                return orig;
            }
        }
        display
    }
}
