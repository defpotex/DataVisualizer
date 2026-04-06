use crate::data::filter::{Filter, FilterOp};
use crate::state::app_state::AppState;
use crate::theme::AppTheme;
use egui::{RichText, Ui, Window};

pub struct AddFilterDialog {
    pub is_open: bool,
    selected_col_idx: usize,
    selected_op_idx: usize,
    value_buf: String,
}

impl Default for AddFilterDialog {
    fn default() -> Self {
        Self {
            is_open: false,
            selected_col_idx: 0,
            selected_op_idx: 2, // default to ">"
            value_buf: String::new(),
        }
    }
}

impl AddFilterDialog {
    pub fn open(&mut self) {
        self.is_open = true;
        self.selected_col_idx = 0;
        self.selected_op_idx = 2;
        self.value_buf.clear();
    }

    /// Returns `Some(Filter)` when the user clicks Add (id=0 placeholder; caller assigns real id).
    pub fn show(&mut self, ui: &mut Ui, theme: &AppTheme, state: &AppState) -> Option<Filter> {
        if !self.is_open { return None; }

        let c = &theme.colors;
        let s = &theme.spacing;
        let mut result = None;
        let mut close = false;

        // Collect all columns from all sources.
        let all_cols: Vec<String> = state.sources.iter()
            .flat_map(|src| src.schema.fields.iter().map(|f| f.name.clone()))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        Window::new(
            RichText::new("Add Filter")
                .color(c.text_primary)
                .size(s.font_body + 1.0)
                .strong(),
        )
        .collapsible(false)
        .resizable(false)
        .min_width(300.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
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

            // Clamp indices.
            if self.selected_col_idx >= all_cols.len() { self.selected_col_idx = 0; }

            // Column picker.
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

            // Op + value on one row.
            ui.label(RichText::new("Condition").color(c.text_secondary).size(s.font_small));
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let ops = FilterOp::all();
                let op_label = ops[self.selected_op_idx.min(ops.len() - 1)].label();
                egui::ComboBox::from_id_salt("filter_op")
                    .selected_text(RichText::new(op_label).color(c.text_primary).size(s.font_body))
                    .width(60.0)
                    .show_ui(ui, |ui| {
                        for (idx, op) in ops.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.selected_op_idx,
                                idx,
                                RichText::new(op.label()).color(c.text_primary).size(s.font_body),
                            );
                        }
                    });

                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.value_buf)
                        .desired_width(ui.available_width())
                        .text_color(c.text_primary)
                        .hint_text("value")
                        .font(egui::FontSelection::FontId(
                            egui::FontId::proportional(s.font_body),
                        )),
                );
            });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            let ops = FilterOp::all();
            let can_add = !self.value_buf.trim().is_empty() && !all_cols.is_empty();

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
                    result = Some(Filter::new(
                        0, // placeholder id
                        all_cols[self.selected_col_idx].clone(),
                        ops[op_idx].clone(),
                        self.value_buf.trim().to_string(),
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
