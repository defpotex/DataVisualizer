use crate::data::filter::{distinct_values, Filter, FilterOp};
use crate::state::app_state::AppState;
use crate::theme::AppTheme;
use egui::{RichText, Ui, Window};
use std::collections::HashSet;

/// How many distinct values to compute/show in the picker.
const DISTINCT_LIMIT: usize = 100;

pub struct AddFilterDialog {
    pub is_open: bool,
    selected_col_idx: usize,
    selected_op_idx: usize,
    // Single-value mode (comparison ops)
    value_buf: String,
    // Set mode (In / NotIn)
    checked_values: HashSet<String>,
    // Cached distinct values for the current column
    distinct: Vec<String>,
    /// Col index for which `distinct` was last computed (avoids redundant recomputation).
    distinct_for_col: Option<usize>,
}

impl Default for AddFilterDialog {
    fn default() -> Self {
        Self {
            is_open: false,
            selected_col_idx: 0,
            selected_op_idx: 2, // default to ">"
            value_buf: String::new(),
            checked_values: HashSet::new(),
            distinct: Vec::new(),
            distinct_for_col: None,
        }
    }
}

impl AddFilterDialog {
    pub fn open(&mut self) {
        self.is_open = true;
        self.selected_col_idx = 0;
        self.selected_op_idx = 2;
        self.value_buf.clear();
        self.checked_values.clear();
        self.distinct.clear();
        self.distinct_for_col = None;
    }

    /// Returns `Some(Filter)` when the user clicks Add (id=0 placeholder; caller assigns real id).
    pub fn show(&mut self, ui: &mut Ui, theme: &AppTheme, state: &AppState) -> Option<Filter> {
        if !self.is_open { return None; }

        let c = &theme.colors;
        let s = &theme.spacing;
        let mut result = None;
        let mut close = false;

        // Collect all columns from all sources (sorted, deduplicated).
        let all_cols: Vec<String> = state.sources.iter()
            .flat_map(|src| src.schema.fields.iter().map(|f| f.name.clone()))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        // Clamp indices.
        if self.selected_col_idx >= all_cols.len() { self.selected_col_idx = 0; }

        // Recompute distinct values when column changes.
        if !all_cols.is_empty()
            && self.distinct_for_col != Some(self.selected_col_idx)
        {
            let col_name = &all_cols[self.selected_col_idx];
            self.distinct = distinct_values(&state.sources, col_name, DISTINCT_LIMIT);
            self.distinct_for_col = Some(self.selected_col_idx);
            // Reset value state when switching columns.
            self.value_buf.clear();
            self.checked_values.clear();
        }

        let ops = FilterOp::all();
        let current_op = &ops[self.selected_op_idx.min(ops.len() - 1)];
        let is_set_mode = current_op.is_set_op();

        let screen = ui.ctx().screen_rect();
        let default_pos = egui::pos2(
            (screen.center().x - 170.0).max(screen.min.x),
            (screen.center().y - 150.0).max(screen.min.y),
        );

        Window::new(
            RichText::new("Add Filter")
                .color(c.text_primary)
                .size(s.font_body + 1.0)
                .strong(),
        )
        .collapsible(false)
        .resizable(false)
        .min_width(340.0)
        .default_pos(default_pos)
        .order(egui::Order::Foreground)
        .frame(egui::Frame {
            fill: c.bg_panel,
            stroke: egui::Stroke::new(1.0, c.border),
            corner_radius: egui::CornerRadius::from(6.0_f32),
            inner_margin: egui::Margin::from(16.0_f32),
            ..Default::default()
        })
        .show(ui.ctx(), |ui| {
            if all_cols.is_empty() {
                ui.label(RichText::new("No data sources loaded.").color(c.accent_warning).size(s.font_body));
                ui.add_space(8.0);
                if ui.button(RichText::new("Close").color(c.text_secondary)).clicked() { close = true; }
                return;
            }

            // ── Column picker ─────────────────────────────────────────────────
            ui.label(RichText::new("Column").color(c.text_secondary).size(s.font_small));
            ui.add_space(2.0);
            egui::ComboBox::from_id_salt("filter_col")
                .selected_text(
                    RichText::new(all_cols[self.selected_col_idx].as_str())
                        .color(c.text_data)
                        .size(s.font_body)
                        .monospace(),
                )
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (idx, col) in all_cols.iter().enumerate() {
                        ui.selectable_value(
                            &mut self.selected_col_idx,
                            idx,
                            RichText::new(col).color(c.text_data).size(s.font_body).monospace(),
                        );
                    }
                });

            ui.add_space(10.0);

            // ── Operator picker ───────────────────────────────────────────────
            ui.label(RichText::new("Operator").color(c.text_secondary).size(s.font_small));
            ui.add_space(2.0);
            let op_label = ops[self.selected_op_idx.min(ops.len() - 1)].label();
            egui::ComboBox::from_id_salt("filter_op")
                .selected_text(RichText::new(op_label).color(c.text_primary).size(s.font_body))
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (idx, op) in ops.iter().enumerate() {
                        // Insert a visual separator before "in" group.
                        if idx == 6 {
                            ui.separator();
                        }
                        ui.selectable_value(
                            &mut self.selected_op_idx,
                            idx,
                            RichText::new(op.label()).color(c.text_primary).size(s.font_body),
                        );
                    }
                });

            ui.add_space(10.0);

            // ── Value section ─────────────────────────────────────────────────
            if is_set_mode {
                // Set mode: checkbox list of distinct values.
                show_set_picker(
                    ui,
                    &self.distinct,
                    &mut self.checked_values,
                    theme,
                );
            } else {
                // Comparison mode: text entry + click-to-fill value chips.
                ui.label(RichText::new("Value").color(c.text_secondary).size(s.font_small));
                ui.add_space(2.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.value_buf)
                        .desired_width(ui.available_width())
                        .text_color(c.text_primary)
                        .hint_text("type a value…")
                        .font(egui::FontSelection::FontId(
                            egui::FontId::proportional(s.font_body),
                        )),
                );

                // Value chips (click to fill text box).
                if !self.distinct.is_empty() {
                    ui.add_space(6.0);
                    show_value_chips(ui, &self.distinct, &mut self.value_buf, theme);
                }
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            // ── Buttons ───────────────────────────────────────────────────────
            let can_add = if is_set_mode {
                !self.checked_values.is_empty()
            } else {
                !self.value_buf.trim().is_empty()
            };

            ui.horizontal(|ui| {
                let add_btn = egui::Button::new(
                    RichText::new("Add Filter")
                        .color(if can_add { c.bg_app } else { c.text_secondary })
                        .size(s.font_body)
                        .strong(),
                )
                .fill(if can_add { c.accent_primary } else { c.widget_bg })
                .min_size(egui::vec2(110.0, 0.0));

                if ui.add_enabled(can_add, add_btn).clicked() {
                    let op_idx = self.selected_op_idx.min(ops.len() - 1);
                    let value = if is_set_mode {
                        // Sort for deterministic display order.
                        let mut vals: Vec<&str> = self.checked_values.iter().map(|s| s.as_str()).collect();
                        vals.sort_unstable();
                        vals.join("|")
                    } else {
                        self.value_buf.trim().to_string()
                    };
                    result = Some(Filter::new(
                        0, // placeholder id
                        all_cols[self.selected_col_idx].clone(),
                        ops[op_idx].clone(),
                        value,
                    ));
                    close = true;
                }
                ui.add_space(8.0);
                if ui.button(RichText::new("Cancel").color(c.text_secondary).size(s.font_body)).clicked() {
                    close = true;
                }
            });
        });

        if close { self.is_open = false; }
        result
    }
}

// ── Checkbox list for set mode ────────────────────────────────────────────────

fn show_set_picker(
    ui: &mut Ui,
    distinct: &[String],
    checked: &mut HashSet<String>,
    theme: &AppTheme,
) {
    let c = &theme.colors;
    let s = &theme.spacing;

    ui.horizontal(|ui| {
        ui.label(RichText::new("Select values").color(c.text_secondary).size(s.font_small));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let all_checked = !distinct.is_empty() && distinct.iter().all(|v| checked.contains(v));
            let mut all = all_checked;
            if ui.checkbox(&mut all, RichText::new("all").color(c.text_secondary).size(s.font_small)).changed() {
                if all {
                    for v in distinct { checked.insert(v.clone()); }
                } else {
                    checked.clear();
                }
            }
        });
    });
    ui.add_space(4.0);

    if distinct.is_empty() {
        ui.label(
            RichText::new("(no values found)")
                .color(c.text_secondary)
                .size(s.font_small)
                .italics(),
        );
        return;
    }

    let row_h = s.font_body + 8.0;
    let list_h = (row_h * (distinct.len().min(8) as f32)).max(60.0);

    egui::Frame::default()
        .fill(c.bg_app)
        .stroke(egui::Stroke::new(1.0, c.border))
        .corner_radius(egui::CornerRadius::from(4.0_f32))
        .inner_margin(egui::Margin::from(egui::vec2(6.0, 4.0)))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(list_h)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    for val in distinct {
                        let mut is_checked = checked.contains(val);
                        let response = ui.checkbox(
                            &mut is_checked,
                            RichText::new(val).color(c.text_data).size(s.font_body).monospace(),
                        );
                        if response.changed() {
                            if is_checked {
                                checked.insert(val.clone());
                            } else {
                                checked.remove(val);
                            }
                        }
                    }
                });
        });

    if !checked.is_empty() {
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!("{} selected", checked.len()))
                .color(c.accent_secondary)
                .size(s.font_small),
        );
    }
}

// ── Click-to-fill chips for comparison mode ───────────────────────────────────

fn show_value_chips(
    ui: &mut Ui,
    distinct: &[String],
    value_buf: &mut String,
    theme: &AppTheme,
) {
    let c = &theme.colors;
    let s = &theme.spacing;

    ui.label(RichText::new("Quick-fill").color(c.text_secondary).size(s.font_small));
    ui.add_space(2.0);

    let row_h = s.font_body + 8.0;
    let list_h = (row_h * (distinct.len().min(6) as f32)).max(48.0);

    egui::Frame::default()
        .fill(c.bg_app)
        .stroke(egui::Stroke::new(1.0, c.border))
        .corner_radius(egui::CornerRadius::from(4.0_f32))
        .inner_margin(egui::Margin::from(egui::vec2(6.0, 4.0)))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(list_h)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    for val in distinct {
                        let is_active = value_buf.as_str() == val.as_str();
                        let btn = egui::Button::new(
                            RichText::new(val)
                                .color(if is_active { c.bg_app } else { c.text_data })
                                .size(s.font_body)
                                .monospace(),
                        )
                        .fill(if is_active { c.accent_secondary } else { egui::Color32::TRANSPARENT })
                        .stroke(egui::Stroke::new(
                            0.0,
                            egui::Color32::TRANSPARENT,
                        ))
                        .min_size(egui::vec2(ui.available_width(), 0.0));

                        if ui.add(btn).clicked() {
                            *value_buf = val.clone();
                        }
                    }
                });
        });
}
