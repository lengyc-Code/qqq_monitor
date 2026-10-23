use super::super::DesktopApp;
use crate::{components::*, i18n, pixel, shots, theme};
use eframe::egui::{self, RichText, vec2};
use qqq_domain::{CaptureEnd, Clip, Shortcut};
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

/// What a tile asks for.
enum Action {
    Copy(PathBuf),
    Open(PathBuf),
    Delete(PathBuf),
}

impl DesktopApp {
    /// Starts requested screenshots and handles finished ones. Runs every frame, whichever
    /// page is shown.
    pub(in crate::app) fn screenshot_logic(&mut self, ctx: &egui::Context) {
        if self.capture.is_none() && (self.desktop.screenshot_requested)() {
            self.start_screenshot(false);
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
    pub(in crate::app) fn open(&mut self, path: &std::path::Path) {
        if let Err(reason) = (self.desktop.open_path)(path) {
            self.message = Some(Notice::OpenFailed(reason));
        }
    }

    pub(in crate::app) fn screenshots(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
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
            self.hotkey_row(ui, Shortcut::Screenshot, width);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.add_sized([width, 30.0], egui::Label::new(t.keep));
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
