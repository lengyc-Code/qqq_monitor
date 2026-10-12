use super::super::DesktopApp;
use crate::{components::*, i18n, mascot, pixel, theme};
use eframe::egui::{self, RichText, vec2};
use qqq_engine::{Language, MonitorCommand, MonitorView};

/// History budget shown in diagnostics; matches the engine's allocation limit.
const HISTORY_BUDGET: f64 = 16.0 * 1024.0 * 1024.0;

impl DesktopApp {
    pub(in crate::app) fn settings(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        if ui.available_width() >= 900.0 {
            ui.columns(2, |uis| {
                let height = self.preferences(&mut uis[0]);
                self.diagnostics(&mut uis[1], view, height);
            });
        } else {
            self.preferences(ui);
            self.diagnostics(ui, view, 0.0);
        }
        about(ui, view);
    }

    /// Returns the card's outer height so a neighbouring card can match it.
    fn preferences(&mut self, ui: &mut egui::Ui) -> f32 {
        let t = i18n::t();
        theme::card()
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                tag(ui, Some(&pixel::ICON_GEAR), t.preferences, theme::ACCENT);
                let width = label_width(
                    ui,
                    &[
                        t.language,
                        t.sampling_interval,
                        t.default_range,
                        t.animations,
                        t.edge_docking,
                        t.minimize_to_tray,
                    ],
                    14.0,
                ) + 8.0;
                // Each row is a label column followed by controls; chips and long texts
                // wrap beside the label rather than under it.
                let row_label = |ui: &mut egui::Ui, text: &str| {
                    ui.add_sized([width, 30.0], egui::Label::new(text));
                };
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    row_label(ui, t.language);
                    ui.horizontal_wrapped(|ui| {
                        let choices = [(None, t.follow_system)]
                            .into_iter()
                            .chain(Language::ALL.map(|l| (Some(l), i18n::native_name(l))));
                        for (language, label) in choices {
                            if theme::chip(ui, label, self.config.language == language).clicked()
                                && self.active_config.language != language
                            {
                                // Applied and saved at once, like the network adapter choice.
                                self.config.language = language;
                                let mut config = self.active_config.clone();
                                config.language = language;
                                self.save_config(config, true);
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    row_label(ui, t.sampling_interval);
                    ui.horizontal_wrapped(|ui| {
                        for ms in [500, 1000, 2000] {
                            let label = (t.seconds)(ms as f64 / 1000.0);
                            if theme::chip(ui, &label, self.config.sampling_ms == ms).clicked() {
                                self.config.sampling_ms = ms;
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    row_label(ui, t.default_range);
                    ui.horizontal_wrapped(|ui| {
                        for (secs, label) in [
                            (60, (t.seconds)(60.0)),
                            (300, (t.minutes)(5)),
                            (900, (t.minutes)(15)),
                        ] {
                            if theme::chip(ui, &label, self.config.history_secs == secs).clicked() {
                                self.config.history_secs = secs;
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    row_label(ui, t.animations);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    if ui
                        .checkbox(&mut self.config.animations, t.animations_option)
                        .changed()
                    {
                        theme::apply(ui.ctx(), self.config.animations);
                    }
                });
                ui.horizontal(|ui| {
                    row_label(ui, t.edge_docking);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    ui.checkbox(&mut self.config.edge_docking, t.edge_docking_option);
                });
                ui.horizontal(|ui| {
                    row_label(ui, t.minimize_to_tray);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    ui.checkbox(&mut self.config.minimize_to_tray, t.minimize_to_tray_option);
                });
                ui.horizontal(|ui| {
                    let width = (label_width(ui, &[t.apply_save], 14.0) + 76.0).max(150.0);
                    if theme::pixel_button(
                        ui,
                        t.apply_save,
                        Some(&pixel::ICON_HEART),
                        true,
                        vec2(width, 38.0),
                    )
                    .clicked()
                    {
                        self.range = self.config.history_secs;
                        self.save_config(self.config.clone(), true);
                    }
                    ui.label(
                        RichText::new(i18n::save_status(&self.view.save_status))
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                });
            })
            .response
            .rect
            .height()
    }

    fn diagnostics(&mut self, ui: &mut egui::Ui, view: &MonitorView, min_height: f32) {
        let t = i18n::t();
        theme::card().min_height(min_height).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            tag(
                ui,
                Some(&pixel::ICON_CLOCK),
                t.runtime_status,
                theme::NETWORK,
            );
            ui.label((t.batches)(
                view.diagnostics.received_batches,
                view.diagnostics.dropped_batches,
            ));
            ui.label((t.history_usage)(&bytes(
                view.diagnostics.history_bytes as f64,
            )));
            let used = view.diagnostics.history_bytes as f64 / HISTORY_BUDGET * 100.0;
            segment_bar(ui, Some(used), theme::level_color(used, theme::BLUE), 16.0);
            if view.diagnostics.collector_errors.is_empty() {
                ui.label(RichText::new(t.collectors_ok).size(12.0).color(theme::GOOD));
            }
            for (source, error) in &view.diagnostics.collector_errors {
                ui.label(
                    RichText::new((t.labeled)(source, &i18n::reason(error)))
                        .size(12.0)
                        .color(theme::ERROR),
                );
            }
            if theme::pixel_button(
                ui,
                t.refresh_devices,
                Some(&pixel::ICON_STAR),
                false,
                vec2(
                    (label_width(ui, &[t.refresh_devices], 14.0) + 76.0).max(160.0),
                    38.0,
                ),
            )
            .clicked()
            {
                self.command(MonitorCommand::RefreshInventory);
            }
        });
    }
}

fn about(ui: &mut egui::Ui, view: &MonitorView) {
    let t = i18n::t();
    theme::card().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            mascot::portrait(ui, view, 2.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.horizontal(|ui| {
                    pixel::label(
                        ui,
                        "QQQ MONITOR",
                        2.0,
                        theme::PINK,
                        pixel::Effect::Drop(theme::INK),
                    );
                    pixel::label(
                        ui,
                        concat!("v", env!("CARGO_PKG_VERSION")),
                        1.5,
                        theme::MUTED,
                        pixel::Effect::Plain,
                    );
                });
                ui.label(t.about_mint);
                ui.label(
                    RichText::new(t.about_animations)
                        .size(12.0)
                        .color(theme::MUTED),
                );
            });
        });
    });
}
