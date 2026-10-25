use super::{
    super::{DesktopApp, Page},
    tools::Tool,
};
use crate::{components::*, i18n, note, notes::Note, pixel, shots::LocalTime, theme};
use eframe::egui::{self, RichText, vec2};
use qqq_domain::Shortcut;
use qqq_engine::Notice;
use std::sync::Arc;

/// Lines of a note a card shows below its title.
const PREVIEW_LINES: usize = 3;
/// Characters of each line a card shows.
const LINE_CHARS: usize = 120;

/// State of the notes tab.
#[derive(Default)]
pub(in crate::app) struct NotePage {
    /// The note whose delete button waits for confirmation.
    confirm: Option<u64>,
}

/// What a note card asks for.
enum Action {
    Open(u64),
    Delete(u64),
    Confirm(Option<u64>),
}

impl DesktopApp {
    /// Applies the answers of the note files' worker. Runs every frame, whichever page is
    /// shown.
    pub(in crate::app) fn note_logic(&mut self) {
        if self.page != Page::Tools || self.tool != Tool::Notes {
            self.note_page = NotePage::default();
        }
        note::lock(&self.note).notes.update();
    }

    pub(in crate::app) fn notes(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
        let shared = Arc::clone(&self.note);
        let directory = note::lock(&shared).notes.directory.clone();
        theme::card().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            let size =
                |ui: &egui::Ui, text: &str| vec2(label_width(ui, &[text], 14.0) + 64.0, 34.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let open = size(ui, t.note_open);
                if theme::pixel_button(ui, t.note_open, Some(&pixel::ICON_NOTE), true, open)
                    .clicked()
                {
                    note::lock(&shared).show(ui.ctx());
                }
                let new = size(ui, t.note_new);
                if theme::pixel_button(ui, t.note_new, Some(&pixel::ICON_PLUS), false, new)
                    .clicked()
                {
                    let mut window = note::lock(&shared);
                    window.create();
                    window.show(ui.ctx());
                }
                let folder = size(ui, t.shot_folder);
                if theme::pixel_button(ui, t.shot_folder, Some(&pixel::ICON_DISK), false, folder)
                    .clicked()
                {
                    match std::fs::create_dir_all(&directory) {
                        Ok(()) => self.open(&directory),
                        Err(error) => self.message = Some(Notice::NoteFile(error.to_string())),
                    }
                }
            });
            let width = label_width(ui, &[t.shot_hotkey], 14.0) + 8.0;
            self.hotkey_row(ui, Shortcut::Note, width);
            ui.label(
                RichText::new((t.note_about)(&directory.display().to_string()))
                    .size(12.0)
                    .color(theme::MUTED),
            );
        });
        let window = note::lock(&shared);
        let notes: Vec<_> = window
            .notes
            .entries
            .iter()
            .filter(|note| !note.is_blank())
            .collect();
        section_title(
            ui,
            &pixel::ICON_NOTE,
            t.note_list,
            &(t.note_count)(notes.len()),
        );
        if let Some(error) = &window.notes.error {
            ui.label(
                RichText::new(i18n::notice(&Notice::NoteFile(error.text().into())))
                    .size(12.0)
                    .color(theme::ERROR),
            );
        }
        if notes.is_empty() {
            let text = if window.notes.listed {
                t.note_empty
            } else {
                t.note_loading
            };
            ui.label(RichText::new(text).color(theme::MUTED));
            return;
        }
        let mut action = None;
        for note in notes {
            let confirm = self.note_page.confirm == Some(note.id);
            if let Some(chosen) = card(ui, note, confirm, self.desktop.system.local_time) {
                action = Some(chosen);
            }
        }
        drop(window);
        match action {
            Some(Action::Open(id)) => note::lock(&shared).open(ui.ctx(), id),
            Some(Action::Delete(id)) => {
                note::lock(&shared).notes.delete(id);
                self.note_page.confirm = None;
            }
            Some(Action::Confirm(id)) => self.note_page.confirm = id,
            None => {}
        }
    }
}

/// A note's title, when it changed, its first lines and its buttons.
fn card(ui: &mut egui::Ui, note: &Note, confirm: bool, local_time: LocalTime) -> Option<Action> {
    let t = i18n::t();
    let mut action = None;
    let response = theme::card().clickable().show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            pixel::icon(ui, &pixel::ICON_NOTE, 1.5, theme::INK, theme::GOLD);
            let title = match note.title() {
                "" => t.note_untitled,
                title => title,
            };
            ui.add(egui::Label::new(RichText::new(title).strong()).truncate());
        });
        ui.label(
            RichText::new(note::changed(note.modified, local_time))
                .size(12.0)
                .color(theme::MUTED),
        );
        let preview = preview(&note.text);
        if !preview.is_empty() {
            ui.label(RichText::new(preview).size(12.0));
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let button = |ui: &mut egui::Ui, label: &str, selected: bool| {
                let size = vec2(label_width(ui, &[label], 14.0) + 26.0, 30.0);
                theme::pixel_button(ui, label, None, selected, size).clicked()
            };
            if button(ui, t.open, false) {
                action = Some(Action::Open(note.id));
            }
            if confirm {
                if button(ui, t.note_delete_confirm_short, true) {
                    action = Some(Action::Delete(note.id));
                }
                if button(ui, t.cancel, false) {
                    action = Some(Action::Confirm(None));
                }
            } else if button(ui, t.delete, false) {
                action = Some(Action::Confirm(Some(note.id)));
            }
        });
    });
    if action.is_none() && response.response.clicked() {
        action = Some(Action::Open(note.id));
    }
    action
}

/// The lines after a note's title, each cut to a readable length.
fn preview(text: &str) -> String {
    let mut lines: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .skip(1)
        .take(PREVIEW_LINES + 1)
        .map(|line| match line.char_indices().nth(LINE_CHARS) {
            Some((at, _)) => format!("{}…", &line[..at]),
            None => line.to_owned(),
        })
        .collect();
    if lines.len() > PREVIEW_LINES {
        lines.truncate(PREVIEW_LINES);
        if let Some(last) = lines.last_mut() {
            last.push_str(" …");
        }
    }
    lines.join("\n")
}
