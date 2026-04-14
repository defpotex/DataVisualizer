use crate::data::loader::load_csv_async;
use crate::state::app_state::AppState;
use crate::theme::{AppTheme, ThemePreset};
use crate::ui::{left_pane::LeftPane, menu_bar::MenuBar, plot_area::PlotArea, right_pane::RightPane};
use crate::ui::plot_grid::PlotAction;
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

        let (had_events, sync_events) = self.app_state.poll_events();
        for evt in sync_events {
            self.plot_area.apply_sync_event(evt);
        }
        if had_events {
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
        if self.right_pane_visible {
            egui::Panel::right("right_pane")
                .resizable(true)
                .default_width(200.0)
                .min_width(140.0)
                .max_width(400.0)
                .frame(side_panel_frame(&theme))
                .show(ctx, |ui| {
                    let legends = self.plot_area.legend_data();
                    self.right_pane.show(ui, &theme, &legends);
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
        let plot_actions = self.plot_area.show_windows(ctx, &theme, self.central_rect, GRID_SIZE, &self.app_state.perf, self.app_state.selection.as_ref());
        for action in plot_actions {
            self.handle_plot_action(action);
        }

        if let Some(a) = menu_action { self.handle_menu_action(a); }
        if let Some(a) = pane_action { self.handle_pane_action(a); }
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
    ToggleLegendPane,
}

pub enum PaneAction {
    OpenCsv,
    RemoveSource(usize),
    AddPlot(crate::ui::add_plot_dialog::NewPlotConfig),
    RemovePlot(usize),
    AddFilter(crate::data::filter::Filter),
    RemoveFilter(usize),
    ToggleFilter(usize),
}

impl DataVisualizerApp {
    fn handle_menu_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::OpenCsv => self.open_csv_dialog(),
            MenuAction::ToggleLegendPane => self.right_pane_visible = !self.right_pane_visible,
        }
    }

    fn handle_pane_action(&mut self, action: PaneAction) {
        use crate::plot::plot_config::PlotConfig;
        use crate::ui::add_plot_dialog::NewPlotConfig;
        match action {
            PaneAction::OpenCsv => self.open_csv_dialog(),

            PaneAction::RemoveSource(id) => {
                self.plot_area.remove_plots_for_source(id);
                self.app_state.plots.retain(|p| p.source_id() != id);
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
                }
            }

            PaneAction::RemovePlot(id) => {
                self.plot_area.remove_plot(id);
                self.app_state.plots.retain(|p| p.id() != id);
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
                use crate::data::filter::{Filter, FilterOp};
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
        }
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
