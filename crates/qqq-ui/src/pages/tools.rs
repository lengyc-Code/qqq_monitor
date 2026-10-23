use super::super::{DesktopApp, Page};
use crate::{components::label_width, i18n, pixel, theme};
use eframe::egui::{self, RichText, vec2};
use qqq_domain::{Hotkey, Shortcut};
use qqq_engine::Notice;

/// Tool shown on the tools page.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum Tool {
    #[default]
    Screenshots,
    Clipboard,
    Notes,
}

/// Why the last key press became no shortcut.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Problem {
    Invalid,
    /// The other shortcut has these keys.
    Taken,
}

/// Recording of a new shortcut on the tools page.
#[derive(Default)]
pub(in crate::app) struct Recorder {
    /// The shortcut the next key press replaces.
    pub(in crate::app) shortcut: Option<Shortcut>,
    problem: Option<Problem>,
}

impl DesktopApp {
    /// Registers each shortcut as the settings ask. While a new shortcut is being recorded,
    /// no shortcut is registered, so its keys reach the page.
    pub(in crate::app) fn hotkey_logic(&mut self) {
        if self.page != Page::Tools {
            self.recorder = Recorder::default();
        }
        if self.capture.is_some() {
            return;
        }
        for (slot, shortcut) in Shortcut::ALL.into_iter().enumerate() {
            let config = &self.active_config;
            let wanted = config
                .hotkey(shortcut)
                .filter(|_| config.shortcut(shortcut).1 && self.recorder.shortcut.is_none());
            if self.hotkeys[slot] == Some(wanted) {
                continue;
            }
            self.hotkeys[slot] = Some(wanted);
            match (self.desktop.set_hotkey)(shortcut, wanted) {
                Ok(())
                    if matches!(
                        &self.message,
                        Some(Notice::HotkeyUnavailable(s, _)) if *s == shortcut
                    ) =>
                {
                    self.message = None
                }
                Ok(()) => {}
                Err(reason) => self.message = Some(Notice::HotkeyUnavailable(shortcut, reason)),
            }
        }
    }

    /// Tabs for each tool above the chosen tool's content.
    pub(in crate::app) fn tools(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            for (tool, icon, label) in [
                (Tool::Screenshots, &pixel::ICON_CAMERA, t.tool_screenshots),
                (Tool::Clipboard, &pixel::ICON_CLIPBOARD, t.tool_clipboard),
                (Tool::Notes, &pixel::ICON_NOTE, t.tool_notes),
            ] {
                let size = vec2(label_width(ui, &[label], 14.0) + 64.0, 34.0);
                if theme::pixel_button(ui, label, Some(icon), self.tool == tool, size).clicked() {
                    self.tool = tool;
                    self.recorder = Recorder::default();
                }
            }
        });
        ui.add_space(4.0);
        match self.tool {
            Tool::Screenshots => self.screenshots(ui),
            Tool::Clipboard => self.clipboard(ui),
            Tool::Notes => self.notes(ui),
        }
    }

    /// Turns a key press into the new shortcut while one is recorded.
    fn record_hotkey(&mut self, ui: &egui::Ui) {
        let Some(shortcut) = self.recorder.shortcut else {
            return;
        };
        let presses: Vec<_> = ui.input(|i| {
            i.events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } => Some((*key, *modifiers)),
                    _ => None,
                })
                .collect()
        });
        for (key, modifiers) in presses {
            if key == egui::Key::Escape {
                self.recorder = Recorder::default();
                return;
            }
            let mut text = String::new();
            for (held, name) in [
                (modifiers.ctrl, "Ctrl+"),
                (modifiers.alt, "Alt+"),
                (modifiers.shift, "Shift+"),
            ] {
                if held {
                    text.push_str(name);
                }
            }
            text.push_str(key.name());
            let Some(hotkey) = Hotkey::parse(&text) else {
                self.recorder.problem = Some(Problem::Invalid);
                continue;
            };
            let taken = Shortcut::ALL
                .into_iter()
                .any(|other| other != shortcut && self.active_config.hotkey(other) == Some(hotkey));
            if taken {
                self.recorder.problem = Some(Problem::Taken);
                continue;
            }
            self.recorder = Recorder::default();
            self.apply_now(|config| {
                let (keys, enabled) = config.shortcut_mut(shortcut);
                *keys = hotkey.to_string();
                *enabled = true;
            });
            return;
        }
    }

    /// The row of a shortcut: a switch that names its keys and a button that records new
    /// keys, or the prompt while recording.
    pub(in crate::app) fn hotkey_row(&mut self, ui: &mut egui::Ui, shortcut: Shortcut, width: f32) {
        let t = i18n::t();
        self.record_hotkey(ui);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_sized([width, 30.0], egui::Label::new(t.shot_hotkey));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.vertical(|ui| {
                if self.recorder.shortcut == Some(shortcut) {
                    ui.label(RichText::new(t.shot_hotkey_press).color(theme::PINK));
                    if let Some(problem) = self.recorder.problem {
                        let text = match problem {
                            Problem::Invalid => t.shot_hotkey_invalid,
                            Problem::Taken => t.hotkey_taken,
                        };
                        ui.label(RichText::new(text).size(12.0).color(theme::ERROR));
                    }
                    return;
                }
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let (keys, mut on) = self.active_config.shortcut(shortcut);
                    let option = hotkey_option(shortcut, keys);
                    if ui.checkbox(&mut on, option).changed() {
                        self.apply_now(|config| *config.shortcut_mut(shortcut).1 = on);
                    }
                    let change = vec2(label_width(ui, &[t.shot_hotkey_change], 14.0) + 30.0, 30.0);
                    if theme::pixel_button(ui, t.shot_hotkey_change, None, false, change).clicked()
                    {
                        self.recorder = Recorder {
                            shortcut: Some(shortcut),
                            problem: None,
                        };
                    }
                });
            });
        });
    }
}

/// The option that registers a shortcut, named by its keys.
pub(in crate::app) fn hotkey_option(shortcut: Shortcut, keys: &str) -> String {
    let t = i18n::t();
    match shortcut {
        Shortcut::Screenshot => (t.shot_hotkey_option)(keys),
        Shortcut::Note => (t.note_hotkey_option)(keys),
    }
}
