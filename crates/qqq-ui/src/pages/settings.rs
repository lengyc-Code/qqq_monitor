use super::super::DesktopApp;
use crate::{components::bytes, theme};
use eframe::egui::{self, RichText};
use qqq_engine::{MonitorCommand, MonitorView};

impl DesktopApp {
    pub(in crate::app) fn settings(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        theme::card().show(ui, |ui| {
            ui.label(RichText::new("监控偏好").size(18.0));
            ui.horizontal(|ui| {
                ui.label("采样间隔");
                for (ms, label) in [(500, "0.5 秒"), (1000, "1 秒"), (2000, "2 秒")] {
                    ui.selectable_value(&mut self.config.sampling_ms, ms, label);
                }
            });
            ui.horizontal(|ui| {
                ui.label("默认历史范围");
                for (secs, label) in [(60, "60 秒"), (300, "5 分钟"), (900, "15 分钟")] {
                    ui.selectable_value(&mut self.config.history_secs, secs, label);
                }
            });
            if ui
                .checkbox(&mut self.config.animations, "启用轻量界面动效")
                .changed()
            {
                theme::apply(ui.ctx(), self.config.animations);
            }
            if ui.button("应用并保存").clicked() {
                self.range = self.config.history_secs;
                self.save_config(self.config.clone(), true);
            }
            ui.label(&view.save_status);
        });
        ui.add_space(12.0);
        theme::card().show(ui, |ui| {
            ui.label(RichText::new("运行状态").size(18.0));
            ui.label(format!(
                "接收采样批次 {} · 丢失批次 {}",
                view.diagnostics.received_batches, view.diagnostics.dropped_batches
            ));
            ui.label(format!(
                "历史数据占用 {} / 16 MiB",
                bytes(view.diagnostics.history_bytes as f64)
            ));
            if ui.button("立即刷新设备").clicked() {
                self.command(MonitorCommand::RefreshInventory);
            }
            for (source, error) in &view.diagnostics.collector_errors {
                ui.label(
                    RichText::new(format!("{source}：{error}")).color(egui::Color32::LIGHT_RED),
                );
            }
        });
    }
}
