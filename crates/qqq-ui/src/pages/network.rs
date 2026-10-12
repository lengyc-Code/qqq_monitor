use super::super::DesktopApp;
use crate::{components::*, i18n, pixel, theme};
use eframe::egui::{self, RichText};
use qqq_domain::{DeviceKind, MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn network(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        let t = i18n::t();
        if device_picker(
            ui,
            view,
            "network-selector",
            &mut self.network,
            DeviceKind::Network,
            (&pixel::ICON_NETWORK, t.adapter, theme::NETWORK),
        ) {
            self.config.selected_network = view
                .devices
                .get(&self.network)
                .and_then(|d| d.persistent_id.clone());
            let mut config = self.active_config.clone();
            config.selected_network = self.config.selected_network.clone();
            self.save_config(config, true);
        }
        let receive = MetricKey::new(&self.network, "receive");
        let send = MetricKey::new(&self.network, "send");
        let session = |key: &MetricKey| {
            bytes(view.metrics.get(key).map(|r| r.session_bytes).unwrap_or(0) as f64)
        };
        ui.columns(2, |uis| {
            MetricCard {
                title: t.receive,
                icon: &pixel::ICON_NETWORK,
                tone: theme::NETWORK,
                series: &[(&receive, theme::BLUE)],
                detail: (t.session_received)(&session(&receive)),
                template: RATE_TEMPLATE,
            }
            .show(&mut uis[0], view, false);
            MetricCard {
                title: t.send,
                icon: &pixel::ICON_NETWORK,
                tone: theme::CPU,
                series: &[(&send, theme::TEAL)],
                detail: (t.session_sent)(&session(&send)),
                template: RATE_TEMPLATE,
            }
            .show(&mut uis[1], view, false);
        });
        history_chart(
            ui,
            view,
            "network-history",
            t.network_chart,
            Unit::BytesPerSecond,
            self.range,
            &[
                (&receive, t.receive, theme::BLUE),
                (&send, t.send, theme::TEAL),
            ],
            200.0,
        );
        ui.label(RichText::new(t.network_note).size(12.0).color(theme::MUTED));
    }
}
