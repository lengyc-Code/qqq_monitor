use crate::pixel;
use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Shadow, Stroke, TextStyle, vec2,
};
use std::sync::Arc;

// Palette taken from the mascot: teal hair, a hot-pink streak, the gold
// bell, the white blouse and the black outfit. Near-black ink draws outlines
// and text; bright tones are for fills, bars and chart lines, not small text.
pub const INK: Color32 = Color32::from_rgb(30, 34, 48);
pub const MUTED: Color32 = Color32::from_rgb(94, 106, 125);
pub const PAGE: Color32 = Color32::from_rgb(238, 248, 250);
pub const PAGE_DOT: Color32 = Color32::from_rgb(212, 237, 242);
pub const PAPER: Color32 = Color32::WHITE;
pub const WELL: Color32 = Color32::from_rgb(246, 251, 252);
pub const SHADOW: Color32 = Color32::from_rgb(186, 220, 229);
pub const LINE: Color32 = Color32::from_rgb(216, 235, 240);
pub const TEAL: Color32 = Color32::from_rgb(34, 184, 207);
pub const TEAL_SOFT: Color32 = Color32::from_rgb(211, 243, 248);
pub const PINK: Color32 = Color32::from_rgb(255, 79, 139);
pub const PINK_SOFT: Color32 = Color32::from_rgb(255, 222, 234);
pub const GOLD: Color32 = Color32::from_rgb(242, 182, 48);
pub const GOLD_SOFT: Color32 = Color32::from_rgb(255, 240, 200);
pub const BLUE: Color32 = Color32::from_rgb(64, 132, 240);
pub const BLUE_SOFT: Color32 = Color32::from_rgb(219, 233, 255);
/// Orange between gold and red for high but not critical levels.
pub const WARM: Color32 = Color32::from_rgb(255, 138, 61);
pub const GOOD: Color32 = Color32::from_rgb(14, 128, 110);
pub const WARNING: Color32 = Color32::from_rgb(180, 97, 14);
pub const ERROR: Color32 = Color32::from_rgb(208, 40, 79);
// Dark frame around the page (sidebar and status bar), like the black outfit.
pub const NIGHT: Color32 = Color32::from_rgb(27, 30, 43);
pub const NIGHT_PANEL: Color32 = Color32::from_rgb(40, 45, 64);
pub const NIGHT_HOVER: Color32 = Color32::from_rgb(52, 60, 86);
pub const NIGHT_EDGE: Color32 = Color32::from_rgb(70, 80, 108);
pub const NIGHT_SHADOW: Color32 = Color32::from_rgb(12, 13, 20);
pub const NIGHT_TEXT: Color32 = Color32::from_rgb(232, 246, 248);
pub const NIGHT_MUTED: Color32 = Color32::from_rgb(143, 163, 181);
pub const CARD_PADDING: f32 = 12.0;
pub const SECTION_GAP: f32 = 10.0;
pub const SECTION_TITLE: f32 = 14.0;
/// Hard drop-shadow distance of panels, in logical pixels.
pub const LIFT: f32 = 4.0;

/// Bright color for marks plus a soft tint for fills behind ink text.
#[derive(Clone, Copy)]
pub struct Tone {
    pub main: Color32,
    pub soft: Color32,
}
pub const CPU: Tone = Tone {
    main: TEAL,
    soft: TEAL_SOFT,
};
pub const MEMORY: Tone = Tone {
    main: PINK,
    soft: PINK_SOFT,
};
pub const DISK: Tone = Tone {
    main: GOLD,
    soft: GOLD_SOFT,
};
pub const NETWORK: Tone = Tone {
    main: BLUE,
    soft: BLUE_SOFT,
};
pub const ACCENT: Tone = Tone {
    main: PINK,
    soft: PINK_SOFT,
};

fn hard_shadow() -> Shadow {
    Shadow {
        offset: [3, 3],
        blur: 0,
        spread: 0,
        color: SHADOW,
    }
}

pub fn apply(ctx: &egui::Context, animations: bool) {
    ctx.set_theme(egui::Theme::Light);
    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.panel_fill = PAGE;
    style.visuals.window_fill = PAPER;
    style.visuals.override_text_color = Some(INK);
    style.visuals.weak_text_color = Some(MUTED);
    style.visuals.extreme_bg_color = WELL;
    style.visuals.faint_bg_color = WELL;
    style.visuals.code_bg_color = WELL;
    style.visuals.hyperlink_color = INK;
    style.visuals.warn_fg_color = WARNING;
    style.visuals.error_fg_color = ERROR;
    style.visuals.window_corner_radius = 0.into();
    style.visuals.menu_corner_radius = 0.into();
    style.visuals.window_stroke = Stroke::new(2.0, INK);
    style.visuals.window_shadow = hard_shadow();
    style.visuals.popup_shadow = hard_shadow();
    style.visuals.selection.bg_fill = TEAL_SOFT;
    style.visuals.selection.stroke = Stroke::new(2.0, INK);
    style.visuals.text_cursor.stroke = Stroke::new(2.0, PINK);
    for visuals in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        visuals.corner_radius = 0.into();
        visuals.fg_stroke = Stroke::new(1.5, INK);
        visuals.bg_stroke = Stroke::new(2.0, INK);
        visuals.expansion = 0.0;
    }
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(2.0, LINE);
    style.visuals.widgets.noninteractive.bg_fill = PAPER;
    style.visuals.widgets.inactive.bg_fill = PAPER;
    style.visuals.widgets.inactive.weak_bg_fill = PAPER;
    style.visuals.widgets.hovered.bg_fill = TEAL_SOFT;
    style.visuals.widgets.hovered.weak_bg_fill = TEAL_SOFT;
    style.visuals.widgets.active.bg_fill = TEAL;
    style.visuals.widgets.active.weak_bg_fill = TEAL;
    style.visuals.widgets.open.bg_fill = TEAL_SOFT;
    style.visuals.widgets.open.weak_bg_fill = TEAL_SOFT;
    style.spacing.item_spacing = vec2(10.0, 8.0);
    style.spacing.button_padding = vec2(10.0, 5.0);
    style.spacing.interact_size.y = 26.0;
    style.spacing.icon_width = 16.0;
    style.spacing.icon_width_inner = 8.0;
    style.animation_time = if animations { 0.18 } else { 0.0 };
    for (text_style, size) in [
        (TextStyle::Body, 14.0),
        (TextStyle::Button, 14.0),
        (TextStyle::Heading, 22.0),
        (TextStyle::Small, 11.0),
    ] {
        style
            .text_styles
            .insert(text_style, FontId::new(size, FontFamily::Proportional));
    }
    ctx.set_style_of(egui::Theme::Light, style);
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "NotoSC".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/NotoSansCJKsc-Regular.otf"
        ))),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "NotoSC".into());
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .push("NotoSC".into());
    ctx.set_fonts(fonts);
}

/// Outlined pixel panel with a hard shadow. Layout, hover and keyboard
/// behavior still come from egui; only the background is custom painted.
pub struct Card {
    min_height: f32,
    clickable: bool,
}

pub fn card() -> Card {
    Card {
        min_height: 0.0,
        clickable: false,
    }
}

impl Card {
    /// Minimum outer height including the shadow, e.g. to match a neighbouring card.
    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = height;
        self
    }
    /// Make the whole card a click target with a pink hover shadow.
    pub fn clickable(mut self) -> Self {
        self.clickable = true;
        self
    }
    /// The card always spans the available width, minus its shadow.
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let ctx = ui.ctx().clone();
        let lift = pixel::unit(&ctx, LIFT);
        let margin = (CARD_PADDING + pixel::unit(&ctx, 2.0)).round();
        // Scroll content can report a width larger than its viewport after a
        // wide child, so stay inside the visible clip rect as well.
        let width = ui
            .available_width()
            .min(ui.clip_rect().right() - ui.cursor().left());
        let background = ui.painter().add(egui::Shape::Noop);
        let inner = egui::Frame::new()
            .inner_margin(margin as i8)
            .outer_margin(egui::Margin {
                left: 0,
                top: 0,
                right: lift.ceil() as i8,
                bottom: lift.ceil() as i8,
            })
            .show(ui, |ui| {
                ui.set_width((width - lift.ceil() - margin * 2.0).max(40.0));
                ui.set_min_height(self.min_height - lift.ceil() - margin * 2.0);
                contents(ui)
            });
        let mut response = inner.response;
        if self.clickable {
            response = response
                .interact(egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        let rect = egui::Rect::from_min_max(
            response.rect.min,
            response.rect.max - egui::Vec2::splat(lift.ceil()),
        );
        let shadow = if self.clickable && (response.hovered() || response.has_focus()) {
            PINK
        } else {
            SHADOW
        };
        ui.painter().set(
            background,
            pixel::panel(&ctx, rect, PAPER, INK, Some((shadow, LIFT))),
        );
        egui::InnerResponse::new(inner.inner, response)
    }
}

/// Pixel push button for the light page: lifts on a hard shadow and sinks
/// onto it while pressed.
pub fn pixel_button(
    ui: &mut egui::Ui,
    text: &str,
    icon: Option<&pixel::Icon>,
    selected: bool,
    size: egui::Vec2,
) -> egui::Response {
    push_button(ui, text, icon, selected, size, false)
}

/// Pixel push button for the dark navigation frame.
pub fn nav_button(
    ui: &mut egui::Ui,
    text: &str,
    icon: &pixel::Icon,
    selected: bool,
    size: egui::Vec2,
) -> egui::Response {
    push_button(ui, text, Some(icon), selected, size, true)
}

fn push_button(
    ui: &mut egui::Ui,
    text: &str,
    icon: Option<&pixel::Icon>,
    selected: bool,
    size: egui::Vec2,
    night: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, text));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let ctx = ui.ctx().clone();
    let lift = pixel::unit(&ctx, 3.0);
    let pressed = response.is_pointer_button_down_on();
    let face = egui::Rect::from_min_max(rect.min, rect.max - egui::Vec2::splat(lift))
        .translate(egui::Vec2::splat(if pressed { lift } else { 0.0 }));
    let hovered = response.hovered() || response.has_focus();
    let (fill, shadow, outline, ink) = match (night, selected, hovered) {
        (false, true, _) => (TEAL, INK, INK, INK),
        (true, true, _) => (TEAL, PINK, INK, INK),
        (false, false, true) => (TEAL_SOFT, PINK, INK, INK),
        (false, false, false) => (PAPER, SHADOW, INK, INK),
        (true, false, true) => (NIGHT_HOVER, PINK, NIGHT_EDGE, NIGHT_TEXT),
        (true, false, false) => (NIGHT_PANEL, NIGHT_SHADOW, NIGHT_EDGE, NIGHT_TEXT),
    };
    let painter = ui.painter();
    painter.add(pixel::panel(
        &ctx,
        face,
        fill,
        outline,
        (!pressed).then_some((shadow, 3.0)),
    ));
    let galley = painter.layout_no_wrap(text.to_owned(), FontId::proportional(14.0), ink);
    let mut x = face.center().x - galley.size().x / 2.0;
    if let Some(icon) = icon {
        let unit = pixel::unit(&ctx, 2.0);
        let origin = pixel::snap(
            &ctx,
            egui::pos2(face.left() + 14.0, face.center().y - 4.5 * unit),
        );
        let mut shapes = Vec::new();
        let accent = match (selected, night) {
            (true, _) => PAPER,
            (false, true) => TEAL,
            (false, false) => PINK,
        };
        pixel::icon_shapes(&mut shapes, origin, unit, icon, ink, accent);
        painter.extend(shapes);
        x = origin.x + 9.0 * unit + 12.0;
    }
    painter.galley(
        egui::pos2(x, face.center().y - galley.size().y / 2.0),
        galley,
        ink,
    );
    if response.has_focus() {
        painter.rect_stroke(
            face.expand(3.0),
            0.0,
            Stroke::new(2.0, BLUE),
            egui::StrokeKind::Outside,
        );
    }
    response
}

/// Compact selectable chip sized to its label.
pub fn chip(ui: &mut egui::Ui, text: &str, selected: bool) -> egui::Response {
    let width = ui
        .painter()
        .layout_no_wrap(text.to_owned(), FontId::proportional(14.0), INK)
        .size()
        .x;
    pixel_button(ui, text, None, selected, vec2(width + 26.0, 30.0))
}

/// Heat color of a CPU core: pale teal rising to teal up to 60 %, then light
/// to hot pink. Two ramps avoid the muddy mix between teal and pink.
pub fn core_fill(usage: f64) -> Color32 {
    let t = (usage / 100.0).clamp(0.0, 1.0) as f32;
    let (from, to, t) = if t < 0.6 {
        (Color32::from_rgb(228, 247, 250), TEAL, t / 0.6)
    } else {
        (Color32::from_rgb(255, 176, 204), PINK, (t - 0.6) / 0.4)
    };
    lerp(from, to, t)
}

pub fn lerp(from: Color32, to: Color32, t: f32) -> Color32 {
    let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgb(
        channel(from.r(), to.r()),
        channel(from.g(), to.g()),
        channel(from.b(), to.b()),
    )
}

/// Color for a fill level such as memory or volume usage.
pub fn level_color(percent: f64, normal: Color32) -> Color32 {
    if percent >= 90.0 {
        ERROR
    } else if percent >= 75.0 {
        WARM
    } else {
        normal
    }
}
