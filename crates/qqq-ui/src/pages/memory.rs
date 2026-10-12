use super::super::DesktopApp;
use crate::{components::*, i18n, pixel, theme};
use eframe::egui::{self, RichText};
use qqq_domain::{MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn memory(&self, ui: &mut egui::Ui, view: &MonitorView) {
        let t = i18n::t();
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
                (&pixel::ICON_MEMORY, t.memory_in_use, theme::MEMORY),
                &format_value(view.value("memory", "used"), Unit::Bytes),
                &(t.share_of_total)(&format_value(usage, Unit::Percent)),
                usage.map(|u| (u, theme::level_color(u, theme::PINK))),
            );
            stat_card(
                &mut uis[1],
                (&pixel::ICON_HEART, t.available, theme::NETWORK),
                &format_value(available, Unit::Bytes),
                &(t.allocatable)(&format_value(available_percent, Unit::Percent)),
                available_percent.map(|p| (p, theme::BLUE)),
            );
            stat_card(
                &mut uis[2],
                (&pixel::ICON_STAR, t.memory_total, theme::ACCENT),
                &format_value(total, Unit::Bytes),
                t.memory_total_detail,
                None,
            );
        });
        let key = MetricKey::new("memory", "usage");
        history_chart(
            ui,
            view,
            "memory-history",
            t.memory_chart,
            Unit::Percent,
            self.range,
            &[(&key, t.memory_usage, theme::PINK)],
            280.0,
        );
        ui.label(RichText::new(t.memory_note).size(12.0).color(theme::MUTED));
    }
}
