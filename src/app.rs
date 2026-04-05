use crate::theme::{AppTheme, ThemePreset};
use crate::ui::{left_pane::LeftPane, menu_bar::MenuBar, plot_area::PlotArea};
use eframe::Storage;
use egui::Context;
use serde::{Deserialize, Serialize};

const STORAGE_KEY: &str = "datavisualizer_app_state";

// ── Persistent state (saved across restarts) ──────────────────────────────────

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

    menu_bar: MenuBar,
    left_pane: LeftPane,
    plot_area: PlotArea,
}

impl DataVisualizerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load persistent state (window geometry is handled by eframe automatically
        // via `persist_window: true`; we handle our own UI state here).
        let persisted: PersistentState = cc
            .storage
            .and_then(|s| eframe::get_value(s, STORAGE_KEY))
            .unwrap_or_default();

        let theme = AppTheme::from_preset(persisted.theme_preset);

        // Apply theme to egui style immediately so first frame is correct.
        let mut style = (*cc.egui_ctx.style()).clone();
        theme.apply_to_style(&mut style);
        cc.egui_ctx.set_style(style);

        // Load embedded JetBrains Mono for monospace data values.
        setup_fonts(&cc.egui_ctx);

        Self {
            left_pane_width: persisted.left_pane_width,
            menu_bar: MenuBar::default(),
            left_pane: LeftPane::default(),
            plot_area: PlotArea::default(),
            theme,
        }
    }

    /// Switch to a new theme preset at runtime.
    /// Called from menus in a future phase.
    #[allow(dead_code)]
    pub fn apply_theme(&mut self, preset: ThemePreset, ctx: &Context) {
        self.theme = AppTheme::from_preset(preset);
        let mut style = (*ctx.style()).clone();
        self.theme.apply_to_style(&mut style);
        ctx.set_style(style);
    }
}

impl eframe::App for DataVisualizerApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Re-apply theme every frame. egui Styles are stored in an Arc so this
        // is a cheap pointer swap, not a deep copy. Doing it here guarantees
        // popup/menu windows (which open in a new egui pass) always inherit
        // our visuals rather than any defaults eframe may reset between frames.
        {
            let mut style = (*ctx.style()).clone();
            self.theme.apply_to_style(&mut style);
            ctx.set_style(style);
        }

        let theme = &self.theme;

        // ── Top menu bar ──────────────────────────────────────────────────────
        egui::TopBottomPanel::top("menu_bar")
            .frame(menu_bar_frame(theme))
            .show(ctx, |ui| {
                self.menu_bar.show(ui, theme);
            });

        // ── Left side pane with drag-to-resize ───────────────────────────────
        let pane_width = self.left_pane_width;
        egui::SidePanel::left("left_pane")
            .resizable(true)
            .default_width(pane_width)
            .min_width(theme.spacing.left_pane_min_width)
            .max_width(theme.spacing.left_pane_max_width)
            .frame(side_panel_frame(theme))
            .show(ctx, |ui| {
                // Track width changes so we can persist them
                self.left_pane_width = ui.available_width() + ui.spacing().item_spacing.x;
                self.left_pane.show(ui, theme);
            });

        // ── Main plot area ────────────────────────────────────────────────────
        egui::CentralPanel::default()
            .frame(plot_area_frame(theme))
            .show(ctx, |ui| {
                self.plot_area.show(ui, theme);
            });
    }

    fn save(&mut self, storage: &mut dyn Storage) {
        let state = PersistentState {
            theme_preset: self.theme.preset,
            left_pane_width: self.left_pane_width,
        };
        eframe::set_value(storage, STORAGE_KEY, &state);
    }
}

// ── Frame builders — use theme colors so all chrome stays in sync ─────────────

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

// ── Font setup ────────────────────────────────────────────────────────────────

fn setup_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();

    // egui ships with its own monospace font (Hack). We alias it here so
    // future phases can swap in JetBrains Mono by adding the bytes and
    // changing the font name — one place, zero ripple.
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "Hack".to_owned());

    ctx.set_fonts(fonts);
}
