use super::super::DesktopApp;
use crate::{components::*, pixel, theme};
use eframe::egui::{self, RichText};
use qqq_domain::{DeviceKind, MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn gpu(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        if device_picker(
            ui,
            view,
            "gpu-selector",
            &mut self.gpu,
            DeviceKind::Gpu,
            (&pixel::ICON_GPU, "显卡", theme::GPU),
        ) {
            self.gpu_engine.clear();
        }
        let id = self.gpu.clone();
        let usage = MetricKey::new(&id, "usage");
        let memory = MetricKey::new(&id, "memory_usage");
        let engine = MetricKey::new(&id, &self.gpu_engine);
        let mut engines: Vec<(&str, Option<f64>)> = view
            .metrics
            .iter()
            .filter(|(key, _)| key.device == id)
            .filter_map(|(key, reading)| {
                let kind = key.metric.strip_prefix("engine:")?;
                Some((kind, reading.value))
            })
            .collect();
        engines.sort_by_key(|(kind, _)| (engine_rank(kind), *kind));
        let busiest = engines
            .iter()
            .filter_map(|(kind, value)| value.map(|value| (*kind, value)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let memory_kind = view
            .devices
            .get(&id)
            .and_then(|d| d.details.iter().find(|(name, _)| name == "显存类型"))
            .map(|(_, value)| value.as_str())
            .unwrap_or("显存");

        ui.columns(2, |uis| {
            MetricCard {
                title: "GPU 使用率",
                icon: &pixel::ICON_GPU,
                tone: theme::GPU,
                series: &[(&usage, theme::GOLD)],
                detail: busiest.map_or_else(
                    || "等待引擎数据".into(),
                    |(kind, value)| {
                        format!(
                            "最忙引擎 {} · {}",
                            engine_name(kind),
                            format_value(Some(value), Unit::Percent)
                        )
                    },
                ),
                template: RATE_TEMPLATE,
            }
            .show(&mut uis[0], view, false);
            MetricCard {
                title: "显存",
                icon: &pixel::ICON_MEMORY,
                tone: theme::VRAM,
                series: &[(&memory, theme::BLUE)],
                detail: format!(
                    "{memory_kind} 已用 {} / {}",
                    format_value(view.value(&id, "memory_used"), Unit::Bytes),
                    format_value(view.value(&id, "memory_total"), Unit::Bytes)
                ),
                template: RATE_TEMPLATE,
            }
            .show(&mut uis[1], view, false);
        });

        let engine_label = self
            .gpu_engine
            .strip_prefix("engine:")
            .map(engine_name)
            .unwrap_or_default();
        let mut series = vec![(&usage, "GPU", theme::GOLD), (&memory, "显存", theme::BLUE)];
        if !engine_label.is_empty() {
            series.push((&engine, engine_label.as_str(), theme::PINK));
        }
        history_chart(
            ui,
            view,
            "gpu-history",
            "GPU 与显存使用率",
            Unit::Percent,
            self.range,
            &series,
            150.0,
        );

        ui.horizontal(|ui| {
            section_title(
                ui,
                &pixel::ICON_GPU,
                "引擎占用",
                "同类引擎取最忙的一个，点击查看历史",
            );
            heat_legend(ui);
        });
        if engines.is_empty() {
            ui.label(
                RichText::new("等待引擎数据；未提供引擎计数器的显卡只显示显存")
                    .size(12.0)
                    .color(theme::MUTED),
            );
        }
        let columns = ((ui.available_width() / 130.0) as usize).clamp(2, 6);
        for row in engines.chunks(columns) {
            ui.columns(columns, |uis| {
                for (index, (kind, value)) in row.iter().enumerate() {
                    let metric = format!("engine:{kind}");
                    let name = engine_name(kind);
                    if heat_tile(
                        &mut uis[index],
                        &name,
                        &format!("{name} 引擎"),
                        *value,
                        self.gpu_engine == metric,
                    )
                    .clicked()
                    {
                        self.gpu_engine = metric;
                    }
                }
            });
        }

        section_title(
            ui,
            &pixel::ICON_MEMORY,
            "显存明细",
            "专用显存位于显卡板载内存，共享 GPU 内存取自系统内存",
        );
        ui.columns(2, |uis| {
            for (ui, (title, used, total, tone)) in uis.iter_mut().zip([
                ("专用显存", "dedicated_used", "dedicated_total", theme::VRAM),
                (
                    "共享 GPU 内存",
                    "shared_used",
                    "shared_total",
                    theme::ACCENT,
                ),
            ]) {
                let used = view.value(&id, used);
                let total = view.value(&id, total);
                let percent = used
                    .zip(total)
                    .filter(|(_, total)| *total > 0.0)
                    .map(|(used, total)| (used / total * 100.0).min(100.0));
                stat_card(
                    ui,
                    (&pixel::ICON_MEMORY, title, tone),
                    &format_value(used, Unit::Bytes),
                    &format!(
                        "共 {}  ·  占 {}",
                        format_value(total, Unit::Bytes),
                        format_value(percent, Unit::Percent)
                    ),
                    percent.map(|p| (p, theme::level_color(p, tone.main))),
                );
            }
        });
        ui.label(
            RichText::new(
                "GPU 使用率与任务管理器口径一致：各进程在同一引擎上的占用相加，再取最忙的引擎；不同类型的引擎不相加。",
            )
            .size(12.0)
            .color(theme::MUTED),
        );
    }
}

/// Display name of a counter engine type such as `videodecode` or `compute_1`.
fn engine_name(kind: &str) -> String {
    let (base, index) = match kind.rsplit_once('_') {
        Some((base, index)) if index.chars().all(|c| c.is_ascii_digit()) => (base, Some(index)),
        _ => (kind, None),
    };
    let name = match base {
        "3d" => "3D".to_string(),
        "copy" => "复制".into(),
        "videodecode" => "视频解码".into(),
        "videoencode" => "视频编码".into(),
        "videoprocessing" => "视频处理".into(),
        "compute" => "计算".into(),
        "cuda" => "CUDA".into(),
        "graphics" => "图形".into(),
        "legacyoverlay" => "旧版叠加".into(),
        "overlay" => "叠加".into(),
        "security" => "安全".into(),
        "vr" => "VR".into(),
        "ofa" => "光流".into(),
        other => other.to_uppercase(),
    };
    match index {
        Some(index) => format!("{name} {index}"),
        None => name,
    }
}

/// Rendering and compute engines first, then copy and video, then the rest.
fn engine_rank(kind: &str) -> usize {
    let base = kind.split('_').next().unwrap_or(kind);
    [
        "3d",
        "compute",
        "cuda",
        "graphics",
        "copy",
        "videodecode",
        "videoencode",
        "videoprocessing",
    ]
    .iter()
    .position(|known| *known == base)
    .unwrap_or(usize::MAX)
}
