//! The note window: a frameless window of its own that stays above other windows. The
//! note shortcut and the tray menu show and hide it; it writes the current note as
//! Markdown, shows it rendered, or lists every note.
//!
//! The window exists, hidden, from the main window's first frame, so the shortcut shows it
//! even while the main window is minimized and runs no interface of its own. It paints in
//! its own passes, so typing repaints only the note.

use crate::{
    components::{ago, label_width, timestamp},
    dock::Desktop,
    i18n, markdown,
    notes::{Note, NoteError, Notes},
    pixel,
    shots::LocalTime,
    theme,
};
use eframe::egui::{
    self, Color32, FontId, Key, KeyboardShortcut, Modifiers, RichText, Sense, Stroke, Vec2,
    ViewportCommand, pos2, text::LayoutJob, vec2,
};
use qqq_domain::Reason;
use qqq_engine::{AppConfig, NOTE_MIN_SIZE};
use std::{
    hash::{DefaultHasher, Hash, Hasher},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::SystemTime,
};

/// Height of the header with the note's title and buttons, in points.
const HEADER: f32 = 36.0;
/// Height of the status line, in points.
const STATUS: f32 = 22.0;
/// Side of the resize grip in the lower right corner, in points.
const GRIP: f32 = 16.0;
/// Distance of a new window from the work area's top and right edges, in points.
const MARGIN: f32 = 32.0;

const TOGGLE_VIEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::E);
const NEW_NOTE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);
const SAVE_NOW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);

/// Identity of the note window among the application's windows.
pub fn id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("qqq-note")
}

/// What the window shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    /// The Markdown source, for writing.
    Write,
    /// The rendered note.
    Read,
    /// Every note, to pick one.
    List,
}

pub struct NoteWindow {
    pub notes: Notes,
    /// The window is meant to be on the screen.
    shown: bool,
    view: View,
    /// The view the list returns to.
    back: View,
    /// The editor takes the keyboard focus on the next frame.
    focus: bool,
    /// The delete button waits for confirmation.
    confirm: bool,
    open_url: fn(&str) -> Result<(), Reason>,
    local_time: LocalTime,
    /// Why the latest link did not open.
    link_error: Option<Reason>,
    /// Hash of the highlighted text, its layout job and bold sections.
    highlighted: Option<(u64, LayoutJob, Vec<bool>)>,
}

/// The note window as both windows reach it. Both paint on the GUI thread, one at a time.
pub type Shared = Arc<Mutex<NoteWindow>>;

/// Locks the shared note window; a panic in an earlier pass leaves its state usable.
pub fn lock(shared: &Shared) -> MutexGuard<'_, NoteWindow> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

impl NoteWindow {
    pub fn new(notes: Notes, desktop: &Desktop) -> Self {
        Self {
            notes,
            shown: false,
            view: View::Write,
            back: View::Write,
            focus: false,
            confirm: false,
            open_url: desktop.open_url,
            local_time: desktop.local_time,
            link_error: None,
            highlighted: None,
        }
    }

    /// Shows the window over the other windows with the keyboard focus, or hides it when it
    /// already has the focus.
    pub fn toggle(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|i| {
            i.raw
                .viewports
                .get(&id())
                .and_then(|info| info.focused)
                .unwrap_or(false)
        });
        if self.shown && focused {
            self.hide(ctx);
        } else {
            self.show(ctx);
        }
    }

    /// Shows the window with the keyboard focus.
    pub fn show(&mut self, ctx: &egui::Context) {
        self.shown = true;
        if self.view == View::Write {
            self.focus = true;
        }
        ctx.send_viewport_cmd_to(id(), ViewportCommand::Visible(true));
        ctx.send_viewport_cmd_to(id(), ViewportCommand::Focus);
    }

    /// Shows a note in the window.
    pub fn open(&mut self, ctx: &egui::Context, note: u64) {
        self.notes.select(note);
        if self.view == View::List {
            self.view = self.back;
        }
        self.confirm = false;
        self.show(ctx);
    }

    /// Writes every edit and takes the window off the screen.
    pub fn hide(&mut self, ctx: &egui::Context) {
        self.notes.flush(true);
        self.shown = false;
        self.confirm = false;
        ctx.send_viewport_cmd_to(id(), ViewportCommand::Visible(false));
    }

    /// Draws the window's content; called in the note window's own passes.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.notes.update();
        if ctx.input(|i| i.viewport().close_requested()) {
            self.hide(&ctx);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&TOGGLE_VIEW)) {
            self.switch_view();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&NEW_NOTE)) {
            self.create();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&SAVE_NOW)) {
            self.notes.flush(true);
        }
        let window = ui.max_rect();
        self.header(ui);
        self.status(ui);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::PAPER)
                    .inner_margin(egui::Margin {
                        left: 14,
                        right: 10,
                        top: 10,
                        bottom: 6,
                    }),
            )
            .show(ui, |ui| {
                if self.confirm {
                    self.confirm_delete(ui);
                }
                match self.view {
                    View::Write => self.write(ui),
                    View::Read => self.read(ui),
                    View::List => self.list(ui),
                }
            });
        grip(ui, window);
        // A frameless window has no border of its own.
        let border = pixel::unit(&ctx, 2.0);
        ui.painter().rect_stroke(
            pixel::snap_rect(&ctx, window),
            0.0,
            Stroke::new(border, theme::INK),
            egui::StrokeKind::Inside,
        );
        if let Some(left) = self.notes.flush(false) {
            ctx.request_repaint_after(left);
        }
    }

    fn switch_view(&mut self) {
        self.view = match self.view {
            View::Write => View::Read,
            View::Read => View::Write,
            View::List => self.back,
        };
        self.focus = self.view == View::Write;
    }

    /// Starts a new note in the editor.
    pub fn create(&mut self) {
        self.notes.create();
        self.view = View::Write;
        self.focus = true;
        self.confirm = false;
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
        egui::Panel::top("note-header")
            .exact_size(HEADER)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::NIGHT)
                    .inner_margin(egui::Margin {
                        left: 10,
                        right: 8,
                        top: 6,
                        bottom: 4,
                    }),
            )
            .show(ui, |ui| {
                let ctx = ui.ctx().clone();
                // The whole header, margins included, moves the window; its buttons sit on
                // top.
                let rect = ui.max_rect();
                let header =
                    egui::Rect::from_min_max(rect.min - vec2(10.0, 6.0), rect.max + vec2(8.0, 4.0));
                let drag = ui.interact(header, ui.id().with("drag"), Sense::drag());
                if drag.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    pixel::icon(ui, &pixel::ICON_NOTE, 1.5, theme::NIGHT_TEXT, theme::GOLD);
                    ui.add_space(4.0);
                    let size = vec2(26.0, 22.0);
                    let mut close = false;
                    let mut delete = false;
                    let mut new = false;
                    let mut view = false;
                    let mut list = false;
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        close = theme::icon_button(ui, &pixel::ICON_CLOSE, t.note_hide, size, true)
                            .clicked();
                        delete =
                            theme::icon_button(ui, &pixel::ICON_TRASH, t.note_delete, size, true)
                                .clicked();
                        let (icon, label) = match self.view {
                            View::Read => (&pixel::ICON_PEN, t.note_write),
                            _ => (&pixel::ICON_EYE, t.note_read),
                        };
                        view = theme::icon_button(ui, icon, label, size, true).clicked();
                        new = theme::icon_button(ui, &pixel::ICON_PLUS, t.note_new, size, true)
                            .clicked();
                        list = theme::icon_button(ui, &pixel::ICON_LIST, t.note_list, size, true)
                            .clicked();
                        // The title takes the room the buttons leave.
                        let title = match self.view {
                            View::List => t.note_list,
                            _ => match self.notes.current().title() {
                                "" => t.note_untitled,
                                title => title,
                            },
                        };
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(title).size(14.0).color(theme::NIGHT_TEXT),
                                )
                                .truncate()
                                .selectable(false),
                            );
                        });
                    });
                    if close {
                        self.hide(&ctx);
                    }
                    if delete {
                        self.confirm = !self.confirm;
                    }
                    if view {
                        self.switch_view();
                    }
                    if new {
                        self.create();
                    }
                    if list {
                        if self.view == View::List {
                            self.view = self.back;
                        } else {
                            self.back = self.view;
                            self.view = View::List;
                        }
                        self.confirm = false;
                    }
                });
            });
    }

    fn status(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
        egui::Panel::bottom("note-status")
            .exact_size(STATUS)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::WELL)
                    .inner_margin(egui::Margin {
                        left: 12,
                        right: GRIP as i8 + 4,
                        top: 2,
                        bottom: 2,
                    }),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let small =
                        |text: String, color: Color32| RichText::new(text).size(11.0).color(color);
                    let note = self.notes.current();
                    ui.label(small(
                        format!("Markdown · {}", (t.note_chars)(note.text.chars().count())),
                        theme::MUTED,
                    ));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (text, color) = if let Some(error) = &self.notes.error {
                            let text = match error {
                                NoteError::Save(error) => (t.note_save_failed)(error),
                                NoteError::Delete(error) => {
                                    i18n::notice(&qqq_engine::Notice::NoteFile(error.clone()))
                                }
                            };
                            (text, theme::ERROR)
                        } else if let Some(reason) = &self.link_error {
                            (
                                i18n::notice(&qqq_engine::Notice::OpenFailed(reason.clone())),
                                theme::ERROR,
                            )
                        } else if !self.notes.saved() {
                            (t.note_saving.into(), theme::MUTED)
                        } else if note.is_blank() {
                            (String::new(), theme::MUTED)
                        } else {
                            (t.note_saved.into(), theme::GOOD)
                        };
                        ui.add(egui::Label::new(small(text, color)).truncate());
                    });
                });
            });
    }

    fn confirm_delete(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.label(RichText::new(t.note_delete_confirm).color(theme::ERROR));
            let size =
                |ui: &egui::Ui, text: &str| vec2(label_width(ui, &[text], 14.0) + 26.0, 28.0);
            let yes = size(ui, t.delete);
            if theme::pixel_button(ui, t.delete, None, true, yes).clicked() {
                let id = self.notes.current;
                self.notes.delete(id);
                self.confirm = false;
                self.view = if self.notes.current().is_blank() {
                    View::Write
                } else {
                    View::Read
                };
                self.focus = self.view == View::Write;
            }
            let no = size(ui, t.cancel);
            if theme::pixel_button(ui, t.cancel, None, false, no).clicked() {
                self.confirm = false;
            }
        });
        ui.add_space(6.0);
    }

    fn write(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
        let note_id = self.notes.current;
        let mut highlighted = self.highlighted.take();
        let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap: f32| {
            let text = text.as_str();
            let mut hasher = DefaultHasher::new();
            text.hash(&mut hasher);
            let hash = hasher.finish();
            if highlighted.as_ref().is_none_or(|(h, _, _)| *h != hash) {
                let (job, bold) = markdown::highlight(text);
                highlighted = Some((hash, job, bold));
            }
            let (_, job, _) = highlighted.as_ref().expect("highlighted above");
            let mut job = job.clone();
            job.wrap.max_width = wrap;
            ui.painter().layout_job(job)
        };
        let mut focus = std::mem::take(&mut self.focus);
        let shown = egui::ScrollArea::vertical()
            .id_salt(("note-write", note_id))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let note = self.notes.current_mut();
                let edit = egui::TextEdit::multiline(&mut note.text)
                    .id(egui::Id::new(("note-text", note_id)))
                    .frame(egui::Frame::NONE)
                    .desired_width(f32::INFINITY)
                    .min_size(vec2(0.0, ui.available_height()))
                    .lock_focus(true)
                    .hint_text(RichText::new(t.note_hint).color(theme::MUTED))
                    .layouter(&mut layouter);
                let output = edit.show(ui);
                if std::mem::take(&mut focus) {
                    output.response.response.request_focus();
                }
                (
                    output.response.response.changed(),
                    output.galley.job.wrap.max_width,
                    output.galley_pos,
                    output.text_clip_rect.intersect(ui.clip_rect()),
                )
            });
        let (changed, wrap, pos, clip) = shown.inner;
        if let Some((_, job, bold)) = &highlighted {
            let mut job = job.clone();
            job.wrap.max_width = wrap;
            markdown::embolden(&ui.painter_at(clip), &job, bold, pos);
        }
        self.highlighted = highlighted;
        if changed {
            self.notes.edited();
        }
    }

    fn read(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
        let note_id = self.notes.current;
        let mut action = None;
        let mut write = false;
        egui::ScrollArea::vertical()
            .id_salt(("note-read", note_id))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let text = &self.notes.current().text;
                if text.trim().is_empty() {
                    ui.label(RichText::new(t.note_read_empty).color(theme::MUTED));
                } else {
                    action = markdown::show(ui, text);
                }
                // The room below the note starts writing on a double click.
                let rest = ui.available_size().max(vec2(0.0, 24.0));
                write = ui.allocate_response(rest, Sense::click()).double_clicked();
            });
        match action {
            Some(markdown::Action::Toggle(at)) => {
                markdown::toggle(&mut self.notes.current_mut().text, at);
                self.notes.edited();
            }
            Some(markdown::Action::Open(url)) => {
                self.link_error = (self.open_url)(&url).err();
            }
            None => {}
        }
        if write {
            self.view = View::Write;
            self.focus = true;
        }
    }

    fn list(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .id_salt("note-list")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for note in &self.notes.entries {
                    let current = note.id == self.notes.current;
                    if row(ui, note, current, self.local_time).clicked() {
                        chosen = Some(note.id);
                    }
                }
                if !self.notes.listed {
                    ui.label(RichText::new(t.note_loading).color(theme::MUTED));
                }
            });
        if let Some(id) = chosen {
            self.notes.select(id);
            self.confirm = false;
            self.view = if self.notes.current().is_blank() {
                View::Write
            } else {
                self.back
            };
            self.focus = self.view == View::Write;
        }
    }
}

/// A note in the list: its title and when it changed.
fn row(ui: &mut egui::Ui, note: &Note, current: bool, local_time: LocalTime) -> egui::Response {
    let t = i18n::t();
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 44.0), Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if ui.is_rect_visible(rect) {
        let ctx = ui.ctx();
        let fill = if current {
            theme::TEAL_SOFT
        } else if response.hovered() {
            theme::WELL
        } else {
            theme::PAPER
        };
        let outline = if current || response.hovered() {
            theme::INK
        } else {
            theme::LINE
        };
        ui.painter()
            .add(pixel::panel(ctx, rect, fill, outline, None));
        let title = match note.title() {
            "" => t.note_untitled,
            title => title,
        };
        let inner = rect.shrink2(vec2(10.0, 5.0));
        let painter = ui.painter_at(inner);
        let galley = painter.layout_no_wrap(title.into(), FontId::proportional(14.0), theme::INK);
        painter.galley(inner.left_top(), galley, theme::INK);
        let when = changed(note.modified, local_time);
        painter.text(
            inner.left_bottom(),
            egui::Align2::LEFT_BOTTOM,
            when,
            FontId::proportional(11.0),
            theme::MUTED,
        );
    }
    response
}

/// When a note changed: how long ago within a day, its local date and time before that.
pub fn changed(when: SystemTime, local_time: LocalTime) -> String {
    let secs = SystemTime::now()
        .duration_since(when)
        .map_or(0, |d| d.as_secs());
    match local_time(when) {
        Some(parts) if secs >= 86400 => timestamp(parts),
        _ => ago(secs),
    }
}

/// The lower right corner, dragged to resize the window.
fn grip(ui: &mut egui::Ui, window: egui::Rect) {
    let rect = egui::Rect::from_min_max(window.max - Vec2::splat(GRIP), window.max);
    let response = ui
        .interact(rect, ui.id().with("note-grip"), Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeNwSe);
    if response.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::BeginResize(
            egui::viewport::ResizeDirection::SouthEast,
        ));
    }
    let ctx = ui.ctx();
    let dot = pixel::unit(ctx, 2.0);
    let color = if response.hovered() {
        theme::PINK
    } else {
        theme::MUTED
    };
    // Three rows of dots stepping down to the corner.
    for (x, y) in [(2, 0), (1, 1), (2, 1), (0, 2), (1, 2), (2, 2)] {
        let at = pixel::snap(
            ctx,
            pos2(
                rect.right() - 5.0 - (2 - x) as f32 * 4.0 - dot,
                rect.bottom() - 5.0 - (2 - y) as f32 * 4.0 - dot,
            ),
        );
        ui.painter()
            .rect_filled(egui::Rect::from_min_size(at, Vec2::splat(dot)), 0.0, color);
    }
}

/// The note window as it is first created: hidden, frameless and above other windows, at
/// the saved place or near the top right corner of the main window's work area.
pub fn builder(
    config: &AppConfig,
    ctx: &egui::Context,
    desktop: &Desktop,
) -> egui::ViewportBuilder {
    let size = Vec2::from(config.note_size);
    let builder = egui::ViewportBuilder::default()
        .with_title(i18n::t().note_title)
        .with_inner_size(size)
        .with_min_inner_size(NOTE_MIN_SIZE)
        .with_decorations(false)
        .with_resizable(true)
        .with_always_on_top()
        .with_taskbar(false)
        .with_visible(false);
    let position = config.note_position.map(egui::Pos2::from).or_else(|| {
        let ppp = ctx.pixels_per_point();
        let center = ctx.input(|i| i.viewport().outer_rect)?.center() * ppp;
        let (_, work) = (desktop.monitor_at)([center.x, center.y])?;
        Some(pos2(
            work[2] / ppp - size.x - MARGIN,
            work[1] / ppp + MARGIN,
        ))
    });
    match position {
        Some(position) => builder.with_position(position),
        None => builder,
    }
}

/// Saves the note window's place and size.
pub fn store(ctx: &egui::Context, config: &mut AppConfig) {
    let rects = ctx.input(|i| {
        let info = i.raw.viewports.get(&id())?;
        Some((info.outer_rect?, info.inner_rect?))
    });
    if let Some((outer, inner)) = rects {
        config.note_position = Some(outer.min.into());
        config.note_size = [
            inner.width().max(NOTE_MIN_SIZE[0]),
            inner.height().max(NOTE_MIN_SIZE[1]),
        ];
    }
}
