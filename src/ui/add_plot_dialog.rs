use crate::data::schema::FieldKind;
use crate::plot::plot_config::{MapPlotConfig, TileScheme};
use crate::state::app_state::AppState;
use crate::theme::AppTheme;
use egui::{RichText, Ui, Window};

/// Modal dialog for creating a new map plot.
/// Owned by `LeftPane`; rendered as a floating `egui::Window`.
pub struct AddPlotDialog {
    pub is_open: bool,

    // Form state
    title: String,
    selected_source_idx: usize,
    lat_col_idx: usize,
    lon_col_idx: usize,
    tile_scheme: TileScheme,
}

impl Default for AddPlotDialog {
    fn default() -> Self {
        Self {
            is_open: false,
            title: String::from("Map Plot"),
            selected_source_idx: 0,
            lat_col_idx: 0,
            lon_col_idx: 0,
            tile_scheme: TileScheme::CartoDark,
        }
    }
}

impl AddPlotDialog {
    /// Open the dialog and reset to sane defaults.
    pub fn open(&mut self) {
        self.is_open = true;
        self.selected_source_idx = 0;
        self.lat_col_idx = 0;
        self.lon_col_idx = 0;
        self.tile_scheme = TileScheme::CartoDark;
        self.title = String::from("Map Plot");
    }

    /// Show the dialog window.  Returns `Some(MapPlotConfig)` when the user clicks
    /// **Create**, `None` otherwise.
    /// Returns `Some(MapPlotConfig)` when the user clicks **Create**.
    /// The returned config has `id = 0` (placeholder); the caller assigns the real ID.
    pub fn show(&mut self, ui: &mut Ui, theme: &AppTheme, state: &AppState) -> Option<MapPlotConfig> {
        if !self.is_open {
            return None;
        }

        let c = &theme.colors;
        let s = &theme.spacing;
        let mut result = None;
        let mut close = false;

        Window::new(RichText::new("Add Map Plot").color(c.text_primary).size(s.font_body + 1.0).strong())
            .collapsible(false)
            .resizable(false)
            .min_width(360.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .frame(egui::Frame {
                fill: c.bg_panel,
                stroke: egui::Stroke::new(1.0, c.border),
                corner_radius: egui::Rounding::from(6.0_f32),
                inner_margin: egui::Margin::from(16.0_f32),
                ..Default::default()
            })
            .show(ui.ctx(), |ui| {
                // ── Source selector ───────────────────────────────────────────
                ui.label(RichText::new("Data Source").color(c.text_secondary).size(s.font_small));
                ui.add_space(2.0);

                let source_label = state.sources
                    .get(self.selected_source_idx)
                    .map(|s| s.label.as_str())
                    .unwrap_or("(none)");

                egui::ComboBox::from_id_salt("add_plot_source")
                    .selected_text(RichText::new(source_label).color(c.text_primary).size(s.font_body))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for (idx, src) in state.sources.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.selected_source_idx,
                                idx,
                                RichText::new(&src.label).color(c.text_primary).size(s.font_body),
                            );
                        }
                    });

                ui.add_space(10.0);

                // ── Column selectors ──────────────────────────────────────────
                if let Some(source) = state.sources.get(self.selected_source_idx) {
                    let numeric_fields: Vec<&str> = source.schema.fields.iter()
                        .filter(|f| is_numeric_or_geo(&f.kind))
                        .map(|f| f.name.as_str())
                        .collect();

                    // Auto-detect lat/lon defaults when source changes
                    if self.lat_col_idx >= numeric_fields.len() { self.lat_col_idx = 0; }
                    if self.lon_col_idx >= numeric_fields.len() { self.lon_col_idx = 0; }

                    // Try to pre-select best guesses
                    let lat_default = numeric_fields.iter().position(|n| is_lat_name(n)).unwrap_or(0);
                    let lon_default = numeric_fields.iter().position(|n| is_lon_name(n)).unwrap_or(0);
                    if self.lat_col_idx == 0 && lat_default != 0 { self.lat_col_idx = lat_default; }
                    if self.lon_col_idx == 0 && lon_default != 0 { self.lon_col_idx = lon_default; }

                    // Latitude column
                    ui.label(RichText::new("Latitude Column").color(c.text_secondary).size(s.font_small));
                    ui.add_space(2.0);
                    let lat_label = numeric_fields.get(self.lat_col_idx).copied().unwrap_or("(none)");
                    egui::ComboBox::from_id_salt("add_plot_lat")
                        .selected_text(RichText::new(lat_label).color(c.text_data).size(s.font_body).monospace())
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for (idx, name) in numeric_fields.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.lat_col_idx,
                                    idx,
                                    RichText::new(*name).color(c.text_data).size(s.font_body).monospace(),
                                );
                            }
                        });

                    ui.add_space(8.0);

                    // Longitude column
                    ui.label(RichText::new("Longitude Column").color(c.text_secondary).size(s.font_small));
                    ui.add_space(2.0);
                    let lon_label = numeric_fields.get(self.lon_col_idx).copied().unwrap_or("(none)");
                    egui::ComboBox::from_id_salt("add_plot_lon")
                        .selected_text(RichText::new(lon_label).color(c.text_data).size(s.font_body).monospace())
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for (idx, name) in numeric_fields.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.lon_col_idx,
                                    idx,
                                    RichText::new(*name).color(c.text_data).size(s.font_body).monospace(),
                                );
                            }
                        });

                    ui.add_space(10.0);

                    // ── Tile scheme ───────────────────────────────────────────
                    ui.label(RichText::new("Map Tiles").color(c.text_secondary).size(s.font_small));
                    ui.add_space(2.0);
                    egui::ComboBox::from_id_salt("add_plot_tiles")
                        .selected_text(RichText::new(self.tile_scheme.label()).color(c.text_primary).size(s.font_body))
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for scheme in TileScheme::all() {
                                ui.selectable_value(
                                    &mut self.tile_scheme,
                                    scheme.clone(),
                                    RichText::new(scheme.label()).color(c.text_primary).size(s.font_body),
                                );
                            }
                        });

                    ui.add_space(10.0);

                    // ── Title ─────────────────────────────────────────────────
                    ui.label(RichText::new("Plot Title").color(c.text_secondary).size(s.font_small));
                    ui.add_space(2.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut self.title)
                            .desired_width(ui.available_width())
                            .text_color(c.text_primary)
                            .font(egui::FontSelection::FontId(egui::FontId::proportional(s.font_body))),
                    );

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // ── Buttons ───────────────────────────────────────────────
                    let can_create = !numeric_fields.is_empty();

                    ui.horizontal(|ui| {
                        let create_btn = egui::Button::new(
                            RichText::new("Create Plot")
                                .color(if can_create { c.bg_app } else { c.text_secondary })
                                .size(s.font_body)
                                .strong(),
                        )
                        .fill(if can_create { c.accent_primary } else { c.widget_bg })
                        .min_size(egui::vec2(120.0, 0.0));

                        if ui.add_enabled(can_create, create_btn).clicked() {
                            let lat_col = numeric_fields.get(self.lat_col_idx).map(|s| s.to_string()).unwrap_or_default();
                            let lon_col = numeric_fields.get(self.lon_col_idx).map(|s| s.to_string()).unwrap_or_default();
                            result = Some(MapPlotConfig {
                                id: 0, // Placeholder; real ID assigned by app when handling action
                                title: self.title.clone(),
                                source_id: source.id,
                                lat_col,
                                lon_col,
                                color_col: None,
                                tile_scheme: self.tile_scheme.clone(),
                            });
                            close = true;
                        }

                        ui.add_space(8.0);

                        if ui.button(RichText::new("Cancel").color(c.text_secondary).size(s.font_body)).clicked() {
                            close = true;
                        }
                    });
                } else {
                    ui.label(
                        RichText::new("No data sources loaded. Load a CSV first.")
                            .color(c.accent_warning)
                            .size(s.font_body),
                    );
                    ui.add_space(8.0);
                    if ui.button(RichText::new("Close").color(c.text_secondary).size(s.font_body)).clicked() {
                        close = true;
                    }
                }
            });

        if close {
            self.is_open = false;
        }

        result
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn is_numeric_or_geo(kind: &FieldKind) -> bool {
    matches!(
        kind,
        FieldKind::Latitude
            | FieldKind::Longitude
            | FieldKind::Altitude
            | FieldKind::Speed
            | FieldKind::Heading
            | FieldKind::Float
            | FieldKind::Integer
    )
}

fn is_lat_name(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("lat")
}

fn is_lon_name(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("lon") || n.contains("lng")
}
