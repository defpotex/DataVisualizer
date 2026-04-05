/// AppTheme — all visual configuration in one place.
///
/// Every color, font size, and spacing constant lives here.
/// To add a new theme: add a variant to `ThemePreset` and a branch in `AppTheme::from_preset`.
/// No other file needs to change.
use egui::{Color32, FontId, Rounding, Stroke, Style, Visuals};
use serde::{Deserialize, Serialize};

// ── Preset selector ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemePreset {
    EngineeringDark,
    // Future presets — add here, implement in `from_preset` below.
    // LightMode,
    // HighContrast,
    // RadarGreen,
}

impl Default for ThemePreset {
    fn default() -> Self {
        Self::EngineeringDark
    }
}

impl ThemePreset {
    #[allow(dead_code)] // used when theme-switcher UI is implemented (roadmap backlog)
    pub fn label(&self) -> &'static str {
        match self {
            ThemePreset::EngineeringDark => "Engineering Dark",
        }
    }

    #[allow(dead_code)] // used when theme-switcher UI is implemented (roadmap backlog)
    pub fn all() -> &'static [ThemePreset] {
        &[ThemePreset::EngineeringDark]
    }
}

// ── Color palette ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeColors {
    /// Main window background
    pub bg_app: Color32,
    /// Panel / pane backgrounds (left pane, dialogs)
    pub bg_panel: Color32,
    /// Plot / canvas background
    pub bg_plot: Color32,
    /// Subtle panel borders and dividers
    pub border: Color32,
    /// Primary accent: highlights, active selections, cyan glow
    pub accent_primary: Color32,
    /// Secondary accent: good-status green, data values
    pub accent_secondary: Color32,
    /// Warning / error color
    pub accent_warning: Color32,
    /// Primary readable text
    pub text_primary: Color32,
    /// Secondary / muted labels
    pub text_secondary: Color32,
    /// Numeric data values (monospace context)
    pub text_data: Color32,
    /// Interactive widget background (buttons, combos) — rest state
    pub widget_bg: Color32,
    /// Interactive widget background — hovered
    pub widget_bg_hovered: Color32,
    /// Interactive widget background — active/pressed
    pub widget_bg_active: Color32,
    /// Drag handle / resize grip highlight
    pub drag_handle: Color32,
}

// ── Spacing & geometry ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSpacing {
    /// Default left pane width in pixels
    pub left_pane_default_width: f32,
    /// Minimum left pane width
    pub left_pane_min_width: f32,
    /// Maximum left pane width
    pub left_pane_max_width: f32,
    /// Width of the resize drag handle strip
    pub resize_handle_width: f32,
    /// Corner rounding for panels and widgets
    pub rounding: f32,
    /// Standard inner padding for panels
    pub panel_padding: f32,
    /// Section header font size
    pub font_section_header: f32,
    /// Body / label font size
    pub font_body: f32,
    /// Small / secondary label font size
    pub font_small: f32,
    /// Data value font size (monospace)
    pub font_data: f32,
}

// ── Top-level theme ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppTheme {
    pub preset: ThemePreset,
    pub colors: ThemeColors,
    pub spacing: ThemeSpacing,
}

impl Default for AppTheme {
    fn default() -> Self {
        Self::from_preset(ThemePreset::default())
    }
}

impl AppTheme {
    pub fn from_preset(preset: ThemePreset) -> Self {
        match preset {
            ThemePreset::EngineeringDark => Self::engineering_dark(),
        }
    }

    /// Apply this theme to an egui `Style`, mutating it in place.
    /// Call once on app init and again whenever the user switches themes.
    pub fn apply_to_style(&self, style: &mut Style) {
        let c = &self.colors;
        let s = &self.spacing;

        let mut visuals = Visuals::dark();

        // Window / panel backgrounds
        visuals.window_fill = c.bg_panel;
        visuals.panel_fill = c.bg_panel;
        visuals.extreme_bg_color = c.bg_app;
        visuals.faint_bg_color = c.bg_plot;
        visuals.code_bg_color = c.bg_app;

        // Text colors
        visuals.override_text_color = Some(c.text_primary);

        // Window stroke (border)
        visuals.window_stroke = Stroke::new(1.0, c.border);

        // Rounding
        let r = Rounding::same(s.rounding);
        visuals.window_rounding = r;
        visuals.menu_rounding = r;

        // Widgets
        let rw = Rounding::same(s.rounding - 1.0);

        visuals.widgets.inactive.bg_fill = c.widget_bg;
        visuals.widgets.inactive.weak_bg_fill = c.widget_bg;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, c.border);
        visuals.widgets.inactive.rounding = rw;
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, c.text_secondary);

        visuals.widgets.hovered.bg_fill = c.widget_bg_hovered;
        visuals.widgets.hovered.weak_bg_fill = c.widget_bg_hovered;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, c.accent_primary);
        visuals.widgets.hovered.rounding = rw;
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, c.text_primary);

        visuals.widgets.active.bg_fill = c.widget_bg_active;
        visuals.widgets.active.weak_bg_fill = c.widget_bg_active;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, c.accent_primary);
        visuals.widgets.active.rounding = rw;
        visuals.widgets.active.fg_stroke = Stroke::new(1.5, c.accent_primary);

        visuals.widgets.open.bg_fill = c.widget_bg_active;
        visuals.widgets.open.bg_stroke = Stroke::new(1.0, c.accent_primary);
        visuals.widgets.open.rounding = rw;
        visuals.widgets.open.fg_stroke = Stroke::new(1.5, c.accent_primary);

        // Selection highlight
        visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(
            c.accent_primary.r(),
            c.accent_primary.g(),
            c.accent_primary.b(),
            60,
        );
        visuals.selection.stroke = Stroke::new(1.0, c.accent_primary);

        // Hyperlinks
        visuals.hyperlink_color = c.accent_primary;

        // Separator lines
        visuals.window_shadow.blur = 0.0; // no shadow — flat engineering aesthetic
        visuals.window_shadow.spread = 0.0;
        visuals.popup_shadow.blur = 0.0;
        visuals.popup_shadow.spread = 0.0;

        style.visuals = visuals;

        // Spacing
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.window_margin = egui::Margin::same(s.panel_padding);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.menu_margin = egui::Margin::same(4.0);
        style.spacing.indent = 14.0;

        // Text styles
        use egui::TextStyle;
        style.text_styles = [
            (TextStyle::Heading, FontId::proportional(s.font_section_header)),
            (TextStyle::Body, FontId::proportional(s.font_body)),
            (TextStyle::Button, FontId::proportional(s.font_body)),
            (TextStyle::Small, FontId::proportional(s.font_small)),
            (TextStyle::Monospace, FontId::monospace(s.font_data)),
        ]
        .into();
    }

    // ── Preset definitions ────────────────────────────────────────────────────

    fn engineering_dark() -> Self {
        Self {
            preset: ThemePreset::EngineeringDark,
            colors: ThemeColors {
                bg_app:             Color32::from_rgb(0x0D, 0x11, 0x17),
                bg_panel:           Color32::from_rgb(0x16, 0x1B, 0x22),
                bg_plot:            Color32::from_rgb(0x0D, 0x11, 0x17),
                border:             Color32::from_rgb(0x21, 0x26, 0x2D),
                accent_primary:     Color32::from_rgb(0x00, 0xD4, 0xFF),
                accent_secondary:   Color32::from_rgb(0x39, 0xD3, 0x53),
                accent_warning:     Color32::from_rgb(0xF7, 0x81, 0x66),
                text_primary:       Color32::from_rgb(0xE6, 0xED, 0xF3),
                text_secondary:     Color32::from_rgb(0x8B, 0x94, 0x9E),
                text_data:          Color32::from_rgb(0x79, 0xC0, 0xFF),
                widget_bg:          Color32::from_rgb(0x1C, 0x22, 0x2A),
                widget_bg_hovered:  Color32::from_rgb(0x22, 0x2A, 0x35),
                widget_bg_active:   Color32::from_rgb(0x00, 0x3A, 0x4A),
                drag_handle:        Color32::from_rgb(0x00, 0xD4, 0xFF),
            },
            spacing: ThemeSpacing {
                left_pane_default_width: 260.0,
                left_pane_min_width:     180.0,
                left_pane_max_width:     480.0,
                resize_handle_width:     4.0,
                rounding:                4.0,
                panel_padding:           10.0,
                font_section_header:     13.0,
                font_body:               13.0,
                font_small:              11.0,
                font_data:               12.0,
            },
        }
    }
}
