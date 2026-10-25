//! MINT 酱, the mascot: a teal-haired cat girl whose pixel sprites `build.rs` generates
//! from the illustrations in `assets/mascot`, one per mood. Her expression and
//! speech bubble summarise the current CPU and memory load, so the sidebar doubles as a
//! health indicator.

use crate::{i18n, pixel, theme};
use eframe::egui::{self, Color32, FontId, Rect, Shape, Vec2, pos2, vec2};
use qqq_domain::SampleState;
use qqq_engine::MonitorView;

mod sprites {
    include!(concat!(env!("OUT_DIR"), "/mascot_sprites.rs"));
}

const BAND: Color32 = Color32::from_rgb(31, 92, 112);
const BAND_DOT: Color32 = Color32::from_rgb(48, 132, 154);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mood {
    Waiting,
    Relaxed,
    Calm,
    Busy,
    Overloaded,
    Trouble,
}

impl Mood {
    /// Number `n` of the illustration `mint-n.png` drawn for this mood.
    fn illustration(self) -> usize {
        match self {
            Self::Relaxed => 1,
            Self::Trouble => 2,
            Self::Busy => 3,
            Self::Calm => 4,
            Self::Overloaded => 5,
            Self::Waiting => 6,
        }
    }
}

/// Bust sprite texture of a mood, uploaded on first use and kept in the context.
/// Nearest sampling keeps every sprite pixel square when drawn at a whole number of
/// physical pixels.
fn texture(ctx: &egui::Context, mood: Mood) -> egui::TextureHandle {
    let n = mood.illustration();
    let id = egui::Id::new(("mascot", n));
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return texture;
    }
    let texture = ctx.load_texture(
        format!("mascot-bust-{n}"),
        egui::ColorImage::from_rgba_unmultiplied(sprites::BUST_SIZE, sprites::BUST[n - 1]),
        egui::TextureOptions::NEAREST,
    );
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    texture
}

/// Mood and speech from the live readings. Missing values never count as load.
fn assess(view: &MonitorView) -> (Mood, String) {
    let t = i18n::t();
    let ready = |device: &str| {
        view.reading(device, "usage")
            .filter(|r| r.effective_state(view.now) == SampleState::Ready)
            .and_then(|r| r.value)
    };
    let (Some(cpu), Some(memory)) = (ready("cpu"), ready("memory")) else {
        return (Mood::Waiting, t.mint_waiting.into());
    };
    if !view.diagnostics.collector_errors.is_empty() {
        (Mood::Trouble, t.mint_trouble.into())
    } else if cpu >= 85.0 {
        (Mood::Overloaded, (t.mint_cpu_hot)(cpu))
    } else if memory >= 90.0 {
        (Mood::Overloaded, (t.mint_memory_full)(memory))
    } else if cpu >= 50.0 || memory >= 75.0 {
        (Mood::Busy, (t.mint_busy)(cpu, memory))
    } else if cpu < 15.0 {
        (Mood::Relaxed, (t.mint_relaxed)(cpu))
    } else {
        (Mood::Calm, (t.mint_calm)(cpu, memory))
    }
}

/// Character card: a dark frame with a teal halftone band, like the banner behind the
/// character in her key art, and the sprite standing on the frame's bottom edge.
/// `bob` moves the sprite down by that much; the frame hides the overflow.
fn paint_card(painter: &egui::Painter, frame: Rect, unit: f32, mood: Mood, bob: f32) {
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
    let [width, height] = sprites::BUST_SIZE;
    let size = vec2(width as f32, height as f32) * unit;
    let origin = pixel::snap(
        ctx,
        pos2(
            inner.center().x - size.x / 2.0,
            inner.bottom() - size.y + bob,
        ),
    );
    painter
        .with_clip_rect(inner.intersect(painter.clip_rect()))
        .image(
            texture(ctx, mood).id(),
            Rect::from_min_size(origin, size),
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
}

/// Outer size of the character card around a sprite drawn at `unit` per sprite pixel.
fn card_size(ctx: &egui::Context, unit: f32) -> Vec2 {
    let border = pixel::unit(ctx, 2.0);
    let [width, height] = sprites::BUST_SIZE;
    vec2(width as f32, height as f32) * unit + vec2(border * 2.0 + 8.0, border * 2.0 + 6.0)
}

/// Height of the sidebar card around a bubble of `text_height`.
fn card_height(ctx: &egui::Context, text_height: f32) -> f32 {
    let unit = pixel::unit(ctx, 2.0);
    let bubble = text_height + (BUBBLE_PADDING + unit) * 2.0;
    let card = card_size(ctx, pixel::unit(ctx, 1.0));
    bubble + 3.0 * unit + 4.0 + card.y + 6.0 + NAME_HEIGHT
}

/// Room the sidebar keeps below the navigation for the card with a two-line bubble.
pub fn room(ui: &egui::Ui) -> f32 {
    let lines = ui
        .painter()
        .layout_no_wrap("M\nM".into(), FontId::proportional(12.0), theme::INK);
    card_height(ui.ctx(), lines.size().y) + CARD_GAP
}

const BUBBLE_PADDING: f32 = 8.0;
const NAME_HEIGHT: f32 = 16.0;
/// Least space left between the navigation and the card.
const CARD_GAP: f32 = 12.0;

/// Speech bubble, character card and name tag pinned to the bottom of the
/// sidebar. Skipped when the window is too short to fit them below the navigation.
pub fn show(ui: &mut egui::Ui, view: &MonitorView, animate: bool) {
    let ctx = ui.ctx().clone();
    let (mood, speech) = assess(view);
    let unit = pixel::unit(&ctx, 2.0);
    let sprite_unit = pixel::unit(&ctx, 1.0);
    let width = ui.available_width();
    let padding = BUBBLE_PADDING;
    let galley = ui.painter().layout(
        speech.clone(),
        FontId::proportional(12.0),
        theme::INK,
        width - (padding + unit) * 2.0,
    );
    let bubble_height = galley.size().y + (padding + unit) * 2.0;
    let card = card_size(&ctx, sprite_unit);
    let height = card_height(&ctx, galley.size().y);
    let available = ui.available_rect_before_wrap();
    if available.height() < height + CARD_GAP {
        return;
    }
    let rect = Rect::from_min_size(
        pos2(available.left(), available.bottom() - height),
        vec2(width, height),
    );
    let response = ui.allocate_rect(rect, egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &speech));
    response.on_hover_text(i18n::t().mint_hover);

    // Bob once per second, driven by a scheduled repaint instead of a continuous
    // frame loop.
    let time = ctx.input(|i| i.time);
    let bob = if animate && (time as u64).is_multiple_of(2) {
        sprite_unit
    } else {
        0.0
    };
    if animate {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(
            (1.0 - time.fract()).max(0.02),
        ));
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
        |x, y| match tail[y].as_bytes()[x] {
            b'K' => Some(theme::INK),
            b'W' => Some(theme::PAPER),
            _ => None,
        },
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
    paint_card(painter, frame, sprite_unit, mood, bob);
    painter.text(
        pos2(rect.center().x, rect.bottom()),
        egui::Align2::CENTER_BOTTOM,
        i18n::t().mint_name,
        FontId::proportional(11.0),
        theme::NIGHT_MUTED,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use qqq_engine::Language;

    /// Width of the speech text: the 172-point sidebar less its 30 points of margins, less
    /// the bubble's 2-point border and 8-point padding on both sides.
    const TEXT_WIDTH: f32 = 172.0 - 30.0 - 2.0 * (2.0 + 8.0);

    /// The Japanese interface font the platform layer prefers, where the system has it.
    fn system_font(language: Language) -> Option<(Vec<u8>, u32)> {
        let root = std::env::var_os("SystemRoot")?;
        let path = std::path::Path::new(&root)
            .join("Fonts")
            .join("YuGothM.ttc");
        (language == Language::Japanese)
            .then(|| std::fs::read(path).ok())
            .flatten()
            .map(|bytes| (bytes, 1))
    }

    #[test]
    fn every_speech_line_fits_the_bubble() {
        let mut overflowing = Vec::new();
        for language in Language::ALL {
            let ctx = egui::Context::default();
            theme::install_fonts(&ctx, system_font(language));
            // Fonts load on the first pass; its texture upload has no renderer to go to.
            ctx.run_ui(egui::RawInput::default(), |_| {})
                .textures_delta
                .clear();
            let t = i18n::strings(language);
            // The widest readings each mood can show, given the thresholds in `assess`.
            let speeches = [
                t.mint_waiting.to_owned(),
                t.mint_trouble.to_owned(),
                (t.mint_cpu_hot)(100.0),
                (t.mint_memory_full)(100.0),
                (t.mint_busy)(84.0, 89.0),
                (t.mint_relaxed)(14.0),
                (t.mint_calm)(49.0, 74.0),
            ];
            for line in speeches.iter().flat_map(|speech| speech.lines()) {
                let width = ctx.fonts_mut(|fonts| {
                    fonts
                        .layout_no_wrap(line.to_owned(), FontId::proportional(12.0), theme::INK)
                        .size()
                        .x
                });
                if width > TEXT_WIDTH {
                    overflowing.push(format!("{language:?} {line:?}: {width}"));
                }
            }
        }
        assert!(overflowing.is_empty(), "{overflowing:#?}");
    }
}
