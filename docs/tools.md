# Developer Tools

These are standalone CLI binaries in the `tools/` workspace members. They are **not** part of the main application — they exist to generate and replay test data for development and testing.

```
tools/
├── adsb_fetcher/    ← Phase 2: fetch live ADS-B traffic → CSV + Parquet
└── udp_streamer/    ← Phase 3: replay any CSV over UDP (not yet implemented)
```

Output is written to `test_data/` (git-ignored). Create it if it doesn't exist:
```bash
mkdir test_data
```

---

## `adsb_fetcher` — ADS-B CSV Generator

Polls the [OpenSky Network](https://opensky-network.org/apidoc/rest.html) public REST API and writes aircraft state vectors to CSV and Parquet. No API key required.

### Quick Start

```bash
# Default: CONUS bounding box, 60-minute collection, 60s poll interval
cargo run -p adsb_fetcher

# Short test run — 2 polls, 15 seconds apart, Florida region
cargo run -p adsb_fetcher -- --duration 1 --interval 15 --bbox -80,25,-65,35 --name adsb_test

# Run indefinitely until Ctrl+C
cargo run -p adsb_fetcher -- --duration 0
```

### Options

| Flag | Default | Description |
|---|---|---|
| `--output <DIR>` | `test_data/` | Directory to write output files |
| `--duration <MINS>` | `60` | Total collection time in minutes. `0` = run until Ctrl+C |
| `--interval <SECS>` | `60` | Seconds between API polls. Minimum: `10` (OpenSky rate limit) |
| `--bbox <W,S,E,N>` | `-130,24,-60,50` | Geographic bounding box in decimal degrees (CONUS default) |
| `--name <STEM>` | `adsb_conus` | Output filename stem (no extension) |
| `--help` | — | Print usage and exit |

### Output Files

All files are written to `--output` directory:

| File | Description |
|---|---|
| `<name>.csv` | Aircraft state vectors, sorted by timestamp, 11 columns |
| `<name>.parquet` | Same data, Snappy-compressed (~6× smaller than CSV) |
| `<name>.meta.json` | Row count, poll count, time range, bounding box, column list |

### CSV Schema

```
timestamp,icao24,callsign,lat,lon,altitude_m,velocity_ms,heading_deg,vertical_rate_ms,on_ground,squawk
```

| Column | Type | Description |
|---|---|---|
| `timestamp` | integer | Unix epoch seconds (from OpenSky snapshot time) |
| `icao24` | string | ICAO 24-bit aircraft address (hex, e.g. `a1b2c3`) |
| `callsign` | string | Flight number / callsign (e.g. `UAL123`), may be empty |
| `lat` | float | WGS-84 latitude, decimal degrees |
| `lon` | float | WGS-84 longitude, decimal degrees |
| `altitude_m` | float | Barometric altitude in metres (0.0 if unknown) |
| `velocity_ms` | float | Ground speed in metres/second |
| `heading_deg` | float | True track angle in degrees (0 = North, clockwise) |
| `vertical_rate_ms` | float | Vertical rate in m/s (positive = climbing) |
| `on_ground` | bool | `true` if aircraft is reporting on-ground |
| `squawk` | string | Mode A squawk code (4-digit octal, e.g. `1200`), may be empty |

### Example Output

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  ADS-B Fetcher
  Output : test_data/adsb_conus.csv
  BBox   : W=-130 S=24 E=-60 N=50
  Mode   : 60 min, 61 polls planned
  Interval: 60s
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Poll 1/61 — fetching… 1043 aircraft  |     1043 rows total  |  t=1712001600
  Poll 2/61 — fetching… 1051 aircraft  |     2094 rows total  |  t=1712001660
  ...
  Finalizing output (63204 rows)…
    Sorting by timestamp…

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Done!
  Rows     : 63204
  Polls    : 61
  CSV      : test_data/adsb_conus.csv (18.4 MB)
  Parquet  : test_data/adsb_conus.parquet (3.1 MB)
  Metadata : test_data/adsb_conus.meta.json
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### Notes

- **OpenSky rate limit:** Anonymous requests are limited to ~10/minute. The default 60s interval keeps well within this. The tool enforces a minimum of 10s.
- **Null positions:** Aircraft not broadcasting a position are automatically filtered out. Only rows with valid `lat`/`lon` are written.
- **Ctrl+C:** Sends a graceful shutdown signal. The current poll completes, then the tool sorts, writes Parquet, and exits cleanly. Output is always valid.
- **Network errors:** A failed poll prints a warning and is skipped — the tool never crashes on a single HTTP failure.
- **Expected volume:** CONUS at 60s intervals typically yields ~800–1200 aircraft per poll and ~50,000–80,000 rows per hour of collection.

### Recommended Bounding Boxes

| Region | `--bbox` value |
|---|---|
| CONUS (default) | `-130,24,-60,50` |
| Eastern US | `-90,25,-65,47` |
| Western US | `-130,30,-100,50` |
| Florida | `-88,24,-79,31` |
| Europe | `-10,36,30,60` |
| North Atlantic | `-60,40,-10,60` |

---

## `udp_streamer` — UDP Replay Streamer

> **Not yet implemented** — planned for Phase 3.

Will replay any CSV file over UDP as newline-delimited rows, respecting timestamp ordering and supporting a configurable speed multiplier.

**Planned usage:**
```bash
# Replay ADS-B data at 10× speed to localhost:5005
cargo run -p udp_streamer -- --file test_data/adsb_conus.csv --target 127.0.0.1:5005 --speed 10

# Loop continuously
cargo run -p udp_streamer -- --file test_data/adsb_conus.csv --target 127.0.0.1:5005 --loop
```

See [roadmap.md](roadmap.md) Phase 3 for full planned feature list.
