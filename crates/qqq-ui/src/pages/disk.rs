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
            .filter(|d| d.kind == DeviceKind::Volume)
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
                    let ui = &mut uis[index];
                    let usage = view.value(&volume.id, "usage");
                    let detail = |name: &str| {
                        volume
                            .details
                            .iter()
                            .find(|(key, _)| key == name)
                            .map(|(_, value)| value.as_str())
                            .filter(|value| !value.is_empty())
                    };
                    theme::card()
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;
                            ui.horizontal(|ui| {
                                tag(
                                    ui,
                                    Some(&pixel::ICON_DISK),
                                    &device_name(volume),
                                    theme::DISK,
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if volume.online {
                                            pixel::label(
                                                ui,
                                                &format_value(usage, Unit::Percent),
                                                2.0,
                                                theme::INK,
                                                pixel::Effect::Plain,
                                            );
                                        } else {
                                            ui.label(
                                                RichText::new(t.state_offline)
                                                    .size(12.0)
                                                    .color(theme::MUTED),
                                            );
                                        }
                                    },
                                );
                            });
                            segment_bar(
                                ui,
                                usage,
                                theme::level_color(usage.unwrap_or(0.0), theme::GOLD),
                                16.0,
                            );
                            ui.label(
                                RichText::new((t.volume_space)(
                                    &format_value(view.value(&volume.id, "available"), Unit::Bytes),
                                    &format_value(view.value(&volume.id, "total"), Unit::Bytes),
                                ))
                                .size(12.0),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{}  ·  {}",
                                    detail("volume_label").unwrap_or(t.no_label),
                                    detail("file_system").unwrap_or(t.unknown_file_system)
                                ))
                                .size(11.0)
                                .color(theme::MUTED),
                            );
                            metric_status(ui, view, &MetricKey::new(&volume.id, "usage"));
                        })
                        .response
                        .on_hover_text(detail("volume_guid").unwrap_or_default());
                }
            });
        }
    }
}
