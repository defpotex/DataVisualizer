use crate::record::AircraftState;
use serde::Deserialize;

/// OpenSky `/states/all` top-level response envelope.
/// The `states` field is a JSON array of arrays (not objects).
#[derive(Debug, Deserialize)]
pub struct OpenSkyResponse {
    /// Unix timestamp of this snapshot (seconds)
    pub time: i64,
    /// Each inner Vec is one aircraft state vector.
    /// Fields are positional — see STATE_* constants below.
    #[serde(default)]
    pub states: Option<Vec<serde_json::Value>>,
}

// Positional indices into each state vector array
const IDX_ICAO24: usize = 0;
const IDX_CALLSIGN: usize = 1;
const IDX_ON_GROUND: usize = 8;
const IDX_VELOCITY: usize = 9;
const IDX_HEADING: usize = 10;
const IDX_VERT_RATE: usize = 11;
const IDX_LAT: usize = 6;
const IDX_LON: usize = 5;
const IDX_BARO_ALT: usize = 7;
const IDX_SQUAWK: usize = 14;

/// Bounding box for geographic filtering.
#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub lon_min: f64,
    pub lat_min: f64,
    pub lon_max: f64,
    pub lat_max: f64,
}

impl BoundingBox {
    /// CONUS default: roughly contiguous United States
    pub fn conus() -> Self {
        Self { lon_min: -130.0, lat_min: 24.0, lon_max: -60.0, lat_max: 50.0 }
    }

    /// Parse from "W,S,E,N" string
    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 4 {
            return Err(format!("Expected W,S,E,N — got {:?}", s));
        }
        let nums: Result<Vec<f64>, _> = parts.iter().map(|p| p.trim().parse::<f64>()).collect();
        let nums = nums.map_err(|e| format!("Invalid number in bbox: {}", e))?;
        Ok(Self { lon_min: nums[0], lat_min: nums[1], lon_max: nums[2], lat_max: nums[3] })
    }

    pub fn query_params(&self) -> String {
        format!(
            "lamin={}&lomin={}&lamax={}&lomax={}",
            self.lat_min, self.lon_min, self.lat_max, self.lon_max
        )
    }
}

/// Fetch one snapshot from the OpenSky REST API.
/// Returns `Ok(Vec<AircraftState>)` — may be empty if no aircraft in bbox.
/// Returns `Err(String)` on HTTP or parse failure (caller should warn + skip).
pub fn fetch_states(bbox: &BoundingBox) -> Result<(i64, Vec<AircraftState>), String> {
    let url = format!(
        "https://opensky-network.org/api/states/all?{}",
        bbox.query_params()
    );

    let response = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(|e| format!("HTTP error: {}", e))?;

    if response.status() == 429 {
        return Err("Rate limited by OpenSky (429) — consider increasing --interval".to_string());
    }
    if response.status() != 200 {
        return Err(format!("OpenSky returned HTTP {}", response.status()));
    }

    let body: OpenSkyResponse = response
        .into_json()
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let snapshot_time = body.time;
    let states = body.states.unwrap_or_default();
    let mut records = Vec::with_capacity(states.len());

    for state in &states {
        if let Some(record) = parse_state_vector(snapshot_time, state) {
            records.push(record);
        }
    }

    Ok((snapshot_time, records))
}

/// Parse one positional state vector array into an AircraftState.
/// Returns None if lat/lon are null (aircraft not broadcasting position).
fn parse_state_vector(timestamp: i64, state: &serde_json::Value) -> Option<AircraftState> {
    let arr = state.as_array()?;

    // lat/lon must be present — skip ground vehicles without position
    let lat = arr.get(IDX_LAT)?.as_f64()?;
    let lon = arr.get(IDX_LON)?.as_f64()?;

    let icao24 = arr.get(IDX_ICAO24)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let callsign = arr.get(IDX_CALLSIGN)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let on_ground = arr.get(IDX_ON_GROUND)
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let altitude_m = arr.get(IDX_BARO_ALT)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let velocity_ms = arr.get(IDX_VELOCITY)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let heading_deg = arr.get(IDX_HEADING)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let vertical_rate_ms = arr.get(IDX_VERT_RATE)
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let squawk = arr.get(IDX_SQUAWK)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Some(AircraftState {
        timestamp,
        icao24,
        callsign,
        lat,
        lon,
        altitude_m,
        velocity_ms,
        heading_deg,
        vertical_rate_ms,
        on_ground,
        squawk,
    })
}
