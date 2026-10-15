use super::{super::DesktopApp, super::Page, tools::Tool};
use crate::{components::*, i18n, pixel, shots, theme};
use eframe::egui::{self, RichText, vec2};
use qqq_domain::{CaptureEnd, Clip, Hotkey};
use qqq_engine::{Notice, SCREENSHOT_HISTORY};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

/// Time the compositor takes to drop the cloaked window from the screen before the capture.
const HIDE_DELAY: Duration = Duration::from_millis(150);
/// Narrowest screenshot tile, in points.
const TILE_WIDTH: f32 = 240.0;

/// Progress of a screenshot.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::app) enum Snip {
    #[default]
    Idle,
    /// The main window is cloaked; the capture starts once it has left the screen.
    Hiding(Instant),
    /// The user is selecting a region; `cloaked` brings the main window back afterwards.
    Selecting { cloaked: bool },
}

/// State of the screenshots page.
#[derive(Default)]
pub(in crate::app) struct ScreenshotPage {
    /// The next key press becomes the shortcut.
    recording: bool,
    /// The last key press was no valid shortcut.
    invalid: bool,
}

/// What a tile asks for.
enum Action {
    Copy(PathBuf),
    Open(PathBuf),
    Delete(PathBuf),
}

impl DesktopApp {
    /// Registers the shortcut as the settings ask, starts requested screenshots and handles
    /// finished ones. Runs every frame, whichever page is shown.
    pub(in crate::app) fn screenshot_logic(&mut self, ctx: &egui::Context) {
        if self.page != Page::Tools || self.tool != Tool::Screenshots {
            self.screenshot_page = ScreenshotPage::default();
        }
        if self.capture.is_none() {
            // The shortcut stays free while the page records a new one.
            let config = &self.active_config;
            let wanted = config
                .hotkey()
                .filter(|_| config.screenshot_hotkey_enabled && !self.screenshot_page.recording);
            if self.hotkey != Some(wanted) {
                self.hotkey = Some(wanted);
                match (self.desktop.set_hotkey)(wanted) {
                    Ok(()) if matches!(self.message, Some(Notice::HotkeyUnavailable(_))) => {
                        self.message = None
                    }
                    Ok(()) => {}
                    Err(reason) => self.message = Some(Notice::HotkeyUnavailable(reason)),
                }
            }
            if (self.desktop.screenshot_requested)() {
                self.start_screenshot(false);
            }
        }
        if let Snip::Hiding(since) = self.snip {
            match HIDE_DELAY.checked_sub(since.elapsed()) {
                Some(left) => ctx.request_repaint_after(left),
                None => self.begin_capture(true),
            }
        }
        if let Some(end) = (self.desktop.take_capture)() {
            if self.snip == (Snip::Selecting { cloaked: true }) {
                (self.desktop.cloak)(false);
            }
            self.snip = Snip::Idle;
            if let CaptureEnd::Taken(picture) = end {
                let picture = Arc::new(picture);
                self.put_on_clipboard(&Clip::Picture(Arc::clone(&picture)));
                self.shots
                    .save(picture, self.active_config.screenshot_history);
            }
        }
        for event in self.shots.update(ctx) {
            match event {
                shots::Event::Loaded(picture) => {
                    self.put_on_clipboard(&Clip::Picture(Arc::new(picture)))
                }
                shots::Event::Failed(error) => self.message = Some(Notice::ScreenshotFile(error)),
            }
        }
        let keep = self.active_config.screenshot_history;
        if self.shots_kept != keep {
            self.shots_kept = keep;
            self.shots.prune(keep);
        }
    }

    /// Starts a screenshot unless one is under way; `hide` takes the main window off the
    /// screen first.
    fn start_screenshot(&mut self, hide: bool) {
        if self.snip != Snip::Idle {
            return;
        }
        if hide {
            (self.desktop.cloak)(true);
            self.snip = Snip::Hiding(Instant::now());
        } else {
            self.begin_capture(false);
        }
    }

    fn begin_capture(&mut self, cloaked: bool) {
        match (self.desktop.start_capture)(i18n::t().shot_hint) {
            Ok(()) => self.snip = Snip::Selecting { cloaked },
            Err(reason) => {
                if cloaked {
                    (self.desktop.cloak)(false);
                }
                self.snip = Snip::Idle;
                self.message = Some(Notice::ScreenshotFailed(reason));
            }
        }
    }

    /// Puts content on the clipboard, showing why when it fails.
    pub(in crate::app) fn put_on_clipboard(&mut self, clip: &Clip) {
        match (self.desktop.set_clipboard)(clip) {
            Ok(()) if matches!(self.message, Some(Notice::ClipboardFailed(_))) => {
                self.message = None
            }
            Ok(()) => {}
            Err(reason) => self.message = Some(Notice::ClipboardFailed(reason)),
        }
    }

    /// Opens a file or folder with its default program, showing why when it fails.
    fn open(&mut self, path: &std::path::Path) {
        if let Err(reason) = (self.desktop.open_path)(path) {
            self.message = Some(Notice::OpenFailed(reason));
        }
    }

    /// Turns a key press into the new shortcut while the page records one.
    fn record_hotkey(&mut self, ui: &egui::Ui) {
        if !self.screenshot_page.recording {
            return;
        }
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
                self.screenshot_page = ScreenshotPage::default();
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
            match Hotkey::parse(&text) {
                Some(hotkey) => {
                    self.screenshot_page = ScreenshotPage::default();
                    self.apply_now(|config| {
                        config.screenshot_hotkey = hotkey.to_string();
                        config.screenshot_hotkey_enabled = true;
                    });
                    return;
                }
                None => self.screenshot_page.invalid = true,
            }
        }
    }

    pub(in crate::app) fn screenshots(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
        self.record_hotkey(ui);
        theme::card().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 10.0;
            let size =
                |ui: &egui::Ui, text: &str| vec2(label_width(ui, &[text], 14.0) + 64.0, 34.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let take = size(ui, t.shot_take);
                if theme::pixel_button(ui, t.shot_take, Some(&pixel::ICON_CAMERA), true, take)
                    .clicked()
                {
                    self.start_screenshot(true);
                }
                let folder = size(ui, t.shot_folder);
                if theme::pixel_button(ui, t.shot_folder, Some(&pixel::ICON_DISK), false, folder)
                    .clicked()
                {
                    let directory = self.shots.directory.clone();
                    match std::fs::create_dir_all(&directory) {
                        Ok(()) => self.open(&directory),
                        Err(error) => {
                            self.message = Some(Notice::ScreenshotFile(error.to_string()))
                        }
                    }
                }
            });
            let width = label_width(ui, &[t.shot_hotkey, t.keep], 14.0) + 8.0;
            let row_label = |ui: &mut egui::Ui, text: &str| {
                ui.add_sized([width, 30.0], egui::Label::new(text));
            };
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                row_label(ui, t.shot_hotkey);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                ui.vertical(|ui| {
                    if self.screenshot_page.recording {
                        ui.label(RichText::new(t.shot_hotkey_press).color(theme::PINK));
                        if self.screenshot_page.invalid {
                            ui.label(
                                RichText::new(t.shot_hotkey_invalid)
                                    .size(12.0)
                                    .color(theme::ERROR),
                            );
                        }
                        return;
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let mut on = self.active_config.screenshot_hotkey_enabled;
                        let option = (t.shot_hotkey_option)(&self.active_config.screenshot_hotkey);
                        if ui.checkbox(&mut on, option).changed() {
                            self.apply_now(|config| config.screenshot_hotkey_enabled = on);
                        }
                        let change =
                            vec2(label_width(ui, &[t.shot_hotkey_change], 14.0) + 30.0, 30.0);
                        if theme::pixel_button(ui, t.shot_hotkey_change, None, false, change)
                            .clicked()
                        {
                            self.screenshot_page.recording = true;
                        }
                    });
                });
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                row_label(ui, t.keep);
                ui.horizontal_wrapped(|ui| {
                    for count in SCREENSHOT_HISTORY {
                        let chosen = self.active_config.screenshot_history == count;
                        if theme::chip(ui, &(t.shot_count)(count), chosen).clicked() && !chosen {
                            self.apply_now(|config| config.screenshot_history = count);
                        }
                    }
                });
            });
            ui.label(
                RichText::new((t.shot_note)(&self.shots.directory.display().to_string()))
                    .size(12.0)
                    .color(theme::MUTED),
            );
        });
        section_title(
            ui,
            &pixel::ICON_CAMERA,
            t.shot_recent,
            &(t.shot_count)(self.shots.entries.len()),
        );
        if self.shots.entries.is_empty() {
            let text = if self.shots.listed {
                t.shot_empty
            } else {
                t.shot_loading
            };
            ui.label(RichText::new(text).color(theme::MUTED));
            return;
        }
        let gap = ui.spacing().item_spacing.x;
        let columns = ((ui.available_width() + gap) / (TILE_WIDTH + gap))
            .floor()
            .max(1.0) as usize;
        let mut action = None;
        for row in self.shots.entries.chunks(columns) {
            ui.columns(columns, |uis| {
                for (ui, shot) in uis.iter_mut().zip(row) {
                    if let Some(chosen) = tile(ui, shot, self.desktop.local_time) {
                        action = Some(chosen);
                    }
                }
            });
        }
        match action {
            Some(Action::Copy(path)) => self.shots.load(&path),
            Some(Action::Open(path)) => self.open(&path),
            Some(Action::Delete(path)) => self.shots.delete(&path),
            None => {}
        }
    }
}

/// A screenshot's thumbnail, time, size and buttons.
fn tile(ui: &mut egui::Ui, shot: &shots::Shot, local_time: shots::LocalTime) -> Option<Action> {
    let t = i18n::t();
    let file = &shot.file;
    theme::card()
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            let width = ui.available_width();
            let (rect, response) =
                ui.allocate_exact_size(vec2(width, width * 9.0 / 16.0), egui::Sense::click());
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, theme::WELL);
            match &shot.thumbnail {
                Some(texture) => {
                    let size = texture.size_vec2();
                    let scale = (rect.width() / size.x).min(rect.height() / size.y);
                    painter.image(
                        texture.id(),
                        egui::Rect::from_center_size(rect.center(), size * scale),
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
                None => {
                    let unit = pixel::unit(ui.ctx(), 2.0);
                    let origin =
                        pixel::snap(ui.ctx(), rect.center() - egui::Vec2::splat(4.5 * unit));
                    let mut shapes = Vec::new();
                    pixel::icon_shapes(
                        &mut shapes,
                        origin,
                        unit,
                        &pixel::ICON_CAMERA,
                        theme::SHADOW,
                        theme::LINE,
                    );
                    painter.extend(shapes);
                }
            }
            let mut action = None;
            if response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(t.open)
                .clicked()
            {
                action = Some(Action::Open(file.path.clone()));
            }
            let taken = local_time(file.taken).map(timestamp).unwrap_or_default();
            ui.label(RichText::new(taken).size(12.0).strong());
            ui.label(
                RichText::new(format!(
                    "{} × {} · {}",
                    file.width,
                    file.height,
                    bytes(file.bytes as f64)
                ))
                .size(12.0)
                .color(theme::MUTED),
            );
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (label, make) in [
                    (t.copy, Action::Copy as fn(PathBuf) -> Action),
                    (t.open, Action::Open),
                    (t.delete, Action::Delete),
                ] {
                    let size = vec2(label_width(ui, &[label], 14.0) + 26.0, 30.0);
                    if theme::pixel_button(ui, label, None, false, size).clicked() {
                        action = Some(make(file.path.clone()));
                    }
                }
            });
            action
        })
        .inner
}
