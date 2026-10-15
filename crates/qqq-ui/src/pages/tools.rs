use super::super::DesktopApp;
use crate::{components::label_width, i18n, pixel, theme};
use eframe::egui::{self, vec2};

/// Tool shown on the tools page.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum Tool {
    #[default]
    Screenshots,
    Clipboard,
}

impl DesktopApp {
    /// Tabs for each tool above the chosen tool's content.
    pub(in crate::app) fn tools(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for (tool, icon, label) in [
                (Tool::Screenshots, &pixel::ICON_CAMERA, t.tool_screenshots),
                (Tool::Clipboard, &pixel::ICON_CLIPBOARD, t.tool_clipboard),
            ] {
                let size = vec2(label_width(ui, &[label], 14.0) + 64.0, 34.0);
                if theme::pixel_button(ui, label, Some(icon), self.tool == tool, size).clicked() {
                    self.tool = tool;
                }
            }
        });
        ui.add_space(4.0);
        match self.tool {
            Tool::Screenshots => self.screenshots(ui),
            Tool::Clipboard => self.clipboard(ui),
        }
    }
}
