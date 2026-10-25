use super::{super::DesktopApp, tools::hotkey_option};
use crate::{components::*, i18n, pixel, theme};
use eframe::egui::{self, RichText, vec2};
use qqq_domain::Shortcut;
use qqq_engine::{Language, MonitorCommand, MonitorView, Notice};

/// History budget shown in diagnostics; matches the engine's allocation limit.
const HISTORY_BUDGET: f64 = 16.0 * 1024.0 * 1024.0;
/// Widest history usage bar, in points.
const BAR_WIDTH: f32 = 520.0;
/// The logo shown in the README, with MINT holding a monitor panel.
const LOGO: &[u8] = include_bytes!("../../../../docs/images/logo.png");
/// Width of the logo on the page, in points.
const LOGO_WIDTH: f32 = 200.0;
/// The project's home on GitHub.
const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
/// Language, toolkit, platform and license, as under the README title.
const STACK: &str = "Rust 2024 · egui / wgpu (DX12) · Windows x64 · MIT";

impl DesktopApp {
    pub(in crate::app) fn settings(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        // Preferences beside the mascot and version, runtime status underneath.
        if ui.available_width() >= 900.0 {
            ui.columns(2, |uis| {
                let height = self.preferences(&mut uis[0]);
                self.about(&mut uis[1], height);
            });
        } else {
            self.preferences(ui);
            self.about(ui, 0.0);
        }
        self.diagnostics(ui, view);
    }

    /// Returns the card's outer height so a neighbouring card can match it.
    fn preferences(&mut self, ui: &mut egui::Ui) -> f32 {
        let t = i18n::t();
        theme::card()
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                tag(ui, Some(&pixel::ICON_GEAR), t.preferences, theme::ACCENT);
                let width = label_width(
                    ui,
                    &[
                        t.language,
                        t.sampling_interval,
                        t.default_range,
                        t.animations,
                        t.edge_docking,
                        t.minimize_to_tray,
                        t.frame_monitoring,
                        t.tool_screenshots,
                        t.tool_clipboard,
                        t.tool_notes,
                        t.tool_commands,
                        t.autostart,
                    ],
                    14.0,
                ) + 8.0;
                // Each row is a label column followed by controls; chips and long texts
                // wrap beside the label rather than under it.
                let row_label = |ui: &mut egui::Ui, text: &str| {
                    ui.add_sized([width, 30.0], egui::Label::new(text));
                };
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    row_label(ui, t.language);
                    ui.horizontal_wrapped(|ui| {
                        let choices = [(None, t.follow_system)]
                            .into_iter()
                            .chain(Language::ALL.map(|l| (Some(l), i18n::native_name(l))));
                        for (language, label) in choices {
                            if theme::chip(ui, label, self.config.language == language).clicked()
                                && self.active_config.language != language
                            {
                                // Applied and saved at once, like the network adapter choice.
                                self.config.language = language;
                                let mut config = self.active_config.clone();
                                config.language = language;
                                self.save_config(config, true);
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    row_label(ui, t.sampling_interval);
                    ui.horizontal_wrapped(|ui| {
                        for ms in [500, 1000, 2000] {
                            let label = (t.seconds)(ms as f64 / 1000.0);
                            if theme::chip(ui, &label, self.config.sampling_ms == ms).clicked() {
                                self.config.sampling_ms = ms;
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    row_label(ui, t.default_range);
                    ui.horizontal_wrapped(|ui| {
                        for (secs, label) in [
                            (60, (t.seconds)(60.0)),
                            (300, (t.minutes)(5)),
                            (900, (t.minutes)(15)),
                        ] {
                            if theme::chip(ui, &label, self.config.history_secs == secs).clicked() {
                                self.config.history_secs = secs;
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    row_label(ui, t.animations);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    if ui
                        .checkbox(&mut self.config.animations, t.animations_option)
                        .changed()
                    {
                        theme::apply(ui.ctx(), self.config.animations);
                    }
                });
                ui.horizontal(|ui| {
                    row_label(ui, t.edge_docking);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    ui.checkbox(&mut self.config.edge_docking, t.edge_docking_option);
                });
                ui.horizontal(|ui| {
                    row_label(ui, t.minimize_to_tray);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    ui.checkbox(&mut self.config.minimize_to_tray, t.minimize_to_tray_option);
                });
                ui.horizontal(|ui| {
                    row_label(ui, t.frame_monitoring);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    // Applied and saved at once, like the switch on the frames page.
                    let mut on = self.active_config.frame_monitoring;
                    if ui.checkbox(&mut on, t.frame_monitoring_option).changed() {
                        self.set_frame_monitoring(on);
                    }
                });
                // Applied and saved at once, like the switches on the tools page.
                for (shortcut, label) in [
                    (Shortcut::Screenshot, t.tool_screenshots),
                    (Shortcut::Note, t.tool_notes),
                ] {
                    ui.horizontal(|ui| {
                        row_label(ui, label);
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                        let (keys, mut on) = self.active_config.shortcut(shortcut);
                        let option = hotkey_option(shortcut, keys);
                        if ui.checkbox(&mut on, option).changed() {
                            self.apply_now(|config| {
                                if let Some((_, enabled)) = config.shortcut_mut(shortcut) {
                                    *enabled = on;
                                }
                            });
                        }
                    });
                }
                ui.horizontal(|ui| {
                    row_label(ui, t.tool_clipboard);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    let mut on = self.active_config.clipboard_manager;
                    if ui.checkbox(&mut on, t.clip_option).changed() {
                        self.apply_now(|config| config.clipboard_manager = on);
                    }
                });
                ui.horizontal(|ui| {
                    row_label(ui, t.tool_commands);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    let mut on = self.active_config.command_sound;
                    if ui.checkbox(&mut on, t.command_sound_option).changed() {
                        self.apply_now(|config| config.command_sound = on);
                    }
                });
                ui.horizontal(|ui| {
                    row_label(ui, t.autostart);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    ui.vertical(|ui| {
                        // A system setting rather than a preference: it changes at once, and
                        // so does the choice beneath it.
                        let mut enabled =
                            *self.autostart.get_or_insert_with(self.desktop.autostart);
                        if ui.checkbox(&mut enabled, t.autostart_option).changed() {
                            match (self.desktop.set_autostart)(enabled) {
                                Ok(())
                                    if matches!(
                                        self.message,
                                        Some(Notice::AutostartUnchanged(_))
                                    ) =>
                                {
                                    self.message = None
                                }
                                Ok(()) => {}
                                Err(reason) => {
                                    self.message = Some(Notice::AutostartUnchanged(reason))
                                }
                            }
                            self.autostart = None;
                        }
                        let mut to_tray = self.config.autostart_to_tray;
                        if ui
                            .add_enabled(
                                enabled,
                                egui::Checkbox::new(&mut to_tray, t.autostart_to_tray_option),
                            )
                            .changed()
                        {
                            self.config.autostart_to_tray = to_tray;
                            let mut config = self.active_config.clone();
                            config.autostart_to_tray = to_tray;
                            self.save_config(config, true);
                        }
                    });
                });
                ui.horizontal(|ui| {
                    let width = (label_width(ui, &[t.apply_save], 14.0) + 76.0).max(150.0);
                    if theme::pixel_button(
                        ui,
                        t.apply_save,
                        Some(&pixel::ICON_HEART),
                        true,
                        vec2(width, 38.0),
                    )
                    .clicked()
                    {
                        self.range = self.config.history_secs;
                        self.save_config(self.config.clone(), true);
                    }
                    ui.label(
                        RichText::new(i18n::save_status(&self.view.save_status))
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                });
            })
            .response
            .rect
            .height()
    }

    /// The project logo above the name, the version, an introduction and a button to the
    /// GitHub page, at least `min_height` tall to match the card beside it.
    fn about(&mut self, ui: &mut egui::Ui, min_height: f32) {
        let t = i18n::t();
        theme::card().min_height(min_height).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            let bottom = ui.min_rect().bottom();
            tag(ui, Some(&pixel::ICON_HEART), t.about, theme::DISK);
            // The content sits in the middle of the room the neighbouring card leaves, by
            // the height it took in the previous pass.
            let id = ui.id().with("about-content");
            let previous = ui.data(|d| d.get_temp::<f32>(id));
            if let Some(height) = previous {
                ui.add_space(((bottom - ui.cursor().top() - height) / 2.0).max(0.0));
            }
            let top = ui.cursor().top();
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                if let Some(logo) = logo(ui.ctx()) {
                    let size = logo.size_vec2();
                    let width = LOGO_WIDTH.min(ui.available_width());
                    ui.add(egui::Image::from_texture((
                        logo.id(),
                        size * (width / size.x),
                    )));
                }
                pixel::label(
                    ui,
                    "QQQ MONITOR",
                    2.0,
                    theme::PINK,
                    pixel::Effect::Drop(theme::INK),
                );
                pixel::label(
                    ui,
                    concat!("v", env!("CARGO_PKG_VERSION")),
                    1.5,
                    theme::MUTED,
                    pixel::Effect::Plain,
                );
                ui.add_space(4.0);
                ui.label(t.about_intro);
                ui.label(RichText::new(STACK).size(12.0).color(theme::MUTED));
                ui.add_space(4.0);
                let size = vec2(label_width(ui, &[t.about_github], 14.0) + 64.0, 34.0);
                if theme::pixel_button(ui, t.about_github, Some(&pixel::ICON_STAR), false, size)
                    .on_hover_text(REPOSITORY)
                    .clicked()
                {
                    match (self.desktop.open_url)(REPOSITORY) {
                        Ok(()) if matches!(self.message, Some(Notice::OpenFailed(_))) => {
                            self.message = None
                        }
                        Ok(()) => {}
                        Err(reason) => self.message = Some(Notice::OpenFailed(reason)),
                    }
                }
            });
            let height = ui.min_rect().bottom().min(ui.cursor().top()) - top;
            if previous.is_none_or(|p| (p - height).abs() > 0.5) {
                ui.data_mut(|d| d.insert_temp(id, height));
                ui.ctx().request_discard("settings: about card height");
            }
        });
    }

    /// Runtime status across the page's width, with the refresh button on the right.
    fn diagnostics(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let t = i18n::t();
        theme::card().show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.horizontal(|ui| {
                tag(
                    ui,
                    Some(&pixel::ICON_CLOCK),
                    t.runtime_status,
                    theme::NETWORK,
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let size = vec2(
                        (label_width(ui, &[t.refresh_devices], 14.0) + 76.0).max(160.0),
                        38.0,
                    );
                    if theme::pixel_button(
                        ui,
                        t.refresh_devices,
                        Some(&pixel::ICON_STAR),
                        false,
                        size,
                    )
                    .clicked()
                    {
                        self.command(MonitorCommand::RefreshInventory);
                    }
                });
            });
            ui.label((t.batches)(
                view.diagnostics.received_batches,
                view.diagnostics.dropped_batches,
            ));
            ui.label((t.history_usage)(&bytes(
                view.diagnostics.history_bytes as f64,
            )));
            let used = view.diagnostics.history_bytes as f64 / HISTORY_BUDGET * 100.0;
            ui.allocate_ui(vec2(ui.available_width().min(BAR_WIDTH), 16.0), |ui| {
                segment_bar(ui, Some(used), theme::level_color(used, theme::BLUE), 16.0);
            });
            if view.diagnostics.collector_errors.is_empty() {
                ui.label(RichText::new(t.collectors_ok).size(12.0).color(theme::GOOD));
            }
            for (source, error) in &view.diagnostics.collector_errors {
                ui.label(
                    RichText::new((t.labeled)(source, &i18n::reason(error)))
                        .size(12.0)
                        .color(theme::ERROR),
                );
            }
        });
    }
}

/// The project logo of the README, decoded on first use and kept in the context.
fn logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("project-logo");
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    let image = image::load_from_memory_with_format(LOGO, image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let texture = ctx.load_texture(
        "project-logo",
        egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    Some(texture)
}
