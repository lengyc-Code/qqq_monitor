use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, RichText, Shadow, Stroke,
    TextStyle,
};
use std::sync::Arc;

pub const BACKGROUND: Color32 = Color32::from_rgb(235, 240, 244);
pub const SURFACE: Color32 = Color32::from_rgb(239, 243, 247);
pub const BORDER: Color32 = Color32::from_rgb(214, 223, 231);
pub const TEXT: Color32 = Color32::from_rgb(43, 54, 66);
pub const MUTED: Color32 = Color32::from_rgb(100, 113, 127);
pub const CYAN: Color32 = Color32::from_rgb(34, 133, 192);
pub const PURPLE: Color32 = Color32::from_rgb(125, 108, 175);
pub const BLUE: Color32 = Color32::from_rgb(48, 124, 200);
pub const GREEN: Color32 = Color32::from_rgb(34, 129, 105);
pub const WARNING: Color32 = Color32::from_rgb(154, 102, 16);
pub const ERROR: Color32 = Color32::from_rgb(185, 66, 69);
pub const ACCENT_FILL: Color32 = Color32::from_rgb(175, 218, 248);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(30, 77, 111);
pub const GRID: Color32 = Color32::from_rgb(211, 221, 230);
pub const CARD_PADDING: i8 = 12;
pub const SECTION_GAP: f32 = 6.0;
pub const SECTION_TITLE: f32 = 14.0;

fn relief_shadow(highlight: bool) -> Shadow {
    if highlight {
        Shadow {
            offset: [-3, -3],
            blur: 10,
            spread: 0,
            color: Color32::from_white_alpha(220),
        }
    } else {
        Shadow {
            offset: [3, 4],
            blur: 12,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(139, 157, 177, 65),
        }
    }
}

pub fn apply(ctx: &egui::Context, animations: bool) {
    ctx.set_theme(egui::Theme::Light);
    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.panel_fill = BACKGROUND;
    style.visuals.window_fill = SURFACE;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.weak_text_color = Some(MUTED);
    style.visuals.extreme_bg_color = Color32::from_rgb(226, 233, 239);
    style.visuals.faint_bg_color = SURFACE;
    style.visuals.hyperlink_color = BLUE;
    style.visuals.warn_fg_color = WARNING;
    style.visuals.error_fg_color = ERROR;
    style.visuals.window_corner_radius = 12.into();
    style.visuals.menu_corner_radius = 8.into();
    style.visuals.window_stroke = Stroke::new(1.0, Color32::WHITE);
    style.visuals.window_shadow = relief_shadow(false);
    style.visuals.popup_shadow = relief_shadow(false);
    style.visuals.selection.bg_fill = ACCENT_FILL;
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT_TEXT);
    for visuals in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        visuals.corner_radius = 8.into();
        visuals.fg_stroke = Stroke::new(1.0, TEXT);
        visuals.bg_stroke = Stroke::new(1.0, BORDER);
        visuals.expansion = 0.0;
    }
    style.visuals.widgets.noninteractive.bg_fill = SURFACE;
    style.visuals.widgets.inactive.bg_fill = SURFACE;
    style.visuals.widgets.inactive.weak_bg_fill = SURFACE;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(220, 236, 248);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(220, 236, 248);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(149, 192, 222));
    style.visuals.widgets.active.bg_fill = ACCENT_FILL;
    style.visuals.widgets.active.weak_bg_fill = ACCENT_FILL;
    style.visuals.widgets.open.bg_fill = ACCENT_FILL;
    style.visuals.widgets.open.weak_bg_fill = ACCENT_FILL;
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.animation_time = if animations { 0.18 } else { 0.0 };
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(14.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(22.0, FontFamily::Proportional),
    );
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

/// Native frames with a white upper highlight and a cool lower shadow.
/// Keep layout, response and keyboard behavior supplied by egui.
pub struct SoftFrame(egui::Frame);
impl SoftFrame {
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let highlight = ui.painter().add(egui::Shape::Noop);
        let response = self.0.show(ui, contents);
        ui.painter().set(
            highlight,
            relief_shadow(true).as_shape(response.response.rect, self.0.corner_radius),
        );
        response
    }
}

pub fn card() -> SoftFrame {
    SoftFrame(
        egui::Frame::new()
            .fill(SURFACE)
            .stroke(Stroke::new(0.8, Color32::from_white_alpha(190)))
            .shadow(relief_shadow(false))
            .corner_radius(14)
            .inner_margin(CARD_PADDING),
    )
}

fn gradient(rect: egui::Rect, radius: f32, left: Color32, right: Color32) -> egui::Shape {
    let mut mesh = egui::Mesh::default();
    let color_at = |x: f32| {
        let t = ((x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color32::from_rgb(
            lerp(left.r(), right.r()),
            lerp(left.g(), right.g()),
            lerp(left.b(), right.b()),
        )
    };
    mesh.colored_vertex(rect.center(), color_at(rect.center().x));
    let centers = [
        egui::pos2(rect.right() - radius, rect.top() + radius),
        egui::pos2(rect.right() - radius, rect.bottom() - radius),
        egui::pos2(rect.left() + radius, rect.bottom() - radius),
        egui::pos2(rect.left() + radius, rect.top() + radius),
    ];
    for (corner, center) in centers.into_iter().enumerate() {
        for step in 0..=12 {
            let angle = (corner as f32 - 1.0 + step as f32 / 12.0) * std::f32::consts::FRAC_PI_2;
            let point = center + egui::vec2(angle.cos(), angle.sin()) * radius;
            mesh.colored_vertex(point, color_at(point.x));
        }
    }
    let count = mesh.vertices.len() as u32 - 1;
    for index in 1..=count {
        mesh.add_triangle(0, index, if index == count { 1 } else { index + 1 });
    }
    egui::Shape::mesh(mesh)
}

/// Soft relief and gradient use the normal Button for focus, input and accessibility.
pub fn soft_button(
    ui: &mut egui::Ui,
    text: &str,
    selected: bool,
    size: egui::Vec2,
) -> egui::Response {
    let shadow = ui.painter().add(egui::Shape::Noop);
    let highlight = ui.painter().add(egui::Shape::Noop);
    let background = ui.painter().add(egui::Shape::Noop);
    let response = ui.add_sized(
        size,
        egui::Button::new(RichText::new(text).size(14.0).color(if selected {
            ACCENT_TEXT
        } else {
            TEXT
        }))
        .selected(selected)
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::NONE),
    );
    let rect = response.rect;
    let radius = (rect.height() / 2.0).min(12.0);
    ui.painter()
        .set(shadow, relief_shadow(false).as_shape(rect, radius as u8));
    ui.painter()
        .set(highlight, relief_shadow(true).as_shape(rect, radius as u8));
    let (left, right) = if selected {
        (
            Color32::from_rgb(160, 210, 247),
            Color32::from_rgb(105, 183, 238),
        )
    } else if response.hovered() {
        (
            Color32::from_rgb(240, 248, 253),
            Color32::from_rgb(218, 235, 247),
        )
    } else {
        (Color32::from_rgb(246, 249, 251), SURFACE)
    };
    ui.painter()
        .set(background, gradient(rect, radius, left, right));
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.5, BLUE),
            egui::StrokeKind::Inside,
        );
    }
    response
}

pub fn core_fill(usage: f64) -> Color32 {
    let t = (usage / 100.0).clamp(0.0, 1.0);
    let channel = |start: f64, end: f64| (start + (end - start) * t).round() as u8;
    Color32::from_rgb(
        channel(230.0, 133.0),
        channel(240.0, 193.0),
        channel(248.0, 235.0),
    )
}
