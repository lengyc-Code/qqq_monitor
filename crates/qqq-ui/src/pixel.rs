//! Pixel-art primitives: art pixels snapped to whole physical pixels, notched
//! panels, a 5×7 bitmap font for Latin text and numbers, and string sprites.

use eframe::egui::{self, Color32, Pos2, Rect, Shape, Vec2, pos2, vec2};

/// Logical size of one art pixel, rounded to whole physical pixels so edges stay crisp.
pub fn unit(ctx: &egui::Context, logical: f32) -> f32 {
    let ppp = ctx.pixels_per_point();
    (logical * ppp).round().max(1.0) / ppp
}

pub fn snap(ctx: &egui::Context, pos: Pos2) -> Pos2 {
    let ppp = ctx.pixels_per_point();
    pos2((pos.x * ppp).round() / ppp, (pos.y * ppp).round() / ppp)
}

pub fn snap_rect(ctx: &egui::Context, rect: Rect) -> Rect {
    Rect::from_min_max(snap(ctx, rect.min), snap(ctx, rect.max))
}

/// A rectangle with its corner art-pixels removed, built from non-overlapping
/// strips so translucent colors stay even.
pub fn notched(shapes: &mut Vec<Shape>, rect: Rect, cut: f32, color: Color32) {
    if rect.width() <= cut * 2.0 || rect.height() <= cut * 2.0 {
        shapes.push(Shape::rect_filled(rect, 0.0, color));
        return;
    }
    let (l, t, r, b) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    for strip in [
        Rect::from_min_max(pos2(l + cut, t), pos2(r - cut, t + cut)),
        Rect::from_min_max(pos2(l, t + cut), pos2(r, b - cut)),
        Rect::from_min_max(pos2(l + cut, b - cut), pos2(r - cut, b)),
    ] {
        shapes.push(Shape::rect_filled(strip, 0.0, color));
    }
}

/// Outlined pixel panel with an optional hard drop shadow to the lower right.
pub fn panel(
    ctx: &egui::Context,
    rect: Rect,
    fill: Color32,
    outline: Color32,
    shadow: Option<(Color32, f32)>,
) -> Shape {
    let border = unit(ctx, 2.0);
    let rect = snap_rect(ctx, rect);
    let mut shapes = Vec::with_capacity(8);
    if let Some((color, lift)) = shadow
        && lift > 0.0
    {
        let lift = unit(ctx, lift);
        notched(&mut shapes, rect.translate(vec2(lift, lift)), border, color);
    }
    notched(&mut shapes, rect, border, outline);
    shapes.push(Shape::rect_filled(rect.shrink(border), 0.0, fill));
    Shape::Vec(shapes)
}

/// Paints a sprite row by row, merging equal neighbours into one rectangle.
pub fn sprite(
    shapes: &mut Vec<Shape>,
    origin: Pos2,
    unit: f32,
    size: [usize; 2],
    pixel: impl Fn(usize, usize) -> Option<Color32>,
) {
    for y in 0..size[1] {
        let mut x = 0;
        while x < size[0] {
            let Some(color) = pixel(x, y) else {
                x += 1;
                continue;
            };
            let start = x;
            while x < size[0] && pixel(x, y) == Some(color) {
                x += 1;
            }
            shapes.push(Shape::rect_filled(
                Rect::from_min_size(
                    origin + vec2(start as f32 * unit, y as f32 * unit),
                    vec2((x - start) as f32 * unit, unit),
                ),
                0.0,
                color,
            ));
        }
    }
}

/// A 9×9 icon: `#` uses the foreground color, `o` the accent color.
pub type Icon = [&'static str; 9];

pub const ICON_STAR: Icon = [
    "....#....",
    "...###...",
    "...#o#...",
    "####o####",
    ".#ooooo#.",
    "..#ooo#..",
    "..#o#o#..",
    ".##...##.",
    ".#.....#.",
];
pub const ICON_CPU: Icon = [
    "..#.#.#..",
    ".#######.",
    "##.....##",
    ".#.ooo.#.",
    "##.ooo.##",
    ".#.ooo.#.",
    "##.....##",
    ".#######.",
    "..#.#.#..",
];
pub const ICON_MEMORY: Icon = [
    ".........",
    ".........",
    "#########",
    "#.oo.oo.#",
    "#.oo.oo.#",
    "#.......#",
    "#########",
    ".#.#.#.#.",
    ".........",
];
pub const ICON_DISK: Icon = [
    "#######..",
    "#.#o.#.#.",
    "#.#o.#..#",
    "#.####..#",
    "#.......#",
    "#.ooooo.#",
    "#.o...o.#",
    "#.ooooo.#",
    "#########",
];
pub const ICON_NETWORK: Icon = [
    "..#......",
    ".#o#.....",
    "#####....",
    "..#...#..",
    "..#...#..",
    "..#...#..",
    "....#####",
    ".....#o#.",
    "......#..",
];
pub const ICON_GEAR: Icon = [
    "....#....",
    ".#.###.#.",
    "..#####..",
    ".##...##.",
    "###.o.###",
    ".##...##.",
    "..#####..",
    ".#.###.#.",
    "....#....",
];
pub const ICON_CLOCK: Icon = [
    "..#####..",
    ".#ooooo#.",
    "#ooo#ooo#",
    "#ooo#ooo#",
    "#ooo###o#",
    "#ooooooo#",
    "#ooooooo#",
    ".#ooooo#.",
    "..#####..",
];
pub const ICON_HEART: Icon = [
    ".........",
    ".##...##.",
    "#oo#.#oo#",
    "#ooo#ooo#",
    "#ooooooo#",
    ".#ooooo#.",
    "..#ooo#..",
    "...#o#...",
    "....#....",
];
pub const ICON_SPARKLE: Icon = [
    "....#....",
    "....#....",
    "...#o#...",
    "..#ooo#..",
    "##ooooo##",
    "..#ooo#..",
    "...#o#...",
    "....#....",
    "....#....",
];
pub const ICON_GPU: Icon = [
    ".........",
    "########.",
    "#......##",
    "#.ooo..##",
    "#.o.o..##",
    "#.ooo..##",
    "#......##",
    "########.",
    ".#.#.#...",
];
/// Diagonal arrows: grow the docked bar back into the full window.
pub const ICON_EXPAND: Icon = [
    "oooo.....",
    "oo.......",
    "o.#......",
    "o..#.....",
    ".........",
    ".....#..o",
    "......#.o",
    ".......oo",
    ".....oooo",
];
pub const ICON_PROCESS: Icon = [
    "#########",
    "#ooooooo#",
    "#########",
    "#.......#",
    "#.o.###.#",
    "#.......#",
    "#.o.###.#",
    "#.......#",
    "#########",
];
/// Two server units joined to the network.
pub const ICON_REMOTE: Icon = [
    "#########",
    "#o.o....#",
    "#########",
    "#o.o....#",
    "#########",
    "....#....",
    "....#....",
    "###ooo###",
    "...ooo...",
];
/// A game pad: the frames of the foreground game.
pub const ICON_FRAMES: Icon = [
    ".........",
    ".#######.",
    "#.o...o.#",
    "#ooo.o.o#",
    "#.o...o.#",
    "#.......#",
    "#..###..#",
    "#.#...#.#",
    ".#.....#.",
];
/// The mascot's bell: alerts.
pub const ICON_BELL: Icon = [
    "....#....",
    "...###...",
    "..#ooo#..",
    "..#ooo#..",
    ".#ooooo#.",
    ".#ooooo#.",
    "#ooooooo#",
    "#########",
    "...#o#...",
];
/// A toolbox: the tools page.
pub const ICON_TOOLS: Icon = [
    "...###...",
    "..#...#..",
    "#########",
    "#ooooooo#",
    "####o####",
    "#ooo#ooo#",
    "#ooooooo#",
    "#ooooooo#",
    "#########",
];
/// A camera: screenshots.
pub const ICON_CAMERA: Icon = [
    "...###...",
    "#########",
    "#.......#",
    "#..###..#",
    "#.#ooo#.#",
    "#.#ooo#.#",
    "#..###..#",
    "#.......#",
    "#########",
];
/// A clipboard with a pinned note: clipboard history.
pub const ICON_CLIPBOARD: Icon = [
    "...###...",
    ".##ooo##.",
    ".#.....#.",
    ".#.###.#.",
    ".#.....#.",
    ".#.###.#.",
    ".#.....#.",
    ".#.##..#.",
    ".#######.",
];
pub const ICON_DOCK_TOP: Icon = [
    "#########",
    "#ooooooo#",
    "#ooooooo#",
    "#.......#",
    "#.......#",
    "#.......#",
    "#.......#",
    "#########",
    "...###...",
];
pub const ICON_DOCK_LEFT: Icon = [
    "#########",
    "#oo.....#",
    "#oo.....#",
    "#oo.....#",
    "#oo.....#",
    "#oo.....#",
    "#oo.....#",
    "#########",
    "...###...",
];
pub const ICON_DOCK_RIGHT: Icon = [
    "#########",
    "#.....oo#",
    "#.....oo#",
    "#.....oo#",
    "#.....oo#",
    "#.....oo#",
    "#.....oo#",
    "#########",
    "...###...",
];

pub fn icon_shapes(
    shapes: &mut Vec<Shape>,
    origin: Pos2,
    unit: f32,
    icon: &Icon,
    fg: Color32,
    accent: Color32,
) {
    sprite(shapes, origin, unit, [9, 9], |x, y| {
        match icon[y].as_bytes()[x] {
            b'#' => Some(fg),
            b'o' => Some(accent),
            _ => None,
        }
    });
}

/// Draws an icon centered in a rect that is allocated in the current layout.
pub fn icon(
    ui: &mut egui::Ui,
    icon: &Icon,
    logical_unit: f32,
    fg: Color32,
    accent: Color32,
) -> egui::Response {
    let unit = unit(ui.ctx(), logical_unit);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(9.0 * unit), egui::Sense::hover());
    let mut shapes = Vec::new();
    icon_shapes(
        &mut shapes,
        snap(ui.ctx(), rect.min),
        unit,
        icon,
        fg,
        accent,
    );
    ui.painter().extend(shapes);
    response
}

/// Rows of a 5×7 glyph; bit 4 is the leftmost column.
fn glyph(c: char) -> Option<[u8; 7]> {
    Some(match c {
        ' ' => [0; 7],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x0C, 0x04, 0x08],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        '%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03],
        '/' => [0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x00],
        '\\' => [0x00, 0x10, 0x08, 0x04, 0x02, 0x01, 0x00],
        '-' | '—' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        '=' => [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00],
        '_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F],
        '#' => [0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        '?' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
        '<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        '>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '~' => [0x00, 0x00, 0x08, 0x15, 0x02, 0x00, 0x00],
        '*' => [0x00, 0x04, 0x15, 0x0E, 0x15, 0x04, 0x00],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        'a' => [0x00, 0x00, 0x0E, 0x01, 0x0F, 0x11, 0x0F],
        'b' => [0x10, 0x10, 0x16, 0x19, 0x11, 0x11, 0x1E],
        'c' => [0x00, 0x00, 0x0E, 0x10, 0x10, 0x11, 0x0E],
        'd' => [0x01, 0x01, 0x0D, 0x13, 0x11, 0x11, 0x0F],
        'e' => [0x00, 0x00, 0x0E, 0x11, 0x1F, 0x10, 0x0E],
        'f' => [0x06, 0x09, 0x08, 0x1C, 0x08, 0x08, 0x08],
        'g' => [0x00, 0x0F, 0x11, 0x11, 0x0F, 0x01, 0x0E],
        'h' => [0x10, 0x10, 0x16, 0x19, 0x11, 0x11, 0x11],
        'i' => [0x04, 0x00, 0x0C, 0x04, 0x04, 0x04, 0x0E],
        'j' => [0x02, 0x00, 0x06, 0x02, 0x02, 0x12, 0x0C],
        'k' => [0x10, 0x10, 0x12, 0x14, 0x18, 0x14, 0x12],
        'l' => [0x0C, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'm' => [0x00, 0x00, 0x1A, 0x15, 0x15, 0x11, 0x11],
        'n' => [0x00, 0x00, 0x16, 0x19, 0x11, 0x11, 0x11],
        'o' => [0x00, 0x00, 0x0E, 0x11, 0x11, 0x11, 0x0E],
        'p' => [0x00, 0x00, 0x1E, 0x11, 0x1E, 0x10, 0x10],
        'q' => [0x00, 0x00, 0x0D, 0x13, 0x0F, 0x01, 0x01],
        'r' => [0x00, 0x00, 0x16, 0x19, 0x10, 0x10, 0x10],
        's' => [0x00, 0x00, 0x0E, 0x10, 0x0E, 0x01, 0x1E],
        't' => [0x08, 0x08, 0x1C, 0x08, 0x08, 0x09, 0x06],
        'u' => [0x00, 0x00, 0x11, 0x11, 0x11, 0x13, 0x0D],
        'v' => [0x00, 0x00, 0x11, 0x11, 0x11, 0x0A, 0x04],
        'w' => [0x00, 0x00, 0x11, 0x11, 0x15, 0x15, 0x0A],
        'x' => [0x00, 0x00, 0x11, 0x0A, 0x04, 0x0A, 0x11],
        'y' => [0x00, 0x00, 0x11, 0x11, 0x0F, 0x01, 0x0E],
        'z' => [0x00, 0x00, 0x1F, 0x02, 0x04, 0x08, 0x1F],
        _ => return None,
    })
}

/// First column and width of a glyph. Digits keep a fixed width so live
/// values do not jitter; other glyphs are trimmed to their ink.
fn glyph_span(c: char, rows: &[u8; 7]) -> (u32, u32) {
    let bits = rows.iter().fold(0, |all, row| all | row);
    if c.is_ascii_digit() {
        (0, 5)
    } else if bits == 0 {
        (0, 3)
    } else {
        let high = 7 - bits.leading_zeros();
        let low = bits.trailing_zeros();
        (4 - high, high - low + 1)
    }
}

pub fn supports(text: &str) -> bool {
    text.chars().all(|c| glyph(c).is_some())
}

/// Size of `text` in the bitmap font; unsupported characters are skipped.
pub fn text_size(text: &str, unit: f32) -> Vec2 {
    let columns = text
        .chars()
        .filter_map(|c| glyph(c).map(|rows| glyph_span(c, &rows).1 + 1))
        .sum::<u32>()
        .saturating_sub(1);
    vec2(columns as f32 * unit, 7.0 * unit)
}

pub fn text_shapes(shapes: &mut Vec<Shape>, origin: Pos2, text: &str, unit: f32, color: Color32) {
    let mut x = origin.x;
    for c in text.chars() {
        let Some(rows) = glyph(c) else {
            continue;
        };
        let (first, width) = glyph_span(c, &rows);
        sprite(
            shapes,
            pos2(x, origin.y),
            unit,
            [width as usize, 7],
            |column, row| ((rows[row] >> (4 - first - column as u32)) & 1 == 1).then_some(color),
        );
        x += (width + 1) as f32 * unit;
    }
}

/// Decoration behind bitmap text.
#[derive(Clone, Copy)]
pub enum Effect {
    Plain,
    /// One art pixel straight down, like an extruded sign.
    Drop(Color32),
}

/// Bitmap-font label. Text the font cannot draw, such as CJK, falls back to
/// the regular font at a similar size.
pub fn label(
    ui: &mut egui::Ui,
    text: &str,
    logical_unit: f32,
    color: Color32,
    effect: Effect,
) -> egui::Response {
    if !supports(text) {
        return ui.label(
            egui::RichText::new(text)
                .size(logical_unit * 6.0)
                .color(color)
                .strong(),
        );
    }
    let unit = unit(ui.ctx(), logical_unit);
    let drop = if matches!(effect, Effect::Drop(_)) {
        unit
    } else {
        0.0
    };
    let (rect, response) = ui.allocate_exact_size(
        text_size(text, unit) + vec2(0.0, drop),
        egui::Sense::hover(),
    );
    let origin = snap(ui.ctx(), rect.min);
    let mut shapes = Vec::new();
    if let Effect::Drop(shadow) = effect {
        text_shapes(&mut shapes, origin + vec2(0.0, unit), text, unit, shadow);
    }
    text_shapes(&mut shapes, origin, text, unit, color);
    ui.painter().extend(shapes);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    response
}

/// Largest art-pixel size (from `units`, descending) at which `text` fits.
pub fn fitting_unit(ctx: &egui::Context, text: &str, width: f32, units: &[f32]) -> f32 {
    units
        .iter()
        .copied()
        .find(|&logical| text_size(text, unit(ctx, logical)).x + unit(ctx, logical) <= width)
        .unwrap_or(units.last().copied().unwrap_or(1.0))
}
