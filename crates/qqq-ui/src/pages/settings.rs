use super::super::DesktopApp;
use crate::{components::*, mascot, pixel, theme};
use eframe::egui::{self, RichText, vec2};
use qqq_engine::{MonitorCommand, MonitorView};

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
        theme::card()
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                tag(ui, Some(&pixel::ICON_GEAR), "监控偏好", theme::ACCENT);
                let row_label = |ui: &mut egui::Ui, text: &str| {
                    ui.add_sized([92.0, 30.0], egui::Label::new(text));
                };
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    row_label(ui, "采样间隔");
                    for (ms, label) in [(500, "0.5 秒"), (1000, "1 秒"), (2000, "2 秒")] {
                        if theme::chip(ui, label, self.config.sampling_ms == ms).clicked() {
                            self.config.sampling_ms = ms;
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    row_label(ui, "默认历史范围");
                    for (secs, label) in [(60, "60 秒"), (300, "5 分钟"), (900, "15 分钟")] {
                        if theme::chip(ui, label, self.config.history_secs == secs).clicked() {
                            self.config.history_secs = secs;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    row_label(ui, "界面动效");
                    if ui
                        .checkbox(&mut self.config.animations, "启用轻量动效与看板娘眨眼")
                        .changed()
                    {
                        theme::apply(ui.ctx(), self.config.animations);
                    }
                });
                ui.horizontal(|ui| {
                    if theme::pixel_button(
                        ui,
                        "应用并保存",
                        Some(&pixel::ICON_HEART),
                        true,
                        vec2(150.0, 38.0),
                    )
                    .clicked()
                    {
                        self.range = self.config.history_secs;
                        self.save_config(self.config.clone(), true);
                    }
                    ui.label(
                        RichText::new(&self.view.save_status)
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
        theme::card().min_height(min_height).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            tag(ui, Some(&pixel::ICON_CLOCK), "运行状态", theme::NETWORK);
            ui.label(format!(
                "接收采样批次 {}  ·  丢失批次 {}",
                view.diagnostics.received_batches, view.diagnostics.dropped_batches
            ));
            ui.label(format!(
                "历史数据占用 {} / 16 MiB",
                bytes(view.diagnostics.history_bytes as f64)
            ));
            let used = view.diagnostics.history_bytes as f64 / HISTORY_BUDGET * 100.0;
            segment_bar(ui, Some(used), theme::level_color(used, theme::BLUE), 16.0);
            if view.diagnostics.collector_errors.is_empty() {
                ui.label(
                    RichText::new("所有采集源工作正常")
                        .size(12.0)
                        .color(theme::GOOD),
                );
            }
            for (source, error) in &view.diagnostics.collector_errors {
                ui.label(
                    RichText::new(format!("{source}：{error}"))
                        .size(12.0)
                        .color(theme::ERROR),
                );
            }
            if theme::pixel_button(
                ui,
                "立即刷新设备",
                Some(&pixel::ICON_STAR),
                false,
                vec2(160.0, 38.0),
            )
            .clicked()
            {
                self.command(MonitorCommand::RefreshInventory);
            }
        });
    }
}

fn about(ui: &mut egui::Ui, view: &MonitorView) {
    theme::card().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            mascot::portrait(ui, view, 4.0);
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
                ui.label("看板娘 Q 酱会根据 CPU 与内存负载切换表情：悠闲、平静、忙碌和超载。");
                ui.label(
                    RichText::new(
                        "关闭界面动效后表情仍会随负载变化，但不再眨眼与晃动，也不会额外刷新界面。",
                    )
                    .size(12.0)
                    .color(theme::MUTED),
                );
            });
        });
    });
}
