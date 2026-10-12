use super::super::{DesktopApp, Page};
use crate::{components::*, i18n, pixel, theme};
use eframe::egui::{self, vec2};
use qqq_domain::{MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn overview(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let t = i18n::t();
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
                    detail: (t.cores_detail)(&logical_cores(view)),
                    template: RATE_TEMPLATE,
                },
                Page::Cpu,
            ),
            (
                MetricCard {
                    title: t.memory,
                    icon: &pixel::ICON_MEMORY,
                    tone: theme::MEMORY,
                    series: &[(&memory, theme::PINK)],
                    detail: (t.used_of)(
                        &format_value(view.value("memory", "used"), Unit::Bytes),
                        &format_value(view.value("memory", "total"), Unit::Bytes),
                    ),
                    template: RATE_TEMPLATE,
                },
                Page::Memory,
            ),
            (
                MetricCard {
                    title: t.disk_read,
                    icon: &pixel::ICON_DISK,
                    tone: theme::DISK,
                    series: &[(&read, theme::GOLD), (&write, theme::PINK)],
                    detail: format!("{} {}", t.write, current(&write, Unit::BytesPerSecond)),
                    template: RATE_TEMPLATE,
                },
                Page::Disk,
            ),
            (
                MetricCard {
                    title: t.network_receive,
                    icon: &pixel::ICON_NETWORK,
                    tone: theme::NETWORK,
                    series: &[(&receive, theme::BLUE), (&send, theme::TEAL)],
                    detail: format!("{} {}", t.send, current(&send, Unit::BytesPerSecond)),
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
                        .on_hover_text(t.click_for_details)
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
                t.cpu_memory_chart,
                Unit::Percent,
                self.range,
                &[(&cpu, "CPU", theme::TEAL), (&memory, t.memory, theme::PINK)],
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
                t.disk_io,
                Unit::BytesPerSecond,
                self.range,
                &[(&read, t.read, theme::GOLD), (&write, t.write, theme::PINK)],
                110.0,
            );
            history_chart(
                &mut uis[1],
                view,
                "overview-network",
                t.network_io,
                Unit::BytesPerSecond,
                self.range,
                &[
                    (&receive, t.receive, theme::BLUE),
                    (&send, t.send, theme::TEAL),
                ],
                110.0,
            );
        });
    }

    fn system_card(&self, ui: &mut egui::Ui, view: &MonitorView, min_height: f32) {
        let t = i18n::t();
        let name = |id: &str| {
            view.devices
                .get(id)
                .map(device_name)
                .unwrap_or_else(|| "—".into())
        };
        let rows = [
            (&pixel::ICON_CPU, theme::CPU, t.processor, name("cpu")),
            (
                &pixel::ICON_STAR,
                theme::CPU,
                t.logical_cores,
                logical_cores(view),
            ),
            (
                &pixel::ICON_MEMORY,
                theme::MEMORY,
                t.total_memory,
                format_value(view.value("memory", "total"), Unit::Bytes),
            ),
            (
                &pixel::ICON_DISK,
                theme::DISK,
                t.current_disk,
                name(&self.disk),
            ),
            (
                &pixel::ICON_NETWORK,
                theme::NETWORK,
                t.current_adapter,
                name(&self.network),
            ),
            (
                &pixel::ICON_CLOCK,
                theme::ACCENT,
                t.uptime,
                format_value(view.value("system", "uptime"), Unit::Seconds),
            ),
        ];
        theme::card().min_height(min_height).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            tag(
                ui,
                Some(&pixel::ICON_HEART),
                t.device_profile,
                theme::ACCENT,
            );
            ui.add_space(4.0);
            let labels = rows.each_ref().map(|row| row.2);
            let width = label_width(ui, &labels, 12.0) + 12.0;
            for (icon, tone, label, value) in &rows {
                info_row(ui, icon, *tone, label, width, value);
            }
        });
    }
}

pub(super) fn logical_cores(view: &MonitorView) -> String {
    view.devices
        .get("cpu")
        .and_then(|d| d.details.iter().find(|(key, _)| key == "logical_cores"))
        .map(|(_, value)| value.clone())
        .unwrap_or_else(|| "—".into())
}
