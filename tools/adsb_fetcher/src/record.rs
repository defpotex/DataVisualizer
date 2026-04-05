use serde::{Deserialize, Serialize};

/// One row in the output CSV — a single aircraft state vector at a point in time.
/// Column names are lowercase with units embedded, matching the schema heuristics
/// the main app will use for auto-detection in Phase 4.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AircraftState {
    /// Unix timestamp (seconds) from the OpenSky API response
    pub timestamp: i64,
    /// ICAO 24-bit address as hex string (e.g. "a1b2c3")
    pub icao24: String,
    /// Callsign / flight number, trimmed of whitespace
    pub callsign: String,
    /// WGS-84 latitude in decimal degrees
    pub lat: f64,
    /// WGS-84 longitude in decimal degrees
    pub lon: f64,
    /// Barometric altitude in metres (null → 0.0)
    pub altitude_m: f64,
    /// Ground speed in metres/second
    pub velocity_ms: f64,
    /// True track angle (heading) in degrees, 0=North clockwise
    pub heading_deg: f64,
    /// Vertical rate in metres/second (positive = climbing)
    pub vertical_rate_ms: f64,
    /// Whether the aircraft is on the ground
    pub on_ground: bool,
    /// Mode A squawk code (4-digit octal string, e.g. "1200")
    pub squawk: String,
}

impl AircraftState {
    /// Column headers for the CSV — must match field order in `to_csv_row`.
    pub fn csv_headers() -> &'static [&'static str] {
        &[
            "timestamp",
            "icao24",
            "callsign",
            "lat",
            "lon",
            "altitude_m",
            "velocity_ms",
            "heading_deg",
            "vertical_rate_ms",
            "on_ground",
            "squawk",
        ]
    }

    /// Serialize to a CSV string row (no newline).
    pub fn to_csv_row(&self) -> Vec<String> {
        vec![
            self.timestamp.to_string(),
            self.icao24.clone(),
            self.callsign.clone(),
            format!("{:.6}", self.lat),
            format!("{:.6}", self.lon),
            format!("{:.1}", self.altitude_m),
            format!("{:.2}", self.velocity_ms),
            format!("{:.1}", self.heading_deg),
            format!("{:.2}", self.vertical_rate_ms),
            self.on_ground.to_string(),
            self.squawk.clone(),
        ]
    }
}
