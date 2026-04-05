use crate::state::app_state::AppState;
use crate::theme::AppTheme;
use egui::{Align, Layout, RichText, Ui};

#[derive(Default)]
pub struct PlotArea;

impl PlotArea {
    pub fn show(&mut self, ui: &mut Ui, theme: &AppTheme, state: &AppState) {
        if state.has_sources() {
            self.show_has_sources(ui, theme, state);
        } else {
            self.show_empty(ui, theme);
        }
    }

    // ── No sources loaded ─────────────────────────────────────────────────────

    fn show_empty(&mut self, ui: &mut Ui, theme: &AppTheme) {
        let c = &theme.colors;
        let s = &theme.spacing;
        let panel_width = ui.available_width();

        ui.add_space(ui.available_height() * 0.28);

        ui.vertical_centered(|ui| {
            ui.label(RichText::new("◈").color(c.accent_primary).size(52.0));
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

            // Three quick-hint cards, centered as a group
            let group_width = 388.0_f32;
            let offset = (panel_width - group_width).max(0.0) / 2.0;
            ui.horizontal(|ui| {
                ui.add_space(offset);
                quick_hint(ui, "CSV", "Load flat file data", theme);
                ui.add_space(8.0);
                quick_hint(ui, "Parquet", "Load columnar data", theme);
                ui.add_space(8.0);
                quick_hint(ui, "UDP", "Connect live stream", theme);
            });
        });
    }

    // ── Sources loaded, no plots yet ──────────────────────────────────────────

    fn show_has_sources(&mut self, ui: &mut Ui, theme: &AppTheme, state: &AppState) {
        let c = &theme.colors;
        let s = &theme.spacing;

        ui.add_space(ui.available_height() * 0.28);

        ui.vertical_centered(|ui| {
            ui.label(RichText::new("◈").color(c.accent_primary).size(40.0));
            ui.add_space(10.0);

            let src_count = state.sources.len();
            let total_rows: usize = state.sources.iter().map(|s| s.row_count()).sum();
            ui.label(
                RichText::new(format!(
                    "{} source{} loaded  ·  {} rows",
                    src_count,
                    if src_count == 1 { "" } else { "s" },
                    format_count(total_rows),
                ))
                .color(c.accent_secondary)
                .size(s.font_body + 1.0)
                .strong(),
            );

            ui.add_space(8.0);
            ui.label(
                RichText::new("Use  Add Plot  in the left panel to create a visualization.")
                    .color(c.text_secondary)
                    .size(s.font_body),
            );
        });
    }
}

// ── Quick-hint card ───────────────────────────────────────────────────────────

fn quick_hint(ui: &mut Ui, label: &str, desc: &str, theme: &AppTheme) {
    let c = &theme.colors;
    let s = &theme.spacing;

    egui::Frame::default()
        .fill(c.bg_panel)
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

fn format_count(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}
