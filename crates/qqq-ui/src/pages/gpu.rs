use super::super::DesktopApp;
use crate::{components::*, i18n, pixel, theme};
use eframe::egui::{self, RichText};
use qqq_domain::{DeviceKind, MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn gpu(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let t = i18n::t();
        if device_picker(
            ui,
            view,
            "gpu-selector",
            &mut self.gpu,
            DeviceKind::Gpu,
            (&pixel::ICON_GPU, t.graphics_card, theme::GPU),
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
            .and_then(|d| d.details.iter().find(|(key, _)| key == "memory_type"))
            .map(|(key, value)| i18n::detail_value(key, value))
            .unwrap_or(t.gpu_memory);

        ui.columns(2, |uis| {
            MetricCard {
                title: t.gpu_usage,
                icon: &pixel::ICON_GPU,
                tone: theme::GPU,
                series: &[(&usage, theme::GOLD)],
                detail: busiest.map_or_else(
                    || t.engines_waiting.into(),
                    |(kind, value)| {
                        (t.busiest_engine)(
                            &engine_name(kind),
                            &format_value(Some(value), Unit::Percent),
                        )
                    },
                ),
                template: RATE_TEMPLATE,
            }
            .show(&mut uis[0], view, false);
            MetricCard {
                title: t.gpu_memory,
                icon: &pixel::ICON_MEMORY,
                tone: theme::VRAM,
                series: &[(&memory, theme::BLUE)],
                detail: (t.gpu_memory_used)(
                    memory_kind,
                    &format_value(view.value(&id, "memory_used"), Unit::Bytes),
                    &format_value(view.value(&id, "memory_total"), Unit::Bytes),
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
        let mut series = vec![
            (&usage, "GPU", theme::GOLD),
            (&memory, t.gpu_memory, theme::BLUE),
        ];
        if !engine_label.is_empty() {
            series.push((&engine, engine_label.as_str(), theme::PINK));
        }
        history_chart(
            ui,
            view,
            "gpu-history",
            t.gpu_chart,
            Unit::Percent,
            self.range,
            &series,
            150.0,
        );

        ui.horizontal(|ui| {
            section_title(ui, &pixel::ICON_GPU, t.engines_title, t.engines_hint);
            heat_legend(ui);
        });
        if engines.is_empty() {
            ui.label(
                RichText::new(t.engines_missing)
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
                        &(t.engine)(&name),
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
            t.gpu_memory_title,
            t.gpu_memory_hint,
        );
        ui.columns(2, |uis| {
            for (ui, (title, used, total, tone)) in uis.iter_mut().zip([
                (
                    t.dedicated_memory,
                    "dedicated_used",
                    "dedicated_total",
                    theme::VRAM,
                ),
                (
                    t.shared_memory,
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
                    &(t.total_share)(
                        &format_value(total, Unit::Bytes),
                        &format_value(percent, Unit::Percent),
                    ),
                    percent.map(|p| (p, theme::level_color(p, tone.main))),
                );
            }
        });
        ui.label(RichText::new(t.gpu_note).size(12.0).color(theme::MUTED));
    }
}

/// Display name of a counter engine type such as `videodecode` or `compute_1`.
fn engine_name(kind: &str) -> String {
    let (base, index) = match kind.rsplit_once('_') {
        Some((base, index)) if index.chars().all(|c| c.is_ascii_digit()) => (base, Some(index)),
        _ => (kind, None),
    };
    let name = match base {
        "3d" | "cuda" | "vr" => base.to_uppercase(),
        other => (i18n::t().engine_kind)(other).map_or_else(|| other.to_uppercase(), str::to_owned),
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
