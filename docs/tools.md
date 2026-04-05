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

Reads any CSV file and replays rows over UDP as newline-delimited strings, timing delivery to match the original timestamps at a configurable speed multiplier. Used to drive the main app's UDP ingestion (Phase 10) without a live feed.

### Quick Start

```bash
# Stream at 50× speed with header row, to default localhost:5005
cargo run -p udp_streamer -- --file test_data/adsb_conus.csv --speed 50 --header

# Loop continuously at 10× speed
cargo run -p udp_streamer -- --file test_data/adsb_conus.csv --speed 10 --loop

# Send to a remote host
cargo run -p udp_streamer -- --file test_data/adsb_conus.csv --target 192.168.1.10:9000 --speed 5
```

### Options

| Flag | Default | Description |
|---|---|---|
| `--file <PATH>` | *(required)* | CSV file to replay |
| `--target <HOST:PORT>` | `127.0.0.1:5005` | UDP destination address |
| `--speed <MULT>` | `1.0` | Playback speed multiplier. `1.0` = real-time, `60.0` = 1 min/sec |
| `--loop` | off | Restart from row 1 when the file ends |
| `--header` | off | Send the CSV header row as the first UDP packet each pass |
| `--help` | — | Print usage and exit |

### Packet Format

Each UDP packet is one CSV row: `field1,field2,...\n` (newline-terminated, UTF-8). The main app splits on newline and parses as CSV. The `--header` flag sends the column names first so receivers can auto-detect field order.

### Timing Behavior

- Rows are sorted by `timestamp` on load (column must be named `timestamp`).
- Wall-clock delay between packets = (timestamp delta) ÷ speed.
- Rows with identical timestamps are sent in a burst with no delay.
- `--speed 1.0` replays at real time. `--speed 0` is not valid — use a large multiplier (e.g. `--speed 99999`) for max-rate testing.
- The tool sleeps in 50ms chunks so Ctrl+C is always responsive.

### Verify Without the Main App

```bash
# Terminal 1 — listen for raw UDP packets (Linux/macOS/WSL)
nc -ul 5005

# Terminal 2 — stream at 100× with header
cargo run -p udp_streamer -- --file test_data/adsb_conus.csv --speed 100 --header
```

### Example Output

```
  Loading test_data/adsb_conus.csv…
  Formatting 63204 rows…
  Ready: 63204 rows, timestamps 1712001600 → 1712005260

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  UDP Streamer
  File     : test_data/adsb_conus.csv
  Target   : 127.0.0.1:5005
  Rows     : 63204
  Sim span : 3660s (61.0 min)
  Speed    : 50×  →  real duration ~73s
  Loop     : no
  Header   : yes
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

      1000 rows sent  |  sim t=1712001600  |  867 rows/sec  |  1s elapsed
      2000 rows sent  |  sim t=1712001660  |  ...
  ...

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Done.
  Rows sent : 63204
  Passes    : 1
  Elapsed   : 73.2s
  Avg rate  : 863 rows/sec
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### Notes

- **`timestamp` column required:** The CSV must have a column named exactly `timestamp`. The `adsb_fetcher` output satisfies this. For other CSVs, rename the time column or add one.
- **UDP is fire-and-forget:** If the receiver isn't listening, packets are silently dropped — no error. Start the receiver before the streamer.
- **Large files:** All rows are loaded into memory on startup. A 200k-row file uses ~100–200 MB RAM during formatting.
- **Windows `nc`:** The `nc` (netcat) command may not be available on Windows by default. Use WSL, or write a simple Python listener: `python -m udprecv` or a short script.
