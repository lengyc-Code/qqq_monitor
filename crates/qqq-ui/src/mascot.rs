//! Q酱, the pixel mascot: a 40 × 48 chibi bust of a teal-haired cat girl with
//! a pink streak, a black cat plush on her headband and a gold bell on her
//! collar. Her expression and speech bubble summarise the current CPU and
//! memory load, so the sidebar doubles as a health indicator.

use crate::{pixel, theme};
use eframe::egui::{self, Color32, FontId, Rect, Shape, Vec2, pos2, vec2};
use qqq_domain::SampleState;
use qqq_engine::MonitorView;

const WIDTH: usize = 40;
const HEIGHT: usize = 48;
const BLINK_PERIOD: f64 = 4.7;
const BAND: Color32 = Color32::from_rgb(31, 92, 112);
const BAND_DOT: Color32 = Color32::from_rgb(48, 132, 154);
/// Chibi bust without eyes and mouth: a large head with cat ears and fur
/// tufts, ahoge, headband with the cat plush, pink streak and braid, over a
/// simplified collar with gem and bell.
const BASE: [&str; 48] = [
    ".KHKK............KKHK...K.....K....KKHK.",
    ".KHHHK..........KHHK...KNK...KNK..KHHHK.",
    ".KHHHHKK.......KHKK....KNNKKKNNK.KHHHHK.",
    ".KHDHHHHK......KHKhK..KNNNNNNNNnKHHHDHK.",
    "KHHDDHHHHKK.....KHhKKKKNWWNNNWWnKHHDDHHK",
    "KHHDDDDHHHHKK.KKPPhBHHKNWKNNNKWnKDDDDHHK",
    "KHHDDDDDHHHHHKHBPPBBBBKNNNPNPNNnKDDDDHHK",
    "KHHDDDDDDDBBBBBPPHHHHHKNNNNPNNNnKDDDDHHK",
    "KHDDDDDDDBBBHhhPPhhhhhhKNNNNNNnKDDDDDDHK",
    "KHDDKDDHBBBhhHHPPHHHHHHHKKKKKKKBHDDKDDHK",
    "KHDKWKDBBBhHHHpPHHHHHHHHHHHHhhBBBDKWKDHK",
    "HHKWWWKBBHHHHHpPHHHHHHHHHHHHHHHBBKWWWKHH",
    "HKWWWWWKHHHHHpPPHHHHHHHHhHHHHHHHKWWWWWKH",
    "KWWWwWWKHHHdHpPhHHHdHHHdhHHdHHHHKWWwWWWK",
    "HKWwwWWWKHHdpPPhHHHdHHHdhHHdHHHKWWWwwWKH",
    "KWWwWWwKHHHHpPPhdHHHdHHHhHHHdHHHKwWWwWWK",
    ".KWwWWwKHHHHpPHHdHHHdHHHdHHHdHHHKwWWwWK.",
    "..KWWwKHHHHpPPHHdHHHdHHHdHHHdHHHHKwWWK..",
    "...KwKHHHHHpPPHHHdHHHdHHHdHHHdHHHHKwK...",
    "....KHHHHHpHPdHHHdHHHdHHHdHHHdHHHHHK....",
    "....KHHdHHpPPHHHSHHHSHHHSHHHHHHHdHHK....",
    "....KHHdHHHPPHHHSHHHSHHHSHHHHHHHdHHK....",
    "...KHHHdHHHHSHHSSSHSSSHSSSHHHHHHdHHHK...",
    "...KHHHdHHHHSSHSSSSSSSSSSSHSHHHHdHHHK...",
    "...KHHHdHHHSSSSSSSSSSSSSSSSSSHHHdHHHK...",
    "...KHHHdHHHSSSSSSSSSSSSSSSSSSHhHdHHHK...",
    "...KHHHdHHHSSSSSSSSSSSSSSSSSSHhHdHHHK...",
    "..KHHHHdHHHSSSSSSSSSSSSSSSSSSHhHdHHHHK..",
    "..KHdHHdHHHSSSSSSSSSSSSSSSSSSHhHdHHdHK..",
    "..KHdHHdHHHSSSSSSSSSSSSSSSSSSHhHdHHdHK..",
    "..KHdHHdHHHSSSSSSSSSSSSSSSSSSHhHdHHdHK..",
    "..KddHHdHHrrrSSSSSSsSSSSSSSrrrHHdHHddK..",
    ".KHdddHdHHHHSSSSSSSSSSSSSSSSHHHHdHdddHK.",
    ".KddddKHKHHHSSSSSSSSSSSSSSSSHHHHdHddddK.",
    ".KdddKHHdKHHHSSSSSSSSSSSSSSHHHHHdHddddK.",
    ".KdddKdhHKHHHHSSSSSSSSSSSSHHHHHHdHddddK.",
    ".KdddKHHdKHHHHHSSSSSSSSSSHHHHHHHddddddK.",
    ".KddddKDKHHHHHHHSSSSSSSSHHHHHHHHddddddK.",
    ".KdddKDhDKHHHHHHHHSSSSHHHHHHHHHHddddddK.",
    ".KddddKDKHHHHHHHHHHHHHHHHHHHHHHHddddddK.",
    ".KdddKHHdKHHHHHHHHHHHHHHHHHHHHHHddddddK.",
    "KddddKdhHKHHHHKKKHHcCHHKKKHHHHHHdddddddK",
    "KdddddKhKHHHHHKWWKKCCKKWWKHHHHHHdddddddK",
    "KddddddKHKKKKKWWWWWKKWWWWWKKKKKHdddddddK",
    "KddddddKKWwwWWWWWWGYGGWWWWWWwwWKKddddddK",
    "KddddKKwwwWWWWwwwwGGGgwwwwWWWWwwwKKddddK",
    "KddKKwwWWWWWWWWWWWWggWWWWWWWWWWWWwwKKddK",
    "KdKWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWKdK",
];
/// Eyes are 7 × 7 from these columns; `.` keeps the face underneath.
const EYE_ROW: usize = 23;
const EYE_COLUMNS: [usize; 2] = [11, 22];
const MOUTH_ORIGIN: (usize, usize) = (18, 33);
const SWEAT_ORIGIN: (usize, usize) = (29, 17);
const EYE_OPEN_LEFT: [&str; 7] = [
    "KKKKKKK", "KEWWEEE", ".EWEEEE", ".RRReRR", ".RReWeR", ".eRReRe", "..eeee.",
];
const EYE_OPEN_RIGHT: [&str; 7] = [
    "KKKKKKK", "EEEWWEK", "EEEEWE.", "RReRRR.", "ReWeRR.", "eReRRe.", ".eeee..",
];
const EYE_CLOSED: [&str; 7] = [
    "SSSSSSS", "SSSSSSS", "SSSSSSS", "KKKKKKK", "SSSSSSS", "SSSSSSS", "SSSSSSS",
];
const EYE_HAPPY_LEFT: [&str; 7] = [
    "SSSSSSS", "SSSSSSS", "SSSKKSS", "SSKSSKS", "SKSSSSK", "SSSSSSS", "SSSSSSS",
];
const EYE_HAPPY_RIGHT: [&str; 7] = [
    "SSSSSSS", "SSSSSSS", "SSKKSSS", "SKSSKSS", "KSSSSKS", "SSSSSSS", "SSSSSSS",
];
const EYE_SQUEEZE_LEFT: [&str; 7] = [
    "SSSSSSS", "KKSSSSS", "SSKKSSS", "SSSSKKS", "SSKKSSS", "KKSSSSS", "SSSSSSS",
];
const EYE_SQUEEZE_RIGHT: [&str; 7] = [
    "SSSSSSS", "SSSSSKK", "SSSKKSS", "SKKSSSS", "SSSKKSS", "SSSSSKK", "SSSSSSS",
];
const MOUTH_SMILE: [&str; 2] = ["s..s", ".MM."];
const MOUTH_GRIN: [&str; 3] = ["KMMK", "KmmK", ".KK."];
const MOUTH_OPEN: [&str; 3] = [".MM.", "MmmM", ".MM."];
const MOUTH_WAVE: [&str; 2] = ["M.M.", ".M.M"];
const MOUTH_FLAT: [&str; 1] = [".MM."];
const SWEAT: [&str; 4] = [".K.", "KcK", "cWc", ".c."];

fn color(code: u8) -> Option<Color32> {
    Some(match code {
        b'K' => theme::INK,
        b'H' => Color32::from_rgb(43, 181, 207),
        b'h' => Color32::from_rgb(150, 230, 242),
        b'd' => Color32::from_rgb(24, 128, 160),
        b'D' => Color32::from_rgb(17, 88, 118),
        b'P' => theme::PINK,
        b'p' => Color32::from_rgb(206, 46, 108),
        b'W' => Color32::WHITE,
        b'w' => Color32::from_rgb(214, 226, 236),
        b'B' => Color32::from_rgb(40, 44, 60),
        b'N' => Color32::from_rgb(62, 68, 94),
        b'n' => Color32::from_rgb(98, 106, 140),
        b'S' => Color32::from_rgb(255, 236, 226),
        b's' => Color32::from_rgb(246, 204, 188),
        b'E' => Color32::from_rgb(178, 22, 84),
        b'R' => Color32::from_rgb(240, 64, 120),
        b'e' => Color32::from_rgb(255, 150, 186),
        b'r' => Color32::from_rgb(255, 178, 198),
        b'M' => Color32::from_rgb(150, 36, 66),
        b'm' => Color32::from_rgb(255, 128, 148),
        b'G' => theme::GOLD,
        b'g' => Color32::from_rgb(178, 122, 24),
        b'Y' => Color32::from_rgb(255, 230, 140),
        b'C' => Color32::from_rgb(64, 150, 240),
        b'c' => Color32::from_rgb(170, 220, 255),
        _ => return None,
    })
}

/// Code of a stamp placed at `origin` at (x, y), unless outside it or `.`.
fn stamp(rows: &[&str], origin: (usize, usize), x: usize, y: usize) -> Option<u8> {
    let code = *rows
        .get(y.checked_sub(origin.1)?)?
        .as_bytes()
        .get(x.checked_sub(origin.0)?)?;
    (code != b'.').then_some(code)
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
    left: &'static [&'static str],
    right: &'static [&'static str],
    mouth: &'static [&'static str],
    sweat: bool,
}

impl Mood {
    fn face(self, blink: bool) -> Face {
        let (left, right, mouth, sweat): (&[&str], &[&str], &[&str], bool) = match self {
            Self::Waiting => (&EYE_CLOSED, &EYE_CLOSED, &MOUTH_FLAT, false),
            Self::Relaxed => (&EYE_HAPPY_LEFT, &EYE_HAPPY_RIGHT, &MOUTH_GRIN, false),
            Self::Calm => (&EYE_OPEN_LEFT, &EYE_OPEN_RIGHT, &MOUTH_SMILE, false),
            Self::Busy => (&EYE_OPEN_LEFT, &EYE_OPEN_RIGHT, &MOUTH_OPEN, true),
            Self::Overloaded => (&EYE_SQUEEZE_LEFT, &EYE_SQUEEZE_RIGHT, &MOUTH_WAVE, true),
            Self::Trouble => (&EYE_OPEN_LEFT, &EYE_OPEN_RIGHT, &MOUTH_FLAT, true),
        };
        let (left, right) = if blink && left == EYE_OPEN_LEFT {
            (&EYE_CLOSED[..], &EYE_CLOSED[..])
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
        let code = self
            .sweat
            .then(|| stamp(&SWEAT, SWEAT_ORIGIN, x, y))
            .flatten()
            .or_else(|| stamp(self.left, (EYE_COLUMNS[0], EYE_ROW), x, y))
            .or_else(|| stamp(self.right, (EYE_COLUMNS[1], EYE_ROW), x, y))
            .or_else(|| stamp(self.mouth, MOUTH_ORIGIN, x, y))
            .unwrap_or(BASE[y].as_bytes()[x]);
        color(code)
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
    let sprite_unit = pixel::unit(&ctx, 2.0);
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
