#[path = "pages/mod.rs"]
mod pages;

use crate::theme;
use eframe::egui::{self, RichText};
use qqq_domain::{DeviceKind, MetricKey};
use qqq_engine::{AppConfig, CommandOutcome, MonitorClient, MonitorCommand, MonitorView};
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
            Self::Disk => "磁盘与存储",
            Self::Network => "网络流量",
            Self::Settings => "设置",
        }
    }
    fn hint(self) -> &'static str {
        match self {
            Self::Overview => "实时掌握电脑的运行状态",
            Self::Cpu => "观察整体负载与每个逻辑核心",
            Self::Memory => "系统可用物理内存与使用趋势",
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
    volume: String,
    core: String,
    range: u64,
    last_query: Vec<MetricKey>,
    last_range: u64,
    message: String,
    latest_save: Option<u64>,
    capture: Option<CaptureOptions>,
    capture_requested: bool,
    started: Instant,
    closing: bool,
}

impl DesktopApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        client: MonitorClient,
        config: AppConfig,
        capture: Option<CaptureOptions>,
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
            range: config.history_secs,
            active_config: config.clone(),
            config,
            client,
            view,
            page,
            network: String::new(),
            disk: String::new(),
            volume: String::new(),
            core: String::new(),
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
        for (selection, kind) in [
            (&mut self.disk, DeviceKind::Disk),
            (&mut self.volume, DeviceKind::Volume),
        ] {
            if selection.is_empty()
                && let Some(device) = self
                    .view
                    .devices
                    .values()
                    .find(|d| d.kind == kind && d.online)
            {
                *selection = device.id.clone();
            }
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
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("navigation")
            .default_size(188.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(13, 21, 37))
                    .inner_margin(20),
            )
            .show(ui, |ui| {
                ui.add_space(14.0);
                ui.label(RichText::new("QQQ").size(36.0).strong().color(theme::CYAN));
                ui.label(
                    RichText::new("M O N I T O R")
                        .size(12.0)
                        .color(theme::MUTED),
                );
                ui.add_space(36.0);
                ui.label(RichText::new("性能监控").size(11.0).color(theme::MUTED));
                ui.add_space(8.0);
                for (page, symbol, name) in [
                    (Page::Overview, "●", "总览"),
                    (Page::Cpu, "▤", "CPU"),
                    (Page::Memory, "▦", "内存"),
                    (Page::Disk, "▣", "磁盘"),
                    (Page::Network, "↕", "网络"),
                    (Page::Settings, "⚙", "设置"),
                ] {
                    let selected = self.page == page;
                    let text = RichText::new(format!("{symbol}    {name}"))
                        .size(15.0)
                        .color(if selected { theme::CYAN } else { theme::MUTED });
                    if ui
                        .add_sized(
                            [ui.available_width(), 44.0],
                            egui::Button::new(text).selected(selected),
                        )
                        .clicked()
                    {
                        self.page = page;
                    }
                }
                ui.add_space(30.0);
                ui.separator();
                ui.label(
                    RichText::new("WINDOWS / LIVE")
                        .size(10.0)
                        .color(theme::MUTED),
                );
            });
    }
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
        if !self.closing && ctx.input(|i| i.viewport().close_requested()) {
            self.closing = true;
            if self.capture.is_none() {
                let mut config = self.active_config.clone();
                if let Some(rect) = ctx.input(|i| i.viewport().inner_rect) {
                    config.window_size = [rect.width().max(960.0), rect.height().max(640.0)];
                }
                if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
                    config.window_position = Some([rect.left(), rect.top()]);
                }
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
        egui::Panel::bottom("status")
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .inner_margin(egui::Margin::symmetric(20, 8)),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let errors = view.diagnostics.collector_errors.len();
                    ui.label(
                        RichText::new(if errors == 0 {
                            "●  监控运行中"
                        } else {
                            "●  部分采集不可用"
                        })
                        .color(if errors == 0 {
                            theme::GREEN
                        } else {
                            egui::Color32::YELLOW
                        }),
                    );
                    ui.label(
                        RichText::new(format!(
                            "采样 {:.1}s  ·  会话 {:.0}s",
                            view.config.sampling_ms as f64 / 1000.0,
                            view.elapsed
                        ))
                        .size(12.0)
                        .color(theme::MUTED),
                    );
                    if !self.message.is_empty() {
                        ui.label(
                            RichText::new(&self.message)
                                .size(12.0)
                                .color(egui::Color32::YELLOW),
                        );
                    }
                });
            });
        self.sidebar(ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::BACKGROUND).inner_margin(24))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.heading(self.page.title());
                        ui.label(
                            RichText::new(self.page.hint())
                                .color(theme::MUTED)
                                .size(12.0),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        for (secs, label) in [(900, "15 分钟"), (300, "5 分钟"), (60, "60 秒")]
                        {
                            ui.selectable_value(&mut self.range, secs, label);
                        }
                    });
                });
                ui.add_space(16.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.page {
                        Page::Overview => self.overview(ui, &view),
                        Page::Cpu => self.cpu(ui, &view),
                        Page::Memory => self.memory(ui, &view),
                        Page::Disk => self.disk(ui, &view),
                        Page::Network => self.network(ui, &view),
                        Page::Settings => self.settings(ui, &view),
                    });
            });
        self.query();
    }
}
