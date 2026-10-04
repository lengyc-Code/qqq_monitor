use super::super::{DesktopApp, Page};
use crate::{components::*, theme};
use eframe::egui::{self, RichText};
use qqq_domain::{MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn overview(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let cards = [
            (
                MetricKey::new("cpu", "usage"),
                "CPU",
                "处理器总体使用率",
                theme::CYAN,
                Page::Cpu,
            ),
            (
                MetricKey::new("memory", "usage"),
                "内存",
                "物理内存使用率",
                theme::PURPLE,
                Page::Memory,
            ),
            (
                MetricKey::new(&self.disk, "read"),
                "磁盘读取",
                "选中的物理磁盘",
                theme::BLUE,
                Page::Disk,
            ),
            (
                MetricKey::new(&self.network, "receive"),
                "网络接收",
                "选中的网卡",
                theme::GREEN,
                Page::Network,
            ),
        ];
        let columns = if ui.available_width() >= 920.0 { 4 } else { 2 };
        for row in cards.chunks(columns) {
            ui.columns(columns, |uis| {
                for (index, (key, title, hint, color, page)) in row.iter().enumerate() {
                    if metric_card(&mut uis[index], view, key, title, hint, *color).clicked() {
                        self.page = *page;
                    }
                }
            });
            ui.add_space(8.0);
        }
        let cpu = MetricKey::new("cpu", "usage");
        let memory = MetricKey::new("memory", "usage");
        history_chart(
            ui,
            view,
            "overview-cpu",
            "处理器与内存使用趋势",
            Unit::Percent,
            self.range,
            &[(&cpu, "CPU", theme::CYAN), (&memory, "内存", theme::PURPLE)],
            170.0,
        );
        ui.add_space(8.0);
        theme::card().show(ui, |ui| {
            ui.label(RichText::new("设备概览").size(16.0).strong());
            ui.columns(2, |uis| {
                device_details(&mut uis[0], view, "cpu");
                uis[0].label(format!(
                    "内存总量  {}",
                    format_value(view.value("memory", "total"), Unit::Bytes)
                ));
                uis[1].label(format!(
                    "磁盘写入  {}",
                    format_value(view.value(&self.disk, "write"), Unit::BytesPerSecond)
                ));
                uis[1].label(format!(
                    "网络发送  {}",
                    format_value(view.value(&self.network, "send"), Unit::BytesPerSecond)
                ));
                if let Some(device) = view.devices.get(&self.network) {
                    uis[1].label(format!("当前网卡  {}", device.name));
                }
                uis[1].label(format!(
                    "系统运行  {}",
                    format_value(view.value("system", "uptime"), Unit::Seconds)
                ));
            });
        });
    }
}
