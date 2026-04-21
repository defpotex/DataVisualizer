use crate::data::filter::{Filter, FilterOp};
use crate::data::loader::load_csv_async;
use crate::data::schema::FieldKind;
use crate::data::udp_receiver::{start_udp_receiver, UdpStreamConfig};
use crate::state::app_state::AppState;
use crate::theme::{AppTheme, ThemePreset};
use crate::ui::{left_pane::LeftPane, menu_bar::MenuBar, plot_area::PlotArea, right_pane::{LegendAction, RightPane}};
use crate::ui::plot_grid::PlotAction;
use crate::ui::udp_stream_dialog::UdpStreamDialog;
use eframe::Storage;
use egui::Context;
use serde::{Deserialize, Serialize};

const STORAGE_KEY: &str = "datavisualizer_app_state";

// ── Persistent state ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct PersistentState {
    theme_preset: ThemePreset,
    left_pane_width: f32,
    right_pane_visible: bool,
}

impl Default for PersistentState {
    fn default() -> Self {
        let default_theme = AppTheme::default();
        Self {
            theme_preset: default_theme.preset,
            left_pane_width: default_theme.spacing.left_pane_default_width,
            right_pane_visible: true,
        }
    }
}

// ── Main app struct ───────────────────────────────────────────────────────────

pub struct DataVisualizerApp {
    theme: AppTheme,
    app_style: egui::Style,
    left_pane_width: f32,
    right_pane_visible: bool,

    app_state: AppState,

    menu_bar: MenuBar,
    left_pane: LeftPane,
    plot_area: PlotArea,
    right_pane: RightPane,
    udp_dialog: UdpStreamDialog,

    central_rect: egui::Rect,

    /// Puffin HTTP server — serves profiling data to the puffin_viewer app.
    /// Kept alive as long as profiling is enabled.
    puffin_server: Option<puffin_http::Server>,
}

impl DataVisualizerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let persisted: PersistentState = cc
            .storage
            .and_then(|s| eframe::get_value(s, STORAGE_KEY))
            .unwrap_or_default();

        let theme = AppTheme::from_preset(persisted.theme_preset);

        let mut app_style = egui::Style::default();
        theme.apply_to_style(&mut app_style);
        cc.egui_ctx.set_global_style(app_style.clone());

        setup_fonts(&cc.egui_ctx);

        Self {
            app_style,
            left_pane_width: persisted.left_pane_width,
            right_pane_visible: persisted.right_pane_visible,
            app_state: AppState::default(),
            menu_bar: MenuBar::default(),
            left_pane: LeftPane::default(),
            plot_area: PlotArea::default(),
            right_pane: RightPane::default(),
            udp_dialog: UdpStreamDialog::default(),
            central_rect: egui::Rect::from_min_size(
                egui::pos2(260.0, 28.0),
                egui::vec2(1100.0, 860.0),
            ),
            theme,
            puffin_server: None,
        }
    }

    #[allow(dead_code)]
    pub fn apply_theme(&mut self, preset: ThemePreset, _ctx: &Context) {
        self.theme = AppTheme::from_preset(preset);
        self.theme.apply_to_style(&mut self.app_style);
    }

    fn start_udp_stream(&mut self, config: UdpStreamConfig) {
        let id = self.app_state.next_source_id();
        let tx = self.app_state.event_tx.clone();
        match start_udp_receiver(id, config, tx) {
            Ok(handle) => {
                self.app_state.streaming_source_ids.push(id);
                self.app_state.udp_handles.push(handle);
            }
            Err(e) => {
                self.udp_dialog.set_error(e);
            }
        }
    }

    fn open_csv_dialog(&mut self) {
        let id = self.app_state.next_source_id();
        let tx = self.app_state.event_tx.clone();
        std::thread::spawn(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("CSV files", &["csv"])
                .add_filter("All files", &["*"])
                .pick_file()
            {
                load_csv_async(id, path, tx);
            }
        });
    }
}

impl eframe::App for DataVisualizerApp {
    /// eframe 0.34 requires `ui` as the primary trait method; we override `update` instead.
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        puffin::GlobalProfiler::lock().new_frame();
        puffin::profile_function!();

        // Start/stop the puffin HTTP server based on the profiler toggle.
        if self.app_state.perf.show_profiler && self.puffin_server.is_none() {
            match puffin_http::Server::new("0.0.0.0:8585") {
                Ok(server) => {
                    self.puffin_server = Some(server);
                    eprintln!("Puffin profiler serving on http://127.0.0.1:8585");
                }
                Err(e) => eprintln!("Failed to start puffin server: {e}"),
            }
        } else if !self.app_state.perf.show_profiler && self.puffin_server.is_some() {
            self.puffin_server = None;
        }

        ctx.set_global_style(self.app_style.clone());

        // ── Playback frame advance ───────────────────────────────────────
        // Runs BEFORE poll_events so that when a sync completes this frame,
        // the plot transitions to idle after the playback check — giving it
        // at least one rendered frame before the next sync kicks off.
        if self.app_state.playback.is_playing() {
            let dt = ctx.input(|i| i.stable_dt) as f64;
            if self.app_state.playback.advance(dt) {
                self.update_playback_filter();
                // Use throttled sync to avoid cancelling in-flight background
                // computations every frame — only re-sync plots that are idle.
                self.plot_area.sync_all_filters_throttled(&self.app_state);
            }
            ctx.request_repaint();
        }

        let (had_events, sync_events, had_stream_update) = self.app_state.poll_events();
        for evt in sync_events {
            self.plot_area.apply_sync_event(evt);
        }
        if had_stream_update {
            // Re-sync all plots that depend on streaming sources.
            self.plot_area.sync_all_filters_throttled(&self.app_state);
        }
        if had_events {
            ctx.request_repaint();
        }

        // Keep repainting while any UDP stream is active.
        if !self.app_state.udp_handles.is_empty() {
            ctx.request_repaint();
        }

        let theme = self.theme.clone();
        let mut menu_action: Option<MenuAction> = None;
        let mut pane_action: Option<PaneAction> = None;

        // ── Menu bar ──────────────────────────────────────────────────────────
        egui::Panel::top("menu_bar")
            .frame(menu_bar_frame(&theme))
            .show(ctx, |ui| {
                menu_action = self.menu_bar.show(ui, &theme, &mut self.app_state.perf, self.right_pane_visible);
            });

        // ── Right pane (legends) — must come before CentralPanel ──────────────
        let mut legend_actions = Vec::new();
        if self.right_pane_visible {
            egui::Panel::right("right_pane")
                .resizable(true)
                .default_width(200.0)
                .min_width(140.0)
                .max_width(400.0)
                .frame(side_panel_frame(&theme))
                .show(ctx, |ui| {
                    let legends = self.plot_area.legend_data();
                    // Build column list per source for context menu
                    let mut cols_by_source = std::collections::HashMap::new();
                    let mut aliases_by_source = std::collections::HashMap::new();
                    for src in &self.app_state.sources {
                        let cols: Vec<String> = src.schema.fields.iter().map(|f| f.name.clone()).collect();
                        cols_by_source.insert(src.id, cols);
                        aliases_by_source.insert(src.id, src.column_aliases.clone());
                    }
                    legend_actions = self.right_pane.show(ui, &theme, &legends, &cols_by_source, &aliases_by_source);
                });
        }

        // ── Left pane ─────────────────────────────────────────────────────────
        egui::Panel::left("left_pane")
            .resizable(true)
            .default_size(self.left_pane_width)
            .min_size(theme.spacing.left_pane_min_width)
            .max_size(theme.spacing.left_pane_max_width)
            .frame(side_panel_frame(&theme))
            .show(ctx, |ui| {
                self.left_pane_width = ui.available_width() + ui.spacing().item_spacing.x;
                pane_action = self.left_pane.show(ui, &theme, &self.app_state);
            });

        // ── Plot area (central panel) ─────────────────────────────────────────
        const GRID_SIZE: f32 = 40.0;
        let central_response = egui::CentralPanel::default()
            .frame(plot_area_frame(&theme))
            .show(ctx, |ui| {
                self.plot_area.show(ui, &theme, &self.app_state, GRID_SIZE);
            });
        self.central_rect = central_response.response.rect;

        // ── Floating plot windows ─────────────────────────────────────────────
        let available_sources: Vec<(crate::data::source::SourceId, String)> = self.app_state.sources.iter()
            .map(|s| (s.id, s.label.clone()))
            .collect();
        let plot_actions = self.plot_area.show_windows(ctx, &theme, self.central_rect, GRID_SIZE, &self.app_state.perf, self.app_state.selection.as_ref(), &available_sources);
        for action in plot_actions {
            self.handle_plot_action(action);
        }

        // ── UDP Stream dialog ─────────────────────────────────────────────
        if let Some(udp_config) = self.udp_dialog.show(ctx, &theme) {
            self.start_udp_stream(udp_config);
        }

        // ── Legend actions ────────────────────────────────────────────────
        for la in legend_actions {
            self.handle_legend_action(la);
        }

        if let Some(a) = menu_action { self.handle_menu_action(a); }
        if let Some(a) = pane_action { self.handle_pane_action(a); }

        // ── Playback keyboard shortcuts ──────────────────────────────────
        if self.app_state.playback.is_active() {
            let space = ctx.input(|i| i.key_pressed(egui::Key::Space));
            if space {
                self.app_state.playback.toggle_play_pause();
            }
        }
    }

    fn save(&mut self, storage: &mut dyn Storage) {
        let state = PersistentState {
            theme_preset: self.theme.preset,
            left_pane_width: self.left_pane_width,
            right_pane_visible: self.right_pane_visible,
        };
        eframe::set_value(storage, STORAGE_KEY, &state);
    }
}

// ── Action handling ───────────────────────────────────────────────────────────

pub enum MenuAction {
    OpenCsv,
    OpenUdpStream,
    ToggleLegendPane,
}

pub enum PaneAction {
    OpenCsv,
    OpenUdpStream,
    RemoveSource(usize),
    /// Rename a column: (source_id, original_name, new_display_name).
    /// Empty new_display_name clears the alias.
    RenameColumn(usize, String, String),
    /// Pause/resume a live UDP stream.
    ToggleStreamPause(usize),
    /// Stop and disconnect a live UDP stream.
    StopStream(usize),
    AddPlot(crate::ui::add_plot_dialog::NewPlotConfig),
    RemovePlot(usize),
    AddFilter(crate::data::filter::Filter),
    /// Rebind a plot to a different data source.
    RebindPlot(usize, crate::data::source::SourceId),
    RemoveFilter(usize),
    ToggleFilter(usize),
    // Playback actions
    PlaybackStart(crate::data::source::SourceId),
    PlaybackStop,
    PlaybackToggle,
    PlaybackSeek(f64),
    PlaybackStepForward,
    PlaybackStepBackward,
    PlaybackJumpStart,
    PlaybackJumpEnd,
    PlaybackSpeedChanged(f64),
    PlaybackTrailChanged(Option<f64>),
    PlaybackLoopToggle,
    PlaybackColumnChanged(String),
}

impl DataVisualizerApp {
    fn handle_menu_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::OpenCsv => self.open_csv_dialog(),
            MenuAction::OpenUdpStream => self.udp_dialog.open(),
            MenuAction::ToggleLegendPane => self.right_pane_visible = !self.right_pane_visible,
        }
    }

    fn handle_pane_action(&mut self, action: PaneAction) {
        use crate::plot::plot_config::PlotConfig;
        use crate::ui::add_plot_dialog::NewPlotConfig;
        match action {
            PaneAction::OpenCsv => self.open_csv_dialog(),
            PaneAction::OpenUdpStream => self.udp_dialog.open(),

            PaneAction::ToggleStreamPause(id) => {
                if let Some(h) = self.app_state.udp_handles.iter().find(|h| h.source_id == id) {
                    h.toggle_pause();
                }
            }
            PaneAction::StopStream(id) => {
                self.app_state.stop_stream(id);
                // Also remove plots and source.
                self.plot_area.remove_plots_for_source(id);
                self.app_state.plots.retain(|p| p.source_id() != id);
                self.app_state.remove_source(id);
            }

            PaneAction::RenameColumn(source_id, original, new_name) => {
                if let Some(source) = self.app_state.sources.iter_mut().find(|s| s.id == source_id) {
                    if new_name.is_empty() || new_name == original {
                        source.column_aliases.remove(&original);
                    } else {
                        source.column_aliases.insert(original, new_name);
                    }
                }
            }

            PaneAction::RemoveSource(id) => {
                // Stop playback if this source is being played.
                if self.app_state.playback.source_id == Some(id) {
                    self.app_state.playback.stop();
                    self.remove_playback_filter();
                }
                // Stop streaming if this source is a UDP stream.
                if self.app_state.is_streaming(id) {
                    self.app_state.stop_stream(id);
                }
                // Keep plots alive — don't remove them. They'll show "Source removed"
                // with a rebind UI. Just remove the data source.
                self.app_state.remove_source(id);
            }

            PaneAction::AddPlot(new_config) => {
                match new_config {
                    NewPlotConfig::Map(mut config) => {
                        config.id = self.app_state.alloc_plot_id();
                        self.app_state.plots.push(PlotConfig::Map(config.clone()));
                        self.plot_area.add_map_plot(config, &self.app_state, self.central_rect);
                    }
                    NewPlotConfig::Scatter(mut config) => {
                        config.id = self.app_state.alloc_plot_id();
                        self.app_state.plots.push(PlotConfig::Scatter(config.clone()));
                        self.plot_area.add_scatter_plot(config, &self.app_state, self.central_rect);
                    }
                    NewPlotConfig::ScrollChart(mut config) => {
                        config.id = self.app_state.alloc_plot_id();
                        self.app_state.plots.push(PlotConfig::ScrollChart(config.clone()));
                        self.plot_area.add_scroll_chart(config, &self.app_state, self.central_rect);
                    }
                }
            }

            PaneAction::RemovePlot(id) => {
                self.plot_area.remove_plot(id);
                self.app_state.plots.retain(|p| p.id() != id);
            }

            PaneAction::RebindPlot(plot_id, new_source_id) => {
                // Get old source_id before updating
                let old_source_id = self.app_state.plots.iter().find(|p| p.id() == plot_id)
                    .map(|p| p.source_id());

                // Update the plot config's source_id
                if let Some(p) = self.app_state.plots.iter_mut().find(|p| p.id() == plot_id) {
                    match p {
                        PlotConfig::Map(c) => c.source_id = new_source_id,
                        PlotConfig::Scatter(c) => c.source_id = new_source_id,
                        PlotConfig::ScrollChart(c) => c.source_id = new_source_id,
                    }
                }

                // Migrate filters scoped to the old source to the new source
                if let Some(old_id) = old_source_id {
                    for filter in &mut self.app_state.filters {
                        if filter.source_id == Some(old_id) {
                            filter.source_id = Some(new_source_id);
                        }
                    }
                }

                // Update live plot manager config
                self.plot_area.rebind_plot_source(plot_id, new_source_id);
                // Re-sync
                self.plot_area.sync_plot(plot_id, &self.app_state);
            }

            PaneAction::AddFilter(mut filter) => {
                filter.id = self.app_state.alloc_filter_id();
                self.app_state.filters.push(filter);
                self.plot_area.sync_all_filters(&self.app_state);
            }

            PaneAction::RemoveFilter(id) => {
                self.app_state.filters.retain(|f| f.id != id);
                self.plot_area.sync_all_filters(&self.app_state);
            }

            PaneAction::ToggleFilter(id) => {
                if let Some(f) = self.app_state.filters.iter_mut().find(|f| f.id == id) {
                    f.enabled = !f.enabled;
                }
                self.plot_area.sync_all_filters(&self.app_state);
            }

            // ── Playback actions ─────────────────────────────────────────
            PaneAction::PlaybackStart(source_id) => {
                self.start_playback(source_id);
            }
            PaneAction::PlaybackStop => {
                self.app_state.playback.stop();
                self.remove_playback_filter();
                self.plot_area.sync_all_filters(&self.app_state);
            }
            PaneAction::PlaybackToggle => {
                self.app_state.playback.toggle_play_pause();
            }
            PaneAction::PlaybackSeek(t) => {
                self.app_state.playback.seek(t);
                self.update_playback_filter();
                self.plot_area.sync_all_filters(&self.app_state);
            }
            PaneAction::PlaybackStepForward => {
                self.app_state.playback.step_forward();
                self.update_playback_filter();
                self.plot_area.sync_all_filters(&self.app_state);
            }
            PaneAction::PlaybackStepBackward => {
                self.app_state.playback.step_backward();
                self.update_playback_filter();
                self.plot_area.sync_all_filters(&self.app_state);
            }
            PaneAction::PlaybackJumpStart => {
                self.app_state.playback.jump_to_start();
                self.update_playback_filter();
                self.plot_area.sync_all_filters(&self.app_state);
            }
            PaneAction::PlaybackJumpEnd => {
                self.app_state.playback.jump_to_end();
                self.update_playback_filter();
                self.plot_area.sync_all_filters(&self.app_state);
            }
            PaneAction::PlaybackSpeedChanged(speed) => {
                self.app_state.playback.speed = speed;
            }
            PaneAction::PlaybackTrailChanged(trail) => {
                self.app_state.playback.trail_duration = trail;
                self.update_playback_filter();
                self.plot_area.sync_all_filters(&self.app_state);
            }
            PaneAction::PlaybackLoopToggle => {
                self.app_state.playback.loop_enabled = !self.app_state.playback.loop_enabled;
            }
            PaneAction::PlaybackColumnChanged(col_name) => {
                self.change_playback_column(&col_name);
            }
        }
    }

    fn handle_plot_action(&mut self, action: PlotAction) {
        match action {
            PlotAction::Closed(id) => {
                self.app_state.plots.retain(|p| p.id() != id);
                // Clear selection if it came from the closed plot.
                if let Some(sel) = &self.app_state.selection {
                    if sel.plot_id == id {
                        self.app_state.selection = None;
                    }
                }
            }
            PlotAction::ConfigChanged(new_config) => {
                let id = new_config.id();
                if let Some(p) = self.app_state.plots.iter_mut().find(|p| p.id() == id) {
                    *p = new_config;
                }
                self.plot_area.sync_plot(id, &self.app_state);
            }
            PlotAction::SelectionChanged(sel) => {
                self.app_state.selection = sel;
            }
            PlotAction::FilterToSelection(sel) => {
                let values: Vec<String> = {
                    let mut v: Vec<usize> = sel.indices.iter().copied().collect();
                    v.sort_unstable();
                    v.iter().map(|i| i.to_string()).collect()
                };
                let mut filter = Filter {
                    id: 0,
                    source_id: Some(sel.source_id),
                    column: String::new(),
                    op: FilterOp::RowIndices,
                    value: values.join("|"),
                    enabled: true,
                };
                filter.id = self.app_state.alloc_filter_id();
                self.app_state.filters.push(filter);
                self.app_state.selection = None;
                self.plot_area.sync_all_filters(&self.app_state);
            }
            PlotAction::LegendAction(la) => {
                self.handle_legend_action(la);
            }
            PlotAction::RebindSource { plot_id, new_source_id } => {
                use crate::plot::plot_config::PlotConfig;
                // Get old source_id before updating
                let old_source_id = self.app_state.plots.iter().find(|p| p.id() == plot_id)
                    .map(|p| p.source_id());

                if let Some(p) = self.app_state.plots.iter_mut().find(|p| p.id() == plot_id) {
                    match p {
                        PlotConfig::Map(c) => c.source_id = new_source_id,
                        PlotConfig::Scatter(c) => c.source_id = new_source_id,
                        PlotConfig::ScrollChart(c) => c.source_id = new_source_id,
                    }
                }

                // Migrate filters scoped to the old source to the new source
                if let Some(old_id) = old_source_id {
                    for filter in &mut self.app_state.filters {
                        if filter.source_id == Some(old_id) {
                            filter.source_id = Some(new_source_id);
                        }
                    }
                }

                self.plot_area.rebind_plot_source(plot_id, new_source_id);
                self.plot_area.sync_plot(plot_id, &self.app_state);
            }
        }
    }

    fn handle_legend_action(&mut self, action: LegendAction) {
        match action {
            LegendAction::SelectCategory { source_id, plot_id, col, value, additive } => {
                // Find all row indices where col == value
                if let Some(source) = self.app_state.sources.iter().find(|s| s.id == source_id) {
                    let indices = find_rows_matching(&source.df, &col, &value);
                    if additive {
                        if let Some(sel) = &mut self.app_state.selection {
                            for idx in indices {
                                sel.indices.insert(idx);
                            }
                        } else {
                            self.app_state.selection = Some(
                                crate::state::selection::SelectionSet::from_indices(plot_id, source_id, indices),
                            );
                        }
                    } else {
                        self.app_state.selection = Some(
                            crate::state::selection::SelectionSet::from_indices(plot_id, source_id, indices),
                        );
                    }
                }
            }
            LegendAction::FilterToValue { source_id, col, value } => {
                let mut filter = Filter {
                    id: 0,
                    source_id: Some(source_id),
                    column: col,
                    op: FilterOp::Eq,
                    value,
                    enabled: true,
                };
                filter.id = self.app_state.alloc_filter_id();
                self.app_state.filters.push(filter);
                self.plot_area.sync_all_filters(&self.app_state);
            }
            LegendAction::SelectAllSharing { source_id, plot_id, anchor_col, anchor_value, target_col } => {
                // Find all rows where anchor_col == anchor_value, get their target_col values,
                // then select all rows that share any of those target_col values.
                if let Some(source) = self.app_state.sources.iter().find(|s| s.id == source_id) {
                    // Step 1: get target values from anchor matches
                    let anchor_rows = find_rows_matching(&source.df, &anchor_col, &anchor_value);
                    let target_values = get_values_at_rows(&source.df, &target_col, &anchor_rows);

                    // Step 2: find all rows matching any target value
                    let mut all_indices = std::collections::HashSet::new();
                    for tv in &target_values {
                        for idx in find_rows_matching(&source.df, &target_col, tv) {
                            all_indices.insert(idx);
                        }
                    }

                    self.app_state.selection = Some(
                        crate::state::selection::SelectionSet::from_indices(plot_id, source_id, all_indices),
                    );
                }
            }

            LegendAction::FilterToRowValue { source_id, col, row_index } => {
                if let Some(source) = self.app_state.sources.iter().find(|s| s.id == source_id) {
                    if let Some(value) = get_value_at_row(&source.df, &col, row_index) {
                        let mut filter = Filter {
                            id: 0,
                            source_id: Some(source_id),
                            column: col,
                            op: FilterOp::Eq,
                            value,
                            enabled: true,
                        };
                        filter.id = self.app_state.alloc_filter_id();
                        self.app_state.filters.push(filter);
                        self.plot_area.sync_all_filters(&self.app_state);
                    }
                }
            }

            LegendAction::SelectAllSharingRow { source_id, plot_id, row_index, target_col } => {
                if let Some(source) = self.app_state.sources.iter().find(|s| s.id == source_id) {
                    if let Some(value) = get_value_at_row(&source.df, &target_col, row_index) {
                        let indices = find_rows_matching(&source.df, &target_col, &value);
                        self.app_state.selection = Some(
                            crate::state::selection::SelectionSet::from_indices(plot_id, source_id, indices),
                        );
                    }
                }
            }
        }
    }
}

/// Get the string value of a column at a specific row index.
fn get_value_at_row(df: &polars::prelude::DataFrame, col_name: &str, row: usize) -> Option<String> {
    use polars::prelude::*;
    let col = df.column(col_name).ok()?;
    let series = col.as_series()?.clone();
    let cast = series.cast(&DataType::String).unwrap_or(series);
    let ca = cast.str().ok()?;
    if row < ca.len() {
        ca.get(row).map(|s| s.to_string())
    } else {
        None
    }
}

/// Find all row indices where `col == value` (string comparison).
fn find_rows_matching(df: &polars::prelude::DataFrame, col_name: &str, value: &str) -> Vec<usize> {
    use polars::prelude::*;
    let col = match df.column(col_name) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let series = match col.as_series() {
        Some(s) => s.clone(),
        None => return Vec::new(),
    };
    let cast = series.cast(&DataType::String).unwrap_or(series);
    let ca = match cast.str() {
        Ok(c) => c.clone(),
        Err(_) => return Vec::new(),
    };
    ca.into_iter()
        .enumerate()
        .filter_map(|(i, opt)| {
            if opt == Some(value) { Some(i) } else { None }
        })
        .collect()
}

/// Get distinct string values of `col` at the given row indices.
fn get_values_at_rows(df: &polars::prelude::DataFrame, col_name: &str, rows: &[usize]) -> Vec<String> {
    use polars::prelude::*;
    let col = match df.column(col_name) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let series = match col.as_series() {
        Some(s) => s.clone(),
        None => return Vec::new(),
    };
    let cast = series.cast(&DataType::String).unwrap_or(series);
    let ca = match cast.str() {
        Ok(c) => c.clone(),
        Err(_) => return Vec::new(),
    };
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for &i in rows {
        if i < ca.len() {
            if let Some(v) = ca.get(i) {
                let s = v.to_string();
                if seen.insert(s.clone()) {
                    result.push(s);
                }
            }
        }
    }
    result
}

// ── Playback helpers ─────────────────────────────────────────────────────────

/// Sentinel filter ID used for the managed playback temporal filter.
const PLAYBACK_FILTER_ID: usize = usize::MAX;

impl DataVisualizerApp {
    /// Start playback for a source: detect time column, scan min/max, create initial filter.
    fn start_playback(&mut self, source_id: crate::data::source::SourceId) {
        use crate::state::playback::TimeUnit;

        let source = match self.app_state.sources.iter().find(|s| s.id == source_id) {
            Some(s) => s,
            None => return,
        };

        // Find the first timestamp column; fall back to first numeric column.
        let time_field = source
            .schema
            .fields
            .iter()
            .find(|f| f.kind == FieldKind::Timestamp)
            .or_else(|| source.schema.fields.iter().find(|f| f.kind.is_numeric()));
        let time_field = match time_field {
            Some(f) => f.clone(),
            None => return,
        };

        let (time_min, time_max) = match scan_column_range(&source.df, &time_field.name) {
            Some(r) => r,
            None => {
                self.app_state
                    .notifications
                    .push("Cannot start playback: invalid time range.".to_string());
                return;
            }
        };

        let time_unit = if time_field.kind == FieldKind::Timestamp {
            TimeUnit::Seconds
        } else {
            TimeUnit::Raw
        };

        self.app_state
            .playback
            .init_for_source(source_id, time_field.name.clone(), time_min, time_max, time_unit);

        // Create the initial temporal filter.
        self.update_playback_filter();
        self.plot_area.sync_all_filters(&self.app_state);
    }

    /// Switch the playback time column without stopping playback.
    fn change_playback_column(&mut self, col_name: &str) {
        use crate::state::playback::TimeUnit;

        let source_id = match self.app_state.playback.source_id {
            Some(id) => id,
            None => return,
        };
        let source = match self.app_state.sources.iter().find(|s| s.id == source_id) {
            Some(s) => s,
            None => return,
        };

        let field = match source.schema.fields.iter().find(|f| f.name == col_name) {
            Some(f) => f.clone(),
            None => return,
        };

        let (time_min, time_max) = match scan_column_range(&source.df, col_name) {
            Some(r) => r,
            None => {
                self.app_state
                    .notifications
                    .push(format!("Cannot use column '{}': invalid range.", col_name));
                return;
            }
        };

        let time_unit = if field.kind == FieldKind::Timestamp {
            TimeUnit::Seconds
        } else {
            TimeUnit::Raw
        };

        self.app_state
            .playback
            .set_time_column(col_name.to_string(), time_min, time_max, time_unit);

        self.update_playback_filter();
        self.plot_area.sync_all_filters(&self.app_state);
    }

    /// Update (or create) the managed playback temporal filter based on current playback state.
    fn update_playback_filter(&mut self) {
        let pb = &self.app_state.playback;
        let col = match &pb.time_column {
            Some(c) => c.clone(),
            None => return,
        };

        // Remove existing playback filter.
        self.app_state
            .filters
            .retain(|f| f.id != PLAYBACK_FILTER_ID);

        let (op, value) = if let Some(trail) = pb.trail_duration {
            let lo = (pb.current_time - trail).max(pb.time_range.0);
            (FilterOp::TimeRange, format!("{}|{}", lo, pb.current_time))
        } else {
            (FilterOp::TimeLe, format!("{}", pb.current_time))
        };

        let filter = Filter {
            id: PLAYBACK_FILTER_ID,
            source_id: pb.source_id,
            column: col,
            op,
            value,
            enabled: true,
        };
        self.app_state.filters.push(filter);
    }

    /// Remove the managed playback temporal filter.
    fn remove_playback_filter(&mut self) {
        self.app_state
            .filters
            .retain(|f| f.id != PLAYBACK_FILTER_ID);
    }
}

/// Scan a numeric column for its (min, max) as f64. Returns None if invalid.
fn scan_column_range(df: &polars::prelude::DataFrame, col_name: &str) -> Option<(f64, f64)> {
    let col = df.column(col_name).ok()?;
    let series = col.as_series()?.clone();
    let cast = series.cast(&polars::prelude::DataType::Float64).ok()?;
    let ca = cast.f64().ok()?;
    let min = ca.into_iter().flatten().fold(f64::INFINITY, f64::min);
    let max = ca.into_iter().flatten().fold(f64::NEG_INFINITY, f64::max);
    if min.is_finite() && max.is_finite() && min < max {
        Some((min, max))
    } else {
        None
    }
}

// ── Frame builders ────────────────────────────────────────────────────────────

fn menu_bar_frame(theme: &AppTheme) -> egui::Frame {
    egui::Frame {
        fill: theme.colors.bg_panel,
        inner_margin: egui::Margin::from(egui::vec2(8.0, 4.0)),
        stroke: egui::Stroke::new(1.0, theme.colors.border),
        ..Default::default()
    }
}

fn side_panel_frame(theme: &AppTheme) -> egui::Frame {
    egui::Frame {
        fill: theme.colors.bg_panel,
        inner_margin: egui::Margin::from(theme.spacing.panel_padding),
        stroke: egui::Stroke::new(1.0, theme.colors.border),
        ..Default::default()
    }
}

fn plot_area_frame(theme: &AppTheme) -> egui::Frame {
    egui::Frame {
        fill: theme.colors.bg_app,
        inner_margin: egui::Margin::from(0.0_f32),
        ..Default::default()
    }
}

// ── Fonts ─────────────────────────────────────────────────────────────────────

fn setup_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "Hack".to_owned());
    ctx.set_fonts(fonts);
}
