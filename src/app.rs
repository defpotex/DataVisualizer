use crate::data::loader::load_csv_async;
use crate::state::app_state::AppState;
use crate::theme::{AppTheme, ThemePreset};
use crate::ui::{left_pane::LeftPane, menu_bar::MenuBar, plot_area::PlotArea};
use eframe::Storage;
use egui::Context;
use serde::{Deserialize, Serialize};

const STORAGE_KEY: &str = "datavisualizer_app_state";

// ── Persistent state ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct PersistentState {
    theme_preset: ThemePreset,
    left_pane_width: f32,
}

impl Default for PersistentState {
    fn default() -> Self {
        let default_theme = AppTheme::default();
        Self {
            theme_preset: default_theme.preset,
            left_pane_width: default_theme.spacing.left_pane_default_width,
        }
    }
}

// ── Main app struct ───────────────────────────────────────────────────────────

pub struct DataVisualizerApp {
    theme: AppTheme,
    left_pane_width: f32,

    app_state: AppState,

    menu_bar: MenuBar,
    left_pane: LeftPane,
    plot_area: PlotArea,

    /// Central panel rect from the previous frame — used to constrain plot windows.
    central_rect: egui::Rect,
}

impl DataVisualizerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let persisted: PersistentState = cc
            .storage
            .and_then(|s| eframe::get_value(s, STORAGE_KEY))
            .unwrap_or_default();

        let theme = AppTheme::from_preset(persisted.theme_preset);

        let mut style = (*cc.egui_ctx.global_style()).clone();
        theme.apply_to_style(&mut style);
        cc.egui_ctx.set_global_style(style);

        setup_fonts(&cc.egui_ctx);

        Self {
            left_pane_width: persisted.left_pane_width,
            app_state: AppState::default(),
            menu_bar: MenuBar::default(),
            left_pane: LeftPane::default(),
            plot_area: PlotArea::default(),
            // Sensible default; overwritten after first frame's CentralPanel is shown.
            central_rect: egui::Rect::from_min_size(
                egui::pos2(260.0, 28.0),
                egui::vec2(1100.0, 860.0),
            ),
            theme,
        }
    }

    #[allow(dead_code)]
    pub fn apply_theme(&mut self, preset: ThemePreset, ctx: &Context) {
        self.theme = AppTheme::from_preset(preset);
        let mut style = (*ctx.global_style()).clone();
        self.theme.apply_to_style(&mut style);
        ctx.set_global_style(style);
    }

    /// Open a native file dialog and kick off an async CSV load.
    fn open_csv_dialog(&mut self) {
        let id = self.app_state.next_source_id();
        let tx = self.app_state.event_tx.clone();

        // rfd file dialog — runs on a background thread so the UI stays live.
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
    /// eframe 0.34 requires `ui` as the primary trait method.
    /// We override `update` instead (which gives us `ctx`), so this stub is never called.
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Re-apply theme every frame so popups/menus always use our visuals.
        {
            let mut style = (*ctx.global_style()).clone();
            self.theme.apply_to_style(&mut style);
            ctx.set_global_style(style);
        }

        // Drain background-thread events (new sources, errors).
        self.app_state.poll_events();

        // If a load just finished, request a repaint so the UI updates immediately.
        if self.app_state.has_sources() {
            ctx.request_repaint();
        }

        // Clone theme so closures can borrow self mutably without conflict.
        let theme = self.theme.clone();

        // Collect actions from panels — handle after all panels are drawn.
        let mut menu_action: Option<MenuAction> = None;
        let mut pane_action: Option<PaneAction> = None;

        // ── Menu bar ──────────────────────────────────────────────────────────
        egui::Panel::top("menu_bar")
            .frame(menu_bar_frame(&theme))
            .show(ctx, |ui| {
                menu_action = self.menu_bar.show(ui, &theme);
            });

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
        // Capture the central rect so floating windows can be constrained to it.
        let central_response = egui::CentralPanel::default()
            .frame(plot_area_frame(&theme))
            .show(ctx, |ui| {
                self.plot_area.show(ui, &theme, &self.app_state);
            });
        self.central_rect = central_response.response.rect;

        // ── Floating plot windows (drawn after panels so constrain_to works) ──
        // Windows float above panel contents but are bounded to the central rect.
        let closed_plots = self.plot_area.show_windows(ctx, &theme, self.central_rect);
        for id in closed_plots {
            self.app_state.plots.retain(|p| p.id() != id);
        }

        // Handle actions after all panels are drawn (avoids borrow conflicts)
        if let Some(a) = menu_action { self.handle_menu_action(a); }
        if let Some(a) = pane_action { self.handle_pane_action(a); }
    }

    fn save(&mut self, storage: &mut dyn Storage) {
        let state = PersistentState {
            theme_preset: self.theme.preset,
            left_pane_width: self.left_pane_width,
        };
        eframe::set_value(storage, STORAGE_KEY, &state);
    }
}

// ── Action handling ───────────────────────────────────────────────────────────

/// Actions returned from the menu bar that require app-level handling.
pub enum MenuAction {
    OpenCsv,
}

/// Actions returned from the left pane.
pub enum PaneAction {
    OpenCsv,
    RemoveSource(usize),
    AddPlot(crate::plot::plot_config::MapPlotConfig),
    RemovePlot(usize),
}

impl DataVisualizerApp {
    fn handle_menu_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::OpenCsv => self.open_csv_dialog(),
        }
    }

    fn handle_pane_action(&mut self, action: PaneAction) {
        match action {
            PaneAction::OpenCsv => self.open_csv_dialog(),
            PaneAction::RemoveSource(id) => {
                // Remove plots that reference this source before removing the source.
                self.plot_area.remove_plots_for_source(id);
                self.app_state.plots.retain(|p| {
                    if let crate::plot::plot_config::PlotConfig::Map(c) = p { c.source_id != id } else { true }
                });
                self.app_state.remove_source(id);
            }
            PaneAction::AddPlot(mut config) => {
                config.id = self.app_state.alloc_plot_id();
                let plot_config = crate::plot::plot_config::PlotConfig::Map(config.clone());
                self.app_state.plots.push(plot_config);
                self.plot_area.add_map_plot(config, &self.app_state, self.central_rect);
            }
            PaneAction::RemovePlot(id) => {
                self.plot_area.remove_plot(id);
                self.app_state.plots.retain(|p| p.id() != id);
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
