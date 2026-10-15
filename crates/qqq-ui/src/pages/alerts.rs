use super::super::DesktopApp;
use crate::{components::*, i18n, pixel, theme};
use eframe::egui::{self, Color32, RichText, vec2};
use qqq_domain::{DeviceDescriptor, DeviceKind, MetricKey, Unit};
use qqq_engine::{
    AlertCondition, AlertRule, AlertState, MAX_ALERT_RULES, MonitorView, Notice, alert_device,
};
use std::time::{SystemTime, UNIX_EPOCH};

/// Width of the add and edit dialog.
const DIALOG_WIDTH: f32 = 480.0;
/// Hold times the editor offers, in seconds.
const HOLDS: [u64; 6] = [0, 5, 10, 30, 60, 300];
/// Device of the foreground application's frames.
const FRAMES: &str = "frames";
/// Metrics of the frames device, which the editor offers while frame monitoring is off too.
const FRAME_METRICS: [&str; 3] = ["fps", "frame_time", "frame_time_max"];

/// State of the alerts page.
#[derive(Default)]
pub(in crate::app) struct AlertPage {
    editor: Option<Editor>,
}

/// Fields of the add or edit dialog.
struct Editor {
    /// Rule being edited; `None` adds a rule.
    id: Option<String>,
    /// Identity of the device, as rules save it.
    device: String,
    metric: String,
    condition: AlertCondition,
    /// Threshold in the unit the editor shows, such as GiB for an amount of memory.
    threshold: String,
    hold_secs: u64,
    enabled: bool,
    error: Option<String>,
}

/// Factor from the threshold the editor shows to the metric's unit, and the unit shown.
fn scale(unit: Unit) -> (f64, &'static str) {
    match unit {
        Unit::Percent => (1.0, "%"),
        Unit::Bytes => (1024_f64.powi(3), "GiB"),
        Unit::BytesPerSecond => (1024_f64.powi(2), "MiB/s"),
        Unit::FramesPerSecond => (1.0, "FPS"),
        Unit::Milliseconds => (1.0, "ms"),
        Unit::Seconds => (1.0, "s"),
        Unit::Count => (1.0, ""),
    }
}

impl Editor {
    fn new(rule: Option<&AlertRule>) -> Self {
        match rule {
            Some(rule) => {
                let shown = rule.threshold / scale(alert_unit(&rule.metric)).0;
                let text = format!("{shown:.3}");
                Self {
                    id: Some(rule.id.clone()),
                    device: rule.device.clone(),
                    metric: rule.metric.clone(),
                    condition: rule.condition,
                    threshold: text.trim_end_matches('0').trim_end_matches('.').into(),
                    hold_secs: rule.hold_secs,
                    enabled: rule.enabled,
                    error: None,
                }
            }
            None => Self {
                id: None,
                device: String::new(),
                metric: String::new(),
                condition: AlertCondition::Above,
                threshold: String::new(),
                hold_secs: 10,
                enabled: true,
                error: None,
            },
        }
    }

    fn build(&self, rules: &[AlertRule]) -> Result<AlertRule, String> {
        let t = i18n::t();
        if self.device.is_empty() || self.metric.is_empty() {
            return Err(t.alert_missing.into());
        }
        let threshold = self
            .threshold
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| t.alert_bad_threshold.to_string())?;
        let rule = AlertRule {
            id: self.id.clone().unwrap_or_else(|| new_id(rules)),
            enabled: self.enabled,
            device: self.device.clone(),
            metric: self.metric.clone(),
            condition: self.condition,
            threshold: threshold * scale(alert_unit(&self.metric)).0,
            hold_secs: self.hold_secs,
        };
        rule.validate().map_err(|notice| i18n::notice(&notice))?;
        Ok(rule)
    }
}

/// An ID unlike any other rule's.
fn new_id(rules: &[AlertRule]) -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    (0..)
        .map(|n| format!("a{:x}", stamp + n))
        .find(|id| !rules.iter().any(|r| r.id == *id))
        .unwrap_or_default()
}

/// A device the editor offers, with the metrics it can watch.
struct Choice {
    identity: String,
    label: String,
    metrics: Vec<&'static str>,
}

/// Display name of the remote host a device belongs to.
fn host_label<'a>(view: &'a MonitorView, device: &'a DeviceDescriptor) -> Option<&'a str> {
    let host = device.host.as_ref()?;
    Some(
        view.config
            .remote_hosts
            .iter()
            .find(|h| h.id == *host)
            .map_or(host.as_str(), |h| h.label()),
    )
}

/// Devices whose metrics rules can watch: this computer's by kind, then each remote host's.
/// The foreground application is offered while frame monitoring is off as well.
fn choices(view: &MonitorView) -> Vec<Choice> {
    let rank = |kind| {
        [
            DeviceKind::Cpu,
            DeviceKind::Memory,
            DeviceKind::Gpu,
            DeviceKind::Frames,
            DeviceKind::Disk,
            DeviceKind::Volume,
            DeviceKind::Network,
            DeviceKind::System,
        ]
        .iter()
        .position(|k| *k == kind)
    };
    let placeholder = DeviceDescriptor::new(FRAMES, "", DeviceKind::Frames);
    let mut devices: Vec<_> = view
        .devices
        .values()
        .filter(|d| rank(d.kind).is_some())
        .collect();
    if !devices.iter().any(|d| d.kind == DeviceKind::Frames) {
        devices.push(&placeholder);
    }
    devices.sort_by_key(|d| (d.host.clone(), rank(d.kind), device_name(d)));
    devices
        .into_iter()
        .map(|device| Choice {
            identity: device.identity().into(),
            label: alert_device_name(device, host_label(view, device)),
            metrics: if device.kind == DeviceKind::Frames {
                FRAME_METRICS.into()
            } else {
                ALERT_METRICS
                    .into_iter()
                    .filter(|m| view.metrics.contains_key(&MetricKey::new(&device.id, *m)))
                    .collect()
            },
        })
        .filter(|choice| !choice.metrics.is_empty())
        .collect()
}

/// Name of the device a rule watches, also while it is missing.
fn rule_device(view: &MonitorView, rule: &AlertRule) -> String {
    let t = i18n::t();
    match alert_device(view, rule) {
        Some(device) => alert_device_name(device, host_label(view, device)),
        None if rule.device == FRAMES => t.foreground_app.into(),
        None => format!("{} · {}", rule.device, t.alert_device_missing),
    }
}

/// State of a rule in words and as a color.
fn state_text(view: &MonitorView, rule: &AlertRule) -> (String, Color32) {
    let t = i18n::t();
    if !rule.enabled {
        return (t.alert_disabled.into(), theme::MUTED);
    }
    match view.alerts.states.get(&rule.id) {
        Some(AlertState::Normal) => (t.alert_normal.into(), theme::GOOD),
        Some(AlertState::Pending { since }) => {
            let held = view.now.saturating_duration_since(*since).as_secs();
            let left = rule.hold_secs.saturating_sub(held).max(1);
            ((t.alert_pending)(&span(left)), theme::WARNING)
        }
        Some(AlertState::Firing { since }) => {
            let held = view.now.saturating_duration_since(*since).as_secs();
            ((t.alert_firing)(&span(held)), theme::ERROR)
        }
        Some(AlertState::NoData) | None => (t.alert_no_data.into(), theme::MUTED),
    }
}

impl DesktopApp {
    /// Saves the rules right away, like the remote hosts.
    fn save_alerts(&mut self, rules: Vec<AlertRule>) {
        self.config.alerts = rules.clone();
        let mut config = self.active_config.clone();
        config.alerts = rules;
        self.save_config(config, true);
    }

    pub(in crate::app) fn alerts(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let t = i18n::t();
        let rules = self.active_config.alerts.clone();
        theme::card().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let size =
                    |ui: &egui::Ui, text: &str| vec2(label_width(ui, &[text], 14.0) + 64.0, 34.0);
                let add = size(ui, t.alert_add);
                if rules.len() < MAX_ALERT_RULES
                    && theme::pixel_button(
                        ui,
                        t.alert_add,
                        Some(&pixel::ICON_BELL),
                        rules.is_empty(),
                        add,
                    )
                    .clicked()
                {
                    self.alerts.editor = Some(Editor::new(None));
                }
                let test = size(ui, t.alert_test);
                if theme::pixel_button(ui, t.alert_test, Some(&pixel::ICON_SPARKLE), false, test)
                    .clicked()
                {
                    match (self.desktop.notify)(t.alerts_page.title, t.alert_test_body) {
                        Ok(()) if matches!(self.message, Some(Notice::NotificationFailed(_))) => {
                            self.message = None
                        }
                        Ok(()) => {}
                        Err(reason) => self.message = Some(Notice::NotificationFailed(reason)),
                    }
                }
            });
            if rules.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new(t.alert_empty).color(theme::MUTED));
            }
        });
        if !rules.is_empty() {
            section_title(ui, &pixel::ICON_BELL, t.alert_rules, "");
        }
        let mut changed = None;
        let mut removed = None;
        for (index, rule) in rules.iter().enumerate() {
            theme::card().show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let mut enabled = rule.enabled;
                    if ui
                        .checkbox(&mut enabled, "")
                        .on_hover_text(t.alert_enabled)
                        .changed()
                    {
                        changed = Some((index, enabled));
                    }
                    pixel::icon(ui, &pixel::ICON_BELL, 1.5, theme::INK, theme::GOLD);
                    let size = |ui: &egui::Ui, text: &str| {
                        vec2(label_width(ui, &[text], 14.0) + 30.0, 30.0)
                    };
                    let (edit, remove) = (size(ui, t.alert_edit), size(ui, t.alert_remove));
                    let room = (ui.available_width() - edit.x - remove.x - 24.0).max(0.0);
                    let title = format!(
                        "{} · {}",
                        rule_device(view, rule),
                        i18n::metric(&rule.metric)
                    );
                    ui.allocate_ui(vec2(room, 30.0), |ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(title).size(theme::SECTION_TITLE).strong(),
                            )
                            .truncate(),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::pixel_button(ui, t.alert_remove, None, false, remove).clicked() {
                            removed = Some(index);
                        }
                        if theme::pixel_button(ui, t.alert_edit, None, false, edit).clicked() {
                            self.alerts.editor = Some(Editor::new(Some(rule)));
                        }
                    });
                });
                let unit = alert_unit(&rule.metric);
                let current = alert_device(view, rule)
                    .and_then(|device| view.value(&device.id, &rule.metric));
                ui.label(
                    RichText::new(format!(
                        "{}  ·  {}",
                        i18n::alert_condition(rule, unit),
                        (t.alert_now)(&format_value(current, unit))
                    ))
                    .size(12.0)
                    .color(theme::MUTED),
                );
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let (state, color) = state_text(view, rule);
                    pixel_dot(ui, color);
                    ui.label(RichText::new(state).size(12.0).color(color));
                });
            });
        }
        if let Some((index, enabled)) = changed {
            let mut rules = rules.clone();
            rules[index].enabled = enabled;
            self.save_alerts(rules);
        } else if let Some(index) = removed {
            let mut rules = rules.clone();
            rules.remove(index);
            self.save_alerts(rules);
        }
        theme::card().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            tag(ui, Some(&pixel::ICON_CLOCK), t.alert_recent, theme::NETWORK);
            if view.alerts.recent.is_empty() {
                ui.label(
                    RichText::new(t.alert_recent_empty)
                        .size(12.0)
                        .color(theme::MUTED),
                );
            }
            for event in view.alerts.recent.iter() {
                let (title, body) = i18n::alert(event);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.label(
                        RichText::new(ago(view.now.saturating_duration_since(event.at).as_secs()))
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                    ui.label(RichText::new(title).size(12.0).strong());
                    ui.label(RichText::new(body).size(12.0));
                });
            }
        });
        ui.label(RichText::new(t.alert_note).size(12.0).color(theme::MUTED));
        self.alert_editor(ui.ctx(), view);
    }

    fn alert_editor(&mut self, ctx: &egui::Context, view: &MonitorView) {
        let Some(mut editor) = self.alerts.editor.take() else {
            return;
        };
        let t = i18n::t();
        let choices = choices(view);
        let editing = editor.id.is_some();
        let modal = egui::Modal::new(egui::Id::new("alert-editor")).show(ctx, |ui| {
            ui.set_width(DIALOG_WIDTH);
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(
                RichText::new(if editing {
                    t.alert_edit_title
                } else {
                    t.alert_add_title
                })
                .size(16.0)
                .strong(),
            );
            let labels = [
                t.alert_device,
                t.alert_metric,
                t.alert_when,
                t.alert_threshold,
                t.alert_hold,
            ];
            let width = label_width(ui, &labels, 14.0) + 8.0;
            let row = |ui: &mut egui::Ui, label: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.add_sized([width, 30.0], egui::Label::new(label));
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        add(ui)
                    });
                });
            };
            let selected = choices.iter().find(|c| c.identity == editor.device);
            row(ui, t.alert_device, &mut |ui| {
                let text = match selected {
                    Some(choice) => choice.label.clone(),
                    None if editor.device.is_empty() => t.alert_pick_device.into(),
                    None => format!("{} · {}", editor.device, t.alert_device_missing),
                };
                egui::ComboBox::from_id_salt("alert-device")
                    .width(ui.available_width())
                    .selected_text(text)
                    .show_ui(ui, |ui| {
                        for choice in &choices {
                            let chosen = choice.identity == editor.device;
                            if ui.selectable_label(chosen, &choice.label).clicked() && !chosen {
                                editor.device = choice.identity.clone();
                                if !choice.metrics.contains(&editor.metric.as_str()) {
                                    editor.metric = choice.metrics[0].into();
                                }
                            }
                        }
                    });
            });
            let metrics = match selected {
                Some(choice) => choice.metrics.clone(),
                None if !editor.metric.is_empty() => ALERT_METRICS
                    .into_iter()
                    .filter(|m| *m == editor.metric)
                    .collect(),
                None => Vec::new(),
            };
            if !metrics.is_empty() {
                row(ui, t.alert_metric, &mut |ui| {
                    for metric in &metrics {
                        if theme::chip(ui, i18n::metric(metric), editor.metric == *metric).clicked()
                        {
                            editor.metric = (*metric).into();
                        }
                    }
                });
            }
            row(ui, t.alert_when, &mut |ui| {
                for (condition, label) in [
                    (AlertCondition::Above, t.alert_above),
                    (AlertCondition::Below, t.alert_below),
                ] {
                    if theme::chip(ui, label, editor.condition == condition).clicked() {
                        editor.condition = condition;
                    }
                }
            });
            row(ui, t.alert_threshold, &mut |ui| {
                ui.add(egui::TextEdit::singleline(&mut editor.threshold).desired_width(120.0));
                ui.label(scale(alert_unit(&editor.metric)).1);
            });
            row(ui, t.alert_hold, &mut |ui| {
                for secs in HOLDS {
                    let label = if secs == 0 {
                        t.alert_hold_none.into()
                    } else {
                        span(secs)
                    };
                    if theme::chip(ui, &label, editor.hold_secs == secs).clicked() {
                        editor.hold_secs = secs;
                    }
                }
            });
            if editing {
                ui.checkbox(&mut editor.enabled, t.alert_enabled);
            }
            if let Some(error) = &editor.error {
                ui.label(RichText::new(error).size(12.0).color(theme::ERROR));
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let size =
                    |ui: &egui::Ui, text: &str| vec2(label_width(ui, &[text], 14.0) + 40.0, 34.0);
                let (yes, no) = (size(ui, t.alert_save), size(ui, t.cancel));
                let save = theme::pixel_button(ui, t.alert_save, None, true, yes).clicked();
                let cancel = theme::pixel_button(ui, t.cancel, None, false, no).clicked();
                (save, cancel)
            })
            .inner
        });
        let (save, cancel) = modal.inner;
        if cancel || modal.should_close() {
            return;
        }
        if !save {
            self.alerts.editor = Some(editor);
            return;
        }
        let mut rules = self.active_config.alerts.clone();
        match editor.build(&rules) {
            Err(error) => {
                editor.error = Some(error);
                self.alerts.editor = Some(editor);
            }
            Ok(rule) => {
                match rules.iter_mut().find(|r| r.id == rule.id) {
                    Some(entry) => *entry = rule,
                    None => rules.push(rule),
                }
                self.save_alerts(rules);
            }
        }
    }
}
