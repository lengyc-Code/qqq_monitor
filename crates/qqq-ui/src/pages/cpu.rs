use super::super::DesktopApp;
use crate::{components::*, theme};
use eframe::egui::{self, RichText};
use qqq_domain::{DeviceKind, MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn cpu(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        device_details(ui, view, "cpu");
        let cpu = MetricKey::new("cpu", "usage");
        let core = MetricKey::new(&self.core, "usage");
        let mut series = vec![(&cpu, "整体 CPU", theme::CYAN)];
        if !self.core.is_empty() {
            series.push((&core, "选中核心", theme::PURPLE));
        }
        history_chart(
            ui,
            view,
            "cpu-history",
            "CPU 使用率",
            Unit::Percent,
            self.range,
            &series,
            180.0,
        );
        ui.add_space(theme::SECTION_GAP);
        ui.label(RichText::new("逻辑核心 · 点击查看历史").size(theme::SECTION_TITLE));
        let mut cores: Vec<_> = view
            .devices
            .values()
            .filter(|d| d.kind == DeviceKind::Core)
            .collect();
        cores.sort_by_key(|d| {
            d.id.strip_prefix("core:")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0)
        });
        let columns = ((ui.available_width() / 96.0) as usize).clamp(2, 8);
        for row in cores.chunks(columns) {
            ui.columns(columns, |uis| {
                for (index, device) in row.iter().enumerate() {
                    let value = view.value(&device.id, "usage");
                    let text = format!("{}\n{}", device.name, format_value(value, Unit::Percent));
                    let fill = theme::core_fill(value.unwrap_or(0.0));
                    if uis[index]
                        .add_sized(
                            [uis[index].available_width(), 50.0],
                            egui::Button::new(RichText::new(text).size(12.0).color(theme::TEXT))
                                .fill(fill)
                                .selected(self.core == device.id),
                        )
                        .clicked()
                    {
                        self.core = device.id.clone();
                    }
                }
            });
        }
    }
}
