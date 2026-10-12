//! Edge docking: a window pushed against the top, left or right screen edge
//! turns into a compact always-on-top status bar. Dragging the bar along its
//! edge keeps it docked, dragging it onto another edge moves it there, and
//! dragging it away from every edge restores the full window at the drop point.
//!
//! Moves are judged once the window rests with no mouse button held, so the
//! system move loop and Windows snap layouts keep working: snapping or
//! maximizing resizes the window, and only a pure move docks it.

use crate::i18n;
use eframe::egui::{self, Pos2, Rect, Vec2, ViewportCommand, pos2, vec2};
use qqq_engine::{AppConfig, DockEdge};
use std::time::Duration;

/// Smallest inner size of the full window, in points.
pub const MIN_SIZE: Vec2 = vec2(960.0, 640.0);
/// Inner size of the horizontal bar on the top edge, in points.
pub const TOP_BAR: Vec2 = vec2(520.0, 34.0);
/// Inner size of the vertical bar on a side edge, in points.
pub const SIDE_BAR: Vec2 = vec2(52.0, 262.0);
/// How long the window rests before a move counts as finished, in seconds.
const SETTLE: f64 = 0.2;
/// Distance at which the full window reaches an edge; a window flush with it
/// differs only by its one-pixel frame.
const WINDOW_REACH: f32 = 2.0;
/// Distance within which a dragged bar stays on an edge.
const BAR_REACH: f32 = 24.0;

pub fn bar_size(edge: DockEdge) -> Vec2 {
    match edge {
        DockEdge::Top => TOP_BAR,
        DockEdge::Left | DockEdge::Right => SIDE_BAR,
    }
}

/// Rectangle edges as `[left, top, right, bottom]`.
pub type Edges = [f32; 4];

/// Monitor queries answered by the host platform, in physical pixels of the virtual screen.
#[derive(Clone, Copy)]
pub struct Desktop {
    /// Bounds and work area of the monitor nearest to a point.
    pub monitor_at: fn([f32; 2]) -> Option<(Edges, Edges)>,
    /// Whether a point lies on any monitor.
    pub covers: fn([f32; 2]) -> bool,
    /// Whether a mouse button is held, including while the system moves the window.
    pub button_held: fn() -> bool,
}

/// Work area of the monitor around a window, in points, and which of its
/// dockable edges border no other monitor.
struct Screen {
    work: Rect,
    top: bool,
    left: bool,
    right: bool,
}

impl Desktop {
    fn screen(&self, ppp: f32, around: Rect) -> Option<Screen> {
        let center = around.center() * ppp;
        let (bounds, work) = (self.monitor_at)([center.x, center.y])?;
        let open = |x: f32, y: f32| !(self.covers)([x, y]);
        Some(Screen {
            work: Rect::from_min_max(pos2(work[0], work[1]), pos2(work[2], work[3])) / ppp,
            top: open(center.x, bounds[1] - 1.0),
            left: open(bounds[0] - 1.0, center.y),
            // The right and bottom of a monitor rectangle are exclusive.
            right: open(bounds[2], center.y),
        })
    }
}

impl Screen {
    /// Open edge that `rect` reaches within `reach`, preferring the one it crosses furthest.
    fn reached(&self, rect: Rect, reach: f32) -> Option<DockEdge> {
        [
            (DockEdge::Top, self.top, self.work.top() - rect.top()),
            (DockEdge::Left, self.left, self.work.left() - rect.left()),
            (
                DockEdge::Right,
                self.right,
                rect.right() - self.work.right(),
            ),
        ]
        .into_iter()
        .filter(|(_, open, overshoot)| *open && *overshoot >= -reach)
        .max_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(edge, ..)| edge)
    }
    /// Bar position on `edge`, centred on `center` along it and kept inside the work area.
    fn place(&self, edge: DockEdge, size: Vec2, center: Pos2) -> Pos2 {
        let along = |center: f32, low: f32, high: f32, length: f32| {
            (center - length / 2.0).min(high - length).max(low)
        };
        let work = self.work;
        match edge {
            DockEdge::Top => pos2(
                along(center.x, work.left(), work.right(), size.x),
                work.top(),
            ),
            DockEdge::Left => pos2(
                work.left(),
                along(center.y, work.top(), work.bottom(), size.y),
            ),
            DockEdge::Right => pos2(
                work.right() - size.x,
                along(center.y, work.top(), work.bottom(), size.y),
            ),
        }
    }
    /// Corner that keeps a window of `size` at `at` inside the work area.
    fn clamp(&self, at: Pos2, size: Vec2) -> Pos2 {
        let work = self.work;
        pos2(
            at.x.min(work.right() - size.x).max(work.left()),
            at.y.min(work.bottom() - size.y).max(work.top()),
        )
    }
}

/// Full-window geometry to return to, in points.
#[derive(Clone, Copy)]
struct Normal {
    /// Outer position.
    position: Option<Pos2>,
    /// Inner size.
    size: Vec2,
    /// Outer size minus inner size: title bar and borders.
    frame: Vec2,
    maximized: bool,
}

pub struct Dock {
    edge: Option<DockEdge>,
    normal: Normal,
    /// Last observed outer rect and when it last changed.
    last: Option<Rect>,
    changed_at: f64,
    /// Outer rect before the current move; `None` while the window rests.
    moved_from: Option<Rect>,
    /// The next rest follows our own geometry commands, not a user move.
    commanded: bool,
}

/// Window geometry from the current frame's viewport info.
struct Window {
    outer: Rect,
    inner: Rect,
    maximized: bool,
    now: f64,
}

fn window(ctx: &egui::Context) -> Option<Window> {
    ctx.input(|i| {
        let viewport = i.viewport();
        if viewport.minimized == Some(true) {
            return None;
        }
        Some(Window {
            outer: viewport.outer_rect?,
            inner: viewport.inner_rect?,
            maximized: viewport.maximized == Some(true),
            now: i.time,
        })
    })
}

impl Dock {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            edge: config.dock,
            normal: Normal {
                position: config.window_position.map(Pos2::from),
                size: config.window_size.into(),
                frame: Vec2::ZERO,
                maximized: false,
            },
            last: None,
            changed_at: 0.0,
            moved_from: None,
            commanded: false,
        }
    }

    pub fn edge(&self) -> Option<DockEdge> {
        self.edge
    }

    /// Tracks the window and docks, re-aligns or restores it once a move ends.
    pub fn update(&mut self, ctx: &egui::Context, desktop: &Desktop, edge_docking: bool) {
        let Some(window) = window(ctx) else {
            return;
        };
        let Some(last) = self.last else {
            self.last = Some(window.outer);
            self.changed_at = window.now;
            match self.edge {
                // Started docked: put the bar onto its edge of the current monitor layout.
                Some(edge) => self.dock(ctx, desktop, edge, window.outer),
                None if !window.maximized => self.remember(&window),
                None => {}
            }
            return;
        };
        if !same(last, window.outer) {
            self.moved_from.get_or_insert(last);
            self.last = Some(window.outer);
            self.changed_at = window.now;
        }
        if self.moved_from.is_none() && !self.commanded {
            return;
        }
        if window.now - self.changed_at < SETTLE || (desktop.button_held)() {
            ctx.request_repaint_after(Duration::from_secs_f64(SETTLE / 2.0));
            return;
        }
        let commanded = std::mem::take(&mut self.commanded);
        let moved_from = self.moved_from.take();
        let screen = desktop.screen(ctx.pixels_per_point(), window.outer);
        if let (Some(from), Some(screen)) = (moved_from.filter(|_| !commanded), screen) {
            match self.edge {
                Some(_) => {
                    match screen.reached(window.outer, BAR_REACH) {
                        Some(edge) => self.dock(ctx, desktop, edge, window.outer),
                        None => self.restore(ctx, desktop, Some(window.outer.min)),
                    }
                    return;
                }
                None if edge_docking && !window.maximized && same_size(from, window.outer) => {
                    // The title bar belongs to the outer rect; the sides use the client
                    // area because the outer rect includes invisible resize borders.
                    let frame = Rect::from_min_max(
                        pos2(window.inner.left(), window.outer.top()),
                        window.inner.max,
                    );
                    if let Some(edge) = screen.reached(frame, WINDOW_REACH) {
                        self.dock(ctx, desktop, edge, window.outer);
                        return;
                    }
                }
                None => {}
            }
        }
        if self.edge.is_none() && !window.maximized {
            self.remember(&window);
        }
    }

    /// Docks the window to `edge` at once, e.g. from a toolbar button.
    pub fn dock_to(&mut self, ctx: &egui::Context, desktop: &Desktop, edge: DockEdge) {
        if let Some(window) = window(ctx) {
            self.dock(ctx, desktop, edge, window.outer);
        }
    }

    /// Restores the full window, at `at` or else where it was before docking.
    pub fn restore(&mut self, ctx: &egui::Context, desktop: &Desktop, at: Option<Pos2>) {
        let Normal {
            position,
            size,
            frame,
            maximized,
        } = self.normal;
        let current = window(ctx).map(|w| w.outer.min);
        let position = at.or(position).or(current).map(|at| {
            desktop
                .screen(
                    ctx.pixels_per_point(),
                    Rect::from_min_size(at, size + frame),
                )
                .map_or(at, |screen| screen.clamp(at, size + frame))
        });
        let mut commands = vec![
            ViewportCommand::Decorations(true),
            ViewportCommand::Resizable(true),
            ViewportCommand::WindowLevel(egui::WindowLevel::Normal),
            ViewportCommand::MinInnerSize(MIN_SIZE),
            ViewportCommand::InnerSize(size),
        ];
        commands.extend(position.map(ViewportCommand::OuterPosition));
        if maximized {
            commands.push(ViewportCommand::Maximized(true));
        }
        self.apply(ctx, commands);
        self.edge = None;
    }

    /// Saves the full-window geometry and the bar placement for the next start.
    pub fn store(&self, ctx: &egui::Context, config: &mut AppConfig) {
        config.dock = self.edge;
        let window = window(ctx);
        let (size, position) = match &window {
            Some(window) if self.edge.is_some() => {
                config.dock_position = Some(window.outer.min.into());
                (self.normal.size, self.normal.position)
            }
            Some(window) if !window.maximized => (window.inner.size(), Some(window.outer.min)),
            _ => (self.normal.size, self.normal.position),
        };
        config.window_size = [size.x.max(MIN_SIZE.x), size.y.max(MIN_SIZE.y)];
        if let Some(position) = position {
            config.window_position = Some(position.into());
        }
    }

    fn remember(&mut self, window: &Window) {
        self.normal = Normal {
            position: Some(window.outer.min),
            size: window.inner.size(),
            frame: window.outer.size() - window.inner.size(),
            maximized: false,
        };
    }

    /// Turns the window into the bar on `edge`, centred along it on `around`.
    fn dock(&mut self, ctx: &egui::Context, desktop: &Desktop, edge: DockEdge, around: Rect) {
        let Some(screen) = desktop.screen(ctx.pixels_per_point(), around) else {
            return;
        };
        let size = bar_size(edge);
        let mut commands = Vec::new();
        if self.edge.is_none() {
            self.normal.maximized = window(ctx).is_some_and(|w| w.maximized);
            if self.normal.maximized {
                commands.push(ViewportCommand::Maximized(false));
            }
            commands.extend([
                ViewportCommand::Decorations(false),
                ViewportCommand::Resizable(false),
                ViewportCommand::MinInnerSize(Vec2::ZERO),
                ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop),
            ]);
        }
        commands.extend([
            ViewportCommand::InnerSize(size),
            ViewportCommand::OuterPosition(screen.place(edge, size, around.center())),
        ]);
        self.apply(ctx, commands);
        self.edge = Some(edge);
    }

    fn apply(&mut self, ctx: &egui::Context, commands: Vec<ViewportCommand>) {
        for command in commands {
            ctx.send_viewport_cmd(command);
        }
        self.commanded = true;
        self.moved_from = None;
        self.changed_at = ctx.input(|i| i.time);
        ctx.request_repaint_after(Duration::from_secs_f64(SETTLE / 2.0));
    }
}

fn same(a: Rect, b: Rect) -> bool {
    (a.min - b.min).length() < 0.5 && same_size(a, b)
}

fn same_size(a: Rect, b: Rect) -> bool {
    (a.size() - b.size()).length() < 0.5
}

/// Initial window: the full window, or the bar when the last session ended docked.
pub fn viewport(config: &AppConfig) -> egui::ViewportBuilder {
    let builder = egui::ViewportBuilder::default().with_title(i18n::t().window_title);
    let (builder, position) = match config.dock {
        None => (
            builder
                .with_inner_size(config.window_size)
                .with_min_inner_size(MIN_SIZE),
            config.window_position,
        ),
        Some(edge) => (
            builder
                .with_inner_size(bar_size(edge))
                .with_decorations(false)
                .with_resizable(false)
                .with_always_on_top(),
            config.dock_position,
        ),
    };
    match position {
        Some(position) => builder.with_position(position),
        None => builder,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Screen {
        Screen {
            work: Rect::from_min_max(pos2(0.0, 0.0), pos2(1920.0, 1032.0)),
            top: true,
            left: true,
            right: false,
        }
    }

    #[test]
    fn reached_edge_prefers_the_deepest_open_edge() {
        let screen = screen();
        let window = |x: f32, y: f32| Rect::from_min_size(pos2(x, y), vec2(800.0, 600.0));
        assert_eq!(screen.reached(window(300.0, 200.0), WINDOW_REACH), None);
        assert_eq!(
            screen.reached(window(300.0, 1.5), WINDOW_REACH),
            Some(DockEdge::Top)
        );
        assert_eq!(
            screen.reached(window(-40.0, 1.0), WINDOW_REACH),
            Some(DockEdge::Left)
        );
        // Another monitor continues to the right, so that edge never docks.
        assert_eq!(screen.reached(window(1200.0, 300.0), BAR_REACH), None);
    }

    #[test]
    fn bar_is_placed_flush_and_inside_the_work_area() {
        let screen = screen();
        assert_eq!(
            screen.place(DockEdge::Top, TOP_BAR, pos2(960.0, 500.0)),
            pos2(700.0, 0.0)
        );
        assert_eq!(
            screen.place(DockEdge::Top, TOP_BAR, pos2(1900.0, 500.0)),
            pos2(1400.0, 0.0)
        );
        assert_eq!(
            screen.place(DockEdge::Right, SIDE_BAR, pos2(1800.0, 10.0)),
            pos2(1868.0, 0.0)
        );
        assert_eq!(
            screen.clamp(pos2(1500.0, 900.0), vec2(1280.0, 800.0)),
            pos2(640.0, 232.0)
        );
    }
}
