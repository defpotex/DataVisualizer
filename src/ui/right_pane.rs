use crate::plot::colormap::sample_gradient;
use crate::plot::styling::{ColorLegend, PlotLegendData};
use crate::theme::AppTheme;
use egui::{Color32, RichText, Ui};
use std::collections::HashSet;

// ── RightPane ─────────────────────────────────────────────────────────────────

pub struct RightPane {
    /// Plot IDs whose legends are currently collapsed.
    collapsed: HashSet<usize>,
}

impl Default for RightPane {
    fn default() -> Self {
        Self { collapsed: HashSet::new() }
    }
}

impl RightPane {
    /// Render the right legend pane. Call inside `egui::Panel::right`.
    pub fn show(&mut self, ui: &mut Ui, theme: &AppTheme, legends: &[PlotLegendData]) {
        let c = &theme.colors;
        let s = &theme.spacing;

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
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for legend in legends {
                    self.show_legend_card(ui, theme, legend);
                    ui.add_space(6.0);
                }
            });
    }

    fn show_legend_card(&mut self, ui: &mut Ui, theme: &AppTheme, legend: &PlotLegendData) {
        let c = &theme.colors;
        let s = &theme.spacing;
        let is_collapsed = self.collapsed.contains(&legend.plot_id);

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
                        ui.label(
                            RichText::new(format!("Color: {col}"))
                                .color(c.text_secondary)
                                .size(s.font_small),
                        );
                        ui.add_space(4.0);

                        let show_count = entries.len().min(10);
                        for (label, color) in &entries[..show_count] {
                            ui.horizontal(|ui| {
                                color_swatch(ui, *color, 10.0);
                                ui.label(
                                    RichText::new(label)
                                        .color(c.text_data)
                                        .size(s.font_small)
                                        .monospace(),
                                );
                            });
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
                        let rev_label = if *reverse { " ↔" } else { "" };
                        ui.label(
                            RichText::new(format!("Color: {col}  ({}{})", colormap.label(), rev_label))
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
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Size: {}", sz.col))
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
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Opacity: {}", al.col))
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
