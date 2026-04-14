use polars::prelude::*;
use serde::{Deserialize, Serialize};

// ── FilterOp ──────────────────────────────────────────────────────────────────

/// A comparison operator for attribute filters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilterOp {
    Eq,
    NotEq,
    Gt,
    GtEq,
    Lt,
    LtEq,
    /// Value is in a set. `Filter.value` holds pipe-separated members: "a|b|c"
    In,
    /// Value is NOT in a set.
    NotIn,
    /// Keep only rows whose 0-based index is in the set.
    /// `Filter.value` holds pipe-separated row indices: "0|3|42".
    /// `Filter.column` is ignored.
    RowIndices,
}

impl FilterOp {
    pub fn label(&self) -> &str {
        match self {
            FilterOp::Eq    => "=",
            FilterOp::NotEq => "≠",
            FilterOp::Gt    => ">",
            FilterOp::GtEq  => "≥",
            FilterOp::Lt    => "<",
            FilterOp::LtEq  => "≤",
            FilterOp::In    => "in",
            FilterOp::NotIn => "not in",
            FilterOp::RowIndices => "selection",
        }
    }

    pub fn all() -> &'static [FilterOp] {
        &[
            FilterOp::Eq, FilterOp::NotEq,
            FilterOp::Gt, FilterOp::GtEq,
            FilterOp::Lt, FilterOp::LtEq,
            FilterOp::In, FilterOp::NotIn,
        ]
    }

    pub fn is_set_op(&self) -> bool {
        matches!(self, FilterOp::In | FilterOp::NotIn)
    }
}

// ── Filter ────────────────────────────────────────────────────────────────────

/// A single attribute filter: `column op value`.
/// For `In`/`NotIn`, `value` holds pipe-separated members: `"val1|val2|val3"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub id: usize,
    /// Optional source scope. `None` means the filter applies to all sources.
    #[serde(default)]
    pub source_id: Option<crate::data::source::SourceId>,
    /// Column name to filter on.
    pub column: String,
    pub op: FilterOp,
    /// String representation of the threshold / set values.
    /// For In/NotIn this is pipe-separated: "a|b|c".
    /// For RowIndices this is pipe-separated row indices: "0|3|42".
    pub value: String,
    /// Whether this filter is currently active.
    pub enabled: bool,
}

impl Filter {
    pub fn new(id: usize, column: String, op: FilterOp, value: String) -> Self {
        Self { id, source_id: None, column, op, value, enabled: true }
    }

    pub fn label(&self) -> String {
        if self.op == FilterOp::RowIndices {
            let count = self.value.split('|').filter(|s| !s.is_empty()).count();
            return format!("selection ({} pts)", count);
        }
        if self.op.is_set_op() {
            let members: Vec<&str> = self.value.split('|').collect();
            let preview = if members.len() <= 3 {
                members.join(", ")
            } else {
                format!("{}, {} more…", members[..2].join(", "), members.len() - 2)
            };
            format!("{} {} [{}]", self.column, self.op.label(), preview)
        } else {
            format!("{} {} {}", self.column, self.op.label(), self.value)
        }
    }
}

// ── apply_filters ─────────────────────────────────────────────────────────────

/// Apply all enabled filters to a DataFrame, returning a new (filtered) DataFrame.
/// Filters that fail to parse or apply are silently skipped.
/// `source_id` is the source being filtered; source-scoped filters that don't match are skipped.
pub fn apply_filters(df: &DataFrame, filters: &[Filter]) -> DataFrame {
    apply_filters_for_source(df, filters, None)
}

/// Name of the injected column that tracks original row indices through filtering.
pub const ORIG_ROW_COL: &str = "__orig_row_idx__";

/// Apply filters, optionally scoped to a specific source.
/// Injects an `__orig_row_idx__` column before filtering so that `RowIndices`
/// filters always refer to positions in the *original* (unfiltered) DataFrame.
pub fn apply_filters_for_source(df: &DataFrame, filters: &[Filter], source_id: Option<crate::data::source::SourceId>) -> DataFrame {
    puffin::profile_function!();
    // Inject original row index column if not already present.
    let mut result = if df.column(ORIG_ROW_COL).is_ok() {
        df.clone()
    } else {
        let idx_col = Column::new(ORIG_ROW_COL.into(), (0u64..df.height() as u64).collect::<Vec<_>>());
        let mut tmp = df.clone();
        let _ = tmp.with_column(idx_col);
        tmp
    };
    for f in filters {
        if !f.enabled { continue; }
        // Skip source-scoped filters that don't match this source.
        if let Some(filter_src) = f.source_id {
            if let Some(cur_src) = source_id {
                if filter_src != cur_src { continue; }
            }
        }
        if let Some(filtered) = try_apply(&result, f) {
            result = filtered;
        }
    }
    result
}

fn try_apply(df: &DataFrame, f: &Filter) -> Option<DataFrame> {
    // Handle row index selection filter.
    if f.op == FilterOp::RowIndices {
        return try_apply_row_indices(df, f);
    }

    // Handle set ops (In / NotIn).
    if f.op.is_set_op() {
        return try_apply_set(df, f);
    }

    // Comparison ops: try numeric first, fall back to string equality.
    let mask = if let Ok(val) = f.value.trim().parse::<f64>() {
        let col_series = df.column(&f.column).ok()?.as_series()?.clone();
        let cast = col_series.cast(&DataType::Float64).ok()?;
        let ca = cast.f64().ok()?;
        match f.op {
            FilterOp::Eq    => ca.equal(val).into_series(),
            FilterOp::NotEq => ca.not_equal(val).into_series(),
            FilterOp::Gt    => ca.gt(val).into_series(),
            FilterOp::GtEq  => ca.gt_eq(val).into_series(),
            FilterOp::Lt    => ca.lt(val).into_series(),
            FilterOp::LtEq  => ca.lt_eq(val).into_series(),
            FilterOp::In | FilterOp::NotIn | FilterOp::RowIndices => return None, // handled above
        }
    } else {
        // String equality / inequality only.
        let col_series = df.column(&f.column).ok()?.as_series()?.clone();
        let ca = col_series.cast(&DataType::String).ok()?;
        let ca = ca.str().ok()?;
        match f.op {
            FilterOp::Eq    => ca.equal(f.value.trim()).into_series(),
            FilterOp::NotEq => ca.not_equal(f.value.trim()).into_series(),
            _ => return None,
        }
    };

    let bool_ca = mask.bool().ok()?;
    df.filter(bool_ca).ok()
}

fn try_apply_set(df: &DataFrame, f: &Filter) -> Option<DataFrame> {
    let candidates: Vec<&str> = f.value.split('|')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if candidates.is_empty() { return None; }

    let col = df.column(&f.column).ok()?.as_series()?.clone();
    let str_col = col.cast(&DataType::String).ok()?;
    let ca = str_col.str().ok()?;

    // OR together one mask per candidate value.
    let mut combined: Option<BooleanChunked> = None;
    for candidate in &candidates {
        let m = ca.equal(*candidate);
        combined = Some(match combined {
            None => m,
            Some(c) => c | m,
        });
    }

    let mask = combined.unwrap_or_else(|| BooleanChunked::full("".into(), false, df.height()));
    let mask = if matches!(f.op, FilterOp::NotIn) { !mask } else { mask };
    df.filter(&mask).ok()
}

fn try_apply_row_indices(df: &DataFrame, f: &Filter) -> Option<DataFrame> {
    let wanted: std::collections::HashSet<u64> = f.value.split('|')
        .filter_map(|s| s.trim().parse::<u64>().ok())
        .collect();
    if wanted.is_empty() { return None; }

    // Filter by the __orig_row_idx__ column so indices always refer to the
    // original (unfiltered) DataFrame, regardless of earlier filters.
    let col = df.column(ORIG_ROW_COL).ok()?.as_series()?.clone();
    let ca = col.u64().ok()?;
    let mask: BooleanChunked = ca.into_iter()
        .map(|opt_val| opt_val.map(|v| wanted.contains(&v)))
        .collect();
    df.filter(&mask).ok()
}

// ── Distinct values ───────────────────────────────────────────────────────────

/// Compute up to `limit` sorted distinct string values for `col_name` across all sources.
/// Used by the filter dialog to populate the value picker.
pub fn distinct_values(
    sources: &[crate::data::source::DataSource],
    col_name: &str,
    limit: usize,
) -> Vec<String> {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    'outer: for source in sources {
        let col = match source.df.column(col_name) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let series = match col.as_series() {
            Some(s) => s.clone(),
            None => continue,
        };
        let cast = series.cast(&DataType::String).unwrap_or(series);
        if let Ok(ca) = cast.str() {
            for val in ca.into_iter().flatten() {
                seen.insert(val.to_string());
                if seen.len() >= limit {
                    break 'outer;
                }
            }
        }
    }
    seen.into_iter().collect()
}
