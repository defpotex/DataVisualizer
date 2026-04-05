use crate::data::source::SourceId;
use serde::{Deserialize, Serialize};

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

/// Top-level discriminated union of all plot types.
/// Extend with `Scatter(ScatterPlotConfig)`, `Bar(BarPlotConfig)`, etc. in future phases.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlotConfig {
    Map(MapPlotConfig),
}

impl PlotConfig {
    pub fn id(&self) -> usize {
        match self {
            PlotConfig::Map(c) => c.id,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            PlotConfig::Map(c) => &c.title,
        }
    }
}
