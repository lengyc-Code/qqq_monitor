use crate::{pixel, theme};
use eframe::egui::{self, Color32, FontId, RichText, Sense, pos2, vec2};
use egui_plot::{HoverPosition, Line, Plot};
use qqq_domain::{DeviceDescriptor, DeviceKind, MetricKey, MetricReading, SampleState, Unit};
use qqq_engine::MonitorView;

/// Widest strings the big pixel numbers must fit, so a row of cards shares one size.
pub const RATE_TEMPLATE: &str = "1023.9 KiB/s";
pub const BYTES_TEMPLATE: &str = "1023.9 MiB";

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
        Some(SampleState::Ready) => ("实时", theme::GOOD),
        Some(SampleState::Stale) => ("数据过期", theme::WARNING),
        Some(SampleState::Failed) => ("采集失败", theme::ERROR),
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

/// Square pixel dot followed by the reading state; meant for right-to-left rows.
pub fn status_badge(ui: &mut egui::Ui, view: &MonitorView, reading: Option<&MetricReading>) {
    let (status, color) = state_label(view, reading);
    ui.spacing_mut().item_spacing.x = 4.0;
    ui.label(RichText::new(status).size(11.0).color(color));
    pixel_dot(ui, color);
}

pub fn pixel_dot(ui: &mut egui::Ui, color: Color32) {
    let size = pixel::unit(ui.ctx(), 2.0) * 3.0;
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(size), Sense::hover());
    ui.painter()
        .rect_filled(pixel::snap_rect(ui.ctx(), rect), 0.0, color);
}

/// Soft-tinted label with an optional pixel icon, used as a card heading.
pub fn tag(
    ui: &mut egui::Ui,
    icon: Option<&pixel::Icon>,
    text: &str,
    tone: theme::Tone,
) -> egui::Response {
    let ctx = ui.ctx().clone();
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_owned(), FontId::proportional(13.0), theme::INK);
    let icon_unit = pixel::unit(&ctx, 1.5);
    let icon_width = if icon.is_some() {
        9.0 * icon_unit + 6.0
    } else {
        0.0
    };
    let size = vec2(galley.size().x + icon_width + 18.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let mut shapes = vec![pixel::panel(&ctx, rect, tone.soft, theme::INK, None)];
    let mut x = rect.left() + 9.0;
    if let Some(icon) = icon {
        let origin = pixel::snap(&ctx, pos2(x, rect.center().y - 4.5 * icon_unit));
        pixel::icon_shapes(&mut shapes, origin, icon_unit, icon, theme::INK, tone.main);
        x += icon_width;
    }
    let painter = ui.painter();
    painter.extend(shapes);
    painter.galley(
        pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        theme::INK,
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    response
}

/// Section heading with a pixel icon and an optional muted hint.
pub fn section_title(ui: &mut egui::Ui, icon: &pixel::Icon, title: &str, hint: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        pixel::icon(ui, icon, 1.5, theme::INK, theme::PINK);
        ui.label(RichText::new(title).size(theme::SECTION_TITLE).strong());
        if !hint.is_empty() {
            ui.label(RichText::new(hint).size(12.0).color(theme::MUTED));
        }
    });
}

/// Large bitmap-font number, sized so that `template` would still fit.
pub fn big_value(ui: &mut egui::Ui, text: &str, shadow: Color32, template: &str) -> egui::Response {
    let unit = pixel::fitting_unit(ui.ctx(), template, ui.available_width(), &[3.0, 2.0]);
    pixel::label(ui, text, unit, theme::INK, pixel::Effect::Drop(shadow))
}

/// Overview-style metric card: big current value, a detail line and the last minute as pixel bars.
pub struct MetricCard<'a> {
    pub title: &'a str,
    pub icon: &'a pixel::Icon,
    pub tone: theme::Tone,
    /// The first series is the headline value; a second one is mirrored below the bars' axis.
    pub series: &'a [(&'a MetricKey, Color32)],
    pub detail: String,
    pub template: &'a str,
}

impl MetricCard<'_> {
    pub fn show(&self, ui: &mut egui::Ui, view: &MonitorView, clickable: bool) -> egui::Response {
        let key = self.series[0].0;
        let reading = view.metrics.get(key);
        let mut card = theme::card();
        if clickable {
            card = card.clickable();
        }
        card.show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.horizontal(|ui| {
                tag(ui, Some(self.icon), self.title, self.tone);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    status_badge(ui, view, reading);
                });
            });
            let unit = reading.map(|r| r.descriptor.unit).unwrap_or(Unit::Percent);
            big_value(
                ui,
                &format_value(reading.and_then(|r| r.value), unit),
                self.tone.main,
                self.template,
            );
            ui.label(RichText::new(&self.detail).size(12.0).color(theme::MUTED));
            mini_bars(ui, view, self.series, 34.0);
            metric_status(ui, view, key);
        })
        .response
    }
}

/// Compact card with a big pixel value, a muted detail line and an optional usage bar.
pub fn stat_card(
    ui: &mut egui::Ui,
    heading: (&pixel::Icon, &str, theme::Tone),
    value: &str,
    detail: &str,
    bar: Option<(f64, Color32)>,
) -> egui::Response {
    let (icon, title, tone) = heading;
    theme::card()
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            tag(ui, Some(icon), title, tone);
            big_value(ui, value, tone.main, BYTES_TEMPLATE);
            ui.add(egui::Label::new(RichText::new(detail).size(12.0).color(theme::MUTED)).wrap());
            match bar {
                Some((percent, color)) => segment_bar(ui, Some(percent), color, 16.0),
                None => ui.add_space(16.0),
            }
        })
        .response
}

/// The last 60 seconds as pixel columns. One series grows from the floor; a
/// second series is mirrored downwards from a shared axis on the same scale.
pub fn mini_bars(
    ui: &mut egui::Ui,
    view: &MonitorView,
    series: &[(&MetricKey, Color32)],
    height: f32,
) {
    const WINDOW: f64 = 60.0;
    let ctx = ui.ctx().clone();
    let unit = pixel::unit(&ctx, 2.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let rect = pixel::snap_rect(&ctx, rect);
    let pitch = unit * 3.0;
    let columns = ((rect.width() + unit) / pitch).floor().max(1.0) as usize;
    let start = view.elapsed - WINDOW;
    let buckets: Vec<Vec<Option<f64>>> = series
        .iter()
        .map(|(key, _)| {
            let mut buckets = vec![None; columns];
            for point in view.history.get(*key).into_iter().flatten() {
                if let Some(value) = point.value.filter(|_| point.elapsed >= start) {
                    let index = (((point.elapsed - start) / WINDOW) * columns as f64) as usize;
                    let slot = &mut buckets[index.min(columns - 1)];
                    *slot = Some(slot.map_or(value, |old: f64| old.max(value)));
                }
            }
            buckets
        })
        .collect();
    let percent = view
        .metrics
        .get(series[0].0)
        .is_some_and(|r| r.descriptor.unit == Unit::Percent);
    let maximum = if percent {
        100.0
    } else {
        buckets
            .iter()
            .flatten()
            .flatten()
            .fold(1.0_f64, |a, &b| a.max(b))
            * 1.1
    };
    let mirrored = series.len() > 1;
    let axis = if mirrored {
        pixel::snap(&ctx, pos2(0.0, rect.center().y)).y
    } else {
        rect.bottom() - unit
    };
    let levels = if mirrored {
        ((axis - rect.top()) / unit).floor()
    } else {
        ((axis - rect.top()) / unit).floor() + 1.0
    };
    let mut shapes = Vec::new();
    for column in 0..columns {
        let x = rect.left() + column as f32 * pitch;
        shapes.push(egui::Shape::rect_filled(
            egui::Rect::from_min_size(pos2(x, axis), vec2(unit, unit)),
            0.0,
            theme::LINE,
        ));
        for (index, ((_, color), values)) in series.iter().zip(&buckets).enumerate().take(2) {
            let Some(value) = values[column].filter(|v| *v > 0.0) else {
                continue;
            };
            let steps = ((value / maximum).min(1.0) * levels as f64).ceil().max(1.0) as f32;
            let bar = if index == 0 {
                egui::Rect::from_min_max(
                    pos2(x, axis + if mirrored { 0.0 } else { unit } - steps * unit),
                    pos2(x + unit * 2.0, axis + if mirrored { 0.0 } else { unit }),
                )
            } else {
                egui::Rect::from_min_max(
                    pos2(x, axis + unit),
                    pos2(x + unit * 2.0, axis + unit + steps * unit),
                )
            };
            shapes.push(egui::Shape::rect_filled(bar, 0.0, *color));
        }
    }
    ui.painter().extend(shapes);
}

/// HP-style segmented bar for a percentage; unknown values draw an empty bar.
pub fn segment_bar(ui: &mut egui::Ui, percent: Option<f64>, color: Color32, height: f32) {
    let ctx = ui.ctx().clone();
    let unit = pixel::unit(&ctx, 2.0);
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let rect = pixel::snap_rect(&ctx, rect);
    let mut shapes = vec![pixel::panel(&ctx, rect, theme::WELL, theme::INK, None)];
    let inner = rect.shrink(unit * 2.0);
    let segments = ((inner.width() / (unit * 5.0)).floor() as usize).clamp(8, 40);
    let gap = unit;
    let width = (inner.width() - gap * (segments - 1) as f32) / segments as f32;
    let lit = percent
        .map(|p| (p / 100.0 * segments as f64).ceil() as usize)
        .unwrap_or(0)
        .min(segments);
    for index in 0..segments {
        let left = pixel::snap(&ctx, pos2(inner.left() + index as f32 * (width + gap), 0.0)).x;
        let right = pixel::snap(&ctx, pos2(left + width, 0.0)).x;
        let segment = egui::Rect::from_x_y_ranges(left..=right, inner.y_range());
        shapes.push(egui::Shape::rect_filled(
            segment,
            0.0,
            if index < lit { color } else { theme::LINE },
        ));
        if index < lit {
            // One-pixel highlight on top of each lit segment.
            shapes.push(egui::Shape::rect_filled(
                egui::Rect::from_min_size(segment.min, vec2(segment.width(), unit)),
                0.0,
                theme::lerp(color, Color32::WHITE, 0.45),
            ));
        }
    }
    ui.painter().extend(shapes);
    if let Some(percent) = percent {
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::ProgressIndicator,
                true,
                format!("{percent:.1}%"),
            )
        });
    }
}

/// Right-to-left legend of the heat colors used by [`heat_tile`].
pub fn heat_legend(ui: &mut egui::Ui) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.label(RichText::new("100%").size(11.0).color(theme::MUTED));
        for level in [100.0, 75.0, 50.0, 25.0, 0.0] {
            swatch(ui, theme::core_fill(level));
        }
        ui.label(RichText::new("0%").size(11.0).color(theme::MUTED));
    });
}

/// The graphics card with the most memory, which is the discrete card when there is one.
pub fn primary_gpu(view: &MonitorView) -> Option<&DeviceDescriptor> {
    let total = |d: &DeviceDescriptor| view.value(&d.id, "memory_total").unwrap_or(0.0);
    view.devices
        .values()
        .filter(|d| d.kind == DeviceKind::Gpu && d.online)
        .max_by(|a, b| total(a).total_cmp(&total(b)))
}

pub fn swatch(ui: &mut egui::Ui, color: Color32) {
    let unit = pixel::unit(ui.ctx(), 2.0);
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(unit * 5.0), Sense::hover());
    let rect = pixel::snap_rect(ui.ctx(), rect);
    ui.painter().rect_filled(rect, 0.0, theme::INK);
    ui.painter().rect_filled(rect.shrink(unit), 0.0, color);
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
) -> egui::Response {
    theme::card()
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).size(theme::SECTION_TITLE).strong());
                // Legend with current values instead of an overlay covering the newest data.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for (key, label, color) in series.iter().rev() {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        ui.label(
                            RichText::new(format_value(
                                view.metrics.get(key).and_then(|r| r.value),
                                unit,
                            ))
                            .size(12.0)
                            .strong(),
                        );
                        ui.label(RichText::new(*label).size(12.0).color(theme::MUTED));
                        swatch(ui, *color);
                        ui.add_space(8.0);
                    }
                });
            });
            let mut plot = Plot::new(id)
                .width(ui.available_width())
                .height(height)
                .show_background(false)
                .grid_color(theme::LINE)
                .grid_spacing(egui::Rangef::new(45.0, 180.0))
                .cursor_color(theme::PINK)
                .include_y(0.0)
                .include_x(-(range as f64))
                .include_x(0.0)
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .x_axis_formatter(|mark, _| {
                    if mark.value.abs() < 0.5 {
                        "现在".into()
                    } else {
                        format!("{:.0}s", mark.value)
                    }
                })
                .y_axis_formatter(move |mark, _| format_value(Some(mark.value), unit))
                .label_formatter(move |position| match position {
                    HoverPosition::NearDataPoint {
                        plot_name,
                        position,
                        ..
                    } => Some(format!(
                        "{plot_name}  {}\n{:.0} 秒前",
                        format_value(Some(position.y), unit),
                        -position.x
                    )),
                    HoverPosition::Elsewhere { .. } => None,
                });
            if unit == Unit::Percent {
                plot = plot.include_y(100.0);
            }
            plot.show(ui, |plot_ui| {
                let line = |label: &str, points: Vec<[f64; 2]>, color: Color32| {
                    Line::new(label, points)
                        .color(color)
                        .width(2.0)
                        .fill(0.0)
                        .fill_alpha(0.12)
                };
                for (key, label, color) in series {
                    let mut segment = Vec::new();
                    for point in view.history.get(*key).into_iter().flatten() {
                        if let Some(value) = point.value {
                            segment.push([point.elapsed - view.elapsed, value]);
                        } else if !segment.is_empty() {
                            plot_ui.line(line(label, std::mem::take(&mut segment), *color));
                        }
                    }
                    if !segment.is_empty() {
                        plot_ui.line(line(label, segment, *color));
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
        })
        .response
}

/// Icon, muted label and value on one line; long values are truncated with a hover tooltip.
pub fn info_row(
    ui: &mut egui::Ui,
    icon: &pixel::Icon,
    tone: theme::Tone,
    label: &str,
    value: &str,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        pixel::icon(ui, icon, 1.5, theme::INK, tone.main);
        ui.add_sized(
            [60.0, 18.0],
            egui::Label::new(RichText::new(label).size(12.0).color(theme::MUTED)),
        );
        ui.add(egui::Label::new(RichText::new(value).size(13.0)).truncate());
    });
}

pub fn device_details(ui: &mut egui::Ui, view: &MonitorView, device: &str, show_name: bool) {
    if let Some(device) = view.devices.get(device) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            if show_name || !device.online {
                ui.horizontal_wrapped(|ui| {
                    if show_name {
                        ui.label(
                            RichText::new(&device.name)
                                .size(theme::SECTION_TITLE)
                                .strong(),
                        );
                    }
                    if !device.online {
                        ui.label(RichText::new("设备已断开").size(12.0).color(theme::MUTED));
                    }
                });
            }
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                for (name, value) in &device.details {
                    let mut text = egui::text::LayoutJob::default();
                    for (part, color) in [
                        (format!("{name}  "), theme::MUTED),
                        (value.clone(), theme::INK),
                    ] {
                        text.append(
                            &part,
                            0.0,
                            egui::TextFormat {
                                font_id: egui::FontId::proportional(12.0),
                                color,
                                ..Default::default()
                            },
                        );
                    }
                    ui.label(text);
                }
            });
        });
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
        .width(300.0_f32.min(ui.available_width()))
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

/// Device picker card: a tagged selector followed by the device's details.
pub fn device_picker(
    ui: &mut egui::Ui,
    view: &MonitorView,
    id: &str,
    selected: &mut String,
    kind: DeviceKind,
    heading: (&pixel::Icon, &str, theme::Tone),
) -> bool {
    theme::card()
        .show(ui, |ui| {
            let changed = ui
                .horizontal(|ui| {
                    tag(ui, Some(heading.0), heading.1, heading.2);
                    device_selector(ui, view, id, selected, kind)
                })
                .inner;
            device_details(ui, view, selected, false);
            changed
        })
        .inner
}

/// Clickable pixel tile whose heat-colored level rises with a load; `label`
/// sits top left in the bitmap font when it can, the value bottom right.
pub fn heat_tile(
    ui: &mut egui::Ui,
    label: &str,
    name: &str,
    value: Option<f64>,
    selected: bool,
) -> egui::Response {
    let ctx = ui.ctx().clone();
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 54.0), Sense::click());
    let text = format_value(value, Unit::Percent);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            true,
            selected,
            format!("{name} {text}"),
        )
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let lift = pixel::unit(&ctx, 3.0);
    let border = pixel::unit(&ctx, 2.0);
    let small = pixel::unit(&ctx, 1.5);
    let face = pixel::snap_rect(
        &ctx,
        egui::Rect::from_min_max(rect.min, rect.max - egui::Vec2::splat(lift)),
    );
    let hovered = response.hovered() || response.has_focus();
    let shadow = if selected {
        theme::PINK
    } else if hovered {
        theme::PINK_SOFT
    } else {
        theme::SHADOW
    };
    let mut shapes = vec![pixel::panel(
        &ctx,
        face,
        theme::PAPER,
        if selected { theme::PINK } else { theme::INK },
        Some((shadow, 3.0)),
    )];
    let inner = face.shrink(border);
    if let Some(value) = value.filter(|v| *v > 0.0) {
        let height = (inner.height() * (value / 100.0).min(1.0) as f32 / border).ceil() * border;
        shapes.push(egui::Shape::rect_filled(
            egui::Rect::from_min_max(pos2(inner.left(), inner.bottom() - height), inner.max),
            0.0,
            theme::core_fill(value),
        ));
    }
    let corner = pixel::snap(&ctx, inner.min + vec2(6.0, 6.0));
    if pixel::supports(label) {
        pixel::text_shapes(&mut shapes, corner, label, small, theme::INK);
    } else {
        let galley =
            ui.painter()
                .layout_no_wrap(label.to_owned(), FontId::proportional(12.0), theme::INK);
        // Centred on the line a bitmap label would occupy.
        let top = corner.y + 3.5 * small - galley.size().y / 2.0;
        shapes.push(egui::Shape::galley(pos2(corner.x, top), galley, theme::INK));
    }
    let value_size = pixel::text_size(&text, border);
    pixel::text_shapes(
        &mut shapes,
        pixel::snap(
            &ctx,
            pos2(
                inner.right() - 6.0 - value_size.x,
                inner.bottom() - 6.0 - value_size.y,
            ),
        ),
        &text,
        border,
        theme::INK,
    );
    if selected {
        pixel::icon_shapes(
            &mut shapes,
            pixel::snap(
                &ctx,
                pos2(inner.right() - 4.0 - 9.0 * small, inner.top() + 4.0),
            ),
            small,
            &pixel::ICON_HEART,
            theme::INK,
            theme::PINK,
        );
    }
    ui.painter().extend(shapes);
    if response.has_focus() {
        ui.painter().rect_stroke(
            face.expand(2.0),
            0.0,
            egui::Stroke::new(2.0, theme::BLUE),
            egui::StrokeKind::Outside,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
