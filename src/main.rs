#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod bootstrap;

use eframe::wgpu::rwh::{HasWindowHandle, RawWindowHandle};
use qqq_domain::{DeviceKind, SampleState};
use qqq_engine::{ConfigStore, DockEdge, Language, MonitorRuntime};
use qqq_ui::{CaptureOptions, Desktop, DesktopApp, Page, i18n};
use std::{
    path::PathBuf,
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
            "QQQ Monitor\n  --smoke-test          检查四类真实性能采集\n  --capture PATH        启动 GUI，8 秒后截图并退出\n  --page PAGE           截图页面：overview/cpu/memory/gpu/disk/network/settings\n  --dock EDGE           以迷你状态栏吸附在屏幕边缘启动：top/left/right\n  --lang LANG           以指定界面语言启动：zh-CN/en/ja"
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
    let mut runtime = MonitorRuntime::start(qqq_platform_windows::collectors(), loaded)
        .map_err(|notice| i18n::notice(&notice))?;
    if args.iter().any(|s| s == "--smoke-test") {
        return smoke_test(&mut runtime);
    }
    let capture = if let Some(index) = args.iter().position(|s| s == "--capture") {
        let path = args.get(index + 1).ok_or("--capture 需要 PNG 文件路径")?;
        let page = match args
            .iter()
            .position(|s| s == "--page")
            .and_then(|i| args.get(i + 1))
            .map(String::as_str)
            .unwrap_or("overview")
        {
            "overview" => Page::Overview,
            "cpu" => Page::Cpu,
            "memory" => Page::Memory,
            "gpu" => Page::Gpu,
            "disk" => Page::Disk,
            "network" => Page::Network,
            "settings" => Page::Settings,
            _ => return Err("未知截图页面".into()),
        };
        Some(CaptureOptions {
            path: PathBuf::from(path),
            page,
        })
    } else {
        None
    };
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
    };
    let result = eframe::run_native(
        "QQQ Monitor",
        options,
        Box::new(move |cc| {
            if let Ok(handle) = cc.window_handle()
                && let RawWindowHandle::Win32(handle) = handle.as_raw()
            {
                qqq_platform_windows::set_window_icon(handle.hwnd.get());
            }
            Ok(Box::new(DesktopApp::new(
                cc,
                client,
                config,
                capture,
                desktop,
                system_language,
            )))
        }),
    )
    .map_err(|e| e.to_string());
    runtime.shutdown();
    result
}

fn smoke_test(runtime: &mut MonitorRuntime) -> Result<(), String> {
    let client = runtime.client();
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
    let cpu = view
        .reading("cpu", "usage")
        .is_some_and(|v| v.effective_state(view.now) == SampleState::Ready);
    let memory = view
        .reading("memory", "usage")
        .is_some_and(|v| v.effective_state(view.now) == SampleState::Ready);
    let ready_device = |kind| {
        view.devices.values().any(|d| {
            d.kind == kind
                && d.online
                && view.metrics.iter().any(|(key, v)| {
                    key.device == d.id && v.effective_state(view.now) == SampleState::Ready
                })
        })
    };
    runtime.shutdown();
    if cpu
        && memory
        && ready_device(DeviceKind::Disk)
        && ready_device(DeviceKind::Network)
        && ready_device(DeviceKind::Volume)
    {
        println!("PASS：CPU、内存、物理磁盘 I/O、网络和卷容量采集正常");
        Ok(())
    } else {
        Err("核心采集检查未通过，请查看上述指标状态和采集器错误".into())
    }
}
