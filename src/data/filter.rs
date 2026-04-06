use polars::prelude::*;
use serde::{Deserialize, Serialize};

/// A comparison operator for attribute filters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilterOp {
    Eq,
    NotEq,
    Gt,
    GtEq,
    Lt,
    LtEq,
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
        }
    }

    pub fn all() -> &'static [FilterOp] {
        &[
            FilterOp::Eq, FilterOp::NotEq,
            FilterOp::Gt, FilterOp::GtEq,
            FilterOp::Lt, FilterOp::LtEq,
        ]
    }
}

/// A single attribute filter: `column op value`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filter {
    pub id: usize,
    /// Column name to filter on.
    pub column: String,
    pub op: FilterOp,
    /// String representation of the threshold value (parsed at apply time).
    pub value: String,
    /// Whether this filter is currently active.
    pub enabled: bool,
}

impl Filter {
    pub fn new(id: usize, column: String, op: FilterOp, value: String) -> Self {
        Self { id, column, op, value, enabled: true }
    }

    pub fn label(&self) -> String {
        format!("{} {} {}", self.column, self.op.label(), self.value)
    }
}

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
    // Try numeric first, then fall back to string equality.
    let mask = if let Ok(val) = f.value.trim().parse::<f64>() {
        let col_series = df.column(&f.column).ok()?.as_series()?.clone();
        let cast = col_series.cast(&DataType::Float64).ok()?;
        let ca = cast.f64().ok()?;
        let val_s = Series::new("v".into(), &[val]);
        match f.op {
            FilterOp::Eq    => ca.equal(val).ok()?.into_series(),
            FilterOp::NotEq => ca.not_equal(val).ok()?.into_series(),
            FilterOp::Gt    => ca.gt(val).ok()?.into_series(),
            FilterOp::GtEq  => ca.gt_eq(val).ok()?.into_series(),
            FilterOp::Lt    => ca.lt(val).ok()?.into_series(),
            FilterOp::LtEq  => ca.lt_eq(val).ok()?.into_series(),
            _ => { let _ = val_s; return None; }
        }
    } else {
        // String equality only for non-numeric values.
        let col_series = df.column(&f.column).ok()?.as_series()?.clone();
        let ca = col_series.cast(&DataType::String).ok()?;
        let ca = ca.str().ok()?;
        match f.op {
            FilterOp::Eq    => ca.equal(f.value.trim()).into_series(),
            FilterOp::NotEq => ca.not_equal(f.value.trim()).into_series(),
            _ => return None, // ordering on strings not supported
        }
    };

    let bool_ca = mask.bool().ok()?;
    df.filter(bool_ca).ok()
}
