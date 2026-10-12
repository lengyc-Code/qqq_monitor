use super::super::DesktopApp;
use super::overview::logical_cores;
use crate::{components::*, pixel, theme};
use eframe::egui;
use qqq_domain::{DeviceKind, MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn cpu(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let cpu = MetricKey::new("cpu", "usage");
        let core = MetricKey::new(&self.core, "usage");
        let mut cores: Vec<_> = view
            .devices
            .values()
            .filter(|d| d.kind == DeviceKind::Core)
            .collect();
        cores.sort_by_key(|d| core_number(&d.id));
        let busiest = cores
            .iter()
            .filter_map(|d| view.value(&d.id, "usage").map(|value| (*d, value)))
            .max_by(|a, b| a.1.total_cmp(&b.1));

        ui.columns(3, |uis| {
            let usage = view.value("cpu", "usage");
            stat_card(
                &mut uis[0],
                (&pixel::ICON_CPU, "总使用率", theme::CPU),
                &format_value(usage, Unit::Percent),
                "所有逻辑核心的平均负载",
                usage.map(|u| (u, theme::level_color(u, theme::TEAL))),
            );
            stat_card(
                &mut uis[1],
                (&pixel::ICON_STAR, "最忙核心", theme::ACCENT),
                &format_value(busiest.map(|b| b.1), Unit::Percent),
                &busiest
                    .map(|b| b.0.name.clone())
                    .unwrap_or_else(|| "等待采样".into()),
                busiest.map(|b| (b.1, theme::level_color(b.1, theme::PINK))),
            );
            stat_card(
                &mut uis[2],
                (&pixel::ICON_HEART, "逻辑核心", theme::MEMORY),
                &logical_cores(view),
                &view
                    .devices
                    .get("cpu")
                    .map(|d| d.name.clone())
                    .unwrap_or_default(),
                None,
            );
        });

        let mut series = vec![(&cpu, "整体 CPU", theme::TEAL)];
        if !self.core.is_empty() {
            series.push((&core, "选中核心", theme::PINK));
        }
        history_chart(
            ui,
            view,
            "cpu-history",
            "CPU 使用率",
            Unit::Percent,
            self.range,
            &series,
            130.0,
        );

        ui.horizontal(|ui| {
            section_title(ui, &pixel::ICON_CPU, "逻辑核心", "点击核心查看历史");
            heat_legend(ui);
        });
        let columns = ((ui.available_width() / 110.0) as usize).clamp(2, 8);
        for row in cores.chunks(columns) {
            ui.columns(columns, |uis| {
                for (index, device) in row.iter().enumerate() {
                    let selected = self.core == device.id;
                    if heat_tile(
                        &mut uis[index],
                        &format!("#{:02}", core_number(&device.id) + 1),
                        &device.name,
                        view.value(&device.id, "usage"),
                        selected,
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

fn core_number(id: &str) -> usize {
    id.strip_prefix("core:")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0)
}
