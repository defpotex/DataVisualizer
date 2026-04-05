use crate::theme::AppTheme;
use egui::{menu, Align2, Color32, FontId, RichText, Ui};

#[derive(Default)]
pub struct MenuBar;

impl MenuBar {
    pub fn show(&mut self, ui: &mut Ui, theme: &AppTheme) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);

            // ── File ──────────────────────────────────────────────────────────
            menu::bar(ui, |ui| {
                ui.set_style(ui.style().clone());

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
                    menu_entry(ui, "CSV File…", "", theme);
                    menu_entry(ui, "Parquet File…", "", theme);
                    menu_entry(ui, "UDP Stream…", "", theme);
                    menu_entry(ui, "ADS-B Stream…", "", theme);
                    ui.separator();
                    menu_entry_disabled(ui, "Manage Sources…", "", theme);
                });

                menu_item(ui, "Aggregation", theme, |ui| {
                    menu_entry_disabled(ui, "Configure Binning…", "", theme);
                    menu_entry_disabled(ui, "Spatial Aggregation…", "", theme);
                    menu_entry_disabled(ui, "Temporal Aggregation…", "", theme);
                });

                menu_item(ui, "Performance", theme, |ui| {
                    menu_entry_disabled(ui, "Memory Usage…", "", theme);
                    menu_entry_disabled(ui, "Cancel Operation", "Esc", theme);
                });

                menu_item(ui, "Help", theme, |ui| {
                    menu_entry(ui, "Documentation", "", theme);
                    menu_entry(ui, "Boundary File Format", "", theme);
                    ui.separator();
                    menu_entry(ui, "About DataVisualizer", "", theme);
                });
            });
        });
    }
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

fn menu_entry(ui: &mut Ui, label: &str, shortcut: &str, theme: &AppTheme) {
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

    if response.clicked() {
        ui.close_menu();
    }
}

fn menu_entry_disabled(ui: &mut Ui, label: &str, shortcut: &str, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;
    let row_height = s.font_body + 10.0;
    let width = ui.available_width().max(200.0);

    let (rect, _response) = ui.allocate_exact_size(
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

/// Dim a color by blending toward black — used for disabled items.
fn dimmed(c: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 100)
}
