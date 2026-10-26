use super::super::DesktopApp;
use crate::{clips::ClipEntry, components::*, i18n, pixel, theme};
use eframe::egui::{self, RichText, vec2};
use qqq_domain::Clip;
use qqq_engine::{CLIPBOARD_HISTORY, Notice};

/// Paths a files entry lists.
const FILE_LINES: usize = 3;
/// Tallest picture preview, in points.
const PICTURE_HEIGHT: f32 = 120.0;

/// What an entry asks for.
enum Action {
    Copy(u64),
    Remove(u64),
}

impl DesktopApp {
    /// Follows the clipboard while the clipboard manager is on, whose changes arrive as
    /// events, and keeps the history within its size. Runs every frame, whichever page is
    /// shown.
    pub(in crate::app) fn clipboard_logic(&mut self) {
        if self.capture.is_some() {
            return;
        }
        let on = self.active_config.clipboard_manager;
        if self.watching != Some(on) {
            self.watching = Some(on);
            match (self.desktop.input.watch_clipboard)(on) {
                Ok(()) if matches!(self.message, Some(Notice::ClipboardFailed(_))) => {
                    self.message = None
                }
                Ok(()) => {}
                Err(reason) => self.message = Some(Notice::ClipboardFailed(reason)),
            }
        }
        let keep = self.active_config.clipboard_history;
        if self.clips_kept != keep {
            self.clips_kept = keep;
            self.clips.trim(keep);
        }
    }

    pub(in crate::app) fn clipboard(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
        let on = self.active_config.clipboard_manager;
        theme::card().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            ui.scope(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                let mut enabled = on;
                if ui.checkbox(&mut enabled, t.clip_option).changed() {
                    self.apply_now(|config| config.clipboard_manager = enabled);
                }
            });
            if !on {
                ui.label(RichText::new(t.clip_off).size(12.0).color(theme::MUTED));
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let width = label_width(ui, &[t.keep], 14.0) + 8.0;
                ui.add_sized([width, 30.0], egui::Label::new(t.keep));
                ui.horizontal_wrapped(|ui| {
                    for count in CLIPBOARD_HISTORY {
                        let chosen = self.active_config.clipboard_history == count;
                        if theme::chip(ui, &(t.clip_count)(count), chosen).clicked() && !chosen {
                            self.apply_now(|config| config.clipboard_history = count);
                        }
                    }
                    if !self.clips.entries.is_empty() {
                        ui.add_space(6.0);
                        let size = vec2(label_width(ui, &[t.clip_clear], 14.0) + 30.0, 30.0);
                        if theme::pixel_button(ui, t.clip_clear, None, false, size).clicked() {
                            self.clips.clear();
                        }
                    }
                });
            });
            ui.label(RichText::new(t.clip_note).size(12.0).color(theme::MUTED));
        });
        section_title(
            ui,
            &pixel::ICON_CLIPBOARD,
            t.clip_recent,
            &(t.clip_count)(self.clips.entries.len()),
        );
        if self.clips.entries.is_empty() {
            ui.label(RichText::new(t.clip_empty).color(theme::MUTED));
            return;
        }
        let mut action = None;
        for entry in &self.clips.entries {
            if let Some(chosen) = entry_card(ui, entry) {
                action = Some(chosen);
            }
        }
        match action {
            Some(Action::Copy(id)) => {
                if let Some(entry) = self.clips.entries.iter().find(|e| e.id == id) {
                    let clip = entry.clip.clone();
                    self.put_on_clipboard(ui.ctx(), clip.clone());
                    // Without following, the entry moves to the top here as well.
                    if !on {
                        let keep = self.active_config.clipboard_history;
                        self.clips.record(ui.ctx(), clip, keep);
                    }
                }
            }
            Some(Action::Remove(id)) => self.clips.remove(id),
            None => {}
        }
    }
}

/// Kind and size of an entry's content.
fn kind(entry: &ClipEntry) -> (&'static pixel::Icon, String) {
    let t = i18n::t();
    match &*entry.clip {
        Clip::Text(_) => (&pixel::ICON_PROCESS, (t.clip_text)(entry.chars)),
        Clip::Picture(picture) => (
            &pixel::ICON_CAMERA,
            (t.clip_picture)(picture.width, picture.height),
        ),
        Clip::Files(paths) => (&pixel::ICON_DISK, (t.clip_files)(paths.len())),
    }
}

/// An entry with its kind, age, content and buttons. Clicking the content copies it too.
fn entry_card(ui: &mut egui::Ui, entry: &ClipEntry) -> Option<Action> {
    let t = i18n::t();
    let mut action = None;
    theme::card().show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (icon, label) = kind(entry);
            pixel::icon(ui, icon, 1.5, theme::INK, theme::GOLD);
            ui.label(RichText::new(label).size(13.0).strong());
            ui.label(
                RichText::new(ago(entry.at.elapsed().as_secs()))
                    .size(12.0)
                    .color(theme::MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let size =
                    |ui: &egui::Ui, text: &str| vec2(label_width(ui, &[text], 14.0) + 26.0, 30.0);
                let remove = size(ui, t.delete);
                if theme::pixel_button(ui, t.delete, None, false, remove).clicked() {
                    action = Some(Action::Remove(entry.id));
                }
                let copy = size(ui, t.copy);
                if theme::pixel_button(ui, t.copy, None, true, copy).clicked() {
                    action = Some(Action::Copy(entry.id));
                }
            });
        });
        let content = match &*entry.clip {
            Clip::Text(_) => ui.add(
                egui::Label::new(RichText::new(entry.preview.as_ref()).size(13.0))
                    .wrap()
                    .sense(egui::Sense::click()),
            ),
            Clip::Picture(_) => match &entry.thumbnail {
                Some(texture) => {
                    let size = texture.size_vec2();
                    let scale = (PICTURE_HEIGHT / size.y)
                        .min(ui.available_width() / size.x)
                        .min(1.0);
                    ui.add(
                        egui::Image::from_texture((texture.id(), size * scale))
                            .sense(egui::Sense::click()),
                    )
                }
                None => ui.label(""),
            },
            Clip::Files(paths) => {
                let mut lines: Vec<_> = paths.iter().take(FILE_LINES).cloned().collect();
                if paths.len() > FILE_LINES {
                    lines.push((t.clip_more)(paths.len() - FILE_LINES));
                }
                ui.add(
                    egui::Label::new(RichText::new(lines.join("\n")).size(13.0))
                        .wrap()
                        .sense(egui::Sense::click()),
                )
            }
        };
        if content
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(t.copy)
            .clicked()
        {
            action = Some(Action::Copy(entry.id));
        }
    });
    action
}
