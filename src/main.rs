#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod bootstrap;

use eframe::wgpu::rwh::{HasWindowHandle, RawWindowHandle};
use qqq_domain::{DeviceKind, ProcessHost, SampleState};
use qqq_engine::{
    AlertEvent, ConfigStore, DockEdge, Language, MonitorCommand, MonitorRuntime, Services,
};
use qqq_ui::{CaptureOptions, Desktop, DesktopApp, Page, i18n};
use std::{
    path::PathBuf,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

fn main() {
    let _logging = bootstrap::logging();
    if let Err(error) = run() {
        tracing::error!(%error,"启动失败");
        eprintln!("{error}");
        if !std::env::args().any(|v| v == "--smoke-test" || v == "--capture") {
            qqq_platform_windows::show_startup_error(i18n::t().startup_failed, &error);
        }
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|s| s == "--help") {
        println!(
            "QQQ Monitor\n  --smoke-test          检查四类真实性能采集\n  --capture PATH        启动 GUI，8 秒后截图并退出\n  --page PAGE           打开或截图的页面：overview/cpu/memory/gpu/frames/disk/network/processes/remote/alerts/tools/settings\n  --dock EDGE           以迷你状态栏吸附在屏幕边缘启动：top/left/right\n  --lang LANG           以指定界面语言启动：zh-CN/en/ja\n  --autostart           以开机自动启动的方式启动：开启“开机启动时直接收进托盘”时只显示托盘图标"
        );
        return Ok(());
    }
    let loaded = ConfigStore::load_default();
    let mut config = loaded.config.clone();
    if let Some(index) = args.iter().position(|s| s == "--dock") {
        config.dock = Some(match args.get(index + 1).map(String::as_str) {
            Some("top") => DockEdge::Top,
            Some("left") => DockEdge::Left,
            Some("right") => DockEdge::Right,
            _ => return Err("--dock 需要 top、left 或 right".into()),
        });
    }
    if let Some(index) = args.iter().position(|s| s == "--lang") {
        let tag = args.get(index + 1).map(String::as_str);
        config.language = Some(
            Language::ALL
                .into_iter()
                .find(|language| Some(language.tag()) == tag)
                .ok_or("--lang 需要 zh-CN、en 或 ja")?,
        );
    }
    let system_language = Language::preferred(
        qqq_platform_windows::ui_languages()
            .iter()
            .map(String::as_str),
    );
    i18n::set(config.language.unwrap_or(system_language));
    let services = Services {
        processes: qqq_platform_windows::processes(),
        remote: qqq_remote::ssh(qqq_platform_windows::secrets()),
        frames: Some(qqq_platform_windows::frames()),
        // Phrased in the interface language at the time the alert fires.
        notify: Arc::new(|event: &AlertEvent| {
            let (title, body) = i18n::alert(event);
            if let Err(reason) = qqq_platform_windows::notify(&title, &body) {
                tracing::warn!(?reason, rule = event.rule.id, "无法显示告警通知");
            }
        }),
    };
    let mut runtime =
        MonitorRuntime::start_with_services(qqq_platform_windows::collectors(), services, loaded)
            .map_err(|notice| i18n::notice(&notice))?;
    if args.iter().any(|s| s == "--smoke-test") {
        return smoke_test(&mut runtime);
    }
    let page = match args
        .iter()
        .position(|s| s == "--page")
        .map(|i| args.get(i + 1).map(String::as_str))
    {
        None => None,
        Some(name) => Some(match name {
            Some("overview") => Page::Overview,
            Some("cpu") => Page::Cpu,
            Some("memory") => Page::Memory,
            Some("gpu") => Page::Gpu,
            Some("frames") => Page::Frames,
            Some("disk") => Page::Disk,
            Some("network") => Page::Network,
            Some("processes") => Page::Processes,
            Some("remote") => Page::Remote,
            Some("alerts") => Page::Alerts,
            Some("tools") => Page::Tools,
            Some("settings") => Page::Settings,
            _ => return Err("未知页面".into()),
        }),
    };
    let capture = if let Some(index) = args.iter().position(|s| s == "--capture") {
        let path = args.get(index + 1).ok_or("--capture 需要 PNG 文件路径")?;
        Some(CaptureOptions {
            path: PathBuf::from(path),
            page: page.unwrap_or(Page::Overview),
        })
    } else {
        None
    };
    let start_in_tray = capture.is_none()
        && config.autostart_to_tray
        && args
            .iter()
            .any(|s| s == qqq_platform_windows::AUTOSTART_ARGUMENT);
    // An empty icon keeps eframe's default icon off the window; the embedded resource icon
    // is applied once the window exists.
    let viewport = qqq_ui::viewport(&config).with_icon(eframe::egui::IconData::default());
    let mut options = eframe::NativeOptions {
        centered: viewport.position.is_none(),
        viewport,
        ..Default::default()
    };
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
    setup.power_preference = eframe::wgpu::PowerPreference::LowPower;
    options.wgpu_options.wgpu_setup = eframe::egui_wgpu::WgpuSetup::CreateNew(setup);
    let client = runtime.client();
    let desktop = Desktop {
        monitor_at: qqq_platform_windows::monitor_at,
        covers: qqq_platform_windows::covers_point,
        button_held: qqq_platform_windows::mouse_button_held,
        fullscreen_at: qqq_platform_windows::fullscreen_at,
        watch_foreground: qqq_platform_windows::watch_foreground,
        show_tray: qqq_platform_windows::show_tray,
        hide_tray: qqq_platform_windows::hide_tray,
        tray_opened: qqq_platform_windows::tray_opened,
        taskbar_button: qqq_platform_windows::set_taskbar_button,
        cloak: qqq_platform_windows::cloak_window,
        autostart: qqq_platform_windows::autostart,
        set_autostart: qqq_platform_windows::set_autostart,
        notify: qqq_platform_windows::notify,
        prepare_notifications: qqq_platform_windows::prepare_notifications,
        restart_elevated: qqq_platform_windows::restart_elevated,
        set_hotkey: qqq_platform_windows::set_hotkey,
        screenshot_requested: qqq_platform_windows::screenshot_requested,
        note_requested: qqq_platform_windows::note_requested,
        commands_requested: qqq_platform_windows::commands_requested,
        start_powershell: qqq_platform_windows::start_powershell,
        play_sound: qqq_platform_windows::play_sound,
        show_balloon: qqq_platform_windows::show_balloon,
        start_capture: qqq_platform_windows::start_capture,
        take_capture: qqq_platform_windows::take_capture,
        watch_clipboard: qqq_platform_windows::watch_clipboard,
        take_clipboard: qqq_platform_windows::take_clipboard,
        set_clipboard: qqq_platform_windows::set_clipboard,
        open_path: qqq_platform_windows::open_path,
        open_url: qqq_platform_windows::open_url,
        local_time: qqq_platform_windows::local_time,
    };
    let result = eframe::run_native(
        "QQQ Monitor",
        options,
        Box::new(move |cc| {
            let mut tray = false;
            if let Ok(handle) = cc.window_handle()
                && let RawWindowHandle::Win32(handle) = handle.as_raw()
            {
                qqq_platform_windows::set_window_icon(handle.hwnd.get());
                let context = cc.egui_ctx.clone();
                tray = qqq_platform_windows::attach_tray(handle.hwnd.get(), move || {
                    context.request_repaint()
                });
            }
            Ok(Box::new(DesktopApp::new(
                cc,
                client,
                config,
                page,
                capture,
                desktop,
                system_language,
                // Only the icon can bring back a window that starts in the tray.
                start_in_tray && tray,
            )))
        }),
    )
    .map_err(|e| e.to_string());
    // The shell keeps an icon until it is removed, even after the process exits.
    qqq_platform_windows::hide_tray();
    runtime.shutdown();
    result
}

fn smoke_test(runtime: &mut MonitorRuntime) -> Result<(), String> {
    let client = runtime.client();
    let watch = |host: Option<ProcessHost>| {
        client
            .try_command(MonitorCommand::WatchProcesses(host))
            .map_err(|notice| i18n::notice(&notice))
    };
    watch(Some(ProcessHost::Windows))?;
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }
    let view = client.try_latest_view().ok_or("无法读取监控快照")?;
    println!(
        "采样批次 {}，丢失 {}，历史 {} 字节",
        view.diagnostics.received_batches,
        view.diagnostics.dropped_batches,
        view.diagnostics.history_bytes
    );
    for (key, reading) in &view.metrics {
        if !key.device.starts_with("core:") {
            println!(
                "{} / {}: {:?} {:?} {:?}",
                key.device,
                key.metric,
                reading.value,
                reading.descriptor.unit,
                reading.effective_state(view.now)
            );
        }
    }
    for (source, error) in &view.diagnostics.collector_errors {
        eprintln!("{source}: {error:?}");
    }
    for (host, status) in &view.remote.hosts {
        println!("远程主机 {host}：{:?}", status.state);
    }
    let cpu = view
        .reading("cpu", "usage")
        .is_some_and(|v| v.effective_state(view.now) == SampleState::Ready);
    let memory = view
        .reading("memory", "usage")
        .is_some_and(|v| v.effective_state(view.now) == SampleState::Ready);
    let rows = |host: &ProcessHost| {
        client
            .try_latest_view()?
            .processes
            .table
            .as_ref()
            .filter(|table| table.host == *host)
            .map(|table| table.rows.len())
    };
    let windows = rows(&ProcessHost::Windows).unwrap_or(0);
    println!("Windows 进程 {windows} 个");
    for host in &view.processes.hosts {
        let ProcessHost::Wsl(distro) = host else {
            continue;
        };
        watch(Some(host.clone()))?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut count = None;
        while count.is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(100));
            count = rows(host);
        }
        match count {
            Some(count) => println!("WSL {distro} 进程 {count} 个"),
            None => eprintln!("WSL {distro}：未能读取进程"),
        }
    }
    watch(None)?;
    let ready_device = |kind| {
        view.devices.values().any(|d| {
            d.kind == kind
                && d.host.is_none()
                && d.online
                && view.metrics.iter().any(|(key, v)| {
                    key.device == d.id && v.effective_state(view.now) == SampleState::Ready
                })
        })
    };
    runtime.shutdown();
    if cpu
        && memory
        && windows > 0
        && ready_device(DeviceKind::Disk)
        && ready_device(DeviceKind::Network)
        && ready_device(DeviceKind::Volume)
    {
        println!("PASS：CPU、内存、物理磁盘 I/O、网络、卷容量和进程列表采集正常");
        Ok(())
    } else {
        Err("核心采集检查未通过，请查看上述指标状态和采集器错误".into())
    }
}
