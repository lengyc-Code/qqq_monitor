use super::super::DesktopApp;
use super::overview::logical_cores;
use crate::{components::*, pixel, theme};
use eframe::egui::{self, RichText, Sense, pos2, vec2};
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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                ui.label(RichText::new("100%").size(11.0).color(theme::MUTED));
                for level in [100.0, 75.0, 50.0, 25.0, 0.0] {
                    swatch(ui, theme::core_fill(level));
                }
                ui.label(RichText::new("0%").size(11.0).color(theme::MUTED));
            });
        });
        let columns = ((ui.available_width() / 110.0) as usize).clamp(2, 8);
        for row in cores.chunks(columns) {
            ui.columns(columns, |uis| {
                for (index, device) in row.iter().enumerate() {
                    let selected = self.core == device.id;
                    if core_tile(
                        &mut uis[index],
                        core_number(&device.id) + 1,
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

/// Pixel tile whose heat-colored level rises with the core's load.
fn core_tile(
    ui: &mut egui::Ui,
    number: usize,
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
    let label = format!("#{number:02}");
    pixel::text_shapes(
        &mut shapes,
        pixel::snap(&ctx, inner.min + vec2(6.0, 6.0)),
        &label,
        small,
        theme::INK,
    );
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
