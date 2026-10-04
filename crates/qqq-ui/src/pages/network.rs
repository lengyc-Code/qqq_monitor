use super::super::DesktopApp;
use crate::{components::*, theme};
use eframe::egui::{self, RichText};
use qqq_domain::{DeviceKind, MetricKey, Unit};
use qqq_engine::MonitorView;

impl DesktopApp {
    pub(in crate::app) fn network(&mut self, ui: &mut egui::Ui, view: &MonitorView) {
        if device_selector(
            ui,
            view,
            "network-selector",
            &mut self.network,
            DeviceKind::Network,
        ) {
            self.config.selected_network = view
                .devices
                .get(&self.network)
                .and_then(|d| d.persistent_id.clone());
            let mut config = self.active_config.clone();
            config.selected_network = self.config.selected_network.clone();
            self.save_config(config, true);
        }
        device_details(ui, view, &self.network);
        let receive = MetricKey::new(&self.network, "receive");
        let send = MetricKey::new(&self.network, "send");
        ui.columns(2, |uis| {
            metric_card(
                &mut uis[0],
                view,
                &receive,
                "接收速率",
                "当前选中网卡",
                theme::GREEN,
            );
            metric_card(
                &mut uis[1],
                view,
                &send,
                "发送速率",
                "当前选中网卡",
                theme::CYAN,
            );
        });
        ui.add_space(8.0);
        history_chart(
            ui,
            view,
            "network-history",
            "网络收发趋势",
            Unit::BytesPerSecond,
            self.range,
            &[
                (&receive, "接收", theme::GREEN),
                (&send, "发送", theme::CYAN),
            ],
            200.0,
        );
        ui.label(format!(
            "本次会话观测流量：接收 {} · 发送 {}",
            bytes(
                view.metrics
                    .get(&receive)
                    .map(|r| r.session_bytes)
                    .unwrap_or(0) as f64
            ),
            bytes(
                view.metrics
                    .get(&send)
                    .map(|r| r.session_bytes)
                    .unwrap_or(0) as f64
            )
        ));
        ui.label(
            RichText::new("选择单个接口统计，避免物理网卡与 VPN 流量重复相加。")
                .size(12.0)
                .color(theme::MUTED),
        );
    }
}
