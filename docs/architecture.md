# DataVisualizer — Architecture & Design Documentation

> **App Identity:** Production-quality vehicle telemetry visualization tool. Think "Tableau for live/replay vehicle test data" — desktop-native, offline-capable, real-time capable.

---

## Table of Contents
1. [Technology Stack](#technology-stack)
2. [Application Layout](#application-layout)
3. [System Architecture](#system-architecture)
4. [Data Flow](#data-flow)
5. [Key Design Decisions](#key-design-decisions)
6. [Module Breakdown](#module-breakdown)
7. [Reference Applications](#reference-applications)
8. [Changelog](#changelog)

---

## Technology Stack

| Layer | Crate(s) | Rationale | Req. Reference |
|---|---|---|---|
| GUI Framework | `eframe` + `egui` 0.34 | Pure Rust, single binary, immediate-mode (ideal for real-time data), no webview dependency | §1.2, §8.1 |
| Standard Plots | `egui_plot` | Native egui integration, supports zoom/pan/linked axes out of the box | §4.1.3–4.1.5, §3.4.7 |
| Map Rendering | `walkers 0.53` | Slippy map widget for egui; `HttpTiles` + `Plugin` trait for custom overlays; custom tile sources via `TileSource` trait | §4.1.1.1–4.1.1.2 |
| Data Engine | `polars` | Handles CSV + Parquet natively, lazy evaluation, fast filtering/aggregation, columnar | §2.1.1–2.1.2, §3.4.1, §3.4.6 |
| UDP Streaming | `std::net::UdpSocket` + `tokio` | Stdlib UDP + async runtime for non-blocking stream ingestion | §2.1.3–2.1.4 |
| ADSB Decoding | `adsb_deku` (or custom) | Decodes Mode S / ADS-B Beast/AVR/JSON formats | §2.1.4 |
| Geospatial | `geo` + `geojson` + `shapefile` | Geometry ops, boundary loading, point-in-polygon filtering | §2.2, §3.4.1.2, §4.1.2 |
| Serialization | `serde` + `serde_json` + `flate2` + `zip` | Human-readable `.tay` session files with optional compressed data payloads | §5.1.2 |
| Concurrency | `crossbeam-channel` | Lock-free MPSC channels between background data threads and UI thread | §8.1 |
| File Dialogs | `rfd` | Native OS file picker dialogs, no extra runtime deps | §6.1 |

### Why `egui` over alternatives?

| Option | Verdict |
|---|---|
| `egui` + `eframe` | ✅ **Selected.** Single binary, immediate-mode (re-renders on data change), strong ecosystem, active development |
| `iced` | Elm/MVU architecture is elegant but less mature for complex drag/resize layouts; harder to integrate custom renderers |
| `tauri` | Requires webview runtime; violates §1.2.1 if user doesn't have Edge/WebKit; also adds JS complexity |
| `slint` | Good for embedded, less suited to data-heavy interactive desktop apps; commercial license concerns |

### Why `polars` over raw CSV/Arrow parsing?

- Single dependency handles both CSV and Parquet (§2.1.1, §2.1.2)
- Lazy query API enables filter pushdown — only load what's needed into memory
- Built-in group-by, aggregate, conditional expressions (§3.4.1, §3.4.6)
- Returns columnar data that maps cleanly to plot series

---

## Application Layout

```
┌──────────────────────────────────────────────────────────────────────────────┐
│  File   │  Data Sources   │  Data Aggregation   │  Performance   │  Help     │  <- Menu Bar (§6)
├─────────────┬────────────────────────────────────────────────────────────────┤
│             │                                                                  │
│ LEFT PANE   │                    MAIN PLOT AREA                               │
│  (§7)       │                                                                  │
│             │  ┌──────────────────────────┐   ┌─────────────────────────┐   │
│ ▼ Data      │  │                          │   │                         │   │
│   Sources   │  │     MAP PLOT             │   │   SCATTER / BAR PLOT    │   │
│   [+]       │  │  (walkers tile map)      │   │   (egui_plot)           │   │
│   □ file1   │  │                          │   │                         │   │
│   □ UDP:5005│  │   ·  · ·  ·  ·  ·       │   │  ·                      │   │
│             │  │     · · [track]  ·       │   │     · ·  ·              │   │
│ ▼ Add Plot  │  │                          │   │        ·  · ·           │   │
│   [Map]     │  └──────────────────────────┘   └─────────────────────────┘   │
│   [Scatter] │                                                                  │
│   [Bar]     │  ┌────────────────────────────────────────────────────────┐    │
│   [Scroll]  │  │  SCROLL CHART — altitude vs. time (streaming)          │    │
│             │  │  ─────────────────────────────────────── [threshold]   │    │
│ ▼ Filters   │  │  ∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿    │    │
│   [+] Add   │  └────────────────────────────────────────────────────────┘    │
│   • alt>100 │                                                                  │
│   • speed<  │  [ ◀◀  ◀  ▶  ▶▶ ]  [====●========]  1.0x  [⏱ 00:04:23]      │
│             │                     Playback Controls (§3.5)                     │
└─────────────┴────────────────────────────────────────────────────────────────┘
```

### Left Pane Detail

```
┌─────────────────────────┐
│  DATA SOURCES           │
│  ─────────────────────  │
│  [+ Add Source ▼]       │
│    > CSV File...        │
│    > Parquet File...    │
│    > UDP Stream...      │
│    > ADS-B Stream...    │
│                         │
│  ● flight_001.csv       │  <- colored dot = active source
│    Rows: 14,220         │
│    Fields: 12           │
│    [Configure] [Remove] │
│                         │
│  ◌ UDP:5005             │  <- hollow = not yet receiving
│    [Start] [Remove]     │
│                         │
│  ADD PLOT               │
│  ─────────────────────  │
│  Source: [flight_001 ▼] │
│  Type:   [Map       ▼]  │
│  X Axis: [longitude ▼]  │
│  Y Axis: [latitude  ▼]  │
│  Color:  [altitude  ▼]  │
│  [Add Plot]             │
│                         │
│  FILTERS                │
│  ─────────────────────  │
│  [+ Add Filter ▼]       │
│  ✓ altitude > 1000      │
│  ✓ speed < 500          │
│  ✗ region: CONUS        │  <- disabled
│                         │
└─────────────────────────┘
```

### Context Menu (Right-Click on Data Point, §3.4.10)

```
┌────────────────────────────┐
│  Point ID: 4821            │
│  ─────────────────────     │
│  > Inspect Attributes...   │
│  > Filter: Same Track      │
│  > Filter: Within 50nm     │
│  > Set as Origin           │
│  > Copy Coordinates        │
│  > Export Selection...     │
└────────────────────────────┘
```

### Map Scheme Selector (§4.1.1.3)

```
┌─────────────────────────────────────────┐
│  Map Style                              │
│                                         │
│  ○ ░░░ Light (White bg, dark lines)    │  §4.1.1.3.1
│  ● ▓▓▓ Dark (Black bg, light lines)    │  §4.1.1.3.2
│  ○ ▒▒▒ Radar (Black bg, green lines)  │  §4.1.1.3.3
│  ○ ███ Naval (Dark blue, light lines)  │  §4.1.1.3.4
│                                         │
└─────────────────────────────────────────┘
```

---

## System Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    eframe App Loop                       │
│                  (UI Thread, 60fps)                      │
│                                                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────┐ │
│  │ MenuBar  │  │LeftPane  │  │PlotArea  │  │Playback│ │
│  │ (mod ui) │  │(mod ui)  │  │(mod plot)│  │(mod ui)│ │
│  └──────────┘  └──────────┘  └────────┬─┘  └────────┘ │
│                                        │                 │
│  ┌─────────────────────────────────────▼──────────────┐ │
│  │                  AppState                           │ │
│  │  - sources: Vec<DataSource>                        │ │
│  │  - plots: Vec<PlotConfig>                          │ │
│  │  - filters: Vec<Filter>                            │ │
│  │  - selection: SelectionState                       │ │
│  │  - undo_stack: Vec<AppSnapshot>                    │ │
│  │  - playback: PlaybackState                         │ │
│  └────────────────────────────────────────────────────┘ │
└────────────────────────────┬────────────────────────────┘
                             │  crossbeam channels
         ┌───────────────────┼───────────────────┐
         ▼                   ▼                   ▼
┌─────────────────┐  ┌──────────────┐  ┌────────────────┐
│  DataLoader     │  │  UdpReceiver │  │  AdsbDecoder   │
│  Thread         │  │  Thread      │  │  Thread        │
│                 │  │              │  │                │
│  polars:        │  │  UdpSocket   │  │  adsb_deku     │
│  CSV, Parquet   │  │  bind+recv   │  │  Mode S decode │
│                 │  │              │  │                │
│  → DataFrame    │  │  → UdpFrame  │  │  → AircraftMsg │
└─────────────────┘  └──────────────┘  └────────────────┘
         │                   │                   │
         └───────────────────▼───────────────────┘
                    Unified DataRecord stream
                    (normalized schema)
```

### Rendering Pipeline (per frame)

```
AppState
   │
   ├─ apply active filters → FilteredView (lazy polars query)
   │
   ├─ apply playback cursor → TemporalSlice
   │
   ├─ for each PlotConfig:
   │    ├─ MapPlot     → walkers TileMap + custom point/track layer
   │    ├─ ScatterPlot → egui_plot Points + Lines
   │    ├─ BarPlot     → egui_plot Bars
   │    └─ ScrollChart → egui_plot Lines + threshold markers
   │
   └─ render selection overlay, hover tooltips, context menus
```

---

## Data Flow

### Static File Loading

```
User selects file
      │
      ▼
rfd::FileDialog  ──→  path: PathBuf
      │
      ▼
DataLoader thread (tokio::spawn or std::thread)
      │
      ├─ .csv  → polars::CsvReader → DataFrame
      ├─ .parquet → polars::ParquetReader → DataFrame
      │
      ▼
Schema inference:  detect lat/lon/time/altitude fields by name heuristics
      │
      ▼
crossbeam tx.send(DataEvent::Loaded(DataFrame))
      │
      ▼
UI thread receives → AppState.sources.push(DataSource { df, schema })
      │
      ▼
  (ready for plotting)
```

### Streaming (UDP / ADS-B)

```
UdpSocket::bind(addr)
      │
      loop {
        recv_from(buf)
            │
            ├─ raw UDP → parse as CSV line or JSON → DataRecord
            └─ ADS-B   → adsb_deku decode → AircraftState
            │
            ▼
        tx.send(DataEvent::Record(record))
      }
      │
UI thread (each frame):
      while let Ok(event) = rx.try_recv() {
          append to rolling DataFrame
          if df.height() > MAX_STREAM_ROWS { evict oldest }
      }
```

### .tay Session File Format

```
session.tay (ZIP archive)
├── manifest.json          <- layout, plot configs, filter configs, metadata
├── data/
│   ├── source_0.ref       <- path reference to original file (§5.1.1.1.1)
│   └── source_0.parquet   <- compressed data snapshot (§5.1.1.1.2)
└── preview.png            <- optional thumbnail
```

`manifest.json` schema (draft):
```json
{
  "version": "1.0",
  "created": "2026-04-04T00:00:00Z",
  "layout": {
    "plots": [
      {
        "id": "plot_0",
        "type": "MapPlot",
        "position": {"x": 0, "y": 0, "w": 6, "h": 4},
        "config": { "map_scheme": "Dark", "lat_col": "lat", "lon_col": "lon" }
      }
    ]
  },
  "filters": [...],
  "sources": [
    { "id": "source_0", "label": "flight_001.csv", "type": "FileRef" }
  ]
}
```

---

## Key Design Decisions

### D1: Immediate-Mode GUI (egui)
**Driver:** §8.1 (performant, minimize lag), §3.4.7 (zoom/pan), real-time streaming  
**Decision:** Immediate-mode redraws the entire frame every 16ms based on current state. No stale widget state. Streaming data updates are automatically reflected without explicit "invalidate" calls. Retained-mode GUIs (Qt, GTK) require careful invalidation logic that becomes complex with linked plots and real-time data.

### D2: Polars as the Data Engine
**Driver:** §2.1.1, §2.1.2, §3.4.1, §3.4.6  
**Decision:** Polars provides a unified API for CSV and Parquet, lazy query execution (filters don't copy data), and vectorized columnar ops. Alternative was `arrow2` + manual CSV parsing — more control but far more code. Polars trades some binary size for massive feature coverage.

### D3: walkers for Map Tiles
**Driver:** §4.1.1.1 (online tile access), §4.1.1.2 (offline state-level detail)  
**Decision:** `walkers` supports pluggable tile providers and offline tile caching. We ship a bundled set of low-zoom world tiles (zoom 0–5) in the binary via `include_bytes!`. High-zoom tiles are fetched from OpenStreetMap/Stamen/Carto when online and cached to disk. This satisfies both online and offline use cases without shipping gigabytes of tiles.

### D4: crossbeam-channel for Thread Communication
**Driver:** §8.1 (no UI lag), streaming requirements  
**Decision:** Data loading and UDP receive run on background threads. `crossbeam::channel` is used for bounded MPSC communication. The UI thread calls `try_recv()` each frame — non-blocking, no locking stalls. `std::sync::mpsc` was considered but crossbeam is faster and supports `select!` across multiple channels.

### D5: .tay as a ZIP-based Format
**Driver:** §5.1.2 (human-readable package)  
**Decision:** ZIP is readable with any archive tool (human-inspectable), supports mixed binary+JSON content, and is well-supported in Rust via the `zip` crate. A single `.tay` file contains the JSON manifest (human-readable) and optional binary Parquet data (compressed). The `.tay` extension is application-specific to avoid conflicts.

### D6: Undo/Redo via AppState Snapshots
**Driver:** §8.2  
**Decision:** On each user action that modifies non-stream state (add filter, move plot, change color, etc.), push a clone of the relevant `AppState` subset onto an undo stack. `Ctrl+Z` pops from undo → push to redo. Stream-appended data is explicitly excluded (§8.2.1) — the undo stack stores configuration state, not data state.

---

## Module Breakdown

```
Cargo.toml                   # workspace root
├── src/                     # main app (datavisualizer)
│   ├── main.rs
tools/
├── adsb_fetcher/            # test data: fetch ADS-B → CSV/Parquet
│   ├── Cargo.toml
│   └── src/main.rs
├── udp_streamer/            # test data: replay CSV over UDP
│   ├── Cargo.toml
│   └── src/main.rs
test_data/                   # .gitignored output from tools
├── adsb_conus.csv
└── adsb_conus.parquet
```

### Main App Module Tree

```
src/
├── main.rs                  # eframe entry point, app init
├── app.rs                   # AppState struct, top-level update() and draw()
├── state/
│   ├── mod.rs
│   ├── app_state.rs         # Master state struct (sources, plots, perf, events)
│   ├── perf_settings.rs     # PerformanceSettings (max_draw_points, etc.)
│   ├── undo.rs              # UndoStack, AppSnapshot [future]
│   └── session.rs           # Save/load .tay files [future]
├── data/
│   ├── mod.rs
│   ├── loader.rs            # CSV/Parquet loading via polars
│   ├── udp_receiver.rs      # UDP stream thread
│   ├── adsb_decoder.rs      # ADS-B message decoding
│   ├── schema.rs            # Field detection, DataSchema
│   └── filter.rs            # Filter definitions and application
├── ui/
│   ├── mod.rs
│   ├── menu_bar.rs          # Top menu (§6)
│   ├── left_pane.rs         # Left panel (§7) — with AddPlotDialog embedded
│   ├── add_plot_dialog.rs   # Floating modal for creating plots (Phase 5)
│   ├── plot_area.rs         # Hosts PlotGrid, shows empty/has-sources/has-plots states
│   ├── plot_grid.rs         # Responsive 1–2 column grid of plot cells (Phase 5)
│   └── playback_bar.rs      # Playback controls (§3.5) [future]
├── plot/
│   ├── mod.rs
│   ├── map_plot.rs          # Geographic map (walkers 0.53 HttpTiles + Plugin trait)
│   ├── scatter_plot.rs      # X/Y scatter (egui_plot) [future]
│   ├── bar_plot.rs          # Bar chart (egui_plot) [future]
│   ├── scroll_chart.rs      # Streaming scroll chart (egui_plot) [future]
│   └── plot_config.rs       # PlotConfig enum, MapPlotConfig, TileScheme (serde)
├── geo/
│   ├── mod.rs
│   ├── boundary.rs          # Load/parse GeoJSON, SHP boundaries
│   └── spatial_filter.rs    # Point-in-polygon, radial filter
└── assets/
    ├── map_tiles/           # Bundled offline tiles (zoom 0-5)
    └── fonts/               # Embedded UI fonts
```

---

## Reference Applications

These existing tools inform design decisions and UX patterns:

| App | Relevance | Key Lessons |
|---|---|---|
| [QGroundControl](https://qgroundcontrol.com/) | Open-source vehicle GCS, map + telemetry panels | Playback scrubber, linked plots, instrument panels |
| [Mission Planner](https://ardupilot.org/planner/) | ArduPilot GCS, similar data types | Flight data replay, CSV/log loading patterns |
| [kepler.gl](https://kepler.gl/) | Web-based geospatial data viz | Excellent filter/color-by UI patterns, layer model |
| [Grafana](https://grafana.com/) | Streaming dashboards | Multi-panel layout, threshold/tripwire patterns, scroll charts |
| [Tableau](https://www.tableau.com/) | General data viz desktop app | Drag-and-drop field assignment, filter panel UX |
| [PyQGIS / QGIS](https://qgis.org/) | GIS desktop app | Geographic boundary handling, projection management |

---

## Changelog

| Version | Date | Change |
|---|---|---|
| 0.1 | 2026-04-04 | Initial architecture document created |
| 0.2 | 2026-04-04 | Added workspace layout with `tools/` binaries for test data generation |
| 0.3 | 2026-04-04 | Phase 1 built: eframe app, Engineering Dark theme (`src/theme.rs`), three-panel layout, persistent geometry |
| 0.4 | 2026-04-05 | Phases 2–4: ADS-B fetcher, UDP streamer, CSV loading, source panel with schema detection |
| 0.5 | 2026-04-05 | Phase 5: Map plot implemented. Added `src/plot/` module (`plot_config.rs`, `map_plot.rs`), `src/ui/add_plot_dialog.rs`, `src/ui/plot_grid.rs`. Upgraded egui/eframe 0.29→0.34, walkers pinned to 0.53. `PlotConfig` enum enables future scatter/bar/scroll plot types. `AppState` now tracks `plots: Vec<PlotConfig>`. |
| 0.6 | 2026-04-05 | Phase 5 polish: GPU quad mesh point rendering in `PointsPlugin` (single `Shape::mesh` draw call replaces N `circle_filled` calls). Iterative collision resolution in `PlotManager`. `PerformanceSettings` in `AppState` + Performance menu DragValue. `app_style: egui::Style` cached on app struct and re-applied each frame to keep dark theme in all egui popup Areas. Added `src/state/perf_settings.rs`. |
