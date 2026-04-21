use crate::data::source::SourceId;
use crate::plot::colormap::sample_gradient;
use crate::plot::styling::{ColorLegend, PlotLegendData};
use crate::theme::AppTheme;
use egui::{Color32, RichText, Ui};
use std::collections::HashSet;

// ── LegendAction ─────────────────────────────────────────────────────────────

/// Actions produced by legend interactions (click, right-click context menu).
pub enum LegendAction {
    /// User clicked a categorical entry — select all points matching this value.
    /// (source_id, column, value, additive)
    /// `additive` = true means ctrl+click (add to existing selection).
    SelectCategory {
        source_id: SourceId,
        plot_id: usize,
        col: String,
        value: String,
        additive: bool,
    },
    /// User chose "Filter to [value]" from context menu.
    FilterToValue {
        source_id: SourceId,
        col: String,
        value: String,
    },
    /// User chose "Select all sharing -> [column]" from context menu.
    /// We need the source_id, the anchor column+value, and the target column.
    SelectAllSharing {
        source_id: SourceId,
        plot_id: usize,
        anchor_col: String,
        anchor_value: String,
        target_col: String,
    },
    /// Row-index variant of FilterToValue — used when the value isn't known at click time.
    /// App looks up the value of `col` at `row_index` in the source DataFrame.
    FilterToRowValue {
        source_id: SourceId,
        col: String,
        row_index: usize,
    },
    /// Row-index variant of SelectAllSharing — used from point context menus
    /// regardless of color mode. App looks up the anchor value at `row_index`.
    SelectAllSharingRow {
        source_id: SourceId,
        plot_id: usize,
        row_index: usize,
        target_col: String,
    },
}

// ── RightPane ─────────────────────────────────────────────────────────────────

pub struct RightPane {
    /// Plot IDs whose legends are currently collapsed.
    collapsed: HashSet<usize>,
    /// State for right-click context menu.
    ctx_menu_state: Option<CtxMenuState>,
}

struct CtxMenuState {
    source_id: SourceId,
    plot_id: usize,
    col: String,
    value: String,
    /// Available columns for "select all sharing" submenu.
    all_columns: Vec<String>,
    /// Cursor position at right-click time.
    menu_pos: egui::Pos2,
}

impl Default for RightPane {
    fn default() -> Self {
        Self {
            collapsed: HashSet::new(),
            ctx_menu_state: None,
        }
    }
}

impl RightPane {
    /// Render the right legend pane. Returns actions for the caller (app.rs).
    pub fn show(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        legends: &[PlotLegendData],
        all_columns_by_source: &std::collections::HashMap<SourceId, Vec<String>>,
        aliases_by_source: &std::collections::HashMap<SourceId, std::collections::HashMap<String, String>>,
    ) -> Vec<LegendAction> {
        let c = &theme.colors;
        let s = &theme.spacing;
        let mut actions: Vec<LegendAction> = Vec::new();

        // ── Header ────────────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.label(RichText::new("LEGENDS").color(c.text_secondary).size(s.font_small).strong());
        });
        ui.add(egui::Separator::default().horizontal());
        ui.add_space(4.0);

        if legends.is_empty() {
            ui.label(
                RichText::new("No active plots.")
                    .color(c.text_secondary)
                    .size(s.font_small)
                    .italics(),
            );
            return actions;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for legend in legends {
                    let card_actions = self.show_legend_card(ui, theme, legend, all_columns_by_source, aliases_by_source);
                    actions.extend(card_actions);
                    ui.add_space(6.0);
                }
            });

        // ── Context menu (rendered as floating Area) ────────────────────────
        if let Some(state) = &self.ctx_menu_state {
            let mut close_menu = false;
            let menu_pos = state.menu_pos;

            let area_resp = egui::Area::new(egui::Id::new("legend_ctx_menu"))
                .order(egui::Order::Foreground)
                .fixed_pos(menu_pos)
                .show(ui.ctx(), |ui| {
                    egui::Frame::default()
                        .fill(c.bg_panel)
                        .stroke(egui::Stroke::new(1.0, c.border))
                        .corner_radius(egui::CornerRadius::from(4.0_f32))
                        .inner_margin(egui::Margin::from(8.0_f32))
                        .show(ui, |ui| {
                            ui.set_min_width(200.0);

                            // Filter to this value
                            if ui.button(
                                RichText::new(format!("Filter to \"{}\"", state.value))
                                    .size(s.font_small),
                            ).clicked() {
                                actions.push(LegendAction::FilterToValue {
                                    source_id: state.source_id,
                                    col: state.col.clone(),
                                    value: state.value.clone(),
                                });
                                close_menu = true;
                            }

                            // Select all with this value
                            if ui.button(
                                RichText::new(format!("Select all \"{}\"", state.value))
                                    .size(s.font_small),
                            ).clicked() {
                                actions.push(LegendAction::SelectCategory {
                                    source_id: state.source_id,
                                    plot_id: state.plot_id,
                                    col: state.col.clone(),
                                    value: state.value.clone(),
                                    additive: false,
                                });
                                close_menu = true;
                            }

                            ui.separator();

                            // Select all sharing -> [column]
                            ui.label(
                                RichText::new("Select all sharing →")
                                    .color(c.text_secondary)
                                    .size(s.font_small),
                            );
                            let empty_aliases = std::collections::HashMap::new();
                            let source_aliases = aliases_by_source.get(&state.source_id).unwrap_or(&empty_aliases);
                            egui::ScrollArea::vertical()
                                .max_height(200.0)
                                .id_salt("legend_sharing_cols")
                                .show(ui, |ui| {
                                    for target_col in &state.all_columns {
                                        let col_display = source_aliases.get(target_col).map(|a| a.as_str()).unwrap_or(target_col);
                                        if ui.button(
                                            RichText::new(col_display)
                                                .size(s.font_small)
                                                .monospace(),
                                        ).clicked() {
                                            actions.push(LegendAction::SelectAllSharing {
                                                source_id: state.source_id,
                                                plot_id: state.plot_id,
                                                anchor_col: state.col.clone(),
                                                anchor_value: state.value.clone(),
                                                target_col: target_col.clone(),
                                            });
                                            close_menu = true;
                                        }
                                    }
                                });
                        });
                });

            // Close on click outside or after action
            let clicked_outside = ui.input(|i| i.pointer.any_pressed())
                && !area_resp.response.rect.contains(
                    ui.ctx().input(|i| i.pointer.hover_pos().unwrap_or(egui::pos2(-1.0, -1.0)))
                );
            if close_menu || clicked_outside {
                self.ctx_menu_state = None;
            }
        }

        actions
    }

    fn show_legend_card(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        legend: &PlotLegendData,
        all_columns_by_source: &std::collections::HashMap<SourceId, Vec<String>>,
        aliases_by_source: &std::collections::HashMap<SourceId, std::collections::HashMap<String, String>>,
    ) -> Vec<LegendAction> {
        let c = &theme.colors;
        let s = &theme.spacing;
        let is_collapsed = self.collapsed.contains(&legend.plot_id);
        let mut actions: Vec<LegendAction> = Vec::new();

        egui::Frame::default()
            .fill(c.bg_app)
            .stroke(egui::Stroke::new(1.0, c.border))
            .corner_radius(s.rounding)
            .inner_margin(egui::Margin::from(8.0_f32))
            .show(ui, |ui| {
                // ── Card header ──────────────────────────────────────────────
                ui.horizontal(|ui| {
                    // Plot type icon from the color legend variant
                    let icon = match &legend.color {
                        ColorLegend::Solid { .. }       => "◉",
                        ColorLegend::Categorical { .. } => "◈",
                        ColorLegend::Continuous { .. }  => "◉",
                    };
                    ui.label(RichText::new(icon).color(c.accent_secondary).size(s.font_small));
                    ui.label(
                        RichText::new(&legend.plot_title)
                            .color(c.text_primary)
                            .size(s.font_body)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let toggle_label = if is_collapsed { "▸" } else { "▾" };
                        if ui.add(egui::Button::new(
                            RichText::new(toggle_label).color(c.text_secondary).size(s.font_small),
                        ).frame(false)).clicked() {
                            if is_collapsed {
                                self.collapsed.remove(&legend.plot_id);
                            } else {
                                self.collapsed.insert(legend.plot_id);
                            }
                        }
                    });
                });

                if is_collapsed { return; }

                ui.add_space(6.0);

                // ── Color legend ─────────────────────────────────────────────
                match &legend.color {
                    ColorLegend::Solid { color } => {
                        ui.horizontal(|ui| {
                            color_swatch(ui, *color, 12.0);
                            ui.label(
                                RichText::new("Solid color")
                                    .color(c.text_secondary)
                                    .size(s.font_small),
                            );
                        });
                    }

                    ColorLegend::Categorical { col, entries } => {
                        let empty_aliases = std::collections::HashMap::new();
                        let source_aliases = aliases_by_source.get(&legend.source_id).unwrap_or(&empty_aliases);
                        let col_display = source_aliases.get(col).map(|a| a.as_str()).unwrap_or(col);
                        ui.label(
                            RichText::new(format!("Color: {col_display}"))
                                .color(c.text_secondary)
                                .size(s.font_small),
                        );
                        ui.add_space(4.0);

                        let show_count = entries.len().min(50);
                        for (label, color) in &entries[..show_count] {
                            let response = ui.horizontal(|ui| {
                                color_swatch(ui, *color, 10.0);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(label)
                                            .color(c.text_data)
                                            .size(s.font_small)
                                            .monospace(),
                                    )
                                    .sense(egui::Sense::click()),
                                )
                            }).inner;

                            // Left-click: select
                            if response.clicked() {
                                let additive = ui.input(|i| i.modifiers.ctrl);
                                actions.push(LegendAction::SelectCategory {
                                    source_id: legend.source_id,
                                    plot_id: legend.plot_id,
                                    col: col.clone(),
                                    value: label.clone(),
                                    additive,
                                });
                            }

                            // Right-click: open context menu
                            if response.secondary_clicked() {
                                let cols = all_columns_by_source
                                    .get(&legend.source_id)
                                    .cloned()
                                    .unwrap_or_default();
                                let pos = ui.input(|i| i.pointer.hover_pos().unwrap_or(egui::pos2(200.0, 200.0)));
                                self.ctx_menu_state = Some(CtxMenuState {
                                    source_id: legend.source_id,
                                    plot_id: legend.plot_id,
                                    col: col.clone(),
                                    value: label.clone(),
                                    all_columns: cols,
                                    menu_pos: pos,
                                });
                            }
                        }
                        if entries.len() > show_count {
                            ui.label(
                                RichText::new(format!("  … {} more", entries.len() - show_count))
                                    .color(c.text_secondary)
                                    .size(s.font_small),
                            );
                        }
                    }

                    ColorLegend::Continuous { col, colormap, data_min, data_max, reverse } => {
                        let empty_aliases = std::collections::HashMap::new();
                        let source_aliases = aliases_by_source.get(&legend.source_id).unwrap_or(&empty_aliases);
                        let col_display = source_aliases.get(col).map(|a| a.as_str()).unwrap_or(col);
                        let rev_label = if *reverse { " ↔" } else { "" };
                        ui.label(
                            RichText::new(format!("Color: {col_display}  ({}{})", colormap.label(), rev_label))
                                .color(c.text_secondary)
                                .size(s.font_small),
                        );
                        ui.add_space(4.0);

                        // Draw gradient bar
                        let bar_w = ui.available_width().max(60.0);
                        let bar_h = 14.0;
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(bar_w, bar_h),
                            egui::Sense::hover(),
                        );
                        if ui.is_rect_visible(rect) {
                            draw_gradient_bar(ui, rect, colormap, *reverse);
                        }
                        ui.add_space(2.0);

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format_val(*data_min))
                                    .color(c.text_secondary)
                                    .size(s.font_small),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format_val(*data_max))
                                        .color(c.text_secondary)
                                        .size(s.font_small),
                                );
                            });
                        });
                    }
                }

                // ── Size legend ──────────────────────────────────────────────
                if let Some(sz) = &legend.size {
                    let empty_aliases = std::collections::HashMap::new();
                    let source_aliases = aliases_by_source.get(&legend.source_id).unwrap_or(&empty_aliases);
                    let sz_display = source_aliases.get(&sz.col).map(|a| a.as_str()).unwrap_or(&sz.col);
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Size: {}", sz_display))
                            .color(c.text_secondary)
                            .size(s.font_small),
                    );
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("● {:.0}px", sz.min_px)).color(c.text_secondary).size(s.font_small));
                        ui.add_space(4.0);
                        ui.label(RichText::new("→").color(c.text_secondary).size(s.font_small));
                        ui.add_space(4.0);
                        ui.label(RichText::new(format!("● {:.0}px", sz.max_px)).color(c.text_secondary).size(s.font_small));
                    });
                    ui.label(
                        RichText::new(format!("  {} – {}", format_val(sz.data_min), format_val(sz.data_max)))
                            .color(c.text_secondary)
                            .size(s.font_small),
                    );
                }

                // ── Alpha legend ─────────────────────────────────────────────
                if let Some(al) = &legend.alpha {
                    let empty_aliases = std::collections::HashMap::new();
                    let source_aliases = aliases_by_source.get(&legend.source_id).unwrap_or(&empty_aliases);
                    let al_display = source_aliases.get(&al.col).map(|a| a.as_str()).unwrap_or(&al.col);
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Opacity: {}", al_display))
                            .color(c.text_secondary)
                            .size(s.font_small),
                    );
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{:.0}%", al.min_alpha * 100.0))
                                .color(c.text_secondary)
                                .size(s.font_small),
                        );
                        ui.label(RichText::new("→").color(c.text_secondary).size(s.font_small));
                        ui.label(
                            RichText::new(format!("{:.0}%", al.max_alpha * 100.0))
                                .color(c.text_secondary)
                                .size(s.font_small),
                        );
                    });
                    ui.label(
                        RichText::new(format!("  {} – {}", format_val(al.data_min), format_val(al.data_max)))
                            .color(c.text_secondary)
                            .size(s.font_small),
                    );
                }
            });

        actions
    }
}

// ── Drawing helpers ───────────────────────────────────────────────────────────

fn color_swatch(ui: &mut Ui, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, 2.0, color);
    }
}

fn draw_gradient_bar(ui: &Ui, rect: egui::Rect, colormap: &crate::plot::plot_config::Colormap, reverse: bool) {
    let steps = 64_usize;
    let samples = sample_gradient(colormap, steps);
    let step_w = rect.width() / steps as f32;
    let painter = ui.painter();
    painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, Color32::from_gray(60)), egui::StrokeKind::Outside);
    for (i, color) in samples.iter().enumerate() {
        let idx = if reverse { steps - 1 - i } else { i };
        let x = rect.min.x + idx as f32 * step_w;
        let strip = egui::Rect::from_min_size(
            egui::pos2(x, rect.min.y),
            egui::vec2(step_w.ceil() + 0.5, rect.height()),
        );
        painter.rect_filled(strip, 0.0, *color);
    }
}

fn format_val(v: f64) -> String {
    if v.abs() >= 1_000_000.0 {
        format!("{:.2}M", v / 1_000_000.0)
    } else if v.abs() >= 1_000.0 {
        format!("{:.1}K", v / 1_000.0)
    } else if v.fract().abs() < 0.0001 {
        format!("{:.0}", v)
    } else {
        format!("{:.3}", v)
    }
}
