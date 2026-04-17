use crate::data::schema::FieldKind;
use crate::data::source::SourceId;
use serde::{Deserialize, Serialize};

// ── TileScheme ────────────────────────────────────────────────────────────────

/// Which tile provider to use for map backgrounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TileScheme {
    OpenStreetMap,
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

// ── Colormap ──────────────────────────────────────────────────────────────────

/// Color gradient used for continuous data encoding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum Colormap {
    #[default]
    Viridis,
    Plasma,
    Inferno,
    Turbo,
    Grayscale,
}

impl Colormap {
    pub fn label(&self) -> &str {
        match self {
            Colormap::Viridis   => "Viridis",
            Colormap::Plasma    => "Plasma",
            Colormap::Inferno   => "Inferno",
            Colormap::Turbo     => "Turbo",
            Colormap::Grayscale => "Grayscale",
        }
    }

    pub fn all() -> &'static [Colormap] {
        &[Colormap::Viridis, Colormap::Plasma, Colormap::Inferno, Colormap::Turbo, Colormap::Grayscale]
    }
}

// ── ColorMode ─────────────────────────────────────────────────────────────────

/// How data points are colored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColorMode {
    /// All points share the same theme accent color.
    Solid,
    /// Points are colored by distinct values in a column — one color per unique value.
    Categorical { col: String },
    /// Points are colored by a numeric column mapped through a colormap.
    Continuous {
        col: String,
        colormap: Colormap,
        /// User-specified minimum value for color mapping (None = auto from data).
        #[serde(default)]
        color_min: Option<f64>,
        /// User-specified maximum value for color mapping (None = auto from data).
        #[serde(default)]
        color_max: Option<f64>,
        /// Reverse the colormap direction.
        #[serde(default)]
        reverse: bool,
    },
}

impl Default for ColorMode {
    fn default() -> Self { ColorMode::Solid }
}

impl ColorMode {
    pub fn label(&self) -> &str {
        match self {
            ColorMode::Solid => "Solid",
            ColorMode::Categorical { .. } => "By Category",
            ColorMode::Continuous { .. } => "By Value",
        }
    }

    pub fn variant_idx(&self) -> usize {
        match self {
            ColorMode::Solid => 0,
            ColorMode::Categorical { .. } => 1,
            ColorMode::Continuous { .. } => 2,
        }
    }
}

// ── SizeConfig ────────────────────────────────────────────────────────────────

/// Maps a numeric column to point radius (scatter plots only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SizeConfig {
    /// Column to drive point size.
    pub col: String,
    /// Radius in screen pixels at the column minimum.
    pub min_px: f32,
    /// Radius in screen pixels at the column maximum.
    pub max_px: f32,
}

impl Default for SizeConfig {
    fn default() -> Self {
        Self { col: String::new(), min_px: 2.0, max_px: 10.0 }
    }
}

// ── AlphaConfig ───────────────────────────────────────────────────────────────

/// Maps a numeric column to point opacity (scatter plots only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlphaConfig {
    /// Column to drive point opacity.
    pub col: String,
    /// Opacity (0–1) at the column minimum.
    pub min_alpha: f32,
    /// Opacity (0–1) at the column maximum.
    pub max_alpha: f32,
}

impl Default for AlphaConfig {
    fn default() -> Self {
        Self { col: String::new(), min_alpha: 0.2, max_alpha: 1.0 }
    }
}

// ── MapPlotConfig ─────────────────────────────────────────────────────────────

/// Configuration for a geographic map plot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapPlotConfig {
    pub id: usize,
    pub title: String,
    pub source_id: SourceId,
    pub lat_col: String,
    pub lon_col: String,
    pub tile_scheme: TileScheme,
    #[serde(default)]
    pub color_mode: ColorMode,
    #[serde(default)]
    pub size_config: Option<SizeConfig>,
    #[serde(default)]
    pub alpha_config: Option<AlphaConfig>,
    /// Extra columns shown in the hover tooltip (lat and lon are always included).
    #[serde(default)]
    pub hover_fields: Vec<String>,
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
    #[serde(default)]
    pub x_scale: AxisScale,
    #[serde(default)]
    pub y_scale: AxisScale,
    #[serde(default)]
    pub color_mode: ColorMode,
    #[serde(default)]
    pub size_config: Option<SizeConfig>,
    #[serde(default)]
    pub alpha_config: Option<AlphaConfig>,
    /// Extra columns shown in the hover tooltip (x and y are always included).
    #[serde(default)]
    pub hover_fields: Vec<String>,
}

// ── Threshold ────────────────────────────────────────────────────────────────

/// A threshold/tripwire line on a scroll chart. When the value crosses this
/// threshold, the chart region color changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Threshold {
    /// The Y value for the horizontal threshold line.
    pub value: f64,
    /// Color to fill the chart region above this threshold (RGBA).
    pub above_color: [u8; 4],
    /// Color to fill the chart region below this threshold (RGBA).
    pub below_color: [u8; 4],
    /// Label shown next to the threshold line.
    pub label: String,
}

impl Default for Threshold {
    fn default() -> Self {
        Self {
            value: 0.0,
            above_color: [255, 85, 85, 40],   // red tint
            below_color: [80, 225, 130, 40],   // green tint
            label: String::new(),
        }
    }
}

// ── ScrollChartConfig ────────────────────────────────────────────────────────

/// Configuration for a rolling time-series scroll chart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollChartConfig {
    pub id: usize,
    pub title: String,
    pub source_id: SourceId,
    /// Column used for the X (time) axis.
    pub time_col: String,
    /// Columns to plot as Y series.
    pub y_cols: Vec<String>,
    /// Number of seconds (or raw units) of history to show.
    #[serde(default = "default_window_secs")]
    pub window_secs: f64,
    /// Threshold/tripwire lines.
    #[serde(default)]
    pub thresholds: Vec<Threshold>,
}

fn default_window_secs() -> f64 {
    60.0
}

// ── PlotConfig ────────────────────────────────────────────────────────────────

/// Top-level discriminated union of all plot types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlotConfig {
    Map(MapPlotConfig),
    Scatter(ScatterPlotConfig),
    ScrollChart(ScrollChartConfig),
}

impl PlotConfig {
    pub fn id(&self) -> usize {
        match self {
            PlotConfig::Map(c) => c.id,
            PlotConfig::Scatter(c) => c.id,
            PlotConfig::ScrollChart(c) => c.id,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            PlotConfig::Map(c) => &c.title,
            PlotConfig::Scatter(c) => &c.title,
            PlotConfig::ScrollChart(c) => &c.title,
        }
    }

    pub fn source_id(&self) -> SourceId {
        match self {
            PlotConfig::Map(c) => c.source_id,
            PlotConfig::Scatter(c) => c.source_id,
            PlotConfig::ScrollChart(c) => c.source_id,
        }
    }
}
