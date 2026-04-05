use crate::theme::AppTheme;
use egui::{RichText, Ui};

#[derive(Default)]
pub struct LeftPane;

impl LeftPane {
    pub fn show(&mut self, ui: &mut Ui, theme: &AppTheme) {
        let c = &theme.colors;
        let s = &theme.spacing;

        ui.vertical(|ui| {
            // ── DATA SOURCES ──────────────────────────────────────────────────
            section_header(ui, "DATA SOURCES", theme);

            ui.add_space(4.0);
            ui.add_enabled(
                true,
                egui::Button::new(
                    RichText::new("＋  Add Source  ▾")
                        .color(c.accent_primary)
                        .size(s.font_body),
                )
                .min_size(egui::vec2(ui.available_width(), 0.0)),
            );

            ui.add_space(6.0);

            // Empty state hint
            ui.label(
                RichText::new("No sources loaded.")
                    .color(c.text_secondary)
                    .size(s.font_small)
                    .italics(),
            );

            ui.add_space(14.0);
            ui.add(egui::Separator::default().horizontal());
            ui.add_space(8.0);

            // ── ADD PLOT ──────────────────────────────────────────────────────
            section_header(ui, "ADD PLOT", theme);

            ui.add_space(4.0);
            ui.add_enabled(
                false,
                egui::Button::new(
                    RichText::new("＋  Add Plot")
                        .color(c.text_secondary)
                        .size(s.font_body),
                )
                .min_size(egui::vec2(ui.available_width(), 0.0)),
            );

            ui.add_space(6.0);
            ui.label(
                RichText::new("Load a data source first.")
                    .color(c.text_secondary)
                    .size(s.font_small)
                    .italics(),
            );

            ui.add_space(14.0);
            ui.add(egui::Separator::default().horizontal());
            ui.add_space(8.0);

            // ── FILTERS ───────────────────────────────────────────────────────
            section_header(ui, "FILTERS", theme);

            ui.add_space(4.0);
            ui.add_enabled(
                false,
                egui::Button::new(
                    RichText::new("＋  Add Filter")
                        .color(c.text_secondary)
                        .size(s.font_body),
                )
                .min_size(egui::vec2(ui.available_width(), 0.0)),
            );

            ui.add_space(6.0);
            ui.label(
                RichText::new("No active filters.")
                    .color(c.text_secondary)
                    .size(s.font_small)
                    .italics(),
            );

            // ── Bottom: version stamp ─────────────────────────────────────────
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("v0.1.0-dev")
                        .color(c.text_secondary)
                        .size(s.font_small),
                );
            });
        });
    }
}

fn section_header(ui: &mut Ui, label: &str, theme: &AppTheme) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .color(theme.colors.accent_primary)
                .size(theme.spacing.font_small)
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(
                egui::Separator::default()
                    .horizontal()
                    .shrink(0.0),
            );
        });
    });
}
