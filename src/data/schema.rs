/// Semantic meaning we detect from a column name / dtype.
/// Used by the UI to show type icons and by plots to suggest field roles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    Timestamp,
    Latitude,
    Longitude,
    Altitude,
    Speed,
    Heading,
    Flag,    // bool columns
    Integer,
    Float,
    Text,
}

impl FieldKind {
    /// Single-character icon shown next to each field in the left pane.
    pub fn icon(&self) -> &'static str {
        match self {
            FieldKind::Timestamp => "⏱",
            FieldKind::Latitude  => "◎",
            FieldKind::Longitude => "◎",
            FieldKind::Altitude  => "↑",
            FieldKind::Speed     => "→",
            FieldKind::Heading   => "⌖",
            FieldKind::Flag      => "◈",
            FieldKind::Integer   => "#",
            FieldKind::Float     => "~",
            FieldKind::Text      => "A",
        }
    }

    /// Whether this kind represents a numeric value (castable to f64).
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            FieldKind::Timestamp
                | FieldKind::Latitude
                | FieldKind::Longitude
                | FieldKind::Altitude
                | FieldKind::Speed
                | FieldKind::Heading
                | FieldKind::Integer
                | FieldKind::Float
        )
    }

    /// Short label shown after the icon.
    pub fn label(&self) -> &'static str {
        match self {
            FieldKind::Timestamp => "timestamp",
            FieldKind::Latitude  => "latitude",
            FieldKind::Longitude => "longitude",
            FieldKind::Altitude  => "altitude",
            FieldKind::Speed     => "speed",
            FieldKind::Heading   => "heading",
            FieldKind::Flag      => "bool",
            FieldKind::Integer   => "integer",
            FieldKind::Float     => "float",
            FieldKind::Text      => "text",
        }
    }
}

/// One column in a loaded dataset.
#[derive(Debug, Clone)]
pub struct FieldMeta {
    pub name: String,
    pub kind: FieldKind,
}

/// Detected schema for a DataSource.
#[derive(Debug, Clone, Default)]
pub struct DataSchema {
    pub fields: Vec<FieldMeta>,
}

impl DataSchema {
    /// Infer schema from a polars DataFrame by scanning column names and dtypes.
    pub fn infer(df: &polars::prelude::DataFrame) -> Self {

        let fields = df
            .get_columns()
            .iter()
            .map(|col| {
                let name = col.name().to_string();
                let dtype = col.dtype().clone();
                let kind = detect_kind(&name, &dtype);
                FieldMeta { name, kind }
            })
            .collect();

        DataSchema { fields }
    }

    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Returns the first field of each kind, if any.
    #[allow(dead_code)] // used by plot configuration in future phases
    pub fn first_of_kind(&self, kind: &FieldKind) -> Option<&FieldMeta> {
        self.fields.iter().find(|f| &f.kind == kind)
    }

    /// Get display name for a column using provided alias map.
    pub fn display_name<'a>(&'a self, col: &'a str, aliases: &'a std::collections::HashMap<String, String>) -> &'a str {
        aliases.get(col).map(|s| s.as_str()).unwrap_or(col)
    }
}

/// Heuristic: map a column name + polars dtype to a FieldKind.
fn detect_kind(name: &str, dtype: &polars::prelude::DataType) -> FieldKind {
    use polars::prelude::DataType;

    let n = name.to_lowercase();

    // Name-based detection first — more reliable than dtype alone
    if matches_any(&n, &["timestamp", "time", "datetime", "date", "_time", "_ts", "epoch"]) {
        return FieldKind::Timestamp;
    }
    if matches_any(&n, &["lat", "latitude", "_lat"]) {
        return FieldKind::Latitude;
    }
    if matches_any(&n, &["lon", "long", "longitude", "_lon", "lng", "_lng"]) {
        return FieldKind::Longitude;
    }
    if n.starts_with("alt") || n.contains("altitude") || n.ends_with("_alt") || n.ends_with("_m")
        && (n.contains("alt") || n.contains("elev") || n.contains("height"))
    {
        return FieldKind::Altitude;
    }
    if matches_any(&n, &["speed", "velocity", "_ms", "_kts", "_knots", "_mph", "_kph"]) {
        return FieldKind::Speed;
    }
    if matches_any(&n, &["heading", "track", "bearing", "course", "_deg", "direction"]) {
        return FieldKind::Heading;
    }

    // Dtype fallback
    match dtype {
        DataType::Boolean                        => FieldKind::Flag,
        DataType::Int8  | DataType::Int16
        | DataType::Int32 | DataType::Int64
        | DataType::UInt8 | DataType::UInt16
        | DataType::UInt32 | DataType::UInt64   => FieldKind::Integer,
        DataType::Float32 | DataType::Float64   => FieldKind::Float,
        _                                        => FieldKind::Text,
    }
}

/// Returns true if `name` exactly equals any pattern OR ends with any pattern.
fn matches_any(name: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| name == *p || name.ends_with(p))
}
