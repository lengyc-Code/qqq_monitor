use super::super::DesktopApp;
use crate::{components::*, i18n, pixel, theme};
use eframe::egui::{self, Color32, RichText, vec2};
use qqq_domain::{DeviceDescriptor, DeviceKind, MetricKey, Reason, Unit, remote_device};
use qqq_engine::{
    MonitorCommand, MonitorView, RemoteAuthKind, RemoteHost, RemoteState, Secret, needs_user,
};
use std::time::{SystemTime, UNIX_EPOCH};

/// Width of the add and edit dialog.
const DIALOG_WIDTH: f32 = 460.0;

/// State of the remote hosts page.
#[derive(Default)]
pub(in crate::app) struct RemotePage {
    /// Host on screen, by ID.
    selected: Option<String>,
    /// Device IDs of the disk and the network adapter the page charts.
    disk: String,
    network: String,
    editor: Option<Editor>,
    /// Host whose removal waits for confirmation, by ID.
    removing: Option<String>,
}

/// Fields of the add or edit dialog. The secret lives here only while the dialog is open.
struct Editor {
    /// Host being edited; `None` adds a host.
    id: Option<String>,
    name: String,
    address: String,
    port: String,
    user: String,
    auth: RemoteAuthKind,
    key_file: String,
    /// Password, or passphrase of the key file.
    secret: String,
    error: Option<String>,
}

impl Editor {
    fn new(host: Option<&RemoteHost>) -> Self {
        Self {
            id: host.map(|h| h.id.clone()),
            name: host.map(|h| h.name.clone()).unwrap_or_default(),
            address: host.map(|h| h.address.clone()).unwrap_or_default(),
            port: host.map_or(22, |h| h.port).to_string(),
            user: host.map(|h| h.user.clone()).unwrap_or_default(),
            auth: host.map_or(RemoteAuthKind::Password, |h| h.auth),
            key_file: host.and_then(|h| h.key_file.clone()).unwrap_or_default(),
            secret: String::new(),
            error: None,
        }
    }

    /// The host entry and what becomes of its secret: `None` keeps the saved one,
    /// `Some(None)` removes it.
    #[allow(clippy::type_complexity)]
    fn build(&self, hosts: &[RemoteHost]) -> Result<(RemoteHost, Option<Option<Secret>>), String> {
        let t = i18n::t();
        let existing = self
            .id
            .as_ref()
            .and_then(|id| hosts.iter().find(|h| h.id == *id));
        let address = self.address.trim();
        let user = self.user.trim();
        let key_file = self.key_file.trim();
        if address.is_empty() {
            return Err((t.remote_missing)(t.remote_address));
        }
        let port = match self.port.trim().parse::<u16>() {
            Ok(port) if port > 0 => port,
            _ => return Err(t.remote_bad_port.into()),
        };
        if user.is_empty() {
            return Err((t.remote_missing)(t.remote_user));
        }
        if self.auth == RemoteAuthKind::Key && key_file.is_empty() {
            return Err((t.remote_missing)(t.remote_key_file));
        }
        let kept = existing.is_some_and(|h| h.auth == self.auth);
        if self.auth == RemoteAuthKind::Password && self.secret.is_empty() && !kept {
            return Err((t.remote_missing)(t.remote_password));
        }
        let secret = match self.auth {
            _ if !self.secret.is_empty() && self.auth != RemoteAuthKind::Agent => {
                Some(Some(Secret(self.secret.clone())))
            }
            _ if kept || existing.is_none() => None,
            _ => Some(None),
        };
        let host = RemoteHost {
            id: existing.map_or_else(|| new_id(hosts), |h| h.id.clone()),
            name: self.name.trim().into(),
            address: address.into(),
            port,
            user: user.into(),
            auth: self.auth,
            key_file: (self.auth == RemoteAuthKind::Key).then(|| key_file.into()),
            // The trusted key belongs to the machine at that address.
            host_key: existing
                .filter(|h| h.address == address && h.port == port)
                .and_then(|h| h.host_key.clone()),
        };
        host.validate().map_err(|notice| i18n::notice(&notice))?;
        Ok((host, secret))
    }
}

/// An ID unlike any other host's.
fn new_id(hosts: &[RemoteHost]) -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    (0..)
        .map(|n| format!("h{:x}", stamp + n))
        .find(|id| !hosts.iter().any(|h| h.id == *id))
        .unwrap_or_default()
}

/// State of a host in words and as a color.
fn state_text(view: &MonitorView, id: &str) -> (String, Color32) {
    let t = i18n::t();
    match view.remote.hosts.get(id).map(|s| &s.state) {
        Some(RemoteState::Connected) => (t.remote_connected.into(), theme::GOOD),
        Some(RemoteState::Failed(reason)) => {
            let mut text = (t.remote_failed)(&i18n::reason(reason));
            if !needs_user(reason) {
                text.push_str(&format!(" · {}", t.remote_retrying));
            }
            (text, theme::ERROR)
        }
        _ => (t.remote_connecting.into(), theme::MUTED),
    }
}

fn auth_label(auth: RemoteAuthKind) -> &'static str {
    let t = i18n::t();
    match auth {
        RemoteAuthKind::Password => t.remote_auth_password,
        RemoteAuthKind::Key => t.remote_auth_key,
        RemoteAuthKind::Agent => t.remote_auth_agent,
    }
}

/// Number after the last `:` of a device ID, such as the core index.
fn index(id: &str) -> usize {
    id.rsplit_once(':')
        .and_then(|(_, n)| n.parse().ok())
        .unwrap_or(0)
}

impl DesktopApp {
    /// Keeps a configured host selected, and a disk and an adapter of it: the adapter of
    /// the default route when there is one.
    fn remote_defaults(&mut self) {
        let hosts = &self.active_config.remote_hosts;
        if !self
            .remote
            .selected
            .as_ref()
            .is_some_and(|id| hosts.iter().any(|h| h.id == *id))
        {
            self.remote.selected = hosts.first().map(|h| h.id.clone());
            self.remote.disk.clear();
            self.remote.network.clear();
        }
        let Some(host) = self.remote.selected.as_deref() else {
            return;
        };
        let devices = |kind| {
            self.view
                .devices
                .values()
                .filter(move |d| d.kind == kind && d.host.as_deref() == Some(host))
        };
        if !devices(DeviceKind::Network).any(|d| d.id == self.remote.network) {
            self.remote.network = devices(DeviceKind::Network)
                .max_by_key(|d| {
                    (
                        detail(d, "default_route").is_some(),
                        detail(d, "interface_type") == Some("physical"),
                        d.online,
                    )
                })
                .map(|d| d.id.clone())
                .unwrap_or_default();
        }
        if !devices(DeviceKind::Disk).any(|d| d.id == self.remote.disk) {
            self.remote.disk = devices(DeviceKind::Disk)
                .find(|d| d.online)
                .map(|d| d.id.clone())
                .unwrap_or_default();
        }
    }

    /// History the page charts for the selected host.
    pub(in crate::app) fn remote_query(&mut self) -> Vec<MetricKey> {
        self.remote_defaults();
        let Some(host) = &self.remote.selected else {
            return Vec::new();
        };
        let mut keys = vec![
            MetricKey::new(remote_device(host, "cpu"), "usage"),
            MetricKey::new(remote_device(host, "memory"), "usage"),
        ];
        for (device, metrics) in [
            (&self.remote.disk, ["read", "write"]),
            (&self.remote.network, ["receive", "send"]),
        ] {
            if !device.is_empty() {
                keys.extend(metrics.map(|metric| MetricKey::new(device, metric)));
            }
        }
        keys
    }

    /// Saves the host list right away, like the network adapter choice.
    fn save_remote_hosts(&mut self, hosts: Vec<RemoteHost>) {
        self.apply_now(|config| config.remote_hosts = hosts);
    }

    pub(in crate::app) fn remote(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let hosts = self.active_config.remote_hosts.clone();
        self.remote_hosts(ui, view, &hosts);
        let selected = self
            .remote
            .selected
            .as_ref()
            .and_then(|id| hosts.iter().find(|h| h.id == *id));
        if let Some(host) = selected {
            self.remote_connection(ui, view, host);
            self.remote_readings(ui, view, &host.id);
        }
        self.remote_editor(ui.ctx(), view);
        self.remote_removal(ui.ctx());
    }

    fn remote_hosts(&mut self, ui: &mut egui::Ui, view: &MonitorView, hosts: &[RemoteHost]) {
        let t = i18n::t();
        theme::card().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for host in hosts {
                    let (state, color) = state_text(view, &host.id);
                    pixel_dot(ui, color).on_hover_text(&state);
                    let selected = self.remote.selected.as_ref() == Some(&host.id);
                    if theme::chip(ui, host.label(), selected)
                        .on_hover_text(state)
                        .clicked()
                        && !selected
                    {
                        self.remote.selected = Some(host.id.clone());
                        self.remote.disk.clear();
                        self.remote.network.clear();
                    }
                    ui.add_space(8.0);
                }
                let width = label_width(ui, &[t.remote_add], 14.0) + 64.0;
                if theme::pixel_button(
                    ui,
                    t.remote_add,
                    Some(&pixel::ICON_REMOTE),
                    hosts.is_empty(),
                    vec2(width, 32.0),
                )
                .clicked()
                {
                    self.remote.editor = Some(Editor::new(None));
                }
            });
            if hosts.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new(t.remote_empty).color(theme::MUTED));
            }
        });
    }

    fn remote_connection(&mut self, ui: &mut egui::Ui, view: &MonitorView, host: &RemoteHost) {
        let t = i18n::t();
        theme::card().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.horizontal(|ui| {
                tag(ui, Some(&pixel::ICON_REMOTE), host.label(), theme::NETWORK);
                ui.label(
                    RichText::new(format!(
                        "{}@{}:{}  ·  {}",
                        host.user,
                        host.address,
                        host.port,
                        auth_label(host.auth)
                    ))
                    .size(12.0)
                    .color(theme::MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let size = |ui: &egui::Ui, text: &str| {
                        vec2(label_width(ui, &[text], 14.0) + 30.0, 30.0)
                    };
                    let remove = size(ui, t.remote_remove);
                    if theme::pixel_button(ui, t.remote_remove, None, false, remove).clicked() {
                        self.remote.removing = Some(host.id.clone());
                    }
                    let edit = size(ui, t.remote_edit);
                    if theme::pixel_button(ui, t.remote_edit, None, false, edit).clicked() {
                        self.remote.editor = Some(Editor::new(Some(host)));
                    }
                    let reconnect = size(ui, t.remote_reconnect);
                    if theme::pixel_button(ui, t.remote_reconnect, None, false, reconnect).clicked()
                    {
                        self.command(MonitorCommand::Reconnect(host.id.clone()));
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (state, color) = state_text(view, &host.id);
                pixel_dot(ui, color);
                ui.add(egui::Label::new(RichText::new(state).size(12.0).color(color)).wrap());
            });
            let state = view.remote.hosts.get(&host.id).map(|s| &s.state);
            let (warning, button, key) = match state {
                Some(RemoteState::Failed(Reason::HostKeyUnknown { key, fingerprint })) => {
                    ((t.remote_key_unknown)(fingerprint), t.remote_trust, key)
                }
                Some(RemoteState::Failed(Reason::HostKeyChanged { key, fingerprint })) => {
                    ((t.remote_key_changed)(fingerprint), t.remote_trust_new, key)
                }
                _ => return,
            };
            ui.label(RichText::new(warning).size(12.0).color(theme::WARNING));
            let size = vec2(label_width(ui, &[button], 14.0) + 64.0, 34.0);
            if theme::pixel_button(ui, button, Some(&pixel::ICON_HEART), true, size).clicked() {
                let mut hosts = self.active_config.remote_hosts.clone();
                if let Some(entry) = hosts.iter_mut().find(|h| h.id == host.id) {
                    entry.host_key = Some(key.clone());
                    self.save_remote_hosts(hosts);
                }
            }
        });
    }

    fn remote_readings(&mut self, ui: &mut egui::Ui, view: &MonitorView, host: &str) {
        let t = i18n::t();
        let id = |device: &str| remote_device(host, device);
        let (system, cpu_id, memory_id) = (id("system"), id("cpu"), id("memory"));
        let cpu = MetricKey::new(&cpu_id, "usage");
        let memory = MetricKey::new(&memory_id, "usage");
        let read = MetricKey::new(&self.remote.disk, "read");
        let write = MetricKey::new(&self.remote.disk, "write");
        let receive = MetricKey::new(&self.remote.network, "receive");
        let send = MetricKey::new(&self.remote.network, "send");
        let cpu_detail = (t.remote_cpu_detail)(
            &format_value(view.value(&cpu_id, "iowait"), Unit::Percent),
            &format_value(view.value(&system, "processes"), Unit::Count),
        );
        let current =
            |key: &MetricKey, unit| format_value(view.metrics.get(key).and_then(|r| r.value), unit);
        let cards = [
            MetricCard {
                title: "CPU",
                icon: &pixel::ICON_CPU,
                tone: theme::CPU,
                series: &[(&cpu, theme::TEAL)],
                detail: cpu_detail,
                template: RATE_TEMPLATE,
            },
            MetricCard {
                title: t.memory,
                icon: &pixel::ICON_MEMORY,
                tone: theme::MEMORY,
                series: &[(&memory, theme::PINK)],
                detail: (t.used_of)(
                    &format_value(view.value(&memory_id, "used"), Unit::Bytes),
                    &format_value(view.value(&memory_id, "total"), Unit::Bytes),
                ),
                template: RATE_TEMPLATE,
            },
            MetricCard {
                title: t.disk_read,
                icon: &pixel::ICON_DISK,
                tone: theme::DISK,
                series: &[(&read, theme::GOLD), (&write, theme::PINK)],
                detail: format!("{} {}", t.write, current(&write, Unit::BytesPerSecond)),
                template: RATE_TEMPLATE,
            },
            MetricCard {
                title: t.network_receive,
                icon: &pixel::ICON_NETWORK,
                tone: theme::NETWORK,
                series: &[(&receive, theme::BLUE), (&send, theme::TEAL)],
                detail: format!("{} {}", t.send, current(&send, Unit::BytesPerSecond)),
                template: RATE_TEMPLATE,
            },
        ];
        let columns = if ui.available_width() >= 840.0 { 4 } else { 2 };
        for row in cards.chunks(columns) {
            ui.columns(columns, |uis| {
                for (index, card) in row.iter().enumerate() {
                    card.show(&mut uis[index], view, false);
                }
            });
        }
        ui.add_space(theme::SECTION_GAP - 8.0);

        let trend = |ui: &mut egui::Ui| {
            history_chart(
                ui,
                view,
                "remote-cpu",
                t.cpu_memory_chart,
                Unit::Percent,
                self.range,
                &[(&cpu, "CPU", theme::TEAL), (&memory, t.memory, theme::PINK)],
                262.0,
            )
        };
        if ui.available_width() >= 900.0 {
            ui.horizontal_top(|ui| {
                let info_width = 320.0;
                let chart_width = ui.available_width() - info_width - ui.spacing().item_spacing.x;
                let height = ui
                    .allocate_ui_with_layout(
                        vec2(chart_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        trend,
                    )
                    .inner
                    .rect
                    .height();
                ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| remote_profile(ui, view, host, height),
                );
            });
        } else {
            trend(ui);
            remote_profile(ui, view, host, 0.0);
        }

        theme::card().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                tag(ui, Some(&pixel::ICON_DISK), t.physical_disk, theme::DISK);
                device_selector(
                    ui,
                    view,
                    "remote-disk",
                    &mut self.remote.disk,
                    DeviceKind::Disk,
                    Some(host),
                );
                ui.add_space(12.0);
                tag(ui, Some(&pixel::ICON_NETWORK), t.adapter, theme::NETWORK);
                device_selector(
                    ui,
                    view,
                    "remote-network",
                    &mut self.remote.network,
                    DeviceKind::Network,
                    Some(host),
                );
            });
            device_details(ui, view, &self.remote.network, false);
        });
        ui.columns(2, |uis| {
            history_chart(
                &mut uis[0],
                view,
                "remote-disk",
                t.disk_io,
                Unit::BytesPerSecond,
                self.range,
                &[(&read, t.read, theme::GOLD), (&write, t.write, theme::PINK)],
                130.0,
            );
            history_chart(
                &mut uis[1],
                view,
                "remote-network",
                t.network_io,
                Unit::BytesPerSecond,
                self.range,
                &[
                    (&receive, t.receive, theme::BLUE),
                    (&send, t.send, theme::TEAL),
                ],
                130.0,
            );
        });

        let devices = |kind| {
            let mut devices: Vec<&DeviceDescriptor> = view
                .devices
                .values()
                .filter(|d| d.kind == kind && d.host.as_deref() == Some(host))
                .collect();
            devices.sort_by(|a, b| index(&a.id).cmp(&index(&b.id)).then(a.name.cmp(&b.name)));
            devices
        };
        section_title(
            ui,
            &pixel::ICON_DISK,
            t.remote_volumes_title,
            t.remote_volumes_hint,
        );
        let volumes = devices(DeviceKind::Volume);
        if volumes.is_empty() {
            ui.label(
                RichText::new(t.volumes_waiting)
                    .size(12.0)
                    .color(theme::MUTED),
            );
        }
        let columns = if ui.available_width() >= 900.0 { 3 } else { 2 };
        for row in volumes.chunks(columns) {
            ui.columns(columns, |uis| {
                for (index, volume) in row.iter().enumerate() {
                    let subtitle = [detail(volume, "device"), detail(volume, "file_system")]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join("  ·  ");
                    volume_card(&mut uis[index], view, volume, &subtitle, "");
                }
            });
        }

        let cores = devices(DeviceKind::Core);
        if !cores.is_empty() {
            ui.horizontal(|ui| {
                section_title(ui, &pixel::ICON_CPU, t.logical_cores, t.remote_cores_hint);
                heat_legend(ui);
            });
            let columns = ((ui.available_width() / 110.0) as usize).clamp(2, 8);
            for row in cores.chunks(columns) {
                ui.columns(columns, |uis| {
                    for (column, device) in row.iter().enumerate() {
                        heat_tile(
                            &mut uis[column],
                            &format!("#{:02}", index(&device.id) + 1),
                            &device_name(device),
                            view.value(&device.id, "usage"),
                            false,
                        );
                    }
                });
            }
        }
    }

    fn remote_editor(&mut self, ctx: &egui::Context, view: &MonitorView) {
        let Some(mut editor) = self.remote.editor.take() else {
            return;
        };
        let t = i18n::t();
        let editing = editor.id.is_some();
        let modal = egui::Modal::new(egui::Id::new("remote-editor")).show(ctx, |ui| {
            ui.set_width(DIALOG_WIDTH);
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(
                RichText::new(if editing {
                    t.remote_edit_title
                } else {
                    t.remote_add_title
                })
                .size(16.0)
                .strong(),
            );
            if !editing && !view.remote.presets.is_empty() {
                egui::ComboBox::from_id_salt("remote-import")
                    .width(DIALOG_WIDTH)
                    .selected_text(t.remote_import)
                    .show_ui(ui, |ui| {
                        for preset in view.remote.presets.iter() {
                            let mut text = format!("{}  ", preset.alias);
                            if !preset.user.is_empty() {
                                text.push_str(&format!("{}@", preset.user));
                            }
                            text.push_str(&format!("{}:{}", preset.address, preset.port));
                            if ui.selectable_label(false, text).clicked() {
                                editor.name = preset.alias.clone();
                                editor.address = preset.address.clone();
                                editor.port = preset.port.to_string();
                                if !preset.user.is_empty() {
                                    editor.user = preset.user.clone();
                                }
                                if let Some(file) = &preset.key_file {
                                    editor.auth = RemoteAuthKind::Key;
                                    editor.key_file = file.clone();
                                }
                            }
                        }
                    });
            }
            let labels = [
                t.remote_name,
                t.remote_address,
                t.remote_user,
                t.remote_auth,
                t.remote_password,
                t.remote_key_file,
                t.remote_passphrase,
            ];
            let width = label_width(ui, &labels, 14.0) + 8.0;
            let row = |ui: &mut egui::Ui, label: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.add_sized([width, 26.0], egui::Label::new(label));
                    add(ui);
                });
            };
            let field = |ui: &mut egui::Ui, text: &mut String, hint: &str, secret: bool| {
                ui.add(
                    egui::TextEdit::singleline(text)
                        .hint_text(hint)
                        .password(secret)
                        .desired_width(ui.available_width()),
                );
            };
            row(ui, t.remote_name, &mut |ui| {
                field(ui, &mut editor.name, t.remote_name_hint, false)
            });
            row(ui, t.remote_address, &mut |ui| {
                let port_label = label_width(ui, &[t.remote_port], 14.0);
                ui.add(
                    egui::TextEdit::singleline(&mut editor.address)
                        .hint_text(t.remote_address_hint)
                        .desired_width(ui.available_width() - port_label - 80.0),
                );
                ui.label(t.remote_port);
                field(ui, &mut editor.port, "22", false);
            });
            row(ui, t.remote_user, &mut |ui| {
                field(ui, &mut editor.user, "root", false)
            });
            row(ui, t.remote_auth, &mut |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for auth in [
                    RemoteAuthKind::Password,
                    RemoteAuthKind::Key,
                    RemoteAuthKind::Agent,
                ] {
                    if theme::chip(ui, auth_label(auth), editor.auth == auth).clicked() {
                        editor.auth = auth;
                    }
                }
            });
            let kept = editing
                && self
                    .active_config
                    .remote_hosts
                    .iter()
                    .any(|h| Some(&h.id) == editor.id.as_ref() && h.auth == editor.auth);
            match editor.auth {
                RemoteAuthKind::Password => row(ui, t.remote_password, &mut |ui| {
                    let hint = if kept { t.remote_password_keep } else { "" };
                    field(ui, &mut editor.secret, hint, true)
                }),
                RemoteAuthKind::Key => {
                    row(ui, t.remote_key_file, &mut |ui| {
                        field(ui, &mut editor.key_file, t.remote_key_hint, false)
                    });
                    row(ui, t.remote_passphrase, &mut |ui| {
                        let hint = if kept {
                            t.remote_passphrase_keep
                        } else {
                            t.remote_passphrase_hint
                        };
                        field(ui, &mut editor.secret, hint, true)
                    });
                }
                RemoteAuthKind::Agent => {
                    ui.label(
                        RichText::new(t.remote_agent_hint)
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                }
            }
            ui.label(
                RichText::new(t.remote_secret_note)
                    .size(11.0)
                    .color(theme::MUTED),
            );
            if let Some(error) = &editor.error {
                ui.label(RichText::new(error).size(12.0).color(theme::ERROR));
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let size =
                    |ui: &egui::Ui, text: &str| vec2(label_width(ui, &[text], 14.0) + 40.0, 34.0);
                let (yes, no) = (size(ui, t.remote_save), size(ui, t.cancel));
                let save = theme::pixel_button(ui, t.remote_save, None, true, yes).clicked();
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
            self.remote.editor = Some(editor);
            return;
        }
        let mut hosts = self.active_config.remote_hosts.clone();
        match editor.build(&hosts) {
            Err(error) => {
                editor.error = Some(error);
                self.remote.editor = Some(editor);
            }
            Ok((host, secret)) => {
                // The secret is in the store before the host's worker looks for it.
                if let Some(secret) = secret {
                    self.command(MonitorCommand::RemoteSecret {
                        host: host.id.clone(),
                        secret,
                    });
                }
                self.remote.selected = Some(host.id.clone());
                match hosts.iter_mut().find(|h| h.id == host.id) {
                    Some(entry) => *entry = host,
                    None => hosts.push(host),
                }
                self.save_remote_hosts(hosts);
            }
        }
    }

    fn remote_removal(&mut self, ctx: &egui::Context) {
        let Some(id) = self.remote.removing.clone() else {
            return;
        };
        let Some(host) = self
            .active_config
            .remote_hosts
            .iter()
            .find(|h| h.id == id)
            .cloned()
        else {
            self.remote.removing = None;
            return;
        };
        let t = i18n::t();
        let modal = egui::Modal::new(egui::Id::new("remote-remove")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.label(
                RichText::new((t.remote_remove_confirm)(host.label()))
                    .size(15.0)
                    .strong(),
            );
            ui.label(RichText::new(t.remote_remove_warning).color(theme::MUTED));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let size =
                    |ui: &egui::Ui, text: &str| vec2(label_width(ui, &[text], 14.0) + 40.0, 34.0);
                let (yes, no) = (size(ui, t.remote_remove), size(ui, t.cancel));
                let remove = theme::pixel_button(ui, t.remote_remove, None, true, yes).clicked();
                let cancel = theme::pixel_button(ui, t.cancel, None, false, no).clicked();
                (remove, cancel)
            })
            .inner
        });
        let (remove, cancel) = modal.inner;
        if remove {
            self.remote.removing = None;
            self.command(MonitorCommand::RemoteSecret {
                host: id.clone(),
                secret: None,
            });
            let hosts = self
                .active_config
                .remote_hosts
                .iter()
                .filter(|h| h.id != id)
                .cloned()
                .collect();
            self.save_remote_hosts(hosts);
        } else if cancel || modal.should_close() {
            self.remote.removing = None;
        }
    }
}

/// What the host is: system, processor, memory, uptime and load.
fn remote_profile(ui: &mut egui::Ui, view: &MonitorView, host: &str, min_height: f32) {
    let t = i18n::t();
    let device = |id: &str| view.devices.get(&remote_device(host, id));
    let system = device("system");
    let cpu = device("cpu");
    let text = |device: Option<&DeviceDescriptor>, key: &str| {
        device
            .and_then(|d| detail(d, key))
            .unwrap_or("—")
            .to_string()
    };
    let label = |key: &'static str| (t.detail)(key).unwrap_or(key);
    let value = |id: &str, metric: &str, unit| {
        format_value(view.value(&remote_device(host, id), metric), unit)
    };
    let swap = match view.value(&remote_device(host, "memory"), "swap_total") {
        Some(total) if total > 0.0 => (t.used_of)(
            &value("memory", "swap_used", Unit::Bytes),
            &format_value(Some(total), Unit::Bytes),
        ),
        Some(_) => t.remote_no_swap.into(),
        None => "—".into(),
    };
    let rows = [
        (
            &pixel::ICON_REMOTE,
            theme::NETWORK,
            t.remote_hostname,
            system
                .map(|d| d.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "—".into()),
        ),
        (
            &pixel::ICON_STAR,
            theme::ACCENT,
            label("os"),
            text(system, "os"),
        ),
        (
            &pixel::ICON_GEAR,
            theme::ACCENT,
            label("kernel"),
            // The short architecture first, so a long kernel release truncates instead.
            [text(system, "architecture"), text(system, "kernel")].join(" · "),
        ),
        (
            &pixel::ICON_CPU,
            theme::CPU,
            t.processor,
            cpu.map(|d| d.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "—".into()),
        ),
        (
            &pixel::ICON_STAR,
            theme::CPU,
            t.logical_cores,
            text(cpu, "logical_cores"),
        ),
        (
            &pixel::ICON_MEMORY,
            theme::MEMORY,
            t.total_memory,
            value("memory", "total", Unit::Bytes),
        ),
        (&pixel::ICON_MEMORY, theme::MEMORY, t.remote_swap, swap),
        (
            &pixel::ICON_CLOCK,
            theme::ACCENT,
            t.uptime,
            value("system", "uptime", Unit::Seconds),
        ),
        (
            &pixel::ICON_HEART,
            theme::ACCENT,
            t.remote_load,
            // Load averages read best with two decimals, like `uptime` prints them.
            ["load1", "load5", "load15"]
                .map(|metric| {
                    view.value(&remote_device(host, "system"), metric)
                        .map_or_else(|| "—".into(), |load| format!("{load:.2}"))
                })
                .join(" · "),
        ),
    ];
    theme::card().min_height(min_height).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        tag(
            ui,
            Some(&pixel::ICON_HEART),
            t.device_profile,
            theme::ACCENT,
        );
        ui.add_space(4.0);
        let labels = rows.each_ref().map(|row| row.2);
        let width = label_width(ui, &labels, 12.0) + 12.0;
        for (icon, tone, label, value) in &rows {
            info_row(ui, icon, *tone, label, width, value);
        }
    });
}
