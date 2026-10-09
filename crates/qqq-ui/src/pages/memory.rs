use super::super::DesktopApp;
use crate::{components::*, theme};
use eframe::egui::{self, RichText};
use qqq_domain::{MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn memory(&self, ui: &mut egui::Ui, view: &MonitorView) {
        ui.columns(3, |uis| {
            for (i, (metric, title)) in [
                ("used", "已使用"),
                ("available", "可用"),
                ("total", "物理内存总量"),
            ]
            .iter()
            .enumerate()
            {
                theme::card().show(&mut uis[i], |ui| {
                    ui.label(RichText::new(*title).color(theme::MUTED));
                    ui.label(
                        RichText::new(format_value(view.value("memory", metric), Unit::Bytes))
                            .size(26.0)
                            .color(theme::PURPLE),
                    );
                });
            }
        });
        ui.add_space(theme::SECTION_GAP);
        let key = MetricKey::new("memory", "usage");
        history_chart(
            ui,
            view,
            "memory-history",
            "物理内存使用趋势",
            Unit::Percent,
            self.range,
            &[(&key, "内存使用率", theme::PURPLE)],
            220.0,
        );
        ui.label(
            RichText::new("统计系统可用物理内存；已用内存与提交量属于不同指标。")
                .color(theme::MUTED),
        );
    }
}
