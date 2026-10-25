use super::super::{DesktopApp, Page};
use crate::{components::label_width, i18n, pixel, theme};
use eframe::egui::{self, RichText, vec2};
use qqq_domain::{Hotkey, Reason, Shortcut};
use qqq_engine::{AppConfig, Notice};
use std::time::{Duration, Instant};

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

/// Shortcuts registered with the system. A registration that fails, for example while
/// another application holds the keys, is tried again after a wait that doubles from
/// [`RETRY_FIRST`] up to [`RETRY_LAST`]; new settings try at once.
#[derive(Default)]
pub(in crate::app) struct Hotkeys {
    /// Keys each shortcut is registered with, `None` for none; a shortcut missing here has
    /// not been registered yet.
    registered: Vec<Registration>,
    /// Keys the settings asked for at the latest attempt.
    wanted: Vec<Registration>,
    /// When to try the failed registrations again, and how many earlier tries failed.
    retry: Option<(Instant, u32)>,
    /// ID of the quick command each number of a system shortcut belongs to. A number stays
    /// with its command for the whole run, so a press that arrives after quick commands
    /// were removed never runs another one.
    commands: Vec<String>,
}

const RETRY_FIRST: Duration = Duration::from_secs(2);
const RETRY_LAST: Duration = Duration::from_secs(60);

/// A shortcut and whether its registration succeeded.
type Outcome = (Shortcut, Result<(), Reason>);

impl Hotkeys {
    /// The shortcut as the system registers it: a quick command's under its own number.
    fn system(&mut self, shortcut: Shortcut, config: &AppConfig) -> Shortcut {
        let Shortcut::Command(index) = shortcut else {
            return shortcut;
        };
        let Some(command) = config.quick_commands.get(index) else {
            return shortcut;
        };
        let number = match self.commands.iter().position(|id| *id == command.id) {
            Some(number) => number,
            None => {
                self.commands.push(command.id.clone());
                self.commands.len() - 1
            }
        };
        Shortcut::Command(number)
    }

    /// Position in `config` of the quick command whose system shortcut has `number`.
    pub(in crate::app) fn command(&self, number: usize, config: &AppConfig) -> Option<usize> {
        let id = self.commands.get(number)?;
        config.quick_commands.iter().position(|c| c.id == *id)
    }

    /// Registers the shortcuts as `wanted` asks with `set`, unless the registrations
    /// already match it or the failed ones are not due again yet. Returns the outcome of
    /// each registration tried, and the wait before failed ones are tried again.
    fn update(
        &mut self,
        wanted: Vec<Registration>,
        now: Instant,
        mut set: impl FnMut(Shortcut, Option<Hotkey>) -> Result<(), Reason>,
    ) -> (Vec<Outcome>, Option<Duration>) {
        if wanted == self.wanted {
            match self.retry {
                None => return (Vec::new(), None),
                Some((at, _)) if now < at => return (Vec::new(), Some(at - now)),
                Some(_) => {}
            }
        } else {
            self.retry = None;
        }
        let (released, changed) = hotkey_changes(&self.registered, &wanted);
        for shortcut in released {
            let _ = set(shortcut, None);
        }
        let mut registered = wanted.clone();
        let mut outcomes = Vec::new();
        for (shortcut, keys) in changed {
            let outcome = set(shortcut, keys);
            // The shortcut's earlier keys were released before the attempt.
            if outcome.is_err()
                && let Some(entry) = registered.iter_mut().find(|(s, _)| *s == shortcut)
            {
                entry.1 = None;
            }
            outcomes.push((shortcut, outcome));
        }
        let failed = outcomes.iter().any(|(_, outcome)| outcome.is_err());
        let tries = self.retry.map_or(0, |(_, tries)| tries + 1);
        let wait = RETRY_FIRST
            .saturating_mul(1 << tries.min(5))
            .min(RETRY_LAST);
        self.registered = registered;
        self.wanted = wanted;
        self.retry = failed.then_some((now + wait, tries));
        (outcomes, failed.then_some(wait))
    }
}

impl DesktopApp {
    /// Registers each shortcut as the settings ask. While a new shortcut is being recorded,
    /// no shortcut is registered, so its keys reach the page. Recording ends once the
    /// window can no longer receive the keys: away from the tools page, without keyboard
    /// focus, in the tray, minimized or docked. A quick command's shortcut is registered
    /// under a number that stays with the command.
    pub(in crate::app) fn hotkey_logic(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|i| i.viewport().focused) != Some(false);
        if self.page != Page::Tools || !focused || self.asleep(ctx) || self.dock.edge().is_some() {
            self.recorder = Recorder::default();
        }
        if self.capture.is_some() {
            return;
        }
        let config = &self.active_config;
        let hotkeys = &mut self.hotkeys;
        let recording = self.recorder.shortcut.is_some();
        let wanted: Vec<_> = config
            .shortcuts()
            .map(|shortcut| {
                let keys = config
                    .hotkey(shortcut)
                    .filter(|_| config.shortcut(shortcut).1 && !recording);
                (hotkeys.system(shortcut, config), keys)
            })
            .collect();
        let (outcomes, retry) = hotkeys.update(wanted, Instant::now(), self.desktop.set_hotkey);
        for (shortcut, outcome) in outcomes {
            match outcome {
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
        // The GUI wakes for the next try also while the window rests in the tray.
        if let Some(wait) = retry {
            ctx.request_repaint_after(wait);
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
/// Keys can pass from one shortcut to another in one change, such as from a removed quick
/// command to an edited one, so every shortcut that changes or goes away releases its keys
/// before any takes new ones.
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
    use qqq_engine::{CommandSessions, QuickCommand};

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
    fn a_failed_registration_is_tried_again_after_a_growing_wait() {
        let mut hotkeys = Hotkeys::default();
        let wanted = vec![
            (Shortcut::Screenshot, keys("Ctrl+Alt+A")),
            (Shortcut::Note, keys("Ctrl+Alt+N")),
        ];
        let taken = |shortcut: Shortcut, keys: Option<Hotkey>| {
            if shortcut == Shortcut::Screenshot && keys.is_some() {
                Err(Reason::HotkeyTaken)
            } else {
                Ok(())
            }
        };
        let start = Instant::now();
        let (outcomes, retry) = hotkeys.update(wanted.clone(), start, taken);
        assert_eq!(
            outcomes,
            [
                (Shortcut::Screenshot, Err(Reason::HotkeyTaken)),
                (Shortcut::Note, Ok(()))
            ]
        );
        assert_eq!(retry, Some(RETRY_FIRST));
        // Nothing is tried before the wait has passed.
        let early = start + Duration::from_secs(1);
        let (outcomes, retry) = hotkeys.update(wanted.clone(), early, |_, _| panic!("too early"));
        assert!(outcomes.is_empty());
        assert_eq!(retry, Some(Duration::from_secs(1)));
        // Only the failed shortcut is tried again, and another failure waits twice as long.
        let due = start + RETRY_FIRST;
        let mut tried = Vec::new();
        let (_, retry) = hotkeys.update(wanted.clone(), due, |shortcut, keys| {
            tried.push(shortcut);
            taken(shortcut, keys)
        });
        assert_eq!(tried, [Shortcut::Screenshot]);
        assert_eq!(retry, Some(RETRY_FIRST * 2));
        // Once the keys are free, the registration succeeds and nothing waits any more.
        let free = due + RETRY_FIRST * 2;
        let (outcomes, retry) = hotkeys.update(wanted.clone(), free, |_, _| Ok(()));
        assert_eq!(outcomes, [(Shortcut::Screenshot, Ok(()))]);
        assert_eq!(retry, None);
        let later = free + RETRY_LAST;
        let (outcomes, _) = hotkeys.update(wanted, later, |_, _| panic!("nothing to do"));
        assert!(outcomes.is_empty());
    }

    #[test]
    fn new_settings_are_tried_at_once_while_a_retry_waits() {
        let mut hotkeys = Hotkeys::default();
        let start = Instant::now();
        let first = vec![(Shortcut::Screenshot, keys("Ctrl+Alt+A"))];
        hotkeys.update(first, start, |_, _| Err(Reason::HotkeyTaken));
        let second = vec![(Shortcut::Screenshot, keys("Ctrl+Alt+S"))];
        let (outcomes, retry) = hotkeys.update(second, start, |_, _| Ok(()));
        assert_eq!(outcomes, [(Shortcut::Screenshot, Ok(()))]);
        assert_eq!(retry, None);
    }

    #[test]
    fn a_quick_command_keeps_its_shortcut_number_when_another_is_removed() {
        let command = |id: &str| QuickCommand {
            id: id.into(),
            name: id.into(),
            commands: vec!["Get-Date".into()],
            sessions: CommandSessions::Shared,
            window: false,
            directory: String::new(),
            hotkey: "F9".into(),
            hotkey_enabled: true,
        };
        let mut config = AppConfig {
            quick_commands: vec![command("a"), command("b")],
            ..Default::default()
        };
        let mut hotkeys = Hotkeys::default();
        assert_eq!(
            hotkeys.system(Shortcut::Command(0), &config),
            Shortcut::Command(0)
        );
        assert_eq!(
            hotkeys.system(Shortcut::Command(1), &config),
            Shortcut::Command(1)
        );
        // The first quick command goes; the second moves up and keeps its number.
        config.quick_commands.remove(0);
        assert_eq!(
            hotkeys.system(Shortcut::Command(0), &config),
            Shortcut::Command(1)
        );
        // A press under the removed command's number runs nothing.
        assert_eq!(hotkeys.command(0, &config), None);
        assert_eq!(hotkeys.command(1, &config), Some(0));
        assert_eq!(
            hotkeys.system(Shortcut::Screenshot, &config),
            Shortcut::Screenshot
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
