use super::super::DesktopApp;
use crate::{components::*, theme};
use eframe::egui::{self, RichText};
use qqq_domain::{DeviceKind, MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn disk(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        device_selector(ui, view, "disk-selector", &mut self.disk, DeviceKind::Disk);
        device_details(ui, view, &self.disk);
        let read = MetricKey::new(&self.disk, "read");
        let write = MetricKey::new(&self.disk, "write");
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "读取 {}",
                format_value(view.value(&self.disk, "read"), Unit::BytesPerSecond)
            ));
            ui.label(format!(
                "写入 {}",
                format_value(view.value(&self.disk, "write"), Unit::BytesPerSecond)
            ));
        });
        history_chart(
            ui,
            view,
            "disk-history",
            "物理磁盘读写速率",
            Unit::BytesPerSecond,
            self.range,
            &[
                (&read, "读取", theme::BLUE),
                (&write, "写入", theme::PURPLE),
            ],
            176.0,
        );
        ui.add_space(theme::SECTION_GAP);
        ui.label(RichText::new("本地卷容量").size(theme::SECTION_TITLE));
        device_selector(
            ui,
            view,
            "volume-selector",
            &mut self.volume,
            DeviceKind::Volume,
        );
        theme::card().show(ui, |ui| {
            device_details(ui, view, &self.volume);
            metric_status(ui, view, &MetricKey::new(&self.volume, "usage"));
            ui.label(format!(
                "总量 {}  ·  可用 {}",
                format_value(view.value(&self.volume, "total"), Unit::Bytes),
                format_value(view.value(&self.volume, "available"), Unit::Bytes)
            ));
            if let Some(value) = view.value(&self.volume, "usage") {
                ui.add(
                    egui::ProgressBar::new(value as f32 / 100.0)
                        .text(format!("已用 {value:.1}%"))
                        .fill(theme::BLUE),
                );
            }
            ui.label(
                RichText::new("卷容量与物理磁盘 I/O 分别统计")
                    .size(12.0)
                    .color(theme::MUTED),
            );
        });
    }
}
