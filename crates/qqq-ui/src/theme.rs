use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
};
use std::sync::Arc;

pub const BACKGROUND: Color32 = Color32::from_rgb(11, 16, 32);
pub const SURFACE: Color32 = Color32::from_rgb(20, 29, 48);
pub const BORDER: Color32 = Color32::from_rgb(38, 53, 78);
pub const MUTED: Color32 = Color32::from_rgb(168, 182, 206);
pub const CYAN: Color32 = Color32::from_rgb(53, 216, 245);
pub const PURPLE: Color32 = Color32::from_rgb(167, 139, 250);
pub const BLUE: Color32 = Color32::from_rgb(96, 165, 250);
pub const GREEN: Color32 = Color32::from_rgb(52, 211, 153);

pub fn apply(ctx: &egui::Context, animations: bool) {
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = BACKGROUND;
    style.visuals.window_fill = SURFACE;
    style.visuals.override_text_color = Some(Color32::from_rgb(231, 237, 248));
    style.visuals.weak_text_color = Some(MUTED);
    style.visuals.extreme_bg_color = BACKGROUND;
    style.visuals.widgets.noninteractive.bg_fill = SURFACE;
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    style.visuals.widgets.inactive.bg_fill = SURFACE;
    style.visuals.widgets.inactive.weak_bg_fill = SURFACE;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(26, 39, 64);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(26, 39, 64);
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BLUE);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(24, 63, 83);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(24, 63, 83);
    style.visuals.selection.bg_fill = Color32::from_rgb(24, 63, 83);
    style.visuals.selection.stroke = Stroke::new(1.0, CYAN);
    style.spacing.item_spacing = egui::vec2(12.0, 12.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
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
        FontId::new(26.0, FontFamily::Proportional),
    );
    ctx.set_style_of(egui::Theme::Dark, style);
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

pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(12)
        .inner_margin(18)
}
