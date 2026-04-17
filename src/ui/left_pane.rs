use crate::app::PaneAction;
use crate::data::filter::Filter;
use crate::data::schema::FieldKind;
use crate::plot::plot_config::PlotConfig;
use crate::state::app_state::AppState;
use crate::state::playback::{PlaybackMode, TimeUnit};
use crate::theme::AppTheme;
use crate::ui::add_filter_dialog::AddFilterDialog;
use crate::ui::add_plot_dialog::AddPlotDialog;
use egui::{RichText, Ui};

pub struct LeftPane {
    fields_expanded: std::collections::HashSet<usize>,
    section_sources_open: bool,
    section_plots_open: bool,
    section_playback_open: bool,
    section_filters_open: bool,
    add_plot_dialog: AddPlotDialog,
    add_filter_dialog: AddFilterDialog,
}

impl Default for LeftPane {
    fn default() -> Self {
        Self {
            fields_expanded: std::collections::HashSet::new(),
            section_sources_open: true,
            section_plots_open: true,
            section_playback_open: true,
            section_filters_open: true,
            add_plot_dialog: AddPlotDialog::default(),
            add_filter_dialog: AddFilterDialog::default(),
        }
    }
}

impl LeftPane {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        theme: &AppTheme,
        state: &AppState,
    ) -> Option<PaneAction> {
        let mut action = None;
        let c = &theme.colors;
        let s = &theme.spacing;

        let available = ui.available_rect_before_wrap();
        let footer_height = s.font_small + 10.0;
        let scroll_height = (available.height() - footer_height).max(0.0);

        egui::ScrollArea::vertical()
            .max_height(scroll_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(2.0);

                // ── DATA SOURCES ──────────────────────────────────────────────
                collapsible_section(ui, theme, "DATA SOURCES", &mut self.section_sources_open, |ui| {
                    ui.add_space(4.0);
                    let btn = egui::Button::new(
                        RichText::new("＋  Add Source  ▾").color(c.accent_primary).size(s.font_body),
                    ).min_size(egui::vec2(ui.available_width(), 0.0));
                    if ui.add(btn).clicked() { action = Some(PaneAction::OpenCsv); }

                    ui.add_space(6.0);
                    if state.sources.is_empty() {
                        ui.label(RichText::new("No sources loaded.").color(c.text_secondary).size(s.font_small).italics());
                    } else {
                        for source in &state.sources {
                            ui.add_space(4.0);
                            let is_streaming = state.is_streaming(source.id);
                            let is_paused = state.udp_handles.iter()
                                .find(|h| h.source_id == source.id)
                                .map(|h| h.is_paused())
                                .unwrap_or(false);
                            source_card(ui, theme, source, &mut self.fields_expanded, &mut action, is_streaming, is_paused);
                        }
                    }
                    for note in &state.notifications {
                        ui.add_space(4.0);
                        ui.label(RichText::new(format!("⚠ {}", note)).color(c.accent_warning).size(s.font_small));
                    }
                    ui.add_space(6.0);
                });

                ui.add_space(4.0);

                // ── PLOTS ─────────────────────────────────────────────────────
                collapsible_section(ui, theme, "PLOTS", &mut self.section_plots_open, |ui| {
                    ui.add_space(4.0);
                    let btn = egui::Button::new(
                        RichText::new("＋  Add Plot")
                            .color(if state.has_sources() { c.accent_primary } else { c.text_secondary })
                            .size(s.font_body),
                    ).min_size(egui::vec2(ui.available_width(), 0.0));
                    if ui.add_enabled(state.has_sources(), btn).clicked() {
                        self.add_plot_dialog.open();
                    }
                    if !state.has_sources() {
                        ui.add_space(4.0);
                        ui.label(RichText::new("Load a data source first.").color(c.text_secondary).size(s.font_small).italics());
                    }
                    if state.plots.is_empty() && state.has_sources() {
                        ui.add_space(4.0);
                        ui.label(RichText::new("No plots yet.").color(c.text_secondary).size(s.font_small).italics());
                    }
                    for plot_config in &state.plots {
                        ui.add_space(4.0);
                        let source_label = state.sources.iter()
                            .find(|s| s.id == plot_config.source_id())
                            .map(|s| s.label.as_str())
                            .unwrap_or("(removed)");
                        if let Some(remove_id) = plot_card(ui, theme, plot_config, source_label) {
                            action = Some(PaneAction::RemovePlot(remove_id));
                        }
                    }
                    ui.add_space(6.0);
                });

                // Add Plot dialog (rendered outside section frame to float freely).
                if let Some(new_config) = self.add_plot_dialog.show(ui, theme, state) {
                    action = Some(PaneAction::AddPlot(new_config));
                }

                ui.add_space(4.0);

                // ── PLAYBACK ─────────────────────────────────────────────────
                if state.has_sources() {
                    collapsible_section(ui, theme, "PLAYBACK", &mut self.section_playback_open, |ui| {
                        if let Some(a) = playback_section(ui, theme, state) {
                            action = Some(a);
                        }
                    });
                    ui.add_space(4.0);
                }

                // ── FILTERS ───────────────────────────────────────────────────
                collapsible_section(ui, theme, "FILTERS", &mut self.section_filters_open, |ui| {
                    ui.add_space(4.0);
                    let btn = egui::Button::new(
                        RichText::new("＋  Add Filter")
                            .color(if state.has_sources() { c.accent_primary } else { c.text_secondary })
                            .size(s.font_body),
                    ).min_size(egui::vec2(ui.available_width(), 0.0));
                    if ui.add_enabled(state.has_sources(), btn).clicked() {
                        self.add_filter_dialog.open();
                    }

                    // Hide the managed playback filter from the user filter list.
                    let user_filters: Vec<&Filter> = state
                        .filters
                        .iter()
                        .filter(|f| f.id != usize::MAX)
                        .collect();
                    if user_filters.is_empty() {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("No active filters.")
                                .color(c.text_secondary)
                                .size(s.font_small)
                                .italics(),
                        );
                    } else {
                        for filter in user_filters {
                            ui.add_space(4.0);
                            if let Some(a) = filter_card(ui, theme, filter) {
                                action = Some(a);
                            }
                        }
                    }
                    ui.add_space(6.0);
                });

                // Add Filter dialog.
                if let Some(mut filter) = self.add_filter_dialog.show(ui, theme, state) {
                    filter.id = 0; // caller assigns real id
                    action = Some(PaneAction::AddFilter(filter));
                }

                ui.add_space(4.0);
            });

        // Pinned footer.
        ui.add(egui::Separator::default().horizontal());
        ui.label(RichText::new("v0.1.0-dev").color(c.text_secondary).size(s.font_small));

        action
    }
}

// ── Collapsible section ───────────────────────────────────────────────────────

fn collapsible_section(
    ui: &mut Ui,
    theme: &AppTheme,
    title: &str,
    open: &mut bool,
    content: impl FnOnce(&mut Ui),
) {
    let c = &theme.colors;
    let s = &theme.spacing;
    let header_height = s.font_small + 14.0;
    let width = ui.available_width();
    let (header_rect, response) =
        ui.allocate_exact_size(egui::vec2(width, header_height), egui::Sense::click());

    if response.clicked() { *open = !*open; }

    if ui.is_rect_visible(header_rect) {
        let bg = if response.hovered() { c.widget_bg_hovered } else { c.widget_bg };
        ui.painter().rect_filled(header_rect, s.rounding, bg);
        let strip = egui::Rect::from_min_size(header_rect.left_top(), egui::vec2(3.0, header_rect.height()));
        ui.painter().rect_filled(strip, 0.0, c.accent_primary);
        let chevron = if *open { "▾" } else { "▸" };
        ui.painter().text(header_rect.left_center() + egui::vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER, chevron,
            egui::FontId::proportional(s.font_small), c.accent_primary);
        ui.painter().text(header_rect.left_center() + egui::vec2(24.0, 0.0),
            egui::Align2::LEFT_CENTER, title,
            egui::FontId::proportional(s.font_small), c.text_primary);
    }

    if *open {
        egui::Frame::default()
            .fill(c.bg_panel)
            .stroke(egui::Stroke::new(1.0, c.border))
            .corner_radius(egui::CornerRadius { nw: 0, ne: 0, sw: s.rounding as u8, se: s.rounding as u8 })
            .inner_margin(egui::Margin::from(egui::vec2(s.panel_padding, 4.0)))
            .show(ui, content);
    }
}

// ── Source card ───────────────────────────────────────────────────────────────

fn source_card(
    ui: &mut Ui,
    theme: &AppTheme,
    source: &crate::data::source::DataSource,
    expanded: &mut std::collections::HashSet<usize>,
    action: &mut Option<PaneAction>,
    is_streaming: bool,
    is_paused: bool,
) {
    let c = &theme.colors;
    let s = &theme.spacing;
    egui::Frame::default()
        .fill(c.bg_app)
        .stroke(egui::Stroke::new(1.0, if is_streaming { c.accent_primary } else { c.border }))
        .corner_radius(s.rounding)
        .inner_margin(egui::Margin::from(8.0_f32))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let icon = if is_streaming { "◉" } else { "●" };
                let icon_color = if is_streaming && !is_paused { c.accent_primary } else { c.accent_secondary };
                ui.label(RichText::new(icon).color(icon_color).size(s.font_small));
                ui.label(RichText::new(&source.label).color(c.text_primary).size(s.font_body).strong());
                if is_streaming {
                    let status = if is_paused { "PAUSED" } else { "LIVE" };
                    let status_color = if is_paused { c.accent_warning } else { c.accent_primary };
                    ui.label(RichText::new(status).color(status_color).size(s.font_small).strong());
                }
            });
            ui.label(
                RichText::new(format!("{} rows · {} fields", format_count(source.row_count()), source.field_count()))
                    .color(c.text_secondary).size(s.font_small),
            );
            ui.add_space(4.0);
            let is_expanded = expanded.contains(&source.id);
            let toggle_label = if is_expanded { "▾ Fields" } else { "▸ Fields" };
            if ui.add(egui::Button::new(
                RichText::new(toggle_label).color(c.text_secondary).size(s.font_small),
            ).frame(false)).clicked() {
                if is_expanded { expanded.remove(&source.id); } else { expanded.insert(source.id); }
            }
            if is_expanded {
                ui.add_space(2.0);
                for field in &source.schema.fields {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(RichText::new(field.kind.icon()).color(field_icon_color(&field.kind, theme)).size(s.font_small));
                        ui.label(RichText::new(&field.name).color(c.text_data).size(s.font_small).monospace());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(field.kind.label()).color(c.text_secondary).size(s.font_small));
                        });
                    });
                }
                ui.add_space(2.0);
            }
            ui.add_space(4.0);

            if is_streaming {
                // Streaming controls: Pause/Resume + Stop
                ui.horizontal(|ui| {
                    let pause_label = if is_paused { "▶ Resume" } else { "⏸ Pause" };
                    if ui.add(egui::Button::new(
                        RichText::new(pause_label).color(c.accent_primary).size(s.font_small),
                    ).min_size(egui::vec2(ui.available_width() / 2.0 - 4.0, 0.0))).clicked() {
                        *action = Some(PaneAction::ToggleStreamPause(source.id));
                    }
                    if ui.add(egui::Button::new(
                        RichText::new("■ Stop").color(c.accent_warning).size(s.font_small),
                    ).min_size(egui::vec2(ui.available_width(), 0.0))).clicked() {
                        *action = Some(PaneAction::StopStream(source.id));
                    }
                });
            } else {
                if ui.add(egui::Button::new(
                    RichText::new("Remove").color(c.accent_warning).size(s.font_small),
                ).min_size(egui::vec2(ui.available_width(), 0.0))).clicked() {
                    *action = Some(PaneAction::RemoveSource(source.id));
                }
            }
        });
}

// ── Plot card ─────────────────────────────────────────────────────────────────

fn plot_card(ui: &mut Ui, theme: &AppTheme, config: &PlotConfig, source_label: &str) -> Option<usize> {
    let c = &theme.colors;
    let s = &theme.spacing;
    let mut remove_id = None;

    let (icon, type_label, icon_color) = match config {
        PlotConfig::Map(_)         => ("◈", "Map",          c.accent_primary),
        PlotConfig::Scatter(_)     => ("◉", "Scatter",      c.accent_secondary),
        PlotConfig::ScrollChart(_) => ("⏱", "Scroll Chart", c.accent_secondary),
    };

    egui::Frame::default()
        .fill(c.bg_app)
        .stroke(egui::Stroke::new(1.0, c.border))
        .corner_radius(s.rounding)
        .inner_margin(egui::Margin::from(8.0_f32))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).color(icon_color).size(s.font_small));
                ui.label(RichText::new(config.title()).color(c.text_primary).size(s.font_body).strong());
            });
            ui.label(
                RichText::new(format!("{type_label}  ·  {source_label}"))
                    .color(c.text_secondary).size(s.font_small),
            );
            ui.add_space(4.0);
            if ui.add(egui::Button::new(
                RichText::new("Remove").color(c.accent_warning).size(s.font_small),
            ).min_size(egui::vec2(ui.available_width(), 0.0))).clicked() {
                remove_id = Some(config.id());
            }
        });
    remove_id
}

// ── Filter card ───────────────────────────────────────────────────────────────

fn filter_card(ui: &mut Ui, theme: &AppTheme, filter: &Filter) -> Option<PaneAction> {
    let c = &theme.colors;
    let s = &theme.spacing;
    let mut action = None;

    egui::Frame::default()
        .fill(c.bg_app)
        .stroke(egui::Stroke::new(1.0, c.border))
        .corner_radius(s.rounding)
        .inner_margin(egui::Margin::from(8.0_f32))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Enable/disable toggle.
                let dot_color = if filter.enabled { c.accent_primary } else { c.text_secondary };
                if ui.add(egui::Button::new(
                    RichText::new(if filter.enabled { "●" } else { "○" }).color(dot_color).size(s.font_small),
                ).frame(false)).clicked() {
                    action = Some(PaneAction::ToggleFilter(filter.id));
                }
                ui.label(
                    RichText::new(filter.label())
                        .color(if filter.enabled { c.text_primary } else { c.text_secondary })
                        .size(s.font_small)
                        .monospace(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(
                        RichText::new("✕").color(c.accent_warning).size(s.font_small),
                    ).frame(false)).clicked() {
                        action = Some(PaneAction::RemoveFilter(filter.id));
                    }
                });
            });
        });
    action
}

// ── Playback section ─────────────────────────────────────────────────────────

fn playback_section(ui: &mut Ui, theme: &AppTheme, state: &AppState) -> Option<PaneAction> {
    let c = &theme.colors;
    let s = &theme.spacing;
    let playback = &state.playback;
    let mut action = None;

    ui.add_space(4.0);

    if !playback.is_active() {
        // Show source selector to start playback
        ui.label(
            RichText::new("Select a source to begin playback:")
                .color(c.text_secondary)
                .size(s.font_small),
        );
        ui.add_space(4.0);

        for source in &state.sources {
            // Check if source has a timestamp column
            let has_time = source.schema.fields.iter().any(|f| f.kind == FieldKind::Timestamp);
            let label = if has_time {
                format!("▶  {}", source.label)
            } else {
                format!("   {}  (no timestamp)", source.label)
            };
            let btn = egui::Button::new(
                RichText::new(&label)
                    .color(if has_time { c.accent_primary } else { c.text_secondary })
                    .size(s.font_small),
            )
            .min_size(egui::vec2(ui.available_width(), 0.0));
            if ui.add_enabled(has_time, btn).clicked() {
                action = Some(PaneAction::PlaybackStart(source.id));
            }
        }
    } else {
        // Active playback controls
        let source = playback
            .source_id
            .and_then(|id| state.sources.iter().find(|s| s.id == id));
        let source_label = source.map(|s| s.label.as_str()).unwrap_or("(unknown)");
        let time_col = playback
            .time_column
            .as_deref()
            .unwrap_or("(none)");

        ui.label(
            RichText::new(format!("Source: {}", source_label))
                .color(c.text_primary)
                .size(s.font_small)
                .strong(),
        );

        // Column selector — list all numeric columns from the source
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Column:")
                    .color(c.text_secondary)
                    .size(s.font_small),
            );
            let numeric_cols: Vec<String> = source
                .map(|src| {
                    src.schema
                        .fields
                        .iter()
                        .filter(|f| f.kind.is_numeric())
                        .map(|f| f.name.clone())
                        .collect()
                })
                .unwrap_or_default();

            let mut selected = time_col.to_string();
            let combo = egui::ComboBox::from_id_salt("playback_col")
                .width(ui.available_width() - 4.0)
                .selected_text(
                    RichText::new(&selected).size(s.font_small),
                );
            combo.show_ui(ui, |ui| {
                for col in &numeric_cols {
                    if ui.selectable_label(*col == selected, col).clicked() {
                        selected = col.clone();
                    }
                }
            });
            if selected != time_col {
                action = Some(PaneAction::PlaybackColumnChanged(selected));
            }
        });
        ui.add_space(6.0);

        // ── Transport controls ───────────────────────────────────
        ui.horizontal(|ui| {
            let btn_size = egui::vec2(28.0, 22.0);
            // Jump to start
            if ui
                .add(egui::Button::new(
                    RichText::new("|◁").color(c.text_primary).size(s.font_small),
                ).min_size(btn_size))
                .clicked()
            {
                action = Some(PaneAction::PlaybackJumpStart);
            }
            // Step backward
            if ui
                .add(egui::Button::new(
                    RichText::new("◁").color(c.text_primary).size(s.font_small),
                ).min_size(btn_size))
                .clicked()
            {
                action = Some(PaneAction::PlaybackStepBackward);
            }
            // Play/Pause
            let play_label = if playback.is_playing() { "⏸" } else { "▶" };
            if ui
                .add(egui::Button::new(
                    RichText::new(play_label)
                        .color(c.accent_primary)
                        .size(s.font_body),
                ).min_size(btn_size))
                .clicked()
            {
                action = Some(PaneAction::PlaybackToggle);
            }
            // Step forward
            if ui
                .add(egui::Button::new(
                    RichText::new("▷").color(c.text_primary).size(s.font_small),
                ).min_size(btn_size))
                .clicked()
            {
                action = Some(PaneAction::PlaybackStepForward);
            }
            // Jump to end
            if ui
                .add(egui::Button::new(
                    RichText::new("▷|").color(c.text_primary).size(s.font_small),
                ).min_size(btn_size))
                .clicked()
            {
                action = Some(PaneAction::PlaybackJumpEnd);
            }
        });

        ui.add_space(4.0);

        // ── Time scrubber ────────────────────────────────────────
        let mut progress = playback.progress();
        let slider_resp = ui.add(
            egui::Slider::new(&mut progress, 0.0..=1.0)
                .show_value(false)
                .trailing_fill(true),
        );
        if slider_resp.changed() {
            let t = playback.time_range.0 + progress as f64 * playback.duration();
            action = Some(PaneAction::PlaybackSeek(t));
        }

        // Time display
        let elapsed = playback.current_time - playback.time_range.0;
        let total = playback.duration();
        let time_str = match playback.time_unit {
            TimeUnit::Seconds => format!(
                "{} / {}",
                format_duration(elapsed),
                format_duration(total),
            ),
            TimeUnit::Raw => format!(
                "{} / {}",
                format_raw_value(playback.current_time),
                format_raw_value(playback.time_range.1),
            ),
        };
        ui.label(
            RichText::new(time_str)
                .color(c.text_secondary)
                .size(s.font_small)
                .monospace(),
        );

        ui.add_space(6.0);

        // ── Speed control ────────────────────────────────────────
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Speed")
                    .color(c.text_primary)
                    .size(s.font_small),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let speeds = [0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 50.0, 100.0];
                for &spd in speeds.iter().rev() {
                    let selected = (playback.speed - spd).abs() < 0.01;
                    let label = if spd >= 1.0 {
                        format!("{}x", spd as u32)
                    } else {
                        format!("{:.1}x", spd)
                    };
                    let btn = egui::Button::new(
                        RichText::new(&label)
                            .size(s.font_small - 1.0)
                            .color(if selected { c.text_primary } else { c.text_secondary }),
                    )
                    .fill(if selected {
                        c.accent_primary.linear_multiply(0.3)
                    } else {
                        egui::Color32::TRANSPARENT
                    })
                    .rounding(s.rounding - 1.0)
                    .min_size(egui::vec2(0.0, 0.0));
                    if ui.add(btn).clicked() {
                        action = Some(PaneAction::PlaybackSpeedChanged(spd));
                    }
                }
            });
        });

        ui.add_space(4.0);

        // ── Trail duration ───────────────────────────────────────
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Trail")
                    .color(c.text_primary)
                    .size(s.font_small),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let trail_val_f64 = playback.trail_duration.unwrap_or(0.0);
                let mut trail_val = trail_val_f64 as f32;
                let max_trail = playback.duration() as f32;
                let drag_speed = if playback.time_unit == TimeUnit::Seconds { 1.0 } else { max_trail * 0.005 };
                let mut drag_builder = egui::DragValue::new(&mut trail_val)
                    .range(0.0..=max_trail)
                    .speed(drag_speed);
                if playback.time_unit == TimeUnit::Seconds {
                    drag_builder = drag_builder.suffix(" s");
                }
                let drag = ui.add(drag_builder);
                if drag.changed() {
                    let new_trail = if trail_val <= 0.0 {
                        None
                    } else {
                        Some(trail_val as f64)
                    };
                    action = Some(PaneAction::PlaybackTrailChanged(new_trail));
                }
            });
        });
        ui.label(
            RichText::new("0 = show all history up to cursor")
                .color(c.text_secondary)
                .size(s.font_small - 1.0),
        );

        ui.add_space(4.0);

        // ── Loop toggle ──────────────────────────────────────────
        ui.horizontal(|ui| {
            let loop_label = if playback.loop_enabled { "✓ Loop" } else { "  Loop" };
            if ui
                .add(egui::Button::new(
                    RichText::new(loop_label).color(c.text_primary).size(s.font_small),
                ).frame(false))
                .clicked()
            {
                action = Some(PaneAction::PlaybackLoopToggle);
            }
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(2.0);

        // ── Stop button ──────────────────────────────────────────
        if ui
            .add(
                egui::Button::new(
                    RichText::new("Stop Playback")
                        .color(c.accent_warning)
                        .size(s.font_small),
                )
                .min_size(egui::vec2(ui.available_width(), 0.0)),
            )
            .clicked()
        {
            action = Some(PaneAction::PlaybackStop);
        }
    }

    ui.add_space(6.0);
    action
}

/// Format seconds as HH:MM:SS or MM:SS.
fn format_duration(secs: f64) -> String {
    let total = secs.abs() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Format a raw numeric value compactly (no unit suffix).
fn format_raw_value(v: f64) -> String {
    if v.abs() >= 1_000_000.0 {
        format!("{:.0}", v)
    } else if v.abs() >= 1.0 || v == 0.0 {
        // Show up to 2 decimal places, trim trailing zeros
        let s = format!("{:.2}", v);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    } else {
        format!("{:.4}", v)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn field_icon_color(kind: &FieldKind, theme: &AppTheme) -> egui::Color32 {
    let c = &theme.colors;
    match kind {
        FieldKind::Latitude | FieldKind::Longitude => c.accent_primary,
        FieldKind::Timestamp => c.accent_secondary,
        FieldKind::Altitude | FieldKind::Speed | FieldKind::Heading => c.text_data,
        _ => c.text_secondary,
    }
}

fn format_count(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { result.push(','); }
        result.push(ch);
    }
    result.chars().rev().collect()
}
