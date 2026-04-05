use crate::theme::AppTheme;
use egui::{Align, Layout, RichText, Ui};

#[derive(Default)]
pub struct PlotArea;

impl PlotArea {
    pub fn show(&mut self, ui: &mut Ui, theme: &AppTheme) {
        let c = &theme.colors;
        let s = &theme.spacing;

        // Empty state — centered content
        ui.with_layout(Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.28);

                // Large diamond icon
                ui.label(
                    RichText::new("◈")
                        .color(c.accent_primary)
                        .size(52.0),
                );

                ui.add_space(10.0);

                ui.label(
                    RichText::new("No data sources loaded.")
                        .color(c.text_primary)
                        .size(s.font_body + 2.0)
                        .strong(),
                );

                ui.add_space(8.0);

                ui.label(
                    RichText::new("Use  Data Sources → Add Source  or the panel on the left")
                        .color(c.text_secondary)
                        .size(s.font_body),
                );
                ui.label(
                    RichText::new("to load a CSV, Parquet file, or connect to a UDP stream.")
                        .color(c.text_secondary)
                        .size(s.font_body),
                );

                ui.add_space(24.0);

                // Quick-start hints
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 340.0).max(0.0) / 2.0);
                    quick_hint(ui, "CSV", "Load flat file data", theme);
                    ui.add_space(8.0);
                    quick_hint(ui, "Parquet", "Load columnar data", theme);
                    ui.add_space(8.0);
                    quick_hint(ui, "UDP", "Connect live stream", theme);
                });
            });
        });
    }
}

fn quick_hint(ui: &mut Ui, label: &str, desc: &str, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;

    egui::Frame::default()
        .fill(theme.colors.bg_panel)
        .stroke(egui::Stroke::new(1.0, c.border))
        .rounding(s.rounding)
        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.set_width(100.0);
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.label(
                    RichText::new(label)
                        .color(c.accent_primary)
                        .size(s.font_body)
                        .strong(),
                );
                ui.label(
                    RichText::new(desc)
                        .color(c.text_secondary)
                        .size(s.font_small),
                );
            });
        });
}
