use super::super::DesktopApp;
use crate::{components::*, i18n, pixel, theme};
use eframe::egui::{self, RichText};
use qqq_domain::{DeviceKind, MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn disk(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let t = i18n::t();
        device_picker(
            ui,
            view,
            "disk-selector",
            &mut self.disk,
            DeviceKind::Disk,
            (&pixel::ICON_DISK, t.physical_disk, theme::DISK),
        );
        let read = MetricKey::new(&self.disk, "read");
        let write = MetricKey::new(&self.disk, "write");
        ui.columns(2, |uis| {
            MetricCard {
                title: t.read,
                icon: &pixel::ICON_DISK,
                tone: theme::DISK,
                series: &[(&read, theme::GOLD)],
                detail: t.disk_read_detail.into(),
                template: RATE_TEMPLATE,
            }
            .show(&mut uis[0], view, false);
            MetricCard {
                title: t.write,
                icon: &pixel::ICON_DISK,
                tone: theme::ACCENT,
                series: &[(&write, theme::PINK)],
                detail: t.disk_write_detail.into(),
                template: RATE_TEMPLATE,
            }
            .show(&mut uis[1], view, false);
        });
        history_chart(
            ui,
            view,
            "disk-history",
            t.disk_chart,
            Unit::BytesPerSecond,
            self.range,
            &[(&read, t.read, theme::GOLD), (&write, t.write, theme::PINK)],
            140.0,
        );

        section_title(ui, &pixel::ICON_DISK, t.volumes_title, t.volumes_hint);
        let mut volumes: Vec<_> = view
            .devices
            .values()
            .filter(|d| d.kind == DeviceKind::Volume && d.host.is_none())
            .collect();
        volumes.sort_by(|a, b| a.name.cmp(&b.name));
        if volumes.is_empty() {
            ui.label(
                RichText::new(t.volumes_waiting)
                    .size(12.0)
                    .color(theme::MUTED),
            );
        }
        let columns = if ui.available_width() >= 900.0 { 3 } else { 2 };
        for row in volumes.chunks(columns) {
            ui.columns(columns, |uis| {
                for (index, volume) in row.iter().enumerate() {
                    let subtitle = format!(
                        "{}  ·  {}",
                        detail(volume, "volume_label").unwrap_or(t.no_label),
                        detail(volume, "file_system").unwrap_or(t.unknown_file_system)
                    );
                    volume_card(
                        &mut uis[index],
                        view,
                        volume,
                        &subtitle,
                        detail(volume, "volume_guid").unwrap_or_default(),
                    );
                }
            });
        }
    }
}
