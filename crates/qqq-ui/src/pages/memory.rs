use super::super::DesktopApp;
use crate::{components::*, pixel, theme};
use eframe::egui::{self, RichText};
use qqq_domain::{MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn memory(&self, ui: &mut egui::Ui, view: &MonitorView) {
        let usage = view.value("memory", "usage");
        let total = view.value("memory", "total");
        let available = view.value("memory", "available");
        let available_percent = available
            .zip(total)
            .filter(|(_, total)| *total > 0.0)
            .map(|(available, total)| available / total * 100.0);
        ui.columns(3, |uis| {
            stat_card(
                &mut uis[0],
                (&pixel::ICON_MEMORY, "已使用", theme::MEMORY),
                &format_value(view.value("memory", "used"), Unit::Bytes),
                &format!("占总量 {}", format_value(usage, Unit::Percent)),
                usage.map(|u| (u, theme::level_color(u, theme::PINK))),
            );
            stat_card(
                &mut uis[1],
                (&pixel::ICON_HEART, "可用", theme::NETWORK),
                &format_value(available, Unit::Bytes),
                &format!(
                    "还能分配 {}",
                    format_value(available_percent, Unit::Percent)
                ),
                available_percent.map(|p| (p, theme::BLUE)),
            );
            stat_card(
                &mut uis[2],
                (&pixel::ICON_STAR, "物理内存总量", theme::ACCENT),
                &format_value(total, Unit::Bytes),
                "系统可用的物理内存",
                None,
            );
        });
        let key = MetricKey::new("memory", "usage");
        history_chart(
            ui,
            view,
            "memory-history",
            "物理内存使用趋势",
            Unit::Percent,
            self.range,
            &[(&key, "内存使用率", theme::PINK)],
            280.0,
        );
        ui.label(
            RichText::new("统计系统可用物理内存；已用内存与提交量属于不同指标。")
                .size(12.0)
                .color(theme::MUTED),
        );
    }
}
