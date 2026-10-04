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
            220.0,
        );
        ui.add_space(8.0);
        ui.label(RichText::new("逻辑核心 · 点击查看历史").size(16.0));
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
        let columns = ((ui.available_width() / 110.0) as usize).clamp(2, 8);
        for row in cores.chunks(columns) {
            ui.columns(columns, |uis| {
                for (index, device) in row.iter().enumerate() {
                    let value = view.value(&device.id, "usage");
                    let text = format!("{}\n{}", device.name, format_value(value, Unit::Percent));
                    let intensity = (value.unwrap_or(0.0) / 100.0) as f32;
                    let fill = egui::Color32::from_rgb(
                        20,
                        (35.0 + intensity * 60.0) as u8,
                        (55.0 + intensity * 90.0) as u8,
                    );
                    if uis[index]
                        .add_sized(
                            [uis[index].available_width(), 64.0],
                            egui::Button::new(text)
                                .fill(fill)
                                .selected(self.core == device.id),
                        )
                        .clicked()
                    {
                        self.core = device.id.clone();
                    }
                }
            });
            ui.add_space(4.0);
        }
    }
}
