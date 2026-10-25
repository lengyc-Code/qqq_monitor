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
    Commands,
}

/// Why the last key press became no shortcut.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Problem {
    Invalid,
    /// Another shortcut has these keys.
    Taken,
}

/// Recording of a new shortcut on the tools page.
#[derive(Default)]
pub(in crate::app) struct Recorder {
    /// The shortcut the next key press replaces.
    pub(in crate::app) shortcut: Option<Shortcut>,
    problem: Option<Problem>,
}

impl Recorder {
    /// Records new keys for `shortcut`.
    pub(in crate::app) fn start(shortcut: Shortcut) -> Self {
        Self {
            shortcut: Some(shortcut),
            problem: None,
        }
    }
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
        let config = &self.active_config;
        let wanted: Vec<_> = config
            .shortcuts()
            .map(|shortcut| {
                let keys = config
                    .hotkey(shortcut)
                    .filter(|_| config.shortcut(shortcut).1 && self.recorder.shortcut.is_none());
                (shortcut, keys)
            })
            .collect();
        if self.hotkeys == wanted {
            return;
        }
        let (released, changed) = hotkey_changes(&self.hotkeys, &wanted);
        self.hotkeys = wanted;
        for shortcut in released {
            let _ = (self.desktop.set_hotkey)(shortcut, None);
        }
        for (shortcut, wanted) in changed {
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
                (Tool::Commands, &pixel::ICON_TERMINAL, t.tool_commands),
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
            Tool::Commands => self.commands(ui),
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
            let config = &self.active_config;
            let taken = config
                .shortcuts()
                .any(|other| other != shortcut && config.hotkey(other) == Some(hotkey));
            if taken {
                self.recorder.problem = Some(Problem::Taken);
                continue;
            }
            self.recorder = Recorder::default();
            self.apply_now(|config| {
                if let Some((keys, enabled)) = config.shortcut_mut(shortcut) {
                    *keys = hotkey.to_string();
                    *enabled = true;
                }
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
                    // A quick command without keys has no switch, only a button that sets
                    // its first keys.
                    let button = if keys.is_empty() {
                        ui.label(RichText::new(t.hotkey_none).color(theme::MUTED));
                        t.hotkey_set
                    } else {
                        let option = hotkey_option(shortcut, keys);
                        if ui.checkbox(&mut on, option).changed() {
                            self.apply_now(|config| {
                                if let Some((_, enabled)) = config.shortcut_mut(shortcut) {
                                    *enabled = on;
                                }
                            });
                        }
                        t.shot_hotkey_change
                    };
                    let size = vec2(label_width(ui, &[button], 14.0) + 30.0, 30.0);
                    if theme::pixel_button(ui, button, None, false, size).clicked() {
                        self.recorder = Recorder::start(shortcut);
                    }
                });
            });
        });
    }
}

/// Keys of each shortcut, `None` for none.
type Registration = (Shortcut, Option<Hotkey>);

/// What brings the registered shortcuts from `applied` to `wanted`: the shortcuts that
/// release their keys first, then each shortcut whose keys change, with its new keys.
/// Keys move to another quick command when one before it is removed, so every shortcut
/// that changes or goes away releases its keys before any takes new ones.
fn hotkey_changes(
    applied: &[Registration],
    wanted: &[Registration],
) -> (Vec<Shortcut>, Vec<Registration>) {
    let last = |shortcut| {
        applied
            .iter()
            .find(|(s, _)| *s == shortcut)
            .map(|(_, keys)| *keys)
    };
    let changed: Vec<_> = wanted
        .iter()
        .copied()
        .filter(|(shortcut, keys)| last(*shortcut) != Some(*keys))
        .collect();
    let released = applied
        .iter()
        .filter(|(shortcut, keys)| {
            let gone = !wanted.iter().any(|(s, _)| s == shortcut);
            keys.is_some() && (gone || changed.iter().any(|(s, _)| s == shortcut))
        })
        .map(|(shortcut, _)| *shortcut)
        .collect();
    (released, changed)
}

/// The option that registers a shortcut, named by its keys.
pub(in crate::app) fn hotkey_option(shortcut: Shortcut, keys: &str) -> String {
    let t = i18n::t();
    match shortcut {
        Shortcut::Screenshot => (t.shot_hotkey_option)(keys),
        Shortcut::Note => (t.note_hotkey_option)(keys),
        Shortcut::Command(_) => (t.command_hotkey_option)(keys),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(text: &str) -> Option<Hotkey> {
        Hotkey::parse(text)
    }

    #[test]
    fn every_shortcut_registers_at_first() {
        let wanted = [
            (Shortcut::Screenshot, keys("Ctrl+Alt+A")),
            (Shortcut::Note, None),
        ];
        assert_eq!(hotkey_changes(&[], &wanted), (vec![], wanted.to_vec()));
    }

    #[test]
    fn keys_of_a_removed_command_move_up_after_their_release() {
        let applied = [
            (Shortcut::Screenshot, keys("Ctrl+Alt+A")),
            (Shortcut::Command(0), keys("F9")),
            (Shortcut::Command(1), None),
            (Shortcut::Command(2), keys("F10")),
        ];
        // The first quick command is removed; the others move up one position.
        let wanted = [
            (Shortcut::Screenshot, keys("Ctrl+Alt+A")),
            (Shortcut::Command(0), None),
            (Shortcut::Command(1), keys("F10")),
        ];
        let (released, changed) = hotkey_changes(&applied, &wanted);
        assert_eq!(released, [Shortcut::Command(0), Shortcut::Command(2)]);
        assert_eq!(
            changed,
            [
                (Shortcut::Command(0), None),
                (Shortcut::Command(1), keys("F10"))
            ]
        );
    }

    #[test]
    fn recording_releases_every_registered_shortcut() {
        let applied = [
            (Shortcut::Screenshot, keys("Ctrl+Alt+A")),
            (Shortcut::Note, None),
            (Shortcut::Command(0), keys("F9")),
        ];
        let wanted = applied.map(|(shortcut, _)| (shortcut, None));
        let (released, changed) = hotkey_changes(&applied, &wanted);
        assert_eq!(released, [Shortcut::Screenshot, Shortcut::Command(0)]);
        assert_eq!(
            changed,
            [(Shortcut::Screenshot, None), (Shortcut::Command(0), None)]
        );
    }
}
