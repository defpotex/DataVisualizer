# DataVisualizer — Roadmap

> Features are organized two ways: **Functional Tree** (what each feature does and how it nests) and **Priority Build Order** (what to build first). Both views are kept in sync. Status is updated with each code change.

---

## Status Key

| Symbol | Meaning |
|---|---|
| ⬜ | Not started |
| 🔵 | In progress |
| ✅ | Complete & committed |
| ⏸ | Deferred / blocked |

---

## Functional Tree

### F1 — Application Shell
- ⬜ **F1.1** Rust project scaffold (`Cargo.toml`, workspace structure, CI)
- ⬜ **F1.2** `eframe` window with correct title, icon, min size
- ⬜ **F1.3** Three-panel layout: menu bar + left pane + main plot area
- ⬜ **F1.4** Panel resize (left pane drag-to-resize)
- ⬜ **F1.5** Persistent window geometry (size/position saved on exit)

### F0 — Test Data Infrastructure
*(Separate workspace binaries in `tools/`. Not part of the main app — used for development and testing only.)*

- ⬜ **F0.1** ADS-B CSV Generator (`tools/adsb_fetcher`)
  - ⬜ F0.1.1 Fetch live state vectors from OpenSky Network REST API (no auth required for public data)
  - ⬜ F0.1.2 Configurable: bounding box (default: CONUS), polling interval, total duration
  - ⬜ F0.1.3 Write to CSV with standardized schema: `timestamp, icao24, callsign, lat, lon, altitude_m, velocity_ms, heading_deg, vertical_rate_ms, on_ground, squawk`
  - ⬜ F0.1.4 Deduplicate / sort by timestamp before writing
  - ⬜ F0.1.5 Target output: ~50,000–200,000 rows covering several hours of real traffic
  - ⬜ F0.1.6 Also write a companion `.parquet` version of the same data (via polars)
- ⬜ **F0.2** UDP Replay Streamer (`tools/udp_streamer`)
  - ⬜ F0.2.1 Read any CSV file (produced by F0.1 or any other source)
  - ⬜ F0.2.2 Replay rows over UDP as newline-delimited CSV strings, respecting original timestamps
  - ⬜ F0.2.3 Configurable: target `host:port`, speed multiplier (1x–100x), loop mode
  - ⬜ F0.2.4 Print stats to stdout: rows sent, elapsed time, current simulated timestamp
  - ⬜ F0.2.5 Graceful Ctrl+C shutdown

### F2 — Data Ingestion
- ⬜ **F2.1** CSV file loading via `rfd` file dialog + `polars`
  - ⬜ F2.1.1 Schema auto-detection (lat/lon/time/altitude field heuristics)
  - ⬜ F2.1.2 Multi-file loading (§2.1.5)
- ⬜ **F2.2** Parquet file loading
- ⬜ **F2.3** Data source panel (left pane: list loaded sources, row count, fields)
- ⬜ **F2.4** UDP stream ingestion
  - ⬜ F2.4.1 Configure host:port, field mapping
  - ⬜ F2.4.2 Start/stop streaming
  - ⬜ F2.4.3 Rolling buffer with configurable max rows
- ⬜ **F2.5** ADS-B stream decoding
  - ⬜ F2.5.1 Mode S / Beast / AVR format support
  - ⬜ F2.5.2 Aircraft state reconstruction (lat/lon/alt/squawk/callsign)
- ⬜ **F2.6** User-loaded geographic boundary files
  - ⬜ F2.6.1 GeoJSON support
  - ⬜ F2.6.2 Shapefile (.shp) support
  - ⬜ F2.6.3 Help documentation for boundary file format

### F3 — Filtering System
- ⬜ **F3.1** Attribute filter (conditional on any data field, §3.4.1.1)
  - ⬜ F3.1.1 Operators: `=`, `!=`, `>`, `<`, `>=`, `<=`, `contains`, `in`
  - ⬜ F3.1.2 Multi-condition (AND/OR)
- ⬜ **F3.2** Geographic boundary filter (§3.4.1.2)
  - ⬜ F3.2.1 Filter points inside/outside a loaded boundary
- ⬜ **F3.3** Temporal filter (§3.4.1.3)
  - ⬜ F3.3.1 Time range slider
  - ⬜ F3.3.2 Linked to playback cursor
- ⬜ **F3.4** Selection-based filter (§3.4.1.4)
  - ⬜ F3.4.1 "Filter to selection" from plot selection
- ⬜ **F3.5** Radial filter (§3.4.1.5)
  - ⬜ F3.5.1 Click point + enter radius → filter to nearby points
- ⬜ **F3.6** Filter panel in left pane (add/remove/enable/disable filters)

### F4 — Plot Area & Layout
- ⬜ **F4.1** Flexible plot grid (add/remove plots, drag-to-resize)
- ⬜ **F4.2** "Add Plot" dialog (type, source, field assignment)
- ⬜ **F4.3** Per-plot title bar with configure/close buttons
- ⬜ **F4.4** Linked axes across plots (§3.4.9)
  - ⬜ F4.4.1 Link by time axis
  - ⬜ F4.4.2 Link by selection

### F5 — Map Plot
- ⬜ **F5.1** Tile map base layer (`walkers`)
  - ⬜ F5.1.1 Bundled offline tiles (zoom 0–5, world)
  - ⬜ F5.1.2 Online tile fetch + disk cache (OpenStreetMap)
  - ⬜ F5.1.3 Map scheme switcher (Light/Dark/Radar/Naval, §4.1.1.3)
- ⬜ **F5.2** Data point rendering on map
  - ⬜ F5.2.1 Color-by field (§3.4.2)
  - ⬜ F5.2.2 Transparency-by field (§3.4.3)
  - ⬜ F5.2.3 Size-by field (§3.4.4)
- ⬜ **F5.3** Track/path rendering (connect points by time sequence)
- ⬜ **F5.4** Hover tooltip (configurable fields, §3.4.5)
- ⬜ **F5.5** Zoom & pan (§3.4.7)
- ⬜ **F5.6** Point selection on map
  - ⬜ F5.6.1 Single click (§3.4.11.1)
  - ⬜ F5.6.2 Ctrl+click multi-select (§3.4.11.2)
  - ⬜ F5.6.3 Area drag select (§3.4.11.3)
  - ⬜ F5.6.4 Geographic boundary select (§3.4.11.4)
- ⬜ **F5.7** Right-click context menu (§3.4.10)
- ⬜ **F5.8** Geographic boundary overlay (§4.1.2)
  - ⬜ F5.8.1 Render loaded boundaries on map
  - ⬜ F5.8.2 Color/aggregate data by boundary region (§4.1.2.1)

### F6 — Scatter Plot
- ⬜ **F6.1** X/Y scatter with configurable axes
- ⬜ **F6.2** Color/size/transparency by field (§3.4.2–3.4.4)
- ⬜ **F6.3** Hover tooltip (§3.4.5)
- ⬜ **F6.4** Zoom/pan (§3.4.7)
- ⬜ **F6.5** Axis labels, limits, scale (linear/log, §3.4.8)
- ⬜ **F6.6** Point selection (single, ctrl-click, area drag, §3.4.11.1–3)
- ⬜ **F6.7** Right-click context menu (§3.4.10)

### F7 — Bar Graph Plot
- ⬜ **F7.1** Categorical or binned bar chart
- ⬜ **F7.2** Color-by field
- ⬜ **F7.3** Axis labels and limits (§3.4.8)
- ⬜ **F7.4** Hover tooltip

### F8 — Scroll Chart
- ⬜ **F8.1** Time-series rolling line chart (§4.1.5.1)
- ⬜ **F8.2** Configurable window width (show last N seconds)
- ⬜ **F8.3** Threshold/tripwire lines with color override (§4.1.5.2)
  - ⬜ F8.3.1 Above/below threshold → chart region color changes
- ⬜ **F8.4** Zoom/pan
- ⬜ **F8.5** Multi-channel (multiple fields overlaid)

### F9 — Data Styling
- ⬜ **F9.1** Conditional color rules (§3.4.2)
  - ⬜ F9.1.1 Categorical (field = value → color)
  - ⬜ F9.1.2 Continuous colormap (gradient by field value)
- ⬜ **F9.2** Conditional transparency (§3.4.3)
- ⬜ **F9.3** Conditional point size (§3.4.4)
- ⬜ **F9.4** Configurable hover text fields (§3.4.5)
- ⬜ **F9.5** Data aggregation (§3.4.6)
  - ⬜ F9.5.1 Spatial binning (H3 hex or grid)
  - ⬜ F9.5.2 Temporal binning
  - ⬜ F9.5.3 Aggregate functions: count, mean, max, min

### F10 — Playback & Streaming Controls
- ⬜ **F10.1** Treat static files as streaming (§3.5)
  - ⬜ F10.1.1 Playback scrubber (time slider)
  - ⬜ F10.1.2 Play / Pause / Step Forward / Step Back / Jump to End
- ⬜ **F10.2** Playback speed control (§3.5.1) — 0.1x to 100x
- ⬜ **F10.3** Data timeout duration (§3.5.2) — "trail" window per track
- ⬜ **F10.4** Live streaming controls (pause/resume UDP ingestion)

### F11 — Session Persistence
- ⬜ **F11.1** Save session to `.tay` file (§5.1)
  - ⬜ F11.1.1 Save layout + plot configs
  - ⬜ F11.1.2 Save filters
  - ⬜ F11.1.3 Save data: file reference option (§5.1.1.1.1)
  - ⬜ F11.1.4 Save data: embedded compressed extract (§5.1.1.1.2)
- ⬜ **F11.2** Load `.tay` file (restore full session)
- ⬜ **F11.3** Recent files list

### F12 — Menus
- ⬜ **F12.1** File menu (New, Open, Save, Save As, Recent, Exit)
- ⬜ **F12.2** Data Sources menu (Add Source submenu, Manage Sources)
- ⬜ **F12.3** Data Aggregation menu (configure binning/aggregation)
- ⬜ **F12.4** Performance menu (memory usage, operation cancel modal)
- ⬜ **F12.5** Help menu (About, Documentation, Boundary File Format guide)

### F13 — Undo / Redo
- ⬜ **F13.1** Ctrl+Z undo (§8.2)
- ⬜ **F13.2** Ctrl+Y redo
- ⬜ **F13.3** Undo stack excludes streamed data (§8.2.1)

### F14 — Performance & UX
- ⬜ **F14.1** Long-operation progress modal with time estimate (§8.1.1)
- ⬜ **F14.2** Cancel operation button (§8.1.2)
- ⬜ **F14.3** Level-of-detail rendering (thin out points at high zoom-out)
- ⬜ **F14.4** Lazy polars queries (don't materialize full filtered dataset each frame)

---

## Priority Build Order

This is the sequence we follow. Each phase produces a usable, committable milestone.

```
Phase 1 ──► Phase 2 ──► Phase 3 ──► Phase 4 ──► Phase 5 ──► Phase 6 ──► Phase 7+
Foundation  Test Data   Load CSV    Map Plot    Filters    Styling    ...etc
            (CSV gen +
             UDP tool)
```

---

### Phase 1 — Foundation ✅
*Goal: Running app with correct layout. No data yet.*

| ID | Feature | Status | Notes |
|---|---|---|---|
| F1.1 | Rust project scaffold | ✅ | `Cargo.toml`, workspace (app + 2 tool stubs) |
| F1.2 | eframe window | ✅ | Title, programmatic diamond icon, min size 800×500 |
| F1.3 | Three-panel layout | ✅ | Menu bar + resizable left pane + plot area |
| F1.4 | Panel resize | ✅ | egui SidePanel resizable, min/max from theme |
| F1.5 | Window geometry persistence | ✅ | `persist_window: true` + custom pane width via eframe Storage |
| — | Engineering Dark theme | ✅ | `AppTheme` / `ThemePreset` — all colors & spacing centralized |

**Exit criteria:** App launches, shows correct layout, left pane can be resized, window size/pos restored on relaunch. ✅

---

### Phase 2 — Test Data: ADS-B CSV Generator ⬜
*Goal: Produce a large, realistic CSV (and Parquet) test dataset from live ADS-B traffic. Used by all subsequent phases.*

> **Workspace location:** `tools/adsb_fetcher/` — standalone binary, not part of the main app.

| ID | Feature | Status | Notes |
|---|---|---|---|
| F0.1.1 | OpenSky Network API polling | ⬜ | GET `/states/all?lamin=…` — no API key for public data |
| F0.1.2 | Configurable bbox + duration | ⬜ | Default: CONUS, 2 hours, 60s poll interval |
| F0.1.3 | CSV output with standard schema | ⬜ | `timestamp, icao24, callsign, lat, lon, altitude_m, velocity_ms, heading_deg, vertical_rate_ms, on_ground, squawk` |
| F0.1.4 | Dedup + sort by timestamp | ⬜ | Clean output, no duplicate state vectors |
| F0.1.5 | Parquet output (same data) | ⬜ | Via `polars` — tests Parquet loading in Phase 4 |
| F0.1.6 | Volume target | ⬜ | 50k–200k rows; print row count + file size on completion |

**Exit criteria:** Running `cargo run -p adsb_fetcher` produces `test_data/adsb_conus.csv` and `test_data/adsb_conus.parquet` with ≥50,000 rows of real aircraft state vectors.

---

### Phase 3 — Test Data: UDP Replay Streamer ⬜
*Goal: A tool to replay any CSV over UDP — used to test Phase 9 (UDP ingestion) and Phase 10 (live streaming UI).*

> **Workspace location:** `tools/udp_streamer/` — standalone binary, not part of the main app.

| ID | Feature | Status | Notes |
|---|---|---|---|
| F0.2.1 | Read any CSV file | ⬜ | Path via CLI arg |
| F0.2.2 | Replay rows over UDP | ⬜ | Newline-delimited CSV strings, respects timestamp ordering |
| F0.2.3 | CLI config: host:port, speed, loop | ⬜ | e.g. `--target 127.0.0.1:5005 --speed 10 --loop` |
| F0.2.4 | Stdout progress stats | ⬜ | Rows/sec, elapsed, current sim timestamp |
| F0.2.5 | Graceful Ctrl+C shutdown | ⬜ | Flush + exit cleanly |

**Usage example:**
```
cargo run -p udp_streamer -- --file test_data/adsb_conus.csv --target 127.0.0.1:5005 --speed 20 --loop
```

**Exit criteria:** Running the streamer sends UDP packets readable by `nc -ul 5005`; rows arrive in timestamp order at the configured speed multiplier.

---

### Phase 4 — CSV Data Loading ⬜
*Goal: Load a CSV, see data in the source panel, inspect fields. Uses the dataset from Phase 2.*

| ID | Feature | Status | Notes |
|---|---|---|---|
| F2.1 | CSV loading (file dialog) | ⬜ | `rfd` + `polars` |
| F2.1.1 | Schema auto-detection | ⬜ | Lat/lon/time field heuristics |
| F2.1.2 | Multi-file loading | ⬜ | Load multiple sources |
| F2.3 | Data source panel | ⬜ | Left pane: list sources, row count, fields |

**Exit criteria:** User opens a CSV, it appears in the left pane with field list and row count.

---

### Phase 5 — Map Plot ⬜
*Goal: Plot lat/lon data on an interactive map. Uses the ADS-B CSV from Phase 2.*

| ID | Feature | Status | Notes |
|---|---|---|---|
| F4.1 | Plot grid (basic, single plot) | ⬜ | Add/remove plot containers |
| F4.2 | Add Plot dialog | ⬜ | Type, source, field assignment |
| F5.1 | Tile map base layer | ⬜ | `walkers`, offline tiles zoom 0–5 |
| F5.1.2 | Online tile fetch + cache | ⬜ | OSM tiles, disk cache |
| F5.1.3 | Map scheme switcher | ⬜ | Light/Dark/Radar/Naval |
| F5.2 | Data point rendering | ⬜ | Fixed color/size first |
| F5.5 | Zoom & pan | ⬜ | Mouse wheel + drag |
| F5.4 | Hover tooltip | ⬜ | Field values on hover |

**Exit criteria:** User assigns lat/lon fields, sees points on the map, can pan/zoom, hover shows values.

---

### Phase 6 — Scatter Plot + Basic Filters ⬜
*Goal: Second plot type, plus attribute filtering.*

| ID | Feature | Status | Notes |
|---|---|---|---|
| F6.1 | Scatter plot | ⬜ | X/Y with configurable axes |
| F6.4 | Scatter zoom/pan | ⬜ | |
| F6.5 | Axis labels/limits | ⬜ | |
| F3.1 | Attribute filter | ⬜ | Conditional on any field |
| F3.6 | Filter panel | ⬜ | Left pane: add/remove/toggle filters |
| F4.4 | Linked time axis | ⬜ | Scatter + map time-linked |

**Exit criteria:** Scatter plot works; filter added in left pane updates both plots simultaneously.

---

### Phase 7 — Data Styling ⬜
*Goal: Color/size/transparency by field value; hover customization.*

| ID | Feature | Status | Notes |
|---|---|---|---|
| F9.1 | Conditional color | ⬜ | Categorical + continuous colormap |
| F9.2 | Conditional transparency | ⬜ | |
| F9.3 | Conditional point size | ⬜ | |
| F9.4 | Configurable hover text | ⬜ | |

**Exit criteria:** User can color map points by altitude (gradient), set size by speed.

---

### Phase 8 — Point Selection & Context Menu ⬜
*Goal: Select data points, right-click for context actions.*

| ID | Feature | Status | Notes |
|---|---|---|---|
| F5.6 | Map point selection | ⬜ | Single, ctrl-click, area drag |
| F6.6 | Scatter selection | ⬜ | |
| F5.7 | Right-click context menu | ⬜ | Map |
| F6.7 | Right-click context menu | ⬜ | Scatter |
| F3.4 | Filter to selection | ⬜ | |

**Exit criteria:** User can drag-select points on map, right-click to filter to selection.

---

### Phase 9 — Playback Engine ⬜
*Goal: Replay static CSV as streaming data with time controls.*

| ID | Feature | Status | Notes |
|---|---|---|---|
| F10.1 | Playback scrubber | ⬜ | Time slider over data |
| F10.1.2 | Play/Pause/Step/Jump | ⬜ | |
| F10.2 | Speed control | ⬜ | 0.1x–100x |
| F10.3 | Data timeout / trail | ⬜ | Show last N seconds per track |
| F3.3 | Temporal filter | ⬜ | Linked to playback cursor |

**Exit criteria:** User loads CSV, presses Play, sees points animate across the map with speed control.

---

### Phase 10 — UDP Streaming ⬜
*Goal: Real-time live data ingestion. Use the UDP Replay Streamer from Phase 3 to drive this.*

| ID | Feature | Status | Notes |
|---|---|---|---|
| F2.4 | UDP stream ingestion | ⬜ | Configure + start/stop |
| F2.4.3 | Rolling buffer | ⬜ | Configurable max rows |
| F10.4 | Live streaming controls | ⬜ | Pause/resume |
| F8.1 | Scroll chart | ⬜ | Rolling time-series |
| F8.3 | Threshold / tripwires | ⬜ | Color change above/below |

**Exit criteria:** App receives UDP packets, scroll chart shows real-time data, threshold changes chart color.

---

### Phase 11 — ADS-B & Geographic Boundaries ⬜
*Goal: Aviation-specific data + geographic filtering.*

| ID | Feature | Status | Notes |
|---|---|---|---|
| F2.5 | ADS-B decode | ⬜ | Mode S/Beast/AVR |
| F2.6 | Load boundary files | ⬜ | GeoJSON + SHP |
| F3.2 | Geographic boundary filter | ⬜ | Inside/outside region |
| F3.5 | Radial filter | ⬜ | Point + radius |
| F5.8 | Boundary overlay on map | ⬜ | Render + color by region |
| F5.6.4 | Geographic boundary select | ⬜ | Select points within boundary |

**Exit criteria:** User loads GeoJSON, sees boundary on map, can filter points to inside the boundary.

---

### Phase 12 — Bar Chart + Aggregation ⬜
*Goal: Aggregated views and bar chart support.*

| ID | Feature | Status | Notes |
|---|---|---|---|
| F7.1 | Bar chart | ⬜ | Categorical or binned |
| F9.5 | Data aggregation | ⬜ | Spatial/temporal binning |
| F12.3 | Aggregation menu | ⬜ | Configure binning |

---

### Phase 13 — Session Persistence ⬜
*Goal: Save and restore full working sessions.*

| ID | Feature | Status | Notes |
|---|---|---|---|
| F11.1 | Save .tay file | ⬜ | Layout + filters + data option |
| F11.2 | Load .tay file | ⬜ | Full session restore |
| F11.3 | Recent files | ⬜ | |
| F12.1 | File menu | ⬜ | New/Open/Save/Save As/Recent |

---

### Phase 14 — Undo/Redo + Polish ⬜
*Goal: Final quality-of-life features.*

| ID | Feature | Status | Notes |
|---|---|---|---|
| F13.1 | Undo (Ctrl+Z) | ⬜ | AppState snapshot stack |
| F13.2 | Redo (Ctrl+Y) | ⬜ | |
| F14.1 | Progress modal | ⬜ | Time estimate for large ops |
| F14.2 | Cancel operation | ⬜ | Halt running operation |
| F14.3 | LOD rendering | ⬜ | Thin points at zoom-out |
| F12.4 | Performance menu | ⬜ | Memory stats, cancel |
| F12.5 | Help menu | ⬜ | Docs, boundary format guide |
| F1.4 | Panel resize | ⬜ | Polish drag handles |

---

## Changelog

| Date | Phase | Change |
|---|---|---|
| 2026-04-04 | — | Initial roadmap created |
| 2026-04-04 | F0 | Added test data infrastructure phases: ADS-B CSV generator (Phase 2) and UDP replay streamer (Phase 3); existing phases renumbered 4–14 |
| 2026-04-04 | Phase 1 | Foundation complete: workspace scaffold, eframe window, 3-panel layout, Engineering Dark theme, persistent window geometry |
