use crate::data::schema::FieldKind;
use crate::data::source::SourceId;
use serde::{Deserialize, Serialize};

// ── TileScheme ────────────────────────────────────────────────────────────────

/// Which tile provider to use for map backgrounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TileScheme {
    /// Standard OpenStreetMap tiles (light, requires internet).
    OpenStreetMap,
    /// Carto Dark Matter tiles (dark theme, requires internet).
    CartoDark,
}

impl TileScheme {
    pub fn label(&self) -> &str {
        match self {
            TileScheme::OpenStreetMap => "OpenStreetMap (Light)",
            TileScheme::CartoDark => "Carto Dark Matter",
        }
    }

    pub fn all() -> &'static [TileScheme] {
        &[TileScheme::OpenStreetMap, TileScheme::CartoDark]
    }
}

// ── AxisScale ─────────────────────────────────────────────────────────────────

/// Whether an axis encodes values continuously (numeric) or categorically (discrete labels).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum AxisScale {
    #[default]
    Continuous,
    Categorical,
}

impl AxisScale {
    /// Infer axis scale from a field kind. Text/Flag → Categorical, everything else → Continuous.
    pub fn infer(kind: &FieldKind) -> Self {
        match kind {
            FieldKind::Text | FieldKind::Flag => AxisScale::Categorical,
            _ => AxisScale::Continuous,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            AxisScale::Continuous => "Continuous",
            AxisScale::Categorical => "Categorical",
        }
    }
}

// ── MapPlotConfig ─────────────────────────────────────────────────────────────

/// Configuration for a geographic map plot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapPlotConfig {
    /// Unique plot ID within the session.
    pub id: usize,
    /// User-visible title.
    pub title: String,
    /// Which data source to render.
    pub source_id: SourceId,
    /// Name of the latitude column.
    pub lat_col: String,
    /// Name of the longitude column.
    pub lon_col: String,
    /// Optional column used to color-code points (future feature).
    pub color_col: Option<String>,
    /// Tile provider.
    pub tile_scheme: TileScheme,
}

// ── ScatterPlotConfig ─────────────────────────────────────────────────────────

/// Configuration for an X/Y scatter plot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScatterPlotConfig {
    pub id: usize,
    pub title: String,
    pub source_id: SourceId,
    pub x_col: String,
    pub y_col: String,
    /// Optional column for color-coding points (future: Phase 7).
    pub color_col: Option<String>,
    /// Axis encoding for X — auto-inferred from field kind, user-overridable.
    #[serde(default)]
    pub x_scale: AxisScale,
    /// Axis encoding for Y — auto-inferred from field kind, user-overridable.
    #[serde(default)]
    pub y_scale: AxisScale,
}

// ── PlotConfig ────────────────────────────────────────────────────────────────

/// Top-level discriminated union of all plot types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlotConfig {
    Map(MapPlotConfig),
    Scatter(ScatterPlotConfig),
}

impl PlotConfig {
    pub fn id(&self) -> usize {
        match self {
            PlotConfig::Map(c) => c.id,
            PlotConfig::Scatter(c) => c.id,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            PlotConfig::Map(c) => &c.title,
            PlotConfig::Scatter(c) => &c.title,
        }
    }

    pub fn source_id(&self) -> SourceId {
        match self {
            PlotConfig::Map(c) => c.source_id,
            PlotConfig::Scatter(c) => c.source_id,
        }
    }
}
