use crate::{AppConfig, history::HistoryPoint};
use qqq_domain::*;
use std::{collections::BTreeMap, time::Instant};

#[derive(Clone, Debug, Default)]
pub struct Diagnostics {
    pub dropped_batches: u64,
    pub received_batches: u64,
    pub history_bytes: usize,
    pub collector_errors: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct MonitorView {
    pub sequence: u64,
    pub now: Instant,
    pub elapsed: f64,
    pub devices: BTreeMap<String, DeviceDescriptor>,
    pub metrics: BTreeMap<MetricKey, MetricReading>,
    pub history: BTreeMap<MetricKey, Vec<HistoryPoint>>,
    pub config: AppConfig,
    pub save_status: String,
    pub diagnostics: Diagnostics,
}

impl MonitorView {
    pub fn empty(config: AppConfig) -> Self {
        Self {
            sequence: 0,
            now: Instant::now(),
            elapsed: 0.0,
            devices: BTreeMap::new(),
            metrics: BTreeMap::new(),
            history: BTreeMap::new(),
            config,
            save_status: String::new(),
            diagnostics: Diagnostics::default(),
        }
    }
    pub fn reading(&self, device: &str, metric: &str) -> Option<&MetricReading> {
        self.metrics.get(&MetricKey::new(device, metric))
    }
    pub fn value(&self, device: &str, metric: &str) -> Option<f64> {
        self.reading(device, metric).and_then(|v| v.value)
    }
}
