#[path = "pages/mod.rs"]
mod pages;

use crate::{clips, components, dock, i18n, mascot, minibar, pixel, shots, theme, tray};
use eframe::egui::{self, RichText, pos2, vec2};
use qqq_domain::{DeviceKind, Hotkey, MetricKey};
use qqq_engine::{
    AlertState, AppConfig, CommandOutcome, DockEdge, Language, MonitorClient, MonitorCommand,
    MonitorView, Notice, data_directory,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Overview,
    Cpu,
    Memory,
    Gpu,
    Frames,
    Disk,
    Network,
    Processes,
    Remote,
    Alerts,
    Screenshots,
    Clipboard,
    Settings,
}
impl Page {
    fn text(self) -> &'static i18n::PageText {
        let t = i18n::t();
        match self {
            Self::Overview => &t.overview_page,
            Self::Cpu => &t.cpu_page,
            Self::Memory => &t.memory_page,
            Self::Gpu => &t.gpu_page,
            Self::Frames => &t.frames_page,
            Self::Disk => &t.disk_page,
            Self::Network => &t.network_page,
            Self::Processes => &t.processes_page,
            Self::Remote => &t.remote_page,
            Self::Alerts => &t.alerts_page,
            Self::Screenshots => &t.screenshots_page,
            Self::Clipboard => &t.clipboard_page,
            Self::Settings => &t.settings_page,
        }
    }
    /// Bitmap-font label above the title, the same in every language.
    fn kicker(self) -> &'static str {
        match self {
            Self::Overview => "OVERVIEW",
            Self::Cpu => "PROCESSOR",
            Self::Memory => "MEMORY",
            Self::Gpu => "GPU",
            Self::Frames => "FRAMES",
            Self::Disk => "STORAGE",
            Self::Network => "NETWORK",
            Self::Processes => "PROCESSES",
            Self::Remote => "REMOTE",
            Self::Alerts => "ALERTS",
            Self::Screenshots => "CAPTURE",
            Self::Clipboard => "CLIPBOARD",
            Self::Settings => "CONFIG",
        }
    }
}

pub struct CaptureOptions {
    pub path: PathBuf,
    pub page: Page,
}
pub struct DesktopApp {
    client: MonitorClient,
    view: Arc<MonitorView>,
    page: Page,
    config: AppConfig,
    active_config: AppConfig,
    network: String,
    disk: String,
    core: String,
    gpu: String,
    /// Metric name of the engine whose history the GPU page charts, e.g. `engine:3d`.
    gpu_engine: String,
    range: u64,
    last_query: Vec<MetricKey>,
    last_range: u64,
    message: Option<Notice>,
    latest_save: Option<u64>,
    capture: Option<CaptureOptions>,
    capture_requested: bool,
    started: Instant,
    closing: bool,
    dock: dock::Dock,
    desktop: dock::Desktop,
    tray: tray::Tray,
    process: pages::processes::ProcessPage,
    remote: pages::remote::RemotePage,
    alerts: pages::alerts::AlertPage,
    /// Language the "follow system" setting resolves to.
    system_language: Language,
    /// Whether the program starts at sign-in, read when the settings page opens.
    autostart: Option<bool>,
    shots: shots::Shots,
    /// Number of screenshots the library was last told to keep.
    shots_kept: usize,
    snip: pages::screenshots::Snip,
    screenshot_page: pages::screenshots::ScreenshotPage,
    /// Screenshot shortcut last registered; `None` until the first registration.
    hotkey: Option<Option<Hotkey>>,
    clips: clips::Clips,
    /// Number of clipboard entries last kept.
    clips_kept: usize,
    /// Whether the clipboard is followed; `None` until first decided.
    watching: Option<bool>,
}

impl DesktopApp {
    /// Starts on `page`, or the overview; a capture starts on its own page.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        client: MonitorClient,
        config: AppConfig,
        page: Option<Page>,
        capture: Option<CaptureOptions>,
        desktop: dock::Desktop,
        system_language: Language,
        start_in_tray: bool,
    ) -> Self {
        i18n::set(config.language.unwrap_or(system_language));
        theme::install_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx, config.animations);
        let context = cc.egui_ctx.clone();
        client.set_ui_wake(Arc::new(move || context.request_repaint()));
        let view = client
            .try_latest_view()
            .unwrap_or_else(|| Arc::new(MonitorView::empty(config.clone())));
        let page = capture
            .as_ref()
            .map(|v| v.page)
            .or(page)
            .unwrap_or(Page::Overview);
        Self {
            dock: dock::Dock::new(&config),
            shots: shots::Shots::new(
                &cc.egui_ctx,
                data_directory().join("screenshots"),
                config.screenshot_history,
                desktop.local_time,
            ),
            shots_kept: config.screenshot_history,
            snip: Default::default(),
            screenshot_page: Default::default(),
            hotkey: None,
            clips: Default::default(),
            clips_kept: config.clipboard_history,
            watching: None,
            desktop,
            tray: if start_in_tray {
                tray::Tray::hidden(&desktop)
            } else {
                tray::Tray::default()
            },
            process: Default::default(),
            remote: Default::default(),
            alerts: Default::default(),
            system_language,
            autostart: None,
            range: config.history_secs,
            active_config: config.clone(),
            config,
            client,
            view,
            page,
            network: String::new(),
            disk: String::new(),
            core: String::new(),
            gpu: String::new(),
            gpu_engine: String::new(),
            last_query: Vec::new(),
            last_range: 0,
            message: None,
            latest_save: None,
            capture,
            capture_requested: false,
            started: Instant::now(),
            closing: false,
        }
    }
    fn command(&mut self, command: MonitorCommand) {
        if let Err(error) = self.client.try_command(command) {
            self.message = Some(error);
        }
    }
    fn save_config(&mut self, config: AppConfig, explicit: bool) {
        if let Err(error) = config.validate() {
            self.message = Some(error);
            return;
        }
        match self.client.try_command(MonitorCommand::SaveConfig {
            config: config.clone(),
            explicit,
        }) {
            Ok(id) => {
                self.active_config = config;
                self.latest_save = Some(id);
            }
            Err(error) => self.message = Some(error),
        }
    }
    /// Applies a change to both the draft and the accepted settings and saves it at once,
    /// like the language choice.
    fn apply_now(&mut self, change: impl Fn(&mut AppConfig)) {
        change(&mut self.config);
        let mut config = self.active_config.clone();
        change(&mut config);
        self.save_config(config, true);
    }
    fn select_initial_devices(&mut self) {
        // Devices of this computer; remote hosts have their own page.
        let local = || self.view.devices.values().filter(|d| d.host.is_none());
        if self.network.is_empty() {
            let preferred = self.config.selected_network.as_ref().and_then(|guid| {
                local().find(|d| {
                    d.kind == DeviceKind::Network && d.persistent_id.as_ref() == Some(guid)
                })
            });
            let selected = preferred
                .or_else(|| {
                    local().find(|d| {
                        d.kind == DeviceKind::Network
                            && d.online
                            && d.details
                                .iter()
                                .any(|(key, value)| key == "interface_type" && value == "physical")
                    })
                })
                .or_else(|| local().find(|d| d.kind == DeviceKind::Network && d.online))
                .or_else(|| local().find(|d| d.kind == DeviceKind::Network));
            if let Some(device) = selected {
                self.network = device.id.clone();
            }
        }
        if self.disk.is_empty()
            && let Some(device) = local().find(|d| d.kind == DeviceKind::Disk && d.online)
        {
            self.disk = device.id.clone();
        }
        if self.gpu.is_empty()
            && let Some(device) = components::primary_gpu(&self.view)
        {
            self.gpu = device.id.clone();
        }
    }
    fn query(&mut self) {
        let keys = match self.page {
            Page::Overview => vec![
                MetricKey::new("cpu", "usage"),
                MetricKey::new("memory", "usage"),
                MetricKey::new(&self.disk, "read"),
                MetricKey::new(&self.disk, "write"),
                MetricKey::new(&self.network, "receive"),
                MetricKey::new(&self.network, "send"),
            ],
            Page::Cpu => {
                let mut keys = vec![MetricKey::new("cpu", "usage")];
                if !self.core.is_empty() {
                    keys.push(MetricKey::new(&self.core, "usage"));
                }
                keys
            }
            Page::Memory => vec![MetricKey::new("memory", "usage")],
            Page::Gpu => {
                let mut keys = vec![
                    MetricKey::new(&self.gpu, "usage"),
                    MetricKey::new(&self.gpu, "memory_usage"),
                ];
                if !self.gpu_engine.is_empty() {
                    keys.push(MetricKey::new(&self.gpu, &self.gpu_engine));
                }
                keys
            }
            Page::Disk => vec![
                MetricKey::new(&self.disk, "read"),
                MetricKey::new(&self.disk, "write"),
            ],
            Page::Network => vec![
                MetricKey::new(&self.network, "receive"),
                MetricKey::new(&self.network, "send"),
            ],
            Page::Frames => ["fps", "frame_time", "frame_time_max"]
                .map(|metric| MetricKey::new("frames", metric))
                .into(),
            Page::Remote => self.remote_query(),
            Page::Processes
            | Page::Alerts
            | Page::Screenshots
            | Page::Clipboard
            | Page::Settings => Vec::new(),
        };
        if keys != self.last_query || self.range != self.last_range {
            match self.client.try_command(MonitorCommand::SetQuery {
                keys: keys.clone(),
                range_secs: self.range,
            }) {
                Ok(_) => {
                    self.last_query = keys;
                    self.last_range = self.range;
                }
                Err(error) => self.message = Some(error),
            }
        }
    }
    fn sidebar(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let panel = egui::Panel::left("navigation")
            .exact_size(172.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::NIGHT)
                    .inner_margin(egui::Margin {
                        left: 14,
                        right: 16,
                        top: 16,
                        bottom: 10,
                    }),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    pixel::label(
                        ui,
                        "QQQ",
                        4.0,
                        theme::TEAL,
                        pixel::Effect::Drop(theme::PINK),
                    );
                    pixel::icon(ui, &pixel::ICON_SPARKLE, 1.5, theme::GOLD, theme::GOLD_SOFT);
                });
                pixel::label(ui, "MONITOR", 2.0, theme::NIGHT_MUTED, pixel::Effect::Plain);
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    pixel::icon(ui, &pixel::ICON_HEART, 1.0, theme::PINK_SOFT, theme::PINK);
                    ui.label(
                        RichText::new(i18n::t().tagline)
                            .size(11.0)
                            .color(theme::NIGHT_MUTED),
                    );
                });
                let pages = [
                    (Page::Overview, &pixel::ICON_STAR),
                    (Page::Cpu, &pixel::ICON_CPU),
                    (Page::Memory, &pixel::ICON_MEMORY),
                    (Page::Gpu, &pixel::ICON_GPU),
                    (Page::Frames, &pixel::ICON_FRAMES),
                    (Page::Disk, &pixel::ICON_DISK),
                    (Page::Network, &pixel::ICON_NETWORK),
                    (Page::Processes, &pixel::ICON_PROCESS),
                    (Page::Remote, &pixel::ICON_REMOTE),
                    (Page::Alerts, &pixel::ICON_BELL),
                    (Page::Screenshots, &pixel::ICON_CAMERA),
                    (Page::Clipboard, &pixel::ICON_CLIPBOARD),
                    (Page::Settings, &pixel::ICON_GEAR),
                ];
                // Buttons shrink from 40 towards 26 points on shorter windows, leaving room
                // for the mascot card below them; one point absorbs pixel rounding.
                let room = ui.available_height() - mascot::room(ui) - 1.0;
                let pitch = (room / pages.len() as f32).clamp(32.0, 46.0);
                let height = pitch - ui.spacing().item_spacing.y;
                for (page, icon) in pages {
                    if theme::nav_button(
                        ui,
                        page.text().nav,
                        icon,
                        self.page == page,
                        vec2(ui.available_width(), height),
                    )
                    .clicked()
                    {
                        self.page = page;
                    }
                }
                mascot::show(ui, view, self.config.animations);
            });
        let rect = panel.response.rect;
        let border = pixel::unit(ui.ctx(), 2.0);
        ui.painter().rect_filled(
            pixel::snap_rect(
                ui.ctx(),
                egui::Rect::from_x_y_ranges(rect.right() - border..=rect.right(), rect.y_range()),
            ),
            0.0,
            theme::TEAL,
        );
    }
    fn header(&mut self, ui: &mut egui::Ui) {
        let t = i18n::t();
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    pixel::icon(ui, &pixel::ICON_SPARKLE, 1.0, theme::INK, theme::GOLD);
                    pixel::label(
                        ui,
                        self.page.kicker(),
                        1.5,
                        theme::PINK,
                        pixel::Effect::Plain,
                    );
                });
                ui.heading(self.page.text().title);
                ui.label(
                    RichText::new(self.page.text().hint)
                        .color(theme::MUTED)
                        .size(12.0),
                );
            });
            if !matches!(
                self.page,
                Page::Processes
                    | Page::Alerts
                    | Page::Screenshots
                    | Page::Clipboard
                    | Page::Settings
            ) {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (secs, label) in [
                        (900, (t.minutes)(15)),
                        (300, (t.minutes)(5)),
                        (60, (t.seconds)(60.0)),
                    ] {
                        if theme::chip(ui, &label, self.range == secs).clicked() {
                            self.range = secs;
                        }
                    }
                    ui.label(
                        RichText::new(t.history_range)
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                });
            }
        });
    }
}

/// Faint pixel dots with an occasional sparkle behind the page content.
fn backdrop(ui: &egui::Ui) {
    let ctx = ui.ctx();
    let rect = ui.clip_rect();
    let dot = pixel::unit(ctx, 2.0);
    let sparkle = pixel::unit(ctx, 1.0);
    let step = 32.0;
    let mut shapes = Vec::new();
    for row in 0..=(rect.height() / step) as usize {
        for column in 0..=(rect.width() / step) as usize {
            let offset = if row % 2 == 0 { 0.0 } else { step / 2.0 };
            let at = pixel::snap(
                ctx,
                pos2(
                    rect.left() + 12.0 + column as f32 * step + offset,
                    rect.top() + 12.0 + row as f32 * step,
                ),
            );
            if (row * 7 + column * 3) % 11 == 0 {
                pixel::icon_shapes(
                    &mut shapes,
                    at - egui::Vec2::splat(4.0 * sparkle),
                    sparkle,
                    &pixel::ICON_SPARKLE,
                    theme::PAGE_DOT,
                    theme::PAGE,
                );
            } else {
                shapes.push(egui::Shape::rect_filled(
                    egui::Rect::from_min_size(at, egui::Vec2::splat(dot)),
                    0.0,
                    theme::PAGE_DOT,
                ));
            }
        }
    }
    ui.painter().extend(shapes);
}

impl eframe::App for DesktopApp {
    fn on_exit(&mut self) {
        // A screenshot taken just before exiting is still written.
        self.shots.finish(Duration::from_secs(2));
    }
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if let Some(view) = self.client.try_latest_view() {
            self.view = view;
        }
        for result in self.client.take_results() {
            if self.process.finished(result.id) {
                self.process_result(result.result.map(|_| ()));
                continue;
            }
            if self.latest_save.is_some_and(|id| result.id < id) {
                continue;
            }
            match result.result {
                Err(error) => self.message = Some(error),
                Ok(CommandOutcome::Saved) if self.latest_save == Some(result.id) => {
                    self.message = None;
                }
                _ => {}
            }
        }
        let language = self.active_config.language.unwrap_or(self.system_language);
        if language != i18n::language() {
            i18n::set(language);
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(i18n::t().window_title.into()));
        }
        if self.page != Page::Settings {
            // Task Manager can switch starting at sign-in while the page is closed.
            self.autostart = None;
        }
        self.select_initial_devices();
        self.query();
        self.watch_processes(ctx);
        self.screenshot_logic(ctx);
        self.clipboard_logic(ctx);
        if self.page == Page::Alerts {
            // Windows notices a new notification identity with a delay, so it is set up
            // before the first alert.
            (self.desktop.prepare_notifications)();
        }
        if self.capture.is_none() {
            self.tray.update(
                ctx,
                &self.desktop,
                self.dock.edge().is_some(),
                self.active_config.minimize_to_tray,
            );
            if self.tray.opened(ctx, &self.desktop) {
                if self.dock.edge().is_some() {
                    self.dock.restore(ctx, &self.desktop, None);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }
        self.dock
            .set_frames(ctx, &self.desktop, self.active_config.frame_monitoring);
        self.dock
            .update(ctx, &self.desktop, self.active_config.edge_docking);
        if !self.closing && ctx.input(|i| i.viewport().close_requested()) {
            self.closing = true;
            if self.capture.is_none() {
                let mut config = self.active_config.clone();
                self.dock.store(ctx, &mut config);
                self.save_config(config, false);
            }
        }
        if self.capture.is_some()
            && self.started.elapsed() > Duration::from_secs(8)
            && !self.capture_requested
        {
            self.capture_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        if self.capture.is_some() && self.started.elapsed() > Duration::from_secs(20) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        for event in &input.events {
            if let egui::Event::Screenshot { image, .. } = event
                && let Some(capture) = &self.capture
            {
                let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                if let Err(error) = image::save_buffer(
                    &capture.path,
                    &pixels,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                ) {
                    eprintln!("截图保存失败：{error}");
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let t = i18n::t();
        let view = Arc::clone(&self.view);
        if let Some(edge) = self.dock.edge() {
            if minibar::show(ui, &view, edge, &self.gpu, view.config.frame_monitoring) {
                self.dock.restore(ui.ctx(), &self.desktop, None);
            }
            return;
        }
        let status = egui::Panel::bottom("status")
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::NIGHT)
                    .inner_margin(egui::Margin {
                        left: 14,
                        right: 14,
                        top: 7,
                        bottom: 5,
                    }),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let (text, color) = if view.diagnostics.collector_errors.is_empty() {
                        (t.running, theme::TEAL)
                    } else {
                        (t.degraded, theme::GOLD)
                    };
                    ui.spacing_mut().item_spacing.x = 6.0;
                    components::pixel_dot(ui, color);
                    ui.label(RichText::new(text).size(12.0).color(color));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new((t.timing)(
                            view.config.sampling_ms as f64 / 1000.0,
                            view.elapsed,
                        ))
                        .size(12.0)
                        .color(theme::NIGHT_MUTED),
                    );
                    let firing = view
                        .alerts
                        .states
                        .values()
                        .filter(|state| matches!(state, AlertState::Firing { .. }))
                        .count();
                    if firing > 0 {
                        ui.add_space(8.0);
                        components::pixel_dot(ui, theme::PINK);
                        let label = egui::Label::new(
                            RichText::new((t.alerts_firing)(firing))
                                .size(12.0)
                                .color(theme::PINK),
                        )
                        .sense(egui::Sense::click());
                        if ui
                            .add(label)
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            self.page = Page::Alerts;
                        }
                    }
                    if let Some(message) = &self.message {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(i18n::notice(message))
                                .size(12.0)
                                .color(theme::GOLD),
                        );
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for (edge, icon, label) in [
                            (DockEdge::Right, &pixel::ICON_DOCK_RIGHT, t.dock_right),
                            (DockEdge::Left, &pixel::ICON_DOCK_LEFT, t.dock_left),
                            (DockEdge::Top, &pixel::ICON_DOCK_TOP, t.dock_top),
                        ] {
                            if theme::icon_button(ui, icon, label, vec2(26.0, 22.0), true).clicked()
                            {
                                self.dock.dock_to(ui.ctx(), &self.desktop, edge);
                            }
                        }
                        ui.label(
                            RichText::new(t.minibar)
                                .size(12.0)
                                .color(theme::NIGHT_MUTED),
                        );
                    });
                });
            });
        let rect = status.response.rect;
        let border = pixel::unit(ui.ctx(), 2.0);
        ui.painter().rect_filled(
            pixel::snap_rect(
                ui.ctx(),
                egui::Rect::from_x_y_ranges(rect.x_range(), rect.top()..=rect.top() + border),
            ),
            0.0,
            theme::TEAL,
        );
        self.sidebar(ui, &view);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::PAGE)
                    .inner_margin(egui::Margin {
                        left: 18,
                        right: 14,
                        top: 14,
                        bottom: 4,
                    }),
            )
            .show(ui, |ui| {
                backdrop(ui);
                self.header(ui);
                ui.add_space(6.0);
                // The process table scrolls by itself and fills the page.
                if self.page == Page::Processes {
                    self.processes(ui, &view);
                    return;
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        match self.page {
                            Page::Overview => self.overview(ui, &view),
                            Page::Cpu => self.cpu(ui, &view),
                            Page::Memory => self.memory(ui, &view),
                            Page::Gpu => self.gpu(ui, &view),
                            Page::Frames => self.frames(ui, &view),
                            Page::Disk => self.disk(ui, &view),
                            Page::Network => self.network(ui, &view),
                            Page::Processes => {}
                            Page::Remote => self.remote(ui, &view),
                            Page::Alerts => self.alerts(ui, &view),
                            Page::Screenshots => self.screenshots(ui),
                            Page::Clipboard => self.clipboard(ui),
                            Page::Settings => self.settings(ui, &view),
                        }
                        ui.add_space(8.0);
                    });
            });
        self.query();
    }
}
