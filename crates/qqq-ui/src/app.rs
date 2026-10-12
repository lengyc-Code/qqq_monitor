#[path = "pages/mod.rs"]
mod pages;

use crate::{components, dock, mascot, minibar, pixel, theme};
use eframe::egui::{self, RichText, pos2, vec2};
use qqq_domain::{DeviceKind, MetricKey};
use qqq_engine::{AppConfig, CommandOutcome, DockEdge, MonitorClient, MonitorCommand, MonitorView};
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
    Disk,
    Network,
    Settings,
}
impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "系统总览",
            Self::Cpu => "处理器",
            Self::Memory => "物理内存",
            Self::Gpu => "显卡",
            Self::Disk => "磁盘与存储",
            Self::Network => "网络流量",
            Self::Settings => "设置",
        }
    }
    fn kicker(self) -> &'static str {
        match self {
            Self::Overview => "OVERVIEW",
            Self::Cpu => "PROCESSOR",
            Self::Memory => "MEMORY",
            Self::Gpu => "GPU",
            Self::Disk => "STORAGE",
            Self::Network => "NETWORK",
            Self::Settings => "CONFIG",
        }
    }
    fn hint(self) -> &'static str {
        match self {
            Self::Overview => "实时掌握电脑的运行状态",
            Self::Cpu => "观察整体负载与每个逻辑核心",
            Self::Memory => "系统可用物理内存与使用趋势",
            Self::Gpu => "显卡利用率、各引擎占用与显存",
            Self::Disk => "物理磁盘读写与本地卷容量",
            Self::Network => "查看所选网卡的实时收发",
            Self::Settings => "按你的使用习惯调整监控",
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
    message: String,
    latest_save: Option<u64>,
    capture: Option<CaptureOptions>,
    capture_requested: bool,
    started: Instant,
    closing: bool,
    dock: dock::Dock,
    desktop: dock::Desktop,
}

impl DesktopApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        client: MonitorClient,
        config: AppConfig,
        capture: Option<CaptureOptions>,
        desktop: dock::Desktop,
    ) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx, config.animations);
        let context = cc.egui_ctx.clone();
        client.set_ui_wake(Arc::new(move || context.request_repaint()));
        let view = client
            .try_latest_view()
            .unwrap_or_else(|| Arc::new(MonitorView::empty(config.clone())));
        let page = capture.as_ref().map(|v| v.page).unwrap_or(Page::Overview);
        Self {
            dock: dock::Dock::new(&config),
            desktop,
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
            message: String::new(),
            latest_save: None,
            capture,
            capture_requested: false,
            started: Instant::now(),
            closing: false,
        }
    }
    fn command(&mut self, command: MonitorCommand) {
        if let Err(error) = self.client.try_command(command) {
            self.message = error;
        }
    }
    fn save_config(&mut self, config: AppConfig, explicit: bool) {
        if let Err(error) = config.validate() {
            self.message = error;
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
            Err(error) => self.message = error,
        }
    }
    fn select_initial_devices(&mut self) {
        if self.network.is_empty() {
            let preferred = self.config.selected_network.as_ref().and_then(|guid| {
                self.view.devices.values().find(|d| {
                    d.kind == DeviceKind::Network && d.persistent_id.as_ref() == Some(guid)
                })
            });
            let selected = preferred
                .or_else(|| {
                    self.view.devices.values().find(|d| {
                        d.kind == DeviceKind::Network
                            && d.online
                            && d.details.iter().any(|(_, value)| value == "物理网卡")
                    })
                })
                .or_else(|| {
                    self.view
                        .devices
                        .values()
                        .find(|d| d.kind == DeviceKind::Network && d.online)
                })
                .or_else(|| {
                    self.view
                        .devices
                        .values()
                        .find(|d| d.kind == DeviceKind::Network)
                });
            if let Some(device) = selected {
                self.network = device.id.clone();
            }
        }
        if self.disk.is_empty()
            && let Some(device) = self
                .view
                .devices
                .values()
                .find(|d| d.kind == DeviceKind::Disk && d.online)
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
            Page::Settings => Vec::new(),
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
                Err(error) => self.message = error,
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
                        RichText::new("性能监控")
                            .size(11.0)
                            .color(theme::NIGHT_MUTED),
                    );
                });
                for (page, icon, name) in [
                    (Page::Overview, &pixel::ICON_STAR, "总览"),
                    (Page::Cpu, &pixel::ICON_CPU, "CPU"),
                    (Page::Memory, &pixel::ICON_MEMORY, "内存"),
                    (Page::Gpu, &pixel::ICON_GPU, "GPU"),
                    (Page::Disk, &pixel::ICON_DISK, "磁盘"),
                    (Page::Network, &pixel::ICON_NETWORK, "网络"),
                    (Page::Settings, &pixel::ICON_GEAR, "设置"),
                ] {
                    if theme::nav_button(
                        ui,
                        name,
                        icon,
                        self.page == page,
                        vec2(ui.available_width(), 40.0),
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
                ui.heading(self.page.title());
                ui.label(
                    RichText::new(self.page.hint())
                        .color(theme::MUTED)
                        .size(12.0),
                );
            });
            if self.page != Page::Settings {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (secs, label) in [(900, "15 分钟"), (300, "5 分钟"), (60, "60 秒")] {
                        if theme::chip(ui, label, self.range == secs).clicked() {
                            self.range = secs;
                        }
                    }
                    ui.label(RichText::new("历史范围").size(12.0).color(theme::MUTED));
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
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if let Some(view) = self.client.try_latest_view() {
            self.view = view;
        }
        for result in self.client.take_results() {
            if self.latest_save.is_some_and(|id| result.id < id) {
                continue;
            }
            match result.result {
                Err(error) => self.message = error,
                Ok(CommandOutcome::Saved) if self.latest_save == Some(result.id) => {
                    self.message.clear();
                }
                _ => {}
            }
        }
        self.select_initial_devices();
        self.query();
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
        let view = Arc::clone(&self.view);
        if let Some(edge) = self.dock.edge() {
            if minibar::show(ui, &view, edge, &self.gpu) {
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
                        ("监控运行中", theme::TEAL)
                    } else {
                        ("部分采集不可用", theme::GOLD)
                    };
                    ui.spacing_mut().item_spacing.x = 6.0;
                    components::pixel_dot(ui, color);
                    ui.label(RichText::new(text).size(12.0).color(color));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "采样 {:.1}s  ·  会话 {:.0}s",
                            view.config.sampling_ms as f64 / 1000.0,
                            view.elapsed
                        ))
                        .size(12.0)
                        .color(theme::NIGHT_MUTED),
                    );
                    if !self.message.is_empty() {
                        ui.add_space(8.0);
                        ui.label(RichText::new(&self.message).size(12.0).color(theme::GOLD));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for (edge, icon, label) in [
                            (DockEdge::Right, &pixel::ICON_DOCK_RIGHT, "吸附到屏幕右侧"),
                            (DockEdge::Left, &pixel::ICON_DOCK_LEFT, "吸附到屏幕左侧"),
                            (DockEdge::Top, &pixel::ICON_DOCK_TOP, "吸附到屏幕顶部"),
                        ] {
                            if theme::icon_button(ui, icon, label, vec2(26.0, 22.0), true).clicked()
                            {
                                self.dock.dock_to(ui.ctx(), &self.desktop, edge);
                            }
                        }
                        ui.label(
                            RichText::new("迷你状态栏")
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
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        match self.page {
                            Page::Overview => self.overview(ui, &view),
                            Page::Cpu => self.cpu(ui, &view),
                            Page::Memory => self.memory(ui, &view),
                            Page::Gpu => self.gpu(ui, &view),
                            Page::Disk => self.disk(ui, &view),
                            Page::Network => self.network(ui, &view),
                            Page::Settings => self.settings(ui, &view),
                        }
                        ui.add_space(8.0);
                    });
            });
        self.query();
    }
}
