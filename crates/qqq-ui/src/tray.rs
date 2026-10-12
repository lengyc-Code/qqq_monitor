//! Notification-area presence: the window leaves the taskbar for a tray icon while it is
//! docked as the mini bar or hidden after being minimized. Clicking the icon or choosing
//! its "open" item brings the full window back.

use crate::{dock::Desktop, i18n};
use eframe::egui::{self, ViewportCommand};
use qqq_engine::Language;

#[derive(Default)]
pub struct Tray {
    /// The minimized full window is hidden.
    hidden: bool,
    /// Language of the shown icon's text; `None` while no icon is shown.
    icon: Option<Language>,
    /// The taskbar button of the visible window is removed.
    no_button: bool,
}

impl Tray {
    /// Hides the minimized full window when `minimize_to_tray` is set, then matches the
    /// icon and the taskbar button to the window. Called before this frame's geometry
    /// commands, so the button follows the window style that is already in effect.
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        desktop: &Desktop,
        docked: bool,
        minimize_to_tray: bool,
    ) {
        if !self.hidden
            && !docked
            && minimize_to_tray
            && ctx.input(|i| i.viewport().minimized == Some(true))
        {
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            self.hidden = true;
        }
        let icon = (docked || self.hidden).then(i18n::language);
        if icon != self.icon {
            let t = i18n::t();
            match icon {
                Some(_) => (desktop.show_tray)(t.window_title, t.tray_open, t.tray_exit),
                None => (desktop.hide_tray)(),
            }
            self.icon = icon;
        }
        if docked != self.no_button && (desktop.taskbar_button)(!docked) {
            self.no_button = docked;
        }
    }

    /// Whether the user asked for the window from the icon; shows a hidden or minimized
    /// window again.
    pub fn opened(&mut self, ctx: &egui::Context, desktop: &Desktop) -> bool {
        if !(desktop.tray_opened)() {
            return false;
        }
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        self.hidden = false;
        true
    }
}
