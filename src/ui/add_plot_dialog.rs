use crate::data::schema::FieldKind;
use crate::plot::plot_config::{MapPlotConfig, ScatterPlotConfig, TileScheme};
use crate::state::app_state::AppState;
use crate::theme::AppTheme;
use egui::{RichText, Ui, Window};

#[derive(Debug, Clone, PartialEq)]
enum PlotType { Map, Scatter }

/// Modal dialog for creating a new plot (Map or Scatter).
pub struct AddPlotDialog {
    pub is_open: bool,
    plot_type: PlotType,
    title: String,
    selected_source_idx: usize,
    // Map-specific
    lat_col_idx: usize,
    lon_col_idx: usize,
    tile_scheme: TileScheme,
    // Scatter-specific
    x_col_idx: usize,
    y_col_idx: usize,
}

impl Default for AddPlotDialog {
    fn default() -> Self {
        Self {
            is_open: false,
            plot_type: PlotType::Map,
            title: String::from("Map Plot"),
            selected_source_idx: 0,
            lat_col_idx: 0,
            lon_col_idx: 0,
            tile_scheme: TileScheme::CartoDark,
            x_col_idx: 0,
            y_col_idx: 0,
        }
    }
}

/// Return type so the caller knows which variant to add.
pub enum NewPlotConfig {
    Map(MapPlotConfig),
    Scatter(ScatterPlotConfig),
}

impl AddPlotDialog {
    pub fn open(&mut self) {
        self.is_open = true;
        self.selected_source_idx = 0;
        self.lat_col_idx = 0;
        self.lon_col_idx = 0;
        self.x_col_idx = 0;
        self.y_col_idx = 0;
        self.tile_scheme = TileScheme::CartoDark;
        self.title = String::from("Map Plot");
        self.plot_type = PlotType::Map;
    }

    pub fn show(&mut self, ui: &mut Ui, theme: &AppTheme, state: &AppState) -> Option<NewPlotConfig> {
        if !self.is_open { return None; }

        let c = &theme.colors;
        let s = &theme.spacing;
        let mut result = None;
        let mut close = false;

        Window::new(
            RichText::new("Add Plot")
                .color(c.text_primary)
                .size(s.font_body + 1.0)
                .strong(),
        )
        .collapsible(false)
        .resizable(false)
        .min_width(380.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(egui::Frame {
            fill: c.bg_panel,
            stroke: egui::Stroke::new(1.0, c.border),
            corner_radius: egui::CornerRadius::from(6.0_f32),
            inner_margin: egui::Margin::from(16.0_f32),
            ..Default::default()
        })
        .show(ui.ctx(), |ui| {
            // ── Plot type selector ────────────────────────────────────────────
            ui.label(RichText::new("Plot Type").color(c.text_secondary).size(s.font_small));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let map_selected = self.plot_type == PlotType::Map;
                let scatter_selected = self.plot_type == PlotType::Scatter;

                if type_btn(ui, "◈  Map", map_selected, theme).clicked() {
                    self.plot_type = PlotType::Map;
                    self.title = "Map Plot".to_string();
                }
                ui.add_space(6.0);
                if type_btn(ui, "◉  Scatter", scatter_selected, theme).clicked() {
                    self.plot_type = PlotType::Scatter;
                    self.title = "Scatter Plot".to_string();
                }
            });

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(10.0);

            // ── Source selector ───────────────────────────────────────────────
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
                            &mut self.selected_source_idx, idx,
                            RichText::new(&src.label).color(c.text_primary).size(s.font_body),
                        );
                    }
                });

            ui.add_space(10.0);

            if let Some(source) = state.sources.get(self.selected_source_idx) {
                let numeric: Vec<&str> = source.schema.fields.iter()
                    .filter(|f| is_numeric_or_geo(&f.kind))
                    .map(|f| f.name.as_str())
                    .collect();

                if self.lat_col_idx >= numeric.len() { self.lat_col_idx = 0; }
                if self.lon_col_idx >= numeric.len() { self.lon_col_idx = 0; }
                if self.x_col_idx >= numeric.len() { self.x_col_idx = 0; }
                if self.y_col_idx >= numeric.len() { self.y_col_idx = 0; }

                match self.plot_type {
                    PlotType::Map => {
                        // Pre-select lat/lon defaults.
                        let lat_default = numeric.iter().position(|n| is_lat_name(n)).unwrap_or(0);
                        let lon_default = numeric.iter().position(|n| is_lon_name(n)).unwrap_or(0);
                        if self.lat_col_idx == 0 && lat_default != 0 { self.lat_col_idx = lat_default; }
                        if self.lon_col_idx == 0 && lon_default != 0 { self.lon_col_idx = lon_default; }

                        col_picker(ui, "Latitude Column", &numeric, &mut self.lat_col_idx, "add_plot_lat", theme);
                        ui.add_space(8.0);
                        col_picker(ui, "Longitude Column", &numeric, &mut self.lon_col_idx, "add_plot_lon", theme);
                        ui.add_space(10.0);

                        ui.label(RichText::new("Map Tiles").color(c.text_secondary).size(s.font_small));
                        ui.add_space(2.0);
                        egui::ComboBox::from_id_salt("add_plot_tiles")
                            .selected_text(RichText::new(self.tile_scheme.label()).color(c.text_primary).size(s.font_body))
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                for scheme in TileScheme::all() {
                                    ui.selectable_value(
                                        &mut self.tile_scheme, scheme.clone(),
                                        RichText::new(scheme.label()).color(c.text_primary).size(s.font_body),
                                    );
                                }
                            });
                        ui.add_space(10.0);
                    }
                    PlotType::Scatter => {
                        col_picker(ui, "X Axis Column", &numeric, &mut self.x_col_idx, "add_plot_x", theme);
                        ui.add_space(8.0);
                        col_picker(ui, "Y Axis Column", &numeric, &mut self.y_col_idx, "add_plot_y", theme);
                        ui.add_space(10.0);
                    }
                }

                // Title field.
                ui.label(RichText::new("Plot Title").color(c.text_secondary).size(s.font_small));
                ui.add_space(2.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.title)
                        .desired_width(ui.available_width())
                        .text_color(c.text_primary)
                        .font(egui::FontSelection::FontId(
                            egui::FontId::proportional(s.font_body),
                        )),
                );

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                let can_create = !numeric.is_empty();
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
                        result = Some(match self.plot_type {
                            PlotType::Map => NewPlotConfig::Map(MapPlotConfig {
                                id: 0,
                                title: self.title.clone(),
                                source_id: source.id,
                                lat_col: numeric.get(self.lat_col_idx).map(|s| s.to_string()).unwrap_or_default(),
                                lon_col: numeric.get(self.lon_col_idx).map(|s| s.to_string()).unwrap_or_default(),
                                color_col: None,
                                tile_scheme: self.tile_scheme.clone(),
                            }),
                            PlotType::Scatter => NewPlotConfig::Scatter(ScatterPlotConfig {
                                id: 0,
                                title: self.title.clone(),
                                source_id: source.id,
                                x_col: numeric.get(self.x_col_idx).map(|s| s.to_string()).unwrap_or_default(),
                                y_col: numeric.get(self.y_col_idx).map(|s| s.to_string()).unwrap_or_default(),
                                color_col: None,
                            }),
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
                if ui.button(RichText::new("Close").color(c.text_secondary)).clicked() { close = true; }
            }
        });

        if close { self.is_open = false; }
        result
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn type_btn(ui: &mut Ui, label: &str, selected: bool, theme: &AppTheme) -> egui::Response {
    let c = &theme.colors;
    let s = &theme.spacing;
    let btn = egui::Button::new(
        RichText::new(label)
            .color(if selected { c.bg_app } else { c.text_primary })
            .size(s.font_body)
            .strong(),
    )
    .fill(if selected { c.accent_primary } else { c.widget_bg })
    .stroke(egui::Stroke::new(1.0, if selected { c.accent_primary } else { c.border }))
    .min_size(egui::vec2(120.0, 28.0));
    ui.add(btn)
}

fn col_picker(
    ui: &mut Ui,
    label: &str,
    cols: &[&str],
    idx: &mut usize,
    id: &str,
    theme: &AppTheme,
) {
    let c = &theme.colors;
    let s = &theme.spacing;
    ui.label(RichText::new(label).color(c.text_secondary).size(s.font_small));
    ui.add_space(2.0);
    let selected_text = cols.get(*idx).copied().unwrap_or("(none)");
    egui::ComboBox::from_id_salt(id)
        .selected_text(
            RichText::new(selected_text).color(c.text_data).size(s.font_body).monospace(),
        )
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for (i, name) in cols.iter().enumerate() {
                ui.selectable_value(
                    idx, i,
                    RichText::new(*name).color(c.text_data).size(s.font_body).monospace(),
                );
            }
        });
}

fn is_numeric_or_geo(kind: &FieldKind) -> bool {
    matches!(
        kind,
        FieldKind::Latitude | FieldKind::Longitude | FieldKind::Altitude
        | FieldKind::Speed | FieldKind::Heading | FieldKind::Float | FieldKind::Integer
    )
}
fn is_lat_name(n: &str) -> bool { n.to_lowercase().contains("lat") }
fn is_lon_name(n: &str) -> bool {
    let n = n.to_lowercase();
    n.contains("lon") || n.contains("lng")
}
