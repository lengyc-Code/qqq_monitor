use super::super::DesktopApp;
use crate::{components::*, pixel, theme};
use eframe::egui::{self, RichText};
use qqq_domain::{DeviceKind, MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn network(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        if device_picker(
            ui,
            view,
            "network-selector",
            &mut self.network,
            DeviceKind::Network,
            (&pixel::ICON_NETWORK, "网卡", theme::NETWORK),
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
                title: "接收",
                icon: &pixel::ICON_NETWORK,
                tone: theme::NETWORK,
                series: &[(&receive, theme::BLUE)],
                detail: format!("本次会话已接收 {}", session(&receive)),
                template: RATE_TEMPLATE,
            }
            .show(&mut uis[0], view, false);
            MetricCard {
                title: "发送",
                icon: &pixel::ICON_NETWORK,
                tone: theme::CPU,
                series: &[(&send, theme::TEAL)],
                detail: format!("本次会话已发送 {}", session(&send)),
                template: RATE_TEMPLATE,
            }
            .show(&mut uis[1], view, false);
        });
        history_chart(
            ui,
            view,
            "network-history",
            "网络收发趋势",
            Unit::BytesPerSecond,
            self.range,
            &[
                (&receive, "接收", theme::BLUE),
                (&send, "发送", theme::TEAL),
            ],
            200.0,
        );
        ui.label(
            RichText::new(
                "只统计选中的单个接口，避免物理网卡与 VPN 流量重复相加；会话流量为本次应用观测到的有效计数差。",
            )
            .size(12.0)
            .color(theme::MUTED),
        );
    }
}
