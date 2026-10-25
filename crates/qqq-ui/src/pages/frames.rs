use super::super::DesktopApp;
use crate::{components::*, i18n, pixel, theme};
use eframe::egui::{self, RichText, vec2};
use qqq_domain::{MetricKey, Reason, Unit};
use qqq_engine::{MonitorView, Notice};

/// Device of the foreground application's frames, and the source that collects them.
const FRAMES: &str = "frames";

impl DesktopApp {
    /// Switches frame monitoring on or off, applied and saved at once.
    pub(in crate::app) fn set_frame_monitoring(&mut self, enabled: bool) {
        self.apply_now(|config| config.frame_monitoring = enabled);
    }

    pub(in crate::app) fn frames(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let t = i18n::t();
        let enabled = self.active_config.frame_monitoring;
        theme::card().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                tag(
                    ui,
                    Some(&pixel::ICON_FRAMES),
                    t.frame_monitoring,
                    theme::ACCENT,
                );
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                let mut on = enabled;
                if ui.checkbox(&mut on, t.frame_monitoring_option).changed() {
                    self.set_frame_monitoring(on);
                }
            });
            if !enabled {
                ui.label(RichText::new(t.frames_off).size(12.0).color(theme::MUTED));
                return;
            }
            match view.diagnostics.collector_errors.get(FRAMES) {
                Some(Reason::TracingDenied) => {
                    ui.label(
                        RichText::new(t.frames_denied)
                            .size(12.0)
                            .color(theme::WARNING),
                    );
                    let size = vec2(label_width(ui, &[t.frames_restart], 14.0) + 64.0, 34.0);
                    if theme::pixel_button(
                        ui,
                        t.frames_restart,
                        Some(&pixel::ICON_HEART),
                        true,
                        size,
                    )
                    .clicked()
                    {
                        // The new instance opens this page; this one closes the usual way,
                        // which saves the window.
                        match (self.desktop.system.restart_elevated)("--page frames") {
                            Ok(()) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                            Err(reason) => self.message = Some(Notice::ElevationFailed(reason)),
                        }
                    }
                }
                Some(reason) => {
                    ui.label(
                        RichText::new(i18n::reason(reason))
                            .size(12.0)
                            .color(theme::ERROR),
                    );
                }
                None if view
                    .devices
                    .get(FRAMES)
                    .is_some_and(|d| d.online && !d.name.is_empty()) =>
                {
                    ui.label(
                        RichText::new(t.frames_target)
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                    device_details(ui, view, FRAMES, true);
                }
                None => {
                    ui.label(
                        RichText::new(t.frames_waiting)
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                }
            }
        });
        if enabled {
            let fps = MetricKey::new(FRAMES, "fps");
            let mean = MetricKey::new(FRAMES, "frame_time");
            let longest = MetricKey::new(FRAMES, "frame_time_max");
            let current = |key: &MetricKey| {
                format_value(
                    view.metrics.get(key).and_then(|r| r.value),
                    Unit::Milliseconds,
                )
            };
            ui.columns(2, |uis| {
                MetricCard {
                    title: t.fps,
                    icon: &pixel::ICON_FRAMES,
                    tone: theme::CPU,
                    series: &[(&fps, theme::TEAL)],
                    detail: (t.fps_detail)(&current(&mean)),
                    template: RATE_TEMPLATE,
                }
                .show(&mut uis[0], view, false);
                MetricCard {
                    title: t.frame_time,
                    icon: &pixel::ICON_CLOCK,
                    tone: theme::ACCENT,
                    series: &[(&mean, theme::PINK)],
                    detail: (t.frame_time_detail)(&current(&longest)),
                    template: RATE_TEMPLATE,
                }
                .show(&mut uis[1], view, false);
            });
            ui.columns(2, |uis| {
                history_chart(
                    &mut uis[0],
                    view,
                    "frames-fps",
                    t.fps_chart,
                    Unit::FramesPerSecond,
                    self.range,
                    &[(&fps, t.fps, theme::TEAL)],
                    160.0,
                );
                history_chart(
                    &mut uis[1],
                    view,
                    "frames-time",
                    t.frame_time_chart,
                    Unit::Milliseconds,
                    self.range,
                    &[
                        (&mean, t.frame_time, theme::PINK),
                        (&longest, t.frame_time_max, theme::GOLD),
                    ],
                    160.0,
                );
            });
        }
        ui.label(RichText::new(t.frames_note).size(12.0).color(theme::MUTED));
    }
}
