use crate::app::MenuAction;
use crate::state::perf_settings::{GpuPointsMode, PerformanceSettings};
use crate::theme::AppTheme;
use egui::{menu, Align2, Color32, FontId, RichText, Ui};

#[derive(Default)]
pub struct MenuBar;

impl MenuBar {
    /// Returns Some(MenuAction) if the user clicked something requiring app-level handling.
    pub fn show(&mut self, ui: &mut Ui, theme: &AppTheme, perf: &mut PerformanceSettings, right_pane_visible: bool) -> Option<MenuAction> {
        let mut action = None;

        ui.horizontal(|ui| {
            ui.add_space(4.0);

            menu::bar(ui, |ui| {
                menu_item(ui, "File", theme, |ui| {
                    menu_entry(ui, "New Session", "Ctrl+N", theme);
                    menu_entry(ui, "Open Session…", "Ctrl+O", theme);
                    ui.separator();
                    menu_entry_disabled(ui, "Save Session", "Ctrl+S", theme);
                    menu_entry_disabled(ui, "Save Session As…", "Ctrl+Shift+S", theme);
                    ui.separator();
                    menu_section_label(ui, "Recent", theme);
                    menu_entry_disabled(ui, "(none)", "", theme);
                    ui.separator();
                    menu_entry(ui, "Exit", "Alt+F4", theme);
                });

                menu_item(ui, "Data Sources", theme, |ui| {
                    menu_section_label(ui, "Add Source", theme);
                    if menu_entry(ui, "CSV File…", "", theme) {
                        action = Some(MenuAction::OpenCsv);
                        ui.close();
                    }
                    menu_entry_disabled(ui, "Parquet File…", "", theme);
                    menu_entry_disabled(ui, "UDP Stream…", "", theme);
                    menu_entry_disabled(ui, "ADS-B Stream…", "", theme);
                    ui.separator();
                    menu_entry_disabled(ui, "Manage Sources…", "", theme);
                });

                menu_item(ui, "Aggregation", theme, |ui| {
                    menu_entry_disabled(ui, "Configure Binning…", "", theme);
                    menu_entry_disabled(ui, "Spatial Aggregation…", "", theme);
                    menu_entry_disabled(ui, "Temporal Aggregation…", "", theme);
                });

                menu_item(ui, "View", theme, |ui| {
                    let legend_label = if right_pane_visible { "✓ Show Legends" } else { "  Show Legends" };
                    if menu_entry(ui, legend_label, "", theme) {
                        action = Some(MenuAction::ToggleLegendPane);
                        ui.close();
                    }
                });

                menu_item(ui, "Performance", theme, |ui| {
                    perf_settings_ui(ui, perf, theme);
                });

                menu_item(ui, "Help", theme, |ui| {
                    menu_entry(ui, "Documentation", "", theme);
                    menu_entry(ui, "Boundary File Format", "", theme);
                    ui.separator();
                    menu_entry(ui, "About DataVisualizer", "", theme);
                });
            });
        });

        action
    }
}

// ── Performance settings popup ────────────────────────────────────────────────

fn perf_settings_ui(ui: &mut Ui, perf: &mut PerformanceSettings, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;

    ui.set_min_width(260.0);
    ui.add_space(4.0);

    // Section header
    ui.label(
        RichText::new("RENDER SETTINGS")
            .color(c.text_secondary)
            .size(s.font_small),
    );
    ui.add_space(6.0);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Max Points / Plot")
                .color(c.text_primary)
                .size(s.font_body),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::DragValue::new(&mut perf.max_draw_points)
                    .range(1_000..=2_000_000)
                    .speed(1_000.0)
                    .suffix(" pts"),
            );
        });
    });

    ui.add_space(2.0);
    ui.label(
        RichText::new("Drag or click to edit. Points are stride-\nsampled when dataset exceeds this limit.")
            .color(c.text_secondary)
            .size(s.font_small),
    );

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // ── Batched Mesh Rendering ────────────────────────────────────────
    ui.label(
        RichText::new("BATCHED RENDERING")
            .color(c.text_secondary)
            .size(s.font_small),
    );
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Mode")
                .color(c.text_primary)
                .size(s.font_body),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let modes = [GpuPointsMode::Off, GpuPointsMode::Auto, GpuPointsMode::On];
            let labels = ["Off", "Auto", "On"];
            for (&mode, label) in modes.iter().zip(labels.iter()).rev() {
                let selected = perf.gpu_points_mode == mode;
                let btn = egui::Button::new(
                    RichText::new(*label).size(s.font_small).color(if selected { c.text_primary } else { c.text_secondary }),
                ).fill(if selected { c.accent_primary.linear_multiply(0.3) } else { Color32::TRANSPARENT })
                 .rounding(s.rounding - 1.0)
                 .min_size(egui::vec2(36.0, 0.0));
                if ui.add(btn).clicked() {
                    perf.gpu_points_mode = mode;
                }
            }
        });
    });

    if perf.gpu_points_mode == GpuPointsMode::Auto {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Auto Threshold")
                    .color(c.text_primary)
                    .size(s.font_body),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::DragValue::new(&mut perf.gpu_points_threshold)
                        .range(500..=100_000)
                        .speed(500.0)
                        .suffix(" pts"),
                );
            });
        });
    }

    ui.add_space(2.0);
    ui.label(
        RichText::new("Pre-tessellates circles into a single mesh\nfor faster GPU rendering of large datasets.")
            .color(c.text_secondary)
            .size(s.font_small),
    );

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // ── Profiler toggle ────────────────────────────────────────────────
    ui.separator();
    ui.add_space(4.0);
    ui.label(
        RichText::new("PROFILER")
            .color(c.text_secondary)
            .size(s.font_small),
    );
    ui.add_space(4.0);

    let profiler_label = if perf.show_profiler { "✓ Profiler Server" } else { "  Profiler Server" };
    let (prof_rect, prof_resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(200.0), s.font_body + 10.0),
        egui::Sense::click(),
    );
    if ui.is_rect_visible(prof_rect) {
        if prof_resp.hovered() {
            ui.painter().rect_filled(prof_rect, s.rounding - 1.0, c.widget_bg_hovered);
        }
        ui.painter().text(
            prof_rect.left_center() + egui::vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            profiler_label,
            FontId::proportional(s.font_body),
            c.text_primary,
        );
    }
    if prof_resp.clicked() {
        perf.show_profiler = !perf.show_profiler;
    }

    if perf.show_profiler {
        ui.add_space(4.0);
        ui.label(
            RichText::new("Serving on 127.0.0.1:8585\nConnect with puffin_viewer")
                .color(c.accent_secondary)
                .size(s.font_small),
        );
    }

    ui.add_space(4.0);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn menu_item(ui: &mut Ui, label: &str, theme: &AppTheme, contents: impl FnOnce(&mut Ui)) {
    let c = &theme.colors;
    ui.menu_button(
        RichText::new(label)
            .color(c.text_primary)
            .size(theme.spacing.font_body),
        contents,
    );
}

/// Returns true if this entry was clicked.
fn menu_entry(ui: &mut Ui, label: &str, shortcut: &str, theme: &AppTheme) -> bool {
    let c = &theme.colors;
    let s = &theme.spacing;
    let row_height = s.font_body + 10.0;
    let width = ui.available_width().max(200.0);

    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, row_height),
        egui::Sense::click(),
    );

    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(rect, s.rounding - 1.0, c.widget_bg_hovered);
        }
        ui.painter().text(
            rect.left_center() + egui::vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(s.font_body),
            c.text_primary,
        );
        if !shortcut.is_empty() {
            ui.painter().text(
                rect.right_center() - egui::vec2(8.0, 0.0),
                Align2::RIGHT_CENTER,
                shortcut,
                FontId::proportional(s.font_small),
                c.text_secondary,
            );
        }
    }

    response.clicked()
}

fn menu_entry_disabled(ui: &mut Ui, label: &str, shortcut: &str, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;
    let row_height = s.font_body + 10.0;
    let width = ui.available_width().max(200.0);

    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(width, row_height),
        egui::Sense::hover(),
    );

    if ui.is_rect_visible(rect) {
        ui.painter().text(
            rect.left_center() + egui::vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(s.font_body),
            dimmed(c.text_secondary),
        );
        if !shortcut.is_empty() {
            ui.painter().text(
                rect.right_center() - egui::vec2(8.0, 0.0),
                Align2::RIGHT_CENTER,
                shortcut,
                FontId::proportional(s.font_small),
                dimmed(c.text_secondary),
            );
        }
    }
}

fn menu_section_label(ui: &mut Ui, label: &str, theme: &AppTheme) {
    ui.label(
        RichText::new(label.to_uppercase())
            .color(theme.colors.text_secondary)
            .size(theme.spacing.font_small),
    );
}

fn dimmed(c: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 100)
}

fn format_k(n: usize) -> String {
    if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1_000_000.0) }
    else if n >= 1_000 { format!("{:.0}K", n as f64 / 1_000.0) }
    else { n.to_string() }
}
