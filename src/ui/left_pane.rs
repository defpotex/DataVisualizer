use crate::app::PaneAction;
use crate::data::schema::FieldKind;
use crate::state::app_state::AppState;
use crate::theme::AppTheme;
use egui::{RichText, Ui};

#[derive(Default)]
pub struct LeftPane {
    /// Which sources have their field list expanded (by source id)
    expanded: std::collections::HashSet<usize>,
}

impl LeftPane {
    /// Returns Some(PaneAction) if the user triggered something requiring app-level handling.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        state: &AppState,
    ) -> Option<PaneAction> {
        let mut action = None;
        let c = &theme.colors;
        let s = &theme.spacing;

        ui.vertical(|ui| {
            // ── DATA SOURCES ──────────────────────────────────────────────────
            section_header(ui, "DATA SOURCES", theme);
            ui.add_space(4.0);

            // Add source button — always visible
            let btn = egui::Button::new(
                RichText::new("＋  Add Source  ▾")
                    .color(c.accent_primary)
                    .size(s.font_body),
            )
            .min_size(egui::vec2(ui.available_width(), 0.0));

            if ui.add(btn).clicked() {
                action = Some(PaneAction::OpenCsv);
            }

            ui.add_space(6.0);

            if state.sources.is_empty() {
                ui.label(
                    RichText::new("No sources loaded.")
                        .color(c.text_secondary)
                        .size(s.font_small)
                        .italics(),
                );
            } else {
                // One card per source
                for source in &state.sources {
                    ui.add_space(4.0);
                    source_card(ui, theme, source, &mut self.expanded, &mut action);
                }
            }

            // Notifications (load errors)
            for note in &state.notifications {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("⚠ {}", note))
                        .color(c.accent_warning)
                        .size(s.font_small),
                );
            }

            ui.add_space(14.0);
            ui.add(egui::Separator::default().horizontal());
            ui.add_space(8.0);

            // ── ADD PLOT ──────────────────────────────────────────────────────
            section_header(ui, "ADD PLOT", theme);
            ui.add_space(4.0);

            ui.add_enabled(
                state.has_sources(),
                egui::Button::new(
                    RichText::new("＋  Add Plot")
                        .color(if state.has_sources() { c.accent_primary } else { c.text_secondary })
                        .size(s.font_body),
                )
                .min_size(egui::vec2(ui.available_width(), 0.0)),
            );

            if !state.has_sources() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Load a data source first.")
                        .color(c.text_secondary)
                        .size(s.font_small)
                        .italics(),
                );
            }

            ui.add_space(14.0);
            ui.add(egui::Separator::default().horizontal());
            ui.add_space(8.0);

            // ── FILTERS ───────────────────────────────────────────────────────
            section_header(ui, "FILTERS", theme);
            ui.add_space(4.0);

            ui.add_enabled(
                state.has_sources(),
                egui::Button::new(
                    RichText::new("＋  Add Filter")
                        .color(if state.has_sources() { c.accent_primary } else { c.text_secondary })
                        .size(s.font_body),
                )
                .min_size(egui::vec2(ui.available_width(), 0.0)),
            );

            if !state.has_sources() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("No active filters.")
                        .color(c.text_secondary)
                        .size(s.font_small)
                        .italics(),
                );
            }

            // ── Version stamp ─────────────────────────────────────────────────
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("v0.1.0-dev")
                        .color(c.text_secondary)
                        .size(s.font_small),
                );
            });
        });

        action
    }
}

// ── Source card ───────────────────────────────────────────────────────────────

fn source_card(
    ui: &mut Ui,
    theme: &AppTheme,
    source: &crate::data::source::DataSource,
    expanded: &mut std::collections::HashSet<usize>,
    action: &mut Option<PaneAction>,
) {
    let c = &theme.colors;
    let s = &theme.spacing;

    egui::Frame::default()
        .fill(c.bg_app)
        .stroke(egui::Stroke::new(1.0, c.border))
        .rounding(s.rounding)
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            // Header row: dot + name
            ui.horizontal(|ui| {
                ui.label(RichText::new("●").color(c.accent_secondary).size(s.font_small));
                ui.label(
                    RichText::new(&source.label)
                        .color(c.text_primary)
                        .size(s.font_body)
                        .strong(),
                );
            });

            // Stats row
            ui.label(
                RichText::new(format!(
                    "{} rows · {} fields",
                    format_count(source.row_count()),
                    source.field_count()
                ))
                .color(c.text_secondary)
                .size(s.font_small),
            );

            ui.add_space(4.0);

            // Collapsible field list
            let is_expanded = expanded.contains(&source.id);
            let toggle_label = if is_expanded { "▾ Fields" } else { "▸ Fields" };

            if ui
                .add(egui::Button::new(
                    RichText::new(toggle_label)
                        .color(c.text_secondary)
                        .size(s.font_small),
                ).frame(false))
                .clicked()
            {
                if is_expanded {
                    expanded.remove(&source.id);
                } else {
                    expanded.insert(source.id);
                }
            }

            if is_expanded {
                ui.add_space(2.0);
                for field in &source.schema.fields {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        // Icon colored by kind
                        let icon_color = field_icon_color(&field.kind, theme);
                        ui.label(
                            RichText::new(field.kind.icon())
                                .color(icon_color)
                                .size(s.font_small),
                        );
                        ui.label(
                            RichText::new(&field.name)
                                .color(c.text_data)
                                .size(s.font_small)
                                .monospace(),
                        );
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    RichText::new(field.kind.label())
                                        .color(c.text_secondary)
                                        .size(s.font_small),
                                );
                            },
                        );
                    });
                }
                ui.add_space(2.0);
            }

            // Remove button
            ui.add_space(4.0);
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Remove")
                            .color(c.accent_warning)
                            .size(s.font_small),
                    )
                    .min_size(egui::vec2(ui.available_width(), 0.0)),
                )
                .clicked()
            {
                *action = Some(PaneAction::RemoveSource(source.id));
            }
        });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn section_header(ui: &mut Ui, label: &str, theme: &AppTheme) {
    ui.label(
        RichText::new(label)
            .color(theme.colors.accent_primary)
            .size(theme.spacing.font_small)
            .strong(),
    );
}

fn field_icon_color(kind: &FieldKind, theme: &AppTheme) -> egui::Color32 {
    let c = &theme.colors;
    match kind {
        FieldKind::Latitude | FieldKind::Longitude => c.accent_primary,
        FieldKind::Timestamp                        => c.accent_secondary,
        FieldKind::Altitude | FieldKind::Speed
        | FieldKind::Heading                        => c.text_data,
        _                                           => c.text_secondary,
    }
}

/// Format a large number with comma separators.
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
