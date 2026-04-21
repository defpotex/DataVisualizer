use crate::data::udp_receiver::UdpStreamConfig;
use crate::theme::AppTheme;
use egui::RichText;

/// Dialog for configuring a new UDP stream source.
pub struct UdpStreamDialog {
    open: bool,
    draft_bind_addr: String,
    draft_max_rows: usize,
    draft_label: String,
    error_msg: Option<String>,
}

impl Default for UdpStreamDialog {
    fn default() -> Self {
        let defaults = UdpStreamConfig::default();
        Self {
            open: false,
            draft_bind_addr: defaults.bind_addr,
            draft_max_rows: defaults.max_rows,
            draft_label: defaults.label,
            error_msg: None,
        }
    }
}

impl UdpStreamDialog {
    pub fn open(&mut self) {
        self.open = true;
        self.error_msg = None;
    }

    /// Render the dialog. Returns `Some(UdpStreamConfig)` when the user clicks Connect.
    pub fn show(&mut self, ctx: &egui::Context, theme: &AppTheme) -> Option<UdpStreamConfig> {
        if !self.open {
            return None;
        }

        let c = &theme.colors;
        let s = &theme.spacing;
        let mut result: Option<UdpStreamConfig> = None;

        let screen = ctx.screen_rect();
        let default_pos = egui::pos2(
            (screen.center().x - 160.0).max(screen.min.x),
            (screen.center().y - 120.0).max(screen.min.y),
        );

        egui::Window::new("Connect UDP Stream")
            .collapsible(false)
            .resizable(false)
            .default_width(320.0)
            .default_pos(default_pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_min_width(300.0);

                ui.label(
                    RichText::new("Configure a UDP stream source. The app will listen\nfor newline-delimited CSV packets on the specified port.")
                        .color(c.text_secondary)
                        .size(s.font_small),
                );
                ui.add_space(8.0);

                // ── Label ────────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Label").color(c.text_primary).size(s.font_body));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.draft_label)
                                .desired_width(180.0),
                        );
                    });
                });
                ui.add_space(4.0);

                // ── Bind address ─────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Bind Address").color(c.text_primary).size(s.font_body));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.draft_bind_addr)
                                .desired_width(180.0)
                                .hint_text("0.0.0.0:5005"),
                        );
                    });
                });
                ui.label(
                    RichText::new("host:port to listen on (e.g. 0.0.0.0:5005)")
                        .color(c.text_secondary)
                        .size(s.font_small),
                );
                ui.add_space(4.0);

                // ── Max rows ─────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Max Rows").color(c.text_primary).size(s.font_body));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.draft_max_rows)
                                .range(1_000..=10_000_000)
                                .speed(1_000.0)
                                .suffix(" rows"),
                        );
                    });
                });
                ui.label(
                    RichText::new("Rolling buffer: oldest rows are dropped when limit is reached")
                        .color(c.text_secondary)
                        .size(s.font_small),
                );

                ui.add_space(8.0);

                // ── Error message ────────────────────────────────────
                if let Some(err) = &self.error_msg {
                    ui.label(
                        RichText::new(err)
                            .color(c.accent_warning)
                            .size(s.font_small),
                    );
                    ui.add_space(4.0);
                }

                // ── Buttons ──────────────────────────────────────────
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(
                            RichText::new("Connect").color(c.accent_primary).size(s.font_body),
                        ))
                        .clicked()
                    {
                        // Validate
                        if self.draft_bind_addr.trim().is_empty() {
                            self.error_msg = Some("Bind address is required.".to_string());
                        } else if self.draft_label.trim().is_empty() {
                            self.error_msg = Some("Label is required.".to_string());
                        } else {
                            result = Some(UdpStreamConfig {
                                bind_addr: self.draft_bind_addr.trim().to_string(),
                                max_rows: self.draft_max_rows,
                                label: self.draft_label.trim().to_string(),
                            });
                            self.open = false;
                        }
                    }
                    if ui
                        .add(egui::Button::new(
                            RichText::new("Cancel").color(c.text_secondary).size(s.font_body),
                        ))
                        .clicked()
                    {
                        self.open = false;
                    }
                });
            });

        result
    }

    pub fn set_error(&mut self, msg: String) {
        self.error_msg = Some(msg);
        self.open = true;
    }
}
