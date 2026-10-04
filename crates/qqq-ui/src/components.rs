use crate::theme;
use eframe::egui::{self, Color32, RichText, Stroke};
use egui_plot::{Legend, Line, Plot};
use qqq_domain::{DeviceKind, MetricKey, MetricReading, SampleState, Unit};
use qqq_engine::MonitorView;

pub fn bytes(value: f64) -> String {
    if value >= 1024.0_f64.powi(3) {
        format!("{:.2} GiB", value / 1024.0_f64.powi(3))
    } else if value >= 1024.0_f64.powi(2) {
        format!("{:.1} MiB", value / 1024.0_f64.powi(2))
    } else if value >= 1024.0 {
        format!("{:.1} KiB", value / 1024.0)
    } else {
        format!("{value:.0} B")
    }
}
pub fn format_value(value: Option<f64>, unit: Unit) -> String {
    value
        .map(|value| match unit {
            Unit::Percent => format!("{value:.1}%"),
            Unit::Bytes => bytes(value),
            Unit::BytesPerSecond => format!("{}/s", bytes(value)),
            Unit::Seconds => {
                let seconds = value as u64;
                format!(
                    "{} 天 {:02}:{:02}:{:02}",
                    seconds / 86400,
                    seconds / 3600 % 24,
                    seconds / 60 % 60,
                    seconds % 60
                )
            }
        })
        .unwrap_or_else(|| "—".into())
}
pub fn state_label(view: &MonitorView, reading: Option<&MetricReading>) -> (&'static str, Color32) {
    match reading.map(|r| r.effective_state(view.now)) {
        Some(SampleState::Ready) => ("实时", theme::GREEN),
        Some(SampleState::Stale) => ("数据过期", Color32::YELLOW),
        Some(SampleState::Failed) => ("采集失败", Color32::LIGHT_RED),
        Some(SampleState::Unsupported) => ("不支持", theme::MUTED),
        Some(SampleState::Offline) => ("已断开", theme::MUTED),
        _ => ("采集中", theme::MUTED),
    }
}

/// Explain unavailable readings without replacing a real last-known value with zero.
pub fn metric_status(ui: &mut egui::Ui, view: &MonitorView, key: &MetricKey) {
    let Some(reading) = view.metrics.get(key) else {
        return;
    };
    if reading.effective_state(view.now) == SampleState::Ready {
        return;
    }
    let (status, color) = state_label(view, Some(reading));
    let mut message = status.to_string();
    if let Some(at) = reading.last_success {
        message.push_str(&format!(
            " · 上次有效数据 {:.0} 秒前",
            view.now.saturating_duration_since(at).as_secs_f64()
        ));
    }
    if let Some(reason) = &reading.reason {
        message.push_str(&format!(" · {reason}"));
    }
    ui.label(RichText::new(message).size(11.0).color(color));
}

pub fn metric_card(
    ui: &mut egui::Ui,
    view: &MonitorView,
    key: &MetricKey,
    title: &str,
    subtitle: &str,
    color: Color32,
) -> egui::Response {
    let reading = view.metrics.get(key);
    let (status, status_color) = state_label(view, reading);
    let frame = theme::card().show(ui, |ui| {
        ui.set_min_height(140.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).size(16.0).color(color));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(status).size(11.0).color(status_color));
            });
        });
        let unit = reading.map(|r| r.descriptor.unit).unwrap_or(Unit::Percent);
        ui.label(
            RichText::new(format_value(reading.and_then(|r| r.value), unit))
                .size(30.0)
                .strong(),
        );
        ui.label(RichText::new(subtitle).size(11.0).color(theme::MUTED));
        sparkline(ui, view, key, color);
        metric_status(ui, view, key);
    });
    frame
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn sparkline(ui: &mut egui::Ui, view: &MonitorView, key: &MetricKey, color: Color32) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 38.0), egui::Sense::hover());
    let Some(points) = view.history.get(key) else {
        return;
    };
    let maximum = if view
        .metrics
        .get(key)
        .is_some_and(|v| v.descriptor.unit == Unit::Percent)
    {
        100.0
    } else {
        points
            .iter()
            .filter_map(|p| p.value)
            .fold(1.0_f64, f64::max)
            * 1.15
    };
    let mut previous = None;
    for point in points.iter().filter(|p| p.elapsed >= view.elapsed - 60.0) {
        if let Some(value) = point.value {
            let x =
                rect.left() + ((point.elapsed - view.elapsed + 60.0) / 60.0) as f32 * rect.width();
            let y = rect.bottom() - (value / maximum) as f32 * rect.height();
            let position = egui::pos2(x, y);
            if let Some(old) = previous {
                ui.painter()
                    .line_segment([old, position], Stroke::new(1.8, color));
            }
            previous = Some(position);
        } else {
            previous = None;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn history_chart(
    ui: &mut egui::Ui,
    view: &MonitorView,
    id: &str,
    title: &str,
    unit: Unit,
    range: u64,
    series: &[(&MetricKey, &str, Color32)],
    height: f32,
) {
    // Scroll content can report a width larger than its viewport after a wide
    // child. Keep charts inside the visible page and account for card padding.
    let width = ui
        .available_width()
        .min(ui.clip_rect().right() - ui.cursor().left());
    theme::card().show(ui, |ui| {
        ui.set_width((width - 36.0).max(120.0));
        ui.label(RichText::new(title).size(16.0).strong());
        let mut plot = Plot::new(id)
            .width(ui.available_width())
            .height(height)
            .legend(Legend::default())
            .include_y(0.0)
            .include_x(-(range as f64))
            .include_x(0.0)
            .allow_drag(false)
            .allow_zoom(false)
            .x_axis_formatter(|mark, _| {
                if mark.value.abs() < 0.5 {
                    "现在".into()
                } else {
                    format!("{:.0}s", mark.value)
                }
            })
            .y_axis_formatter(move |mark, _| format_value(Some(mark.value), unit));
        if unit == Unit::Percent {
            plot = plot.include_y(100.0);
        }
        plot.show(ui, |plot_ui| {
            for (key, label, color) in series {
                let mut segment = Vec::new();
                if let Some(points) = view.history.get(key) {
                    for point in points {
                        if let Some(value) = point.value {
                            segment.push([point.elapsed - view.elapsed, value]);
                        } else if !segment.is_empty() {
                            plot_ui.line(
                                Line::new(*label, std::mem::take(&mut segment))
                                    .color(*color)
                                    .width(2.0),
                            );
                        }
                    }
                }
                if !segment.is_empty() {
                    plot_ui.line(Line::new(*label, segment).color(*color).width(2.0));
                }
            }
        });
        for (key, _, _) in series {
            metric_status(ui, view, key);
        }
        if series.iter().all(|(key, _, _)| {
            view.history
                .get(*key)
                .is_none_or(|v| v.iter().all(|p| p.value.is_none()))
        }) {
            ui.label(
                RichText::new("等待有效采样；数据不可用时曲线保留缺口")
                    .size(12.0)
                    .color(theme::MUTED),
            );
        }
    });
}

pub fn device_details(ui: &mut egui::Ui, view: &MonitorView, device: &str) {
    if let Some(device) = view.devices.get(device) {
        ui.label(RichText::new(&device.name).size(16.0).strong());
        if !device.online {
            ui.label(RichText::new("设备已断开").color(theme::MUTED));
        }
        for (name, value) in &device.details {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(name).color(theme::MUTED));
                ui.label(value);
            });
        }
    }
}

pub fn device_selector(
    ui: &mut egui::Ui,
    view: &MonitorView,
    id: &str,
    selected: &mut String,
    kind: DeviceKind,
) -> bool {
    let old = selected.clone();
    let text = view
        .devices
        .get(selected)
        .map(|d| format!("{}{}", d.name, if d.online { "" } else { "（已断开）" }))
        .unwrap_or_else(|| "等待发现设备".into());
    egui::ComboBox::from_id_salt(id)
        .width(360.0)
        .selected_text(text)
        .show_ui(ui, |ui| {
            for device in view.devices.values().filter(|d| d.kind == kind) {
                ui.selectable_value(
                    selected,
                    device.id.clone(),
                    format!(
                        "{}{}",
                        device.name,
                        if device.online { "" } else { "（已断开）" }
                    ),
                );
            }
        });
    old != *selected
}
