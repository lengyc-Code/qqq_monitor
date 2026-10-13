//! Compact status bar shown while the window is docked to a screen edge:
//! CPU, memory, GPU and graphics memory, and the frame rate of the foreground
//! application while frame monitoring is on. The top bar shows pixel numbers
//! over segmented level strips; the narrow side bar shows them as pixel dials.
//! Dragging the bar moves the window; double-clicking it or pressing the
//! expand button restores the full window.

use crate::{i18n, pixel, theme};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Shape, Vec2, pos2, vec2};
use qqq_domain::SampleState;
use qqq_engine::{DockEdge, MonitorView};

/// Frame rate that lights a whole strip or ring.
const FULL_FPS: f64 = 120.0;

struct Gauge {
    icon: &'static pixel::Icon,
    label: &'static str,
    tone: theme::Tone,
    value: Option<f64>,
    /// The value is a percentage rather than frames per second.
    percent: bool,
    /// Current rather than stale or unavailable.
    live: bool,
}

impl Gauge {
    /// Lit share of the strip or ring, 0 to 100.
    fn level(&self) -> Option<f64> {
        if self.percent {
            self.value
        } else {
            self.value.map(|fps| (fps / FULL_FPS * 100.0).min(100.0))
        }
    }

    /// Color of the lit segments: warm for a high load or a low frame rate.
    fn color(&self) -> Color32 {
        match self.value {
            Some(value) if self.live && self.percent => theme::level_color(value, self.tone.main),
            Some(fps) if self.live => theme::fps_color(fps, self.tone.main),
            _ => theme::NIGHT_MUTED,
        }
    }

    /// The value as the top bar prints it.
    fn text(&self) -> String {
        match self.value {
            Some(value) if self.percent => format!("{value:.0}%"),
            Some(fps) => format!("{fps:.0}"),
            None => "—".into(),
        }
    }
}

/// Latest value of a metric and whether it is current; stale values stay visible but dimmed.
fn reading(view: &MonitorView, device: &str, metric: &str) -> (Option<f64>, bool) {
    match view.reading(device, metric) {
        Some(r) => match r.effective_state(view.now) {
            SampleState::Ready => (r.value, true),
            SampleState::Stale => (r.value, false),
            _ => (None, false),
        },
        None => (None, false),
    }
}

fn gauges(view: &MonitorView, gpu: &str, frames: bool) -> Vec<Gauge> {
    let t = i18n::t();
    let gauge = |icon, label, tone, (value, live)| Gauge {
        icon,
        label,
        tone,
        value,
        percent: true,
        live,
    };
    let mut gauges = vec![
        gauge(
            &pixel::ICON_CPU,
            "CPU",
            theme::CPU,
            reading(view, "cpu", "usage"),
        ),
        gauge(
            &pixel::ICON_MEMORY,
            t.memory_short,
            theme::MEMORY,
            reading(view, "memory", "usage"),
        ),
        gauge(
            &pixel::ICON_GPU,
            "GPU",
            theme::GPU,
            reading(view, gpu, "usage"),
        ),
        gauge(
            &pixel::ICON_MEMORY,
            t.gpu_memory_short,
            theme::VRAM,
            reading(view, gpu, "memory_usage"),
        ),
    ];
    if frames {
        gauges.push(Gauge {
            percent: false,
            ..gauge(
                &pixel::ICON_FRAMES,
                "FPS",
                theme::CPU,
                reading(view, "frames", "fps"),
            )
        });
    }
    gauges
}

/// Draws the bar into the whole window, with the readings of graphics card `gpu` and,
/// with `frames`, the frame rate. Returns true when the full window should come back.
pub fn show(
    ui: &mut egui::Ui,
    view: &MonitorView,
    edge: DockEdge,
    gpu: &str,
    frames: bool,
) -> bool {
    let mut restore = false;
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::NIGHT))
        .show(ui, |ui| {
            let ctx = ui.ctx().clone();
            let rect = ui.max_rect();
            let gauges = gauges(view, gpu, frames);
            let response = ui
                .interact(rect, ui.id().with("dock-bar"), Sense::click_and_drag())
                .on_hover_cursor(egui::CursorIcon::Grab);
            if response.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            restore |= response.double_clicked();
            let summary = gauges
                .iter()
                .map(|g| format!("{} {}", g.label, g.text()))
                .collect::<Vec<_>>()
                .join(i18n::t().list_separator);
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &summary));

            let border = pixel::unit(&ctx, 2.0);
            let mut shapes = vec![pixel::panel(&ctx, rect, theme::NIGHT, theme::TEAL, None)];
            let inner = rect.shrink(border);
            let button = match edge {
                DockEdge::Top => horizontal(ui, &mut shapes, inner, &gauges),
                DockEdge::Left | DockEdge::Right => vertical(ui, &mut shapes, inner, &gauges),
            };
            ui.painter().extend(shapes);
            restore |= ui
                .scope_builder(egui::UiBuilder::new().max_rect(button), |ui| {
                    let label = i18n::t().restore_window;
                    theme::icon_button(ui, &pixel::ICON_EXPAND, label, button.size(), true)
                })
                .inner
                .clicked();
        });
    restore
}

/// One row: grip, the gauges with icon, label, value and level strip, then the
/// expand button. Returns the button's rect.
fn horizontal(ui: &egui::Ui, shapes: &mut Vec<Shape>, inner: Rect, gauges: &[Gauge]) -> Rect {
    let ctx = ui.ctx();
    let unit = pixel::unit(ctx, 2.0);
    let grip = Rect::from_min_size(inner.min, vec2(16.0, inner.height()));
    grip_dots(ctx, shapes, grip.center(), [2, 4]);
    let button = Rect::from_center_size(
        pos2(inner.right() - 16.0, inner.center().y),
        vec2(26.0, 24.0),
    );
    let cells = Rect::from_x_y_ranges(grip.right()..=button.left() - 4.0, inner.y_range());
    let width = cells.width() / gauges.len() as f32;
    let value_unit = pixel::unit(ctx, 2.0);
    for (index, gauge) in gauges.iter().enumerate() {
        let cell = Rect::from_min_size(
            pos2(cells.left() + index as f32 * width, cells.top()),
            vec2(width, cells.height()),
        );
        if index > 0 {
            separator(ctx, shapes, cell.left(), cell.y_range().shrink(6.0), true);
        }
        let row = cell.top() + 12.0;
        let x = icon_label(ui, shapes, pos2(cell.left() + 8.0, row), gauge);
        let text = gauge.text();
        let size = pixel::text_size(&text, value_unit);
        let value = pos2(
            (cell.right() - 8.0 - size.x).max(x + 6.0),
            row - size.y / 2.0,
        );
        value_text(ctx, shapes, value, &text, value_unit, gauge);
        let strip = Rect::from_min_max(
            pos2(cell.left() + 8.0, cell.bottom() - 4.0 - unit * 1.5),
            pos2(cell.right() - 8.0, cell.bottom() - 4.0),
        );
        level_strip(ctx, shapes, strip, gauge);
    }
    button
}

/// Grip, the dials stacked, then the expand button. Returns the button's rect.
fn vertical(ui: &egui::Ui, shapes: &mut Vec<Shape>, inner: Rect, gauges: &[Gauge]) -> Rect {
    let ctx = ui.ctx();
    let grip = Rect::from_min_size(inner.min, vec2(inner.width(), 14.0));
    grip_dots(ctx, shapes, grip.center(), [4, 2]);
    let footer = Rect::from_min_max(pos2(inner.left(), inner.bottom() - 32.0), inner.max);
    let button = Rect::from_center_size(footer.center(), vec2(26.0, 24.0));
    for y in [grip.bottom(), footer.top()] {
        separator(ctx, shapes, y, inner.x_range().shrink(6.0), false);
    }
    let cells = Rect::from_x_y_ranges(inner.x_range(), grip.bottom()..=footer.top());
    let height = cells.height() / gauges.len() as f32;
    let radius = inner.width() / 2.0 - 2.0;
    for (index, gauge) in gauges.iter().enumerate() {
        let top = cells.top() + index as f32 * height;
        dial(
            ui,
            shapes,
            pos2(cells.center().x, top + 5.0 + radius),
            radius,
            gauge,
        );
    }
    button
}

/// Pixel dial: a 270° ring open at the bottom whose segments light up to the
/// value, with the number inside and the label in the opening.
fn dial(ui: &egui::Ui, shapes: &mut Vec<Shape>, center: Pos2, radius: f32, gauge: &Gauge) {
    const SEGMENTS: f32 = 10.0;
    const SWEEP: f32 = 270.0;
    let ctx = ui.ctx();
    let cell = pixel::unit(ctx, 1.0);
    let cells = (radius * 2.0 / cell).ceil() as usize;
    let origin = pixel::snap(ctx, center - Vec2::splat(cells as f32 * cell / 2.0));
    let middle = cells as f32 / 2.0;
    let outer = radius / cell;
    let inner = outer - (4.0 / cell).max(3.0);
    let lit = gauge
        .level()
        .map_or(0.0, |p| (p / 100.0 * SEGMENTS as f64).ceil() as f32);
    let color = gauge.color();
    pixel::sprite(shapes, origin, cell, [cells, cells], |x, y| {
        let (dx, dy) = (x as f32 + 0.5 - middle, y as f32 + 0.5 - middle);
        let distance = dx.hypot(dy);
        if distance > outer || distance < inner {
            return None;
        }
        // Degrees clockwise from the lower-left end of the arc.
        let angle = (dy.atan2(dx).to_degrees() - 135.0).rem_euclid(360.0);
        if angle > SWEEP {
            return None;
        }
        let position = angle / SWEEP * SEGMENTS;
        // A narrow gap closes each segment, like the cells of a battery meter.
        if position.fract() > 0.8 && position < SEGMENTS - 0.5 {
            return None;
        }
        Some(if position.floor() < lit {
            color
        } else {
            theme::NIGHT_HOVER
        })
    });
    let text = gauge
        .value
        .map_or_else(|| "—".into(), |v| format!("{v:.0}"));
    let hole = 2.0 * inner * cell - 6.0;
    let unit = pixel::unit(ctx, pixel::fitting_unit(ctx, &text, hole, &[2.0, 1.5, 1.0]));
    let size = pixel::text_size(&text, unit);
    value_text(
        ctx,
        shapes,
        center - vec2(size.x / 2.0, size.y / 2.0 + unit),
        &text,
        unit,
        gauge,
    );
    let label = ui.painter().layout_no_wrap(
        gauge.label.to_owned(),
        FontId::proportional(11.0),
        theme::NIGHT_MUTED,
    );
    shapes.push(Shape::galley(
        pos2(
            center.x - label.size().x / 2.0,
            center.y + radius * 0.78 - label.size().y / 2.0,
        ),
        label,
        theme::NIGHT_MUTED,
    ));
}

/// Icon and label centred vertically on `at.y`; returns the label's right edge.
fn icon_label(ui: &egui::Ui, shapes: &mut Vec<Shape>, at: Pos2, gauge: &Gauge) -> f32 {
    let ctx = ui.ctx();
    let icon_unit = pixel::unit(ctx, 1.5);
    let origin = pixel::snap(ctx, pos2(at.x, at.y - 4.5 * icon_unit));
    pixel::icon_shapes(
        shapes,
        origin,
        icon_unit,
        gauge.icon,
        theme::NIGHT_TEXT,
        gauge.tone.main,
    );
    let galley = ui.painter().layout_no_wrap(
        gauge.label.to_owned(),
        FontId::proportional(12.0),
        theme::NIGHT_MUTED,
    );
    let label = pos2(
        origin.x + 9.0 * icon_unit + 5.0,
        at.y - galley.size().y / 2.0,
    );
    let right = label.x + galley.size().x;
    shapes.push(Shape::galley(label, galley, theme::NIGHT_MUTED));
    right
}

fn value_text(
    ctx: &egui::Context,
    shapes: &mut Vec<Shape>,
    at: Pos2,
    text: &str,
    unit: f32,
    gauge: &Gauge,
) {
    let at = pixel::snap(ctx, at);
    if gauge.live {
        pixel::text_shapes(shapes, at + vec2(0.0, unit), text, unit, gauge.tone.main);
        pixel::text_shapes(shapes, at, text, unit, theme::NIGHT_TEXT);
    } else {
        pixel::text_shapes(shapes, at, text, unit, theme::NIGHT_MUTED);
    }
}

/// Segmented level strip; unknown values leave every segment dark.
fn level_strip(ctx: &egui::Context, shapes: &mut Vec<Shape>, rect: Rect, gauge: &Gauge) {
    let unit = pixel::unit(ctx, 2.0);
    let rect = pixel::snap_rect(ctx, rect);
    let pitch = unit * 3.0;
    let segments = ((rect.width() + unit) / pitch).floor().max(1.0) as usize;
    let lit = gauge
        .level()
        .map(|p| (p / 100.0 * segments as f64).ceil() as usize)
        .unwrap_or(0)
        .min(segments);
    let color = gauge.color();
    for index in 0..segments {
        let segment = Rect::from_min_size(
            pos2(rect.left() + index as f32 * pitch, rect.top()),
            vec2(unit * 2.0, rect.height()),
        );
        shapes.push(Shape::rect_filled(
            segment,
            0.0,
            if index < lit {
                color
            } else {
                theme::NIGHT_PANEL
            },
        ));
    }
}

/// Dotted handle showing the bar can be dragged; `grid` is columns × rows.
fn grip_dots(ctx: &egui::Context, shapes: &mut Vec<Shape>, center: Pos2, grid: [usize; 2]) {
    let dot = pixel::unit(ctx, 2.0);
    let pitch = dot * 2.0;
    let size = vec2(grid[0] as f32, grid[1] as f32) * pitch - egui::Vec2::splat(dot);
    let origin = pixel::snap(ctx, center - size / 2.0);
    for column in 0..grid[0] {
        for row in 0..grid[1] {
            shapes.push(Shape::rect_filled(
                Rect::from_min_size(
                    origin + vec2(column as f32 * pitch, row as f32 * pitch),
                    egui::Vec2::splat(dot),
                ),
                0.0,
                theme::NIGHT_EDGE,
            ));
        }
    }
}

/// Two-pixel divider, vertical at `x` or horizontal at `y`.
fn separator(
    ctx: &egui::Context,
    shapes: &mut Vec<Shape>,
    at: f32,
    span: egui::Rangef,
    vertical: bool,
) {
    let width = pixel::unit(ctx, 2.0);
    let rect = if vertical {
        Rect::from_x_y_ranges(at - width / 2.0..=at + width / 2.0, span)
    } else {
        Rect::from_x_y_ranges(span, at - width / 2.0..=at + width / 2.0)
    };
    shapes.push(Shape::rect_filled(
        pixel::snap_rect(ctx, rect),
        0.0,
        theme::NIGHT_EDGE,
    ));
}
