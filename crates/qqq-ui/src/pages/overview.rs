use super::super::{DesktopApp, Page};
use crate::{components::*, pixel, theme};
use eframe::egui::{self, vec2};
use qqq_domain::{MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn overview(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let cpu = MetricKey::new("cpu", "usage");
        let memory = MetricKey::new("memory", "usage");
        let read = MetricKey::new(&self.disk, "read");
        let write = MetricKey::new(&self.disk, "write");
        let receive = MetricKey::new(&self.network, "receive");
        let send = MetricKey::new(&self.network, "send");
        let current =
            |key: &MetricKey, unit| format_value(view.metrics.get(key).and_then(|r| r.value), unit);
        let cards = [
            (
                MetricCard {
                    title: "CPU",
                    icon: &pixel::ICON_CPU,
                    tone: theme::CPU,
                    series: &[(&cpu, theme::TEAL)],
                    detail: format!("逻辑核心 {}", logical_cores(view)),
                    template: RATE_TEMPLATE,
                },
                Page::Cpu,
            ),
            (
                MetricCard {
                    title: "内存",
                    icon: &pixel::ICON_MEMORY,
                    tone: theme::MEMORY,
                    series: &[(&memory, theme::PINK)],
                    detail: format!(
                        "已用 {} / {}",
                        format_value(view.value("memory", "used"), Unit::Bytes),
                        format_value(view.value("memory", "total"), Unit::Bytes)
                    ),
                    template: RATE_TEMPLATE,
                },
                Page::Memory,
            ),
            (
                MetricCard {
                    title: "磁盘读取",
                    icon: &pixel::ICON_DISK,
                    tone: theme::DISK,
                    series: &[(&read, theme::GOLD), (&write, theme::PINK)],
                    detail: format!("写入 {}", current(&write, Unit::BytesPerSecond)),
                    template: RATE_TEMPLATE,
                },
                Page::Disk,
            ),
            (
                MetricCard {
                    title: "网络接收",
                    icon: &pixel::ICON_NETWORK,
                    tone: theme::NETWORK,
                    series: &[(&receive, theme::BLUE), (&send, theme::TEAL)],
                    detail: format!("发送 {}", current(&send, Unit::BytesPerSecond)),
                    template: RATE_TEMPLATE,
                },
                Page::Network,
            ),
        ];
        let columns = if ui.available_width() >= 840.0 { 4 } else { 2 };
        for row in cards.chunks(columns) {
            ui.columns(columns, |uis| {
                for (index, (card, page)) in row.iter().enumerate() {
                    if card
                        .show(&mut uis[index], view, true)
                        .on_hover_text("点击查看详情")
                        .clicked()
                    {
                        self.page = *page;
                    }
                }
            });
        }
        ui.add_space(theme::SECTION_GAP - 8.0);

        let trend = |ui: &mut egui::Ui| {
            history_chart(
                ui,
                view,
                "overview-cpu",
                "处理器与内存使用趋势",
                Unit::Percent,
                self.range,
                &[(&cpu, "CPU", theme::TEAL), (&memory, "内存", theme::PINK)],
                170.0,
            )
        };
        if ui.available_width() >= 900.0 {
            ui.horizontal_top(|ui| {
                let info_width = 300.0;
                let chart_width = ui.available_width() - info_width - ui.spacing().item_spacing.x;
                let height = ui
                    .allocate_ui_with_layout(
                        vec2(chart_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        trend,
                    )
                    .inner
                    .rect
                    .height();
                ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.system_card(ui, view, height),
                );
            });
        } else {
            trend(ui);
            self.system_card(ui, view, 0.0);
        }

        ui.columns(2, |uis| {
            history_chart(
                &mut uis[0],
                view,
                "overview-disk",
                "磁盘读写",
                Unit::BytesPerSecond,
                self.range,
                &[(&read, "读取", theme::GOLD), (&write, "写入", theme::PINK)],
                110.0,
            );
            history_chart(
                &mut uis[1],
                view,
                "overview-network",
                "网络收发",
                Unit::BytesPerSecond,
                self.range,
                &[
                    (&receive, "接收", theme::BLUE),
                    (&send, "发送", theme::TEAL),
                ],
                110.0,
            );
        });
    }

    fn system_card(&self, ui: &mut egui::Ui, view: &MonitorView, min_height: f32) {
        let name = |id: &str| {
            view.devices
                .get(id)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "—".into())
        };
        theme::card().min_height(min_height).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            tag(ui, Some(&pixel::ICON_HEART), "设备档案", theme::ACCENT);
            ui.add_space(4.0);
            info_row(ui, &pixel::ICON_CPU, theme::CPU, "处理器", &name("cpu"));
            info_row(
                ui,
                &pixel::ICON_STAR,
                theme::CPU,
                "逻辑核心",
                &logical_cores(view),
            );
            info_row(
                ui,
                &pixel::ICON_MEMORY,
                theme::MEMORY,
                "内存总量",
                &format_value(view.value("memory", "total"), Unit::Bytes),
            );
            info_row(
                ui,
                &pixel::ICON_DISK,
                theme::DISK,
                "当前磁盘",
                &name(&self.disk),
            );
            info_row(
                ui,
                &pixel::ICON_NETWORK,
                theme::NETWORK,
                "当前网卡",
                &name(&self.network),
            );
            info_row(
                ui,
                &pixel::ICON_CLOCK,
                theme::ACCENT,
                "系统运行",
                &format_value(view.value("system", "uptime"), Unit::Seconds),
            );
        });
    }
}

pub(super) fn logical_cores(view: &MonitorView) -> String {
    view.devices
        .get("cpu")
        .and_then(|d| d.details.iter().find(|(name, _)| name == "逻辑核心"))
        .map(|(_, value)| value.clone())
        .unwrap_or_else(|| "—".into())
}
