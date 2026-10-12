//! Q酱, the pixel mascot: a teal-haired cat girl with a pink streak, a black
//! cat plush on her headband and a gold bell on her collar. Her expression and
//! speech bubble summarise the current CPU and memory load, so the sidebar
//! doubles as a health indicator.

use crate::{pixel, theme};
use eframe::egui::{self, Color32, FontId, Rect, Shape, Vec2, pos2, vec2};
use qqq_domain::SampleState;
use qqq_engine::MonitorView;

const WIDTH: usize = 24;
const HEIGHT: usize = 27;
const BLINK_PERIOD: f64 = 4.7;
const BAND: Color32 = Color32::from_rgb(31, 92, 112);
const BAND_DOT: Color32 = Color32::from_rgb(48, 132, 154);
const BASE: [&str; HEIGHT] = [
    "..K..................K..",
    "..KK................KK..",
    "..KHK.....B..B.....KHK..",
    "..KHWK...KBBBBK...KWHK..",
    "..KHWWK..BWBBWB..KWWHK..",
    "..KHWWWK.BBPPBB.KWWWHK..",
    ".KHHWWWK..BBBB..KWWWHHK.",
    ".KHHHWWHKKKKKKKKHWWHHHK.",
    "KHHHHHWBBBBBBBBBBWHHHHHK",
    "KHHHhHHHPPHHHHHHHHHHHHHK",
    "KHHHHHPPHHHHdHHHHdHHHHHK",
    "KHHHdPPdHHHdSdHHdSdHHdHK",
    "KHHdSPpSdHdSSSddSSSdHdHK",
    "KHdSSpSSSdSSSSSSSSSSdSHK",
    "KHdSSSSSSSSSSSSSSSSSSdHK",
    "KHdSSSSSSSSSSSSSSSSSSdHK",
    "KHdSSSSSSSSSSSSSSSSSSdHK",
    "KHdSSSSSSSSSSSSSSSSSSdHK",
    "KHHdrrSSSSSSSSSSSSrrdHHK",
    "KHHdSSSSSSSSSSSSSSSSdHHK",
    "KHHHdsSSSSSSSSSSSSsdHHHK",
    "KHHHHdKKSSSSSSSSKKdHHHHK",
    "KHHHHHHKKsSSSSsKKHHHHHHK",
    "KHHHHHKWWKKSSKKWWKHHHHHK",
    "KHHHHKWWWWWKKWWWWWKHHHHK",
    "KHHHKWwWWWWGGWWWWwWKHHHK",
    "KHHKWWwWWWBgGBWWWwWWKHHK",
];
/// Top row of both eyes and their left columns; every eye pattern is 4 × 4.
const EYE_ROW: usize = 14;
const EYE_COLUMNS: [usize; 2] = [5, 15];
const MOUTH_ROW: usize = 19;
const MOUTH_COLUMN: usize = 10;
const EYE_OPEN: [&str; 4] = ["KKKK", "EWEE", "EEWE", "eEEe"];
const EYE_CLOSED: [&str; 4] = ["SSSS", "SSSS", "KKKK", "SSSS"];
const EYE_HAPPY: [&str; 4] = ["SSSS", "SKKS", "KSSK", "SSSS"];
const EYE_SQUEEZE_LEFT: [&str; 4] = ["KSSS", "SKKS", "KSSS", "SSSS"];
const EYE_SQUEEZE_RIGHT: [&str; 4] = ["SSSK", "SKKS", "SSSK", "SSSS"];
const MOUTH_GRIN: [&str; 2] = ["KMMK", "SKmS"];
const MOUTH_SMILE: [&str; 2] = ["KSSK", "SKKS"];
const MOUTH_OPEN: [&str; 2] = ["SMMS", "SMMS"];
const MOUTH_WAVE: [&str; 2] = ["SKSK", "KSKS"];
const MOUTH_FLAT: [&str; 2] = ["SKKS", "SSSS"];
const SWEAT: [(usize, usize, u8); 8] = [
    (19, 10, b'K'),
    (18, 11, b'K'),
    (19, 11, b'b'),
    (20, 11, b'K'),
    (18, 12, b'b'),
    (19, 12, b'W'),
    (20, 12, b'K'),
    (19, 13, b'b'),
];

fn color(code: u8) -> Option<Color32> {
    Some(match code {
        b'K' => theme::INK,
        b'H' => Color32::from_rgb(43, 181, 207),
        b'h' => Color32::from_rgb(155, 231, 242),
        b'd' => Color32::from_rgb(28, 134, 166),
        b'P' => theme::PINK,
        b'p' => Color32::from_rgb(214, 52, 112),
        b'W' => Color32::WHITE,
        b'w' => Color32::from_rgb(206, 222, 232),
        b'B' => Color32::from_rgb(46, 50, 66),
        b'S' => Color32::from_rgb(255, 234, 222),
        b's' => Color32::from_rgb(245, 201, 182),
        b'E' => Color32::from_rgb(194, 30, 90),
        b'e' => Color32::from_rgb(255, 127, 168),
        b'r' => Color32::from_rgb(255, 175, 195),
        b'M' => Color32::from_rgb(150, 36, 66),
        b'm' => Color32::from_rgb(255, 120, 140),
        b'G' => theme::GOLD,
        b'g' => Color32::from_rgb(184, 128, 26),
        b'b' => Color32::from_rgb(150, 215, 255),
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mood {
    Waiting,
    Relaxed,
    Calm,
    Busy,
    Overloaded,
    Trouble,
}

struct Face {
    left: &'static [&'static str; 4],
    right: &'static [&'static str; 4],
    mouth: &'static [&'static str; 2],
    sweat: bool,
}

impl Mood {
    fn face(self, blink: bool) -> Face {
        let (left, right, mouth, sweat) = match self {
            Self::Waiting => (&EYE_CLOSED, &EYE_CLOSED, &MOUTH_FLAT, false),
            Self::Relaxed => (&EYE_HAPPY, &EYE_HAPPY, &MOUTH_GRIN, false),
            Self::Calm => (&EYE_OPEN, &EYE_OPEN, &MOUTH_SMILE, false),
            Self::Busy => (&EYE_OPEN, &EYE_OPEN, &MOUTH_OPEN, true),
            Self::Overloaded => (&EYE_SQUEEZE_LEFT, &EYE_SQUEEZE_RIGHT, &MOUTH_WAVE, true),
            Self::Trouble => (&EYE_OPEN, &EYE_OPEN, &MOUTH_FLAT, true),
        };
        let (left, right) = if blink && *left == EYE_OPEN {
            (&EYE_CLOSED, &EYE_CLOSED)
        } else {
            (left, right)
        };
        Face {
            left,
            right,
            mouth,
            sweat,
        }
    }
}

impl Face {
    fn pixel(&self, x: usize, y: usize) -> Option<Color32> {
        if self.sweat
            && let Some((_, _, code)) = SWEAT.iter().find(|(sx, sy, _)| (*sx, *sy) == (x, y))
        {
            return color(*code);
        }
        for (rows, left) in [(self.left, EYE_COLUMNS[0]), (self.right, EYE_COLUMNS[1])] {
            if (EYE_ROW..EYE_ROW + 4).contains(&y) && (left..left + 4).contains(&x) {
                return color(rows[y - EYE_ROW].as_bytes()[x - left]);
            }
        }
        if (MOUTH_ROW..MOUTH_ROW + 2).contains(&y) && (MOUTH_COLUMN..MOUTH_COLUMN + 4).contains(&x)
        {
            return color(self.mouth[y - MOUTH_ROW].as_bytes()[x - MOUTH_COLUMN]);
        }
        color(BASE[y].as_bytes()[x])
    }
}

/// Mood and speech from the live readings. Missing values never count as load.
fn assess(view: &MonitorView) -> (Mood, String) {
    let ready = |device: &str| {
        view.reading(device, "usage")
            .filter(|r| r.effective_state(view.now) == SampleState::Ready)
            .and_then(|r| r.value)
    };
    let (Some(cpu), Some(memory)) = (ready("cpu"), ready("memory")) else {
        return (Mood::Waiting, "正在唤醒传感器…\n请稍等一下～".into());
    };
    if !view.diagnostics.collector_errors.is_empty() {
        (Mood::Trouble, "有些数据采集不到…\n去「设置」看看吧".into())
    } else if cpu >= 85.0 {
        (
            Mood::Overloaded,
            format!("CPU {cpu:.0}%！\n忙得要冒烟啦 >_<"),
        )
    } else if memory >= 90.0 {
        (
            Mood::Overloaded,
            format!("内存用了 {memory:.0}%\n快要装不下啦！"),
        )
    } else if cpu >= 50.0 || memory >= 75.0 {
        (
            Mood::Busy,
            format!("有点忙哦～\nCPU {cpu:.0}%  内存 {memory:.0}%"),
        )
    } else if cpu < 15.0 {
        (
            Mood::Relaxed,
            format!("好悠闲～CPU 才 {cpu:.0}%\n一起喝杯茶吧"),
        )
    } else {
        (
            Mood::Calm,
            format!("一切正常！\nCPU {cpu:.0}%  内存 {memory:.0}%"),
        )
    }
}

/// Character card: a dark frame with a teal halftone band, like the banner
/// behind the character in her key art, and the bust clipped to the frame.
/// `bob` moves the bust down by that much; the frame hides the overflow.
fn paint_card(painter: &egui::Painter, frame: Rect, sprite_unit: f32, face: &Face, bob: f32) {
    let ctx = painter.ctx();
    let frame = pixel::snap_rect(ctx, frame);
    let border = pixel::unit(ctx, 2.0);
    let dot = pixel::unit(ctx, 1.0);
    let mut shapes = vec![pixel::panel(
        ctx,
        frame,
        theme::NIGHT_PANEL,
        theme::NIGHT_EDGE,
        Some((theme::NIGHT_SHADOW, 3.0)),
    )];
    let inner = frame.shrink(border);
    let band = pixel::snap_rect(
        ctx,
        Rect::from_x_y_ranges(
            inner.x_range(),
            inner.top() + inner.height() * 0.36..=inner.top() + inner.height() * 0.64,
        ),
    );
    shapes.push(Shape::rect_filled(band, 0.0, BAND));
    let mut y = band.top() + dot;
    let mut row = 0;
    while y + dot <= band.bottom() {
        let mut x = band.left() + if row % 2 == 0 { dot } else { dot * 3.0 };
        while x + dot <= band.right() {
            shapes.push(Shape::rect_filled(
                Rect::from_min_size(pos2(x, y), Vec2::splat(dot)),
                0.0,
                BAND_DOT,
            ));
            x += dot * 4.0;
        }
        y += dot * 2.0;
        row += 1;
    }
    painter.extend(shapes);
    let size = vec2(WIDTH as f32, HEIGHT as f32) * sprite_unit;
    let origin = pixel::snap(
        ctx,
        pos2(
            inner.center().x - size.x / 2.0,
            inner.bottom() - size.y + bob,
        ),
    );
    let mut sprite = Vec::new();
    pixel::sprite(&mut sprite, origin, sprite_unit, [WIDTH, HEIGHT], |x, y| {
        face.pixel(x, y)
    });
    painter
        .with_clip_rect(inner.intersect(painter.clip_rect()))
        .extend(sprite);
}

/// Outer size of the character card around a bust drawn at `sprite_unit`.
fn card_size(ctx: &egui::Context, sprite_unit: f32) -> Vec2 {
    let border = pixel::unit(ctx, 2.0);
    vec2(WIDTH as f32, HEIGHT as f32) * sprite_unit
        + vec2(border * 2.0 + 12.0, border * 2.0 + sprite_unit * 2.0)
}

/// Speech bubble, character card and name tag pinned to the bottom of the
/// sidebar. Skipped when the window is too short to fit them below the navigation.
pub fn show(ui: &mut egui::Ui, view: &MonitorView, animate: bool) {
    let ctx = ui.ctx().clone();
    let (mood, speech) = assess(view);
    let unit = pixel::unit(&ctx, 2.0);
    let sprite_unit = pixel::unit(&ctx, 3.0);
    let width = ui.available_width();
    let padding = 8.0;
    let galley = ui.painter().layout(
        speech.clone(),
        FontId::proportional(12.0),
        theme::INK,
        width - (padding + unit) * 2.0,
    );
    let bubble_height = galley.size().y + (padding + unit) * 2.0;
    let card = card_size(&ctx, sprite_unit);
    let name_height = 16.0;
    let height = bubble_height + 3.0 * unit + 4.0 + card.y + 6.0 + name_height;
    let available = ui.available_rect_before_wrap();
    if available.height() < height + 12.0 {
        return;
    }
    let rect = Rect::from_min_size(
        pos2(available.left(), available.bottom() - height),
        vec2(width, height),
    );
    let response = ui.allocate_rect(rect, egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &speech));
    response.on_hover_text("看板娘 Q 酱会根据 CPU 与内存负载切换表情");

    // Blink briefly every few seconds and bob once per second; both are
    // driven by scheduled repaints instead of a continuous frame loop.
    let time = ctx.input(|i| i.time);
    let phase = time % BLINK_PERIOD;
    let blink = animate && phase < 0.16;
    let bob = if animate && (time as u64).is_multiple_of(2) {
        sprite_unit
    } else {
        0.0
    };
    if animate {
        let next = if blink {
            0.16 - phase
        } else {
            BLINK_PERIOD - phase
        };
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(next.max(0.02)));
    }

    let mut shapes = Vec::new();
    let bubble = pixel::snap_rect(
        &ctx,
        Rect::from_min_size(rect.min, vec2(width, bubble_height)),
    );
    shapes.push(pixel::panel(
        &ctx,
        bubble,
        theme::PAPER,
        theme::INK,
        Some((theme::TEAL, 3.0)),
    ));
    // Tail pointing at the mascot, overwriting the bubble's bottom border.
    let tail = ["KWWWWK", ".KWWK.", "..KK.."];
    pixel::sprite(
        &mut shapes,
        pixel::snap(
            &ctx,
            pos2(bubble.center().x - 3.0 * unit, bubble.bottom() - unit),
        ),
        unit,
        [6, 3],
        |x, y| color(tail[y].as_bytes()[x]),
    );
    let painter = ui.painter();
    painter.extend(shapes);
    painter.galley(
        bubble.min + vec2(padding + unit, padding + unit),
        galley,
        theme::INK,
    );
    let frame = Rect::from_min_size(
        pos2(
            rect.center().x - card.x / 2.0,
            bubble.bottom() + 3.0 * unit + 4.0,
        ),
        card,
    );
    paint_card(painter, frame, sprite_unit, &mood.face(blink), bob);
    painter.text(
        pos2(rect.center().x, rect.bottom()),
        egui::Align2::CENTER_BOTTOM,
        "看板娘 · Q 酱",
        FontId::proportional(11.0),
        theme::NIGHT_MUTED,
    );
}

/// Larger static character card for the settings page.
pub fn portrait(ui: &mut egui::Ui, view: &MonitorView, logical_unit: f32) -> egui::Response {
    let unit = pixel::unit(ui.ctx(), logical_unit);
    let (rect, response) = ui.allocate_exact_size(card_size(ui.ctx(), unit), egui::Sense::hover());
    paint_card(ui.painter(), rect, unit, &assess(view).0.face(false), 0.0);
    response
}
