use super::{
    super::{DesktopApp, Page},
    tools::{Recorder, Tool},
};
use crate::{components::*, i18n, pixel, theme};
use eframe::egui::{self, RichText, vec2};
use qqq_domain::{CommandSound, ShellSession, Shortcut};
use qqq_engine::{
    CommandSessions, MAX_COMMAND_BYTES, MAX_COMMAND_LINES, MAX_QUICK_COMMANDS, Notice, QuickCommand,
};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

/// Width of the add and edit dialog.
const DIALOG_WIDTH: f32 = 540.0;
/// Command lines a card shows.
const PREVIEW_LINES: usize = 3;
/// Characters of each command line a card shows.
const LINE_CHARS: usize = 120;
/// Longest name of a quick command, in characters.
const NAME_CHARS: usize = 64;

/// A running PowerShell session of a quick command.
pub(in crate::app) struct RunningSession {
    /// ID of the quick command.
    command: String,
    /// The session shows a window, where the user can end it.
    window: bool,
    session: Box<dyn ShellSession>,
}

/// State of the quick commands tab.
#[derive(Default)]
pub(in crate::app) struct CommandPage {
    editor: Option<Editor>,
    /// The quick command whose delete button waits for confirmation.
    confirm: Option<String>,
}

/// Fields of the add or edit dialog.
struct Editor {
    /// Quick command being edited; `None` adds one.
    id: Option<String>,
    name: String,
    /// Command lines, one per line.
    lines: String,
    sessions: CommandSessions,
    window: bool,
    directory: String,
    error: Option<String>,
}

impl Editor {
    fn new(command: Option<&QuickCommand>) -> Self {
        match command {
            Some(command) => Self {
                id: Some(command.id.clone()),
                name: command.name.clone(),
                lines: command.commands.join("\n"),
                sessions: command.sessions,
                window: command.window,
                directory: command.directory.clone(),
                error: None,
            },
            None => Self {
                id: None,
                name: String::new(),
                lines: String::new(),
                sessions: CommandSessions::Shared,
                window: true,
                directory: String::new(),
                error: None,
            },
        }
    }

    /// The quick command the fields describe; an edited one keeps its shortcut.
    fn build(&self, commands: &[QuickCommand]) -> Result<QuickCommand, String> {
        let t = i18n::t();
        let name = self.name.trim();
        if name.is_empty() {
            return Err(t.command_missing_name.into());
        }
        let lines: Vec<String> = self
            .lines
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.trim().is_empty())
            .map(Into::into)
            .collect();
        if lines.is_empty() {
            return Err(t.command_missing_lines.into());
        }
        if lines.len() > MAX_COMMAND_LINES
            || lines.iter().map(String::len).sum::<usize>() > MAX_COMMAND_BYTES
        {
            return Err((t.command_too_long)(MAX_COMMAND_LINES, MAX_COMMAND_BYTES));
        }
        let directory = self.directory.trim();
        let folder = Path::new(directory);
        if !directory.is_empty() && !(folder.is_absolute() && folder.is_dir()) {
            return Err(t.command_bad_directory.into());
        }
        let edited = self
            .id
            .as_ref()
            .and_then(|id| commands.iter().find(|c| c.id == *id));
        let command = QuickCommand {
            id: self.id.clone().unwrap_or_else(|| new_id(commands)),
            name: name.into(),
            commands: lines,
            sessions: self.sessions,
            window: self.window,
            directory: directory.into(),
            hotkey: edited.map(|c| c.hotkey.clone()).unwrap_or_default(),
            hotkey_enabled: edited.is_none_or(|c| c.hotkey_enabled),
        };
        command.validate().map_err(|notice| i18n::notice(&notice))?;
        Ok(command)
    }
}

/// An ID unlike any other quick command's.
fn new_id(commands: &[QuickCommand]) -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    (0..)
        .map(|n| format!("c{:x}", stamp + n))
        .find(|id| !commands.iter().any(|c| c.id == *id))
        .unwrap_or_default()
}

/// What a quick command card asks for.
enum Action {
    Run(usize),
    /// Terminates the running sessions of the quick command with this ID.
    Stop(String),
    Edit(usize),
    Delete(usize),
    Confirm(Option<String>),
}

impl DesktopApp {
    /// Stops following the sessions that ended or whose quick command was removed. Runs
    /// every frame, whichever page is shown.
    pub(in crate::app) fn command_logic(&mut self) {
        if self.page != Page::Tools || self.tool != Tool::Commands {
            self.command_page = CommandPage::default();
        }
        let commands = &self.active_config.quick_commands;
        self.shell_sessions.retain(|running| {
            running.session.running() && commands.iter().any(|c| c.id == running.command)
        });
    }

    /// Starts the sessions of the quick command at `index` and follows them, showing why
    /// when one fails to start. The run plays a sound when the settings ask for one, and
    /// shows a balloon while the window has only its tray icon or is docked.
    pub(in crate::app) fn run_command(&mut self, index: usize) {
        let Some(command) = self.active_config.quick_commands.get(index) else {
            return;
        };
        let sessions = command.sessions();
        let mut started = Ok(());
        for session in &sessions {
            match (self.desktop.system.start_powershell)(
                &session.script,
                &session.title,
                command.window,
                &command.directory,
            ) {
                Ok(session) => self.shell_sessions.push(RunningSession {
                    command: command.id.clone(),
                    window: command.window,
                    session,
                }),
                Err(reason) => {
                    started = Err(reason);
                    break;
                }
            }
        }
        let name = command.name.clone();
        let sound = match started {
            Ok(()) => CommandSound::Started,
            Err(_) => CommandSound::Failed,
        };
        if self.active_config.command_sound {
            (self.desktop.system.play_sound)(sound);
        }
        if self.tray.has_icon() {
            let text = match &started {
                Ok(()) => (i18n::t().command_balloon)(sessions.len()),
                Err(reason) => i18n::notice(&Notice::CommandFailed(reason.clone())),
            };
            (self.desktop.shell.balloon)(&name, &text, started.is_err());
        }
        match started {
            Ok(()) if matches!(self.message, Some(Notice::CommandFailed(_))) => self.message = None,
            Ok(()) => {}
            Err(reason) => self.message = Some(Notice::CommandFailed(reason)),
        }
    }

    /// Terminates the running sessions that `stop` picks, showing why when one keeps
    /// running.
    fn stop_sessions(&mut self, stop: impl Fn(&RunningSession) -> bool) {
        let mut failed = None;
        self.shell_sessions.retain(|running| {
            if !stop(running) {
                return true;
            }
            match running.session.terminate() {
                Ok(()) => false,
                Err(reason) => {
                    failed.get_or_insert(reason);
                    true
                }
            }
        });
        match failed {
            None if matches!(self.message, Some(Notice::CommandStopFailed(_))) => {
                self.message = None
            }
            None => {}
            Some(reason) => self.message = Some(Notice::CommandStopFailed(reason)),
        }
    }

    /// Saves the quick commands right away, like the alert rules.
    fn save_commands(&mut self, commands: Vec<QuickCommand>) {
        self.apply_now(|config| config.quick_commands = commands);
    }

    pub(in crate::app) fn commands(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
        let commands = self.active_config.quick_commands.clone();
        theme::card().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            if commands.len() < MAX_QUICK_COMMANDS {
                let size = vec2(label_width(ui, &[t.command_new], 14.0) + 64.0, 34.0);
                if theme::pixel_button(ui, t.command_new, Some(&pixel::ICON_PLUS), true, size)
                    .clicked()
                {
                    self.command_page.editor = Some(Editor::new(None));
                }
            }
            // Applied and saved at once, like the switch on the settings page.
            let mut on = self.active_config.command_sound;
            if ui.checkbox(&mut on, t.command_sound_option).changed() {
                self.apply_now(|config| config.command_sound = on);
            }
            ui.label(
                RichText::new(t.command_about)
                    .size(12.0)
                    .color(theme::MUTED),
            );
        });
        section_title(
            ui,
            &pixel::ICON_TERMINAL,
            t.command_list,
            &(t.command_count)(commands.len()),
        );
        if commands.is_empty() {
            ui.label(RichText::new(t.command_empty).color(theme::MUTED));
        }
        let mut action = None;
        for (index, command) in commands.iter().enumerate() {
            let confirm = self.command_page.confirm.as_ref() == Some(&command.id);
            let sessions: Vec<_> = self
                .shell_sessions
                .iter()
                .filter(|running| running.command == command.id)
                .collect();
            let running = sessions.len();
            let background = sessions.iter().filter(|running| !running.window).count();
            theme::card().show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                if let Some(chosen) = header(ui, index, command, confirm, running > 0) {
                    action = Some(chosen);
                }
                if confirm && background > 0 {
                    ui.label(
                        RichText::new((t.command_delete_ends)(background))
                            .size(12.0)
                            .color(theme::WARNING),
                    );
                }
                if running > 0 {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        pixel_dot(ui, theme::GOOD);
                        ui.label(
                            RichText::new((t.command_running)(running))
                                .size(12.0)
                                .color(theme::GOOD),
                        );
                    });
                }
                let mut summary = (t.command_summary)(
                    command.sessions == CommandSessions::Shared,
                    command.commands.len(),
                    command.window,
                );
                if !command.directory.is_empty() {
                    summary = format!("{summary} · {}", command.directory);
                }
                ui.label(RichText::new(summary).size(12.0).color(theme::MUTED));
                ui.label(
                    RichText::new(preview(&command.commands))
                        .monospace()
                        .size(12.0),
                );
                let width = label_width(ui, &[t.shot_hotkey], 14.0) + 8.0;
                self.hotkey_row(ui, Shortcut::Command(index), width);
            });
        }
        match action {
            Some(Action::Run(index)) => self.run_command(index),
            Some(Action::Stop(id)) => self.stop_sessions(|running| running.command == id),
            Some(Action::Edit(index)) => {
                self.command_page.editor = Some(Editor::new(commands.get(index)));
            }
            Some(Action::Delete(index)) => {
                let mut commands = commands.clone();
                let removed = commands.remove(index);
                // A session in the background has no other way to end; a window stays for
                // the user to close.
                self.stop_sessions(|running| running.command == removed.id && !running.window);
                // The quick commands after it move up one position, which a recording
                // follows no more.
                if matches!(self.recorder.shortcut, Some(Shortcut::Command(_))) {
                    self.recorder = Recorder::default();
                }
                self.command_page.confirm = None;
                self.save_commands(commands);
            }
            Some(Action::Confirm(id)) => self.command_page.confirm = id,
            None => {}
        }
        self.command_editor(ui.ctx());
    }

    fn command_editor(&mut self, ctx: &egui::Context) {
        let Some(mut editor) = self.command_page.editor.take() else {
            return;
        };
        let t = i18n::t();
        let editing = editor.id.is_some();
        let modal = egui::Modal::new(egui::Id::new("command-editor")).show(ctx, |ui| {
            ui.set_width(DIALOG_WIDTH);
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(
                RichText::new(if editing {
                    t.command_edit_title
                } else {
                    t.command_add_title
                })
                .size(16.0)
                .strong(),
            );
            let labels = [
                t.command_name,
                t.command_lines,
                t.command_sessions,
                t.command_window,
                t.command_directory,
            ];
            let width = label_width(ui, &labels, 14.0) + 8.0;
            let row = |ui: &mut egui::Ui, label: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.add_sized([width, 30.0], egui::Label::new(label));
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        add(ui)
                    });
                });
            };
            row(ui, t.command_name, &mut |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut editor.name)
                        .char_limit(NAME_CHARS)
                        .desired_width(ui.available_width()),
                );
            });
            row(ui, t.command_lines, &mut |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut editor.lines)
                        .code_editor()
                        .hint_text(t.command_lines_hint)
                        .desired_rows(6)
                        .desired_width(ui.available_width()),
                );
            });
            row(ui, t.command_sessions, &mut |ui| {
                for (sessions, label) in [
                    (CommandSessions::Shared, t.command_shared),
                    (CommandSessions::Separate, t.command_separate),
                ] {
                    if theme::chip(ui, label, editor.sessions == sessions).clicked() {
                        editor.sessions = sessions;
                    }
                }
            });
            row(ui, t.command_window, &mut |ui| {
                ui.checkbox(&mut editor.window, t.command_window_option);
            });
            row(ui, t.command_directory, &mut |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut editor.directory)
                        .hint_text(t.command_directory_hint)
                        .desired_width(ui.available_width()),
                );
            });
            if let Some(error) = &editor.error {
                ui.label(RichText::new(error).size(12.0).color(theme::ERROR));
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let size =
                    |ui: &egui::Ui, text: &str| vec2(label_width(ui, &[text], 14.0) + 40.0, 34.0);
                let (yes, no) = (size(ui, t.command_save), size(ui, t.cancel));
                let save = theme::pixel_button(ui, t.command_save, None, true, yes).clicked();
                let cancel = theme::pixel_button(ui, t.cancel, None, false, no).clicked();
                (save, cancel)
            })
            .inner
        });
        let (save, cancel) = modal.inner;
        if cancel || modal.should_close() {
            return;
        }
        if !save {
            self.command_page.editor = Some(editor);
            return;
        }
        let mut commands = self.active_config.quick_commands.clone();
        match editor.build(&commands) {
            Err(error) => {
                editor.error = Some(error);
                self.command_page.editor = Some(editor);
            }
            Ok(command) => {
                match commands.iter_mut().find(|c| c.id == command.id) {
                    Some(entry) => *entry = command,
                    None => commands.push(command),
                }
                self.save_commands(commands);
            }
        }
    }
}

/// A quick command's icon, name and buttons; a running one also offers to stop.
fn header(
    ui: &mut egui::Ui,
    index: usize,
    command: &QuickCommand,
    confirm: bool,
    running: bool,
) -> Option<Action> {
    let t = i18n::t();
    let mut action = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        pixel::icon(ui, &pixel::ICON_TERMINAL, 1.5, theme::INK, theme::TEAL);
        let size = |ui: &egui::Ui, text: &str| vec2(label_width(ui, &[text], 14.0) + 30.0, 30.0);
        let buttons: Vec<(&str, bool, Action)> = if confirm {
            vec![
                (t.cancel, false, Action::Confirm(None)),
                (t.note_delete_confirm_short, true, Action::Delete(index)),
            ]
        } else {
            let mut buttons = vec![
                (t.delete, false, Action::Confirm(Some(command.id.clone()))),
                (t.command_edit, false, Action::Edit(index)),
            ];
            if running {
                buttons.push((t.command_stop, false, Action::Stop(command.id.clone())));
            }
            buttons.push((t.command_run, true, Action::Run(index)));
            buttons
        };
        let taken: f32 = buttons
            .iter()
            .map(|(label, _, _)| size(ui, label).x + 8.0)
            .sum();
        let room = (ui.available_width() - taken - 16.0).max(0.0);
        ui.allocate_ui(vec2(room, 30.0), |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(&command.name)
                        .size(theme::SECTION_TITLE)
                        .strong(),
                )
                .truncate(),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (label, selected, chosen) in buttons {
                if theme::pixel_button(ui, label, None, selected, size(ui, label)).clicked() {
                    action = Some(chosen);
                }
            }
        });
    });
    action
}

/// The first command lines, each cut to a readable length.
fn preview(commands: &[String]) -> String {
    let mut lines: Vec<String> = commands
        .iter()
        .take(PREVIEW_LINES)
        .map(|line| match line.char_indices().nth(LINE_CHARS) {
            Some((at, _)) => format!("{}…", &line[..at]),
            None => line.clone(),
        })
        .collect();
    if commands.len() > PREVIEW_LINES
        && let Some(last) = lines.last_mut()
    {
        last.push_str(" …");
    }
    lines.join("\n")
}
