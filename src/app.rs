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
}

impl DataVisualizerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let persisted: PersistentState = cc
            .storage
            .and_then(|s| eframe::get_value(s, STORAGE_KEY))
            .unwrap_or_default();

        let theme = AppTheme::from_preset(persisted.theme_preset);

        let mut style = (*cc.egui_ctx.style()).clone();
        theme.apply_to_style(&mut style);
        cc.egui_ctx.set_style(style);

        setup_fonts(&cc.egui_ctx);

        Self {
            left_pane_width: persisted.left_pane_width,
            app_state: AppState::default(),
            menu_bar: MenuBar::default(),
            left_pane: LeftPane::default(),
            plot_area: PlotArea::default(),
            theme,
        }
    }

    #[allow(dead_code)]
    pub fn apply_theme(&mut self, preset: ThemePreset, ctx: &Context) {
        self.theme = AppTheme::from_preset(preset);
        let mut style = (*ctx.style()).clone();
        self.theme.apply_to_style(&mut style);
        ctx.set_style(style);
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
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Re-apply theme every frame so popups/menus always use our visuals.
        {
            let mut style = (*ctx.style()).clone();
            self.theme.apply_to_style(&mut style);
            ctx.set_style(style);
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
        egui::TopBottomPanel::top("menu_bar")
            .frame(menu_bar_frame(&theme))
            .show(ctx, |ui| {
                menu_action = self.menu_bar.show(ui, &theme);
            });

        // ── Left pane ─────────────────────────────────────────────────────────
        egui::SidePanel::left("left_pane")
            .resizable(true)
            .default_width(self.left_pane_width)
            .min_width(theme.spacing.left_pane_min_width)
            .max_width(theme.spacing.left_pane_max_width)
            .frame(side_panel_frame(&theme))
            .show(ctx, |ui| {
                self.left_pane_width = ui.available_width() + ui.spacing().item_spacing.x;
                pane_action = self.left_pane.show(ui, &theme, &self.app_state);
            });

        // ── Plot area ─────────────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(plot_area_frame(&theme))
            .show(ctx, |ui| {
                self.plot_area.show(ui, &theme, &self.app_state);
            });

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
            PaneAction::RemoveSource(id) => self.app_state.remove_source(id),
        }
    }
}

// ── Frame builders ────────────────────────────────────────────────────────────

fn menu_bar_frame(theme: &AppTheme) -> egui::Frame {
    egui::Frame {
        fill: theme.colors.bg_panel,
        inner_margin: egui::Margin::symmetric(8.0, 4.0),
        stroke: egui::Stroke::new(1.0, theme.colors.border),
        ..Default::default()
    }
}

fn side_panel_frame(theme: &AppTheme) -> egui::Frame {
    egui::Frame {
        fill: theme.colors.bg_panel,
        inner_margin: egui::Margin::same(theme.spacing.panel_padding),
        stroke: egui::Stroke::new(1.0, theme.colors.border),
        ..Default::default()
    }
}

fn plot_area_frame(theme: &AppTheme) -> egui::Frame {
    egui::Frame {
        fill: theme.colors.bg_app,
        inner_margin: egui::Margin::same(0.0),
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
