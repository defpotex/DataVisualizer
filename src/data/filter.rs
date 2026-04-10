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
    /// Column name to filter on.
    pub column: String,
    pub op: FilterOp,
    /// String representation of the threshold / set values.
    /// For In/NotIn this is pipe-separated: "a|b|c".
    pub value: String,
    /// Whether this filter is currently active.
    pub enabled: bool,
}

impl Filter {
    pub fn new(id: usize, column: String, op: FilterOp, value: String) -> Self {
        Self { id, column, op, value, enabled: true }
    }

    pub fn label(&self) -> String {
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
pub fn apply_filters(df: &DataFrame, filters: &[Filter]) -> DataFrame {
    let mut result = df.clone();
    for f in filters {
        if !f.enabled { continue; }
        if let Some(filtered) = try_apply(result.clone(), f) {
            result = filtered;
        }
    }
    result
}

fn try_apply(df: DataFrame, f: &Filter) -> Option<DataFrame> {
    // Handle set ops first (In / NotIn).
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
            FilterOp::In | FilterOp::NotIn => return None, // handled above
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

fn try_apply_set(df: DataFrame, f: &Filter) -> Option<DataFrame> {
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
