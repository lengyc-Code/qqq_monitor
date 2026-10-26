//! Mutable monitoring state, normalization, history and immutable view publication.

use super::{CollectorEvent, MonitorClient, REMOTE_SOURCE};
use crate::{
    AppConfig, MonitorView, Notice, ProcessTable, SaveStatus,
    alert::Alerts,
    history::{History, HistoryPoint},
    normalize::Normalizer,
    process::ProcessEvent,
    remote::{RemoteState, RemoteStatus, RemoteUpdate, Remotes},
};
use qqq_domain::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) struct EngineState {
    pub(super) view: MonitorView,
    /// Configuration in effect.
    pub(super) config: AppConfig,
    history: History,
    normalizer: Normalizer,
    metric_sources: BTreeMap<MetricKey, Arc<str>>,
    device_sources: BTreeMap<String, Arc<str>>,
    dropped: BTreeMap<Arc<str>, u64>,
    pub(super) query: Vec<MetricKey>,
    pub(super) range: u64,
    pub(super) latest_save: Option<u64>,
    start: Instant,
    /// Host whose processes the GUI watches.
    pub(super) watched: Option<ProcessHost>,
    pub(super) remotes: Remotes,
    alerts: Alerts,
}
impl EngineState {
    pub(super) fn new(
        config: AppConfig,
        warning: Option<Notice>,
        remotes: Remotes,
        alerts: Alerts,
    ) -> Self {
        let range = config.history_secs;
        let mut view = MonitorView::empty();
        view.save_status = warning.map_or(SaveStatus::Loaded, SaveStatus::Recovered);
        Self {
            view,
            config,
            history: History::default(),
            normalizer: Normalizer::default(),
            metric_sources: BTreeMap::new(),
            device_sources: BTreeMap::new(),
            dropped: BTreeMap::new(),
            query: Vec::new(),
            range,
            latest_save: None,
            start: Instant::now(),
            watched: None,
            remotes,
            alerts,
        }
    }
    fn set_remote_state(&mut self, host: &str, state: RemoteState) {
        let hosts = &mut self.view.remote.hosts;
        if hosts.get(host).is_none_or(|status| status.state != state) {
            hosts.insert(
                host.into(),
                RemoteStatus {
                    state,
                    since: Instant::now(),
                },
            );
        }
    }
    /// Takes an update of a remote host's worker. Batches pass through the same checks,
    /// rates and history as this computer's, under the host's device prefix.
    pub(super) fn receive_remote(&mut self, host: &str, update: RemoteUpdate) {
        let source: Arc<str> = format!("{REMOTE_SOURCE}{host}").into();
        match update {
            RemoteUpdate::Connecting => {
                // Keeps the reason visible while a retry connects.
                if !matches!(
                    self.view.remote.hosts.get(host).map(|s| &s.state),
                    Some(RemoteState::Failed(_))
                ) {
                    self.set_remote_state(host, RemoteState::Connecting);
                }
            }
            RemoteUpdate::Batch {
                mut batch,
                period,
                dropped,
            } => {
                self.set_remote_state(host, RemoteState::Connected);
                for device in &mut batch.devices {
                    device.id = remote_device(host, &device.id);
                    device.host = Some(host.into());
                }
                for sample in &mut batch.samples {
                    let key = &mut sample.descriptor.key;
                    key.device = remote_device(host, &key.device);
                }
                self.receive(CollectorEvent {
                    source,
                    period,
                    result: Ok(batch),
                    dropped,
                });
            }
            RemoteUpdate::Failed(reason) => {
                self.set_remote_state(host, RemoteState::Failed(reason.clone()));
                let dropped = self.dropped.get(&source).copied().unwrap_or(0);
                self.receive(CollectorEvent {
                    source,
                    period: Duration::ZERO,
                    result: Err(reason),
                    dropped,
                });
            }
        }
    }
    /// Matches the remote workers to the configured hosts and drops the devices, readings
    /// and history that no longer apply.
    pub(super) fn apply_remote_hosts(&mut self) {
        let hosts = &self.config.remote_hosts;
        for id in self.remotes.apply(hosts) {
            let prefix = remote_prefix(&id);
            let ours = |device: &str| device.starts_with(&prefix);
            if self.view.devices.keys().any(|id| ours(id)) {
                Arc::make_mut(&mut self.view.devices).retain(|id, _| !ours(id));
            }
            self.device_sources.retain(|id, _| !ours(id));
            Arc::make_mut(&mut self.view.metrics).retain(|key, _| !ours(&key.device));
            self.metric_sources.retain(|key, _| !ours(&key.device));
            self.history.series.retain(|key, _| !ours(&key.device));
            self.normalizer.retain(|key| !ours(&key.device));
            self.dropped.remove(format!("{REMOTE_SOURCE}{id}").as_str());
            self.view.remote.hosts.remove(&id);
        }
        let hosts = &self.config.remote_hosts;
        self.view
            .remote
            .hosts
            .retain(|id, _| hosts.iter().any(|h| h.id == *id));
        for host in hosts {
            if !self.view.remote.hosts.contains_key(&host.id) {
                self.view.remote.hosts.insert(
                    host.id.clone(),
                    RemoteStatus {
                        state: RemoteState::Connecting,
                        since: Instant::now(),
                    },
                );
            }
        }
    }
    pub(super) fn receive_processes(&mut self, event: ProcessEvent) {
        let processes = &mut self.view.processes;
        match event {
            ProcessEvent::Hosts(hosts) => processes.hosts = hosts,
            ProcessEvent::Table(host, result) if self.watched.as_ref() == Some(&host) => {
                match result {
                    Ok(rows) => {
                        processes.table = Some(Arc::new(ProcessTable { host, rows }));
                        processes.error = None;
                    }
                    Err(reason) => processes.error = Some(reason),
                }
            }
            ProcessEvent::Table(..) => {}
        }
    }
    pub(super) fn receive(&mut self, event: CollectorEvent) {
        let old_dropped = self
            .dropped
            .insert(Arc::clone(&event.source), event.dropped)
            .unwrap_or(0);
        if event.dropped > old_dropped {
            self.view.diagnostics.dropped_batches += event.dropped - old_dropped;
            let keys: Vec<_> = self
                .metric_sources
                .iter()
                .filter(|(_, s)| **s == event.source)
                .map(|(k, _)| k.clone())
                .collect();
            let gap_at = event
                .result
                .as_ref()
                .map(|batch| batch.started_at)
                .unwrap_or_else(|_| Instant::now());
            for key in keys {
                self.alerts.interrupt(&key);
                self.normalizer.reset(&key);
                self.history.append(
                    key,
                    HistoryPoint {
                        elapsed: (gap_at.saturating_duration_since(self.start).as_secs_f64()
                            - 0.000001)
                            .max(0.0),
                        value: None,
                    },
                );
            }
        }
        match event.result {
            Err(error) => {
                // A remote host reports its failure in its own status, not as a collector
                // of this computer.
                if !event.source.starts_with(REMOTE_SOURCE) {
                    self.view
                        .diagnostics
                        .collector_errors
                        .insert(event.source.to_string(), error.clone());
                }
                for (key, reading) in Arc::make_mut(&mut self.view.metrics) {
                    if self.metric_sources.get(key) == Some(&event.source) {
                        reading.state = SampleState::Failed;
                        reading.reason = Some(error.clone());
                        self.normalizer.reset(key);
                        self.history.append(
                            key.clone(),
                            HistoryPoint {
                                elapsed: self.start.elapsed().as_secs_f64(),
                                value: None,
                            },
                        );
                    }
                }
            }
            Ok(batch) => {
                self.view.diagnostics.received_batches += 1;
                self.view
                    .diagnostics
                    .collector_errors
                    .remove(&*event.source);
                if batch.complete_inventory {
                    let present: BTreeSet<_> =
                        batch.devices.iter().map(|d| d.id.as_str()).collect();
                    let absent: Vec<_> = self
                        .view
                        .devices
                        .iter()
                        .filter(|(id, device)| {
                            device.online
                                && self.device_sources.get(*id) == Some(&event.source)
                                && !present.contains(id.as_str())
                        })
                        .map(|(id, _)| id.clone())
                        .collect();
                    if !absent.is_empty() {
                        let devices = Arc::make_mut(&mut self.view.devices);
                        for id in absent {
                            if let Some(device) = devices.get_mut(&id) {
                                device.online = false;
                            }
                        }
                    }
                }
                for device in batch.devices {
                    self.device_sources
                        .insert(device.id.clone(), Arc::clone(&event.source));
                    // The map is copied only for a change, so views keep sharing it.
                    if self.view.devices.get(&device.id) != Some(&device) {
                        Arc::make_mut(&mut self.view.devices).insert(device.id.clone(), device);
                    }
                }
                self.view.now = Instant::now();
                self.alerts.prepare(&self.view, &self.config);
                for sample in batch.samples {
                    let key = sample.descriptor.key.clone();
                    if self
                        .metric_sources
                        .get(&key)
                        .is_some_and(|source| *source != event.source)
                    {
                        self.view
                            .diagnostics
                            .collector_errors
                            .insert(event.source.to_string(), Reason::KeyConflict);
                        continue;
                    }
                    self.metric_sources
                        .insert(key.clone(), Arc::clone(&event.source));
                    let reading = Arc::make_mut(&mut self.view.metrics)
                        .entry(key.clone())
                        .or_insert_with(|| MetricReading {
                            descriptor: sample.descriptor.clone(),
                            value: None,
                            session_bytes: 0,
                            state: SampleState::WarmingUp,
                            reason: None,
                            last_success: None,
                            last_attempt: sample.at,
                            period: event.period,
                        });
                    if reading.descriptor.unit != sample.descriptor.unit
                        || sample.at < reading.last_attempt
                    {
                        continue;
                    }
                    let interrupted = reading.effective_state(sample.at) != SampleState::Ready;
                    reading.period = event.period;
                    let elapsed = sample
                        .at
                        .saturating_duration_since(self.start)
                        .as_secs_f64();
                    self.normalizer.apply(reading, sample);
                    let interrupted = interrupted || reading.state != SampleState::Ready;
                    self.history.append(
                        key.clone(),
                        HistoryPoint {
                            elapsed,
                            value: (reading.state == SampleState::Ready)
                                .then_some(reading.value)
                                .flatten(),
                        },
                    );
                    if interrupted {
                        self.alerts.interrupt(&key);
                    }
                    self.alerts
                        .evaluate_sample(&mut self.view, &self.config, &key);
                }
                for (key, reading) in Arc::make_mut(&mut self.view.metrics) {
                    if self
                        .view
                        .devices
                        .get(&key.device)
                        .is_some_and(|d| !d.online)
                        && reading.state != SampleState::Offline
                    {
                        reading.state = SampleState::Offline;
                        reading.reason = Some(Reason::Disconnected);
                        self.normalizer.reset(key);
                        self.history.append(
                            key.clone(),
                            HistoryPoint {
                                elapsed: self.start.elapsed().as_secs_f64(),
                                value: None,
                            },
                        );
                    }
                }
            }
        }
        self.evaluate_alerts();
    }
    pub(super) fn evaluate_alerts(&mut self) {
        self.view.now = Instant::now();
        self.alerts.evaluate(&mut self.view, &self.config);
    }
    pub(super) fn publish(&mut self, client: &MonitorClient) {
        self.view.now = Instant::now();
        self.view.elapsed = self.start.elapsed().as_secs_f64();
        self.view.sequence += 1;
        self.history.prune(self.view.elapsed);
        self.view.history = Arc::new(
            self.query
                .iter()
                .map(|key| {
                    (
                        key.clone(),
                        self.history
                            .project(key, self.view.elapsed - self.range as f64, 400),
                    )
                })
                .collect(),
        );
        self.view.diagnostics.history_bytes = self.history.bytes();
        let obsolete: Vec<_> = self
            .view
            .devices
            .iter()
            .filter(|(id, d)| !d.online && !self.history.series.keys().any(|k| k.device == **id))
            .map(|(id, _)| id.clone())
            .collect();
        for id in obsolete {
            Arc::make_mut(&mut self.view.devices).remove(&id);
            self.device_sources.remove(&id);
            Arc::make_mut(&mut self.view.metrics).retain(|k, _| k.device != id);
            self.metric_sources.retain(|k, _| k.device != id);
        }
        let snapshot = Arc::new(self.view.clone());
        *client.latest.lock().unwrap() = snapshot;
        client.control.notify_ui();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AlertCondition, AlertEvent, AlertRule, AlertState, remote::NoRemote, runtime::Control,
    };
    use crossbeam_channel::{Receiver, bounded};
    use std::sync::Mutex;

    fn state() -> (EngineState, Receiver<AlertEvent>) {
        let config = AppConfig {
            alerts: vec![AlertRule {
                id: "cpu".into(),
                enabled: true,
                device: "cpu".into(),
                metric: "usage".into(),
                condition: AlertCondition::Above,
                threshold: 90.0,
                hold_secs: 10,
            }],
            ..AppConfig::default()
        };
        let control = Arc::new(Control::new(&config));
        let remotes = Remotes::new(
            Arc::new(NoRemote),
            control,
            Arc::new(Mutex::new(Vec::new())),
        );
        let (sender, receiver) = bounded(8);
        let mut state = EngineState::new(config, None, remotes, Alerts::new(sender));
        state.start = Instant::now() - Duration::from_secs(60);
        (state, receiver)
    }

    fn batch(at: Instant, values: Vec<SampleValue>, dropped: u64) -> CollectorEvent {
        CollectorEvent {
            source: "cpu-memory".into(),
            period: Duration::from_secs(30),
            dropped,
            result: Ok(SampleBatch {
                devices: vec![DeviceDescriptor::new("cpu", "CPU", DeviceKind::Cpu)],
                samples: values
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| {
                        MetricSample::new(
                            "cpu",
                            "usage",
                            Unit::Percent,
                            value,
                            at + Duration::from_millis(index as u64),
                        )
                    })
                    .collect(),
                started_at: at,
                finished_at: at,
                complete_inventory: true,
            }),
        }
    }

    #[test]
    fn dropped_samples_restart_the_alert_hold_time_before_a_ready_gauge() {
        let (mut state, notified) = state();
        let at = Instant::now() - Duration::from_secs(30);
        state.receive(batch(at, vec![SampleValue::Gauge(95.0)], 0));
        let again = at + Duration::from_secs(12);
        state.receive(batch(again, vec![SampleValue::Gauge(95.0)], 1));
        assert_eq!(
            state.view.alerts.states["cpu"],
            AlertState::Pending { since: again }
        );
        assert!(notified.is_empty());
        let key = MetricKey::new("cpu", "usage");
        assert!(
            state
                .history
                .project(&key, 0.0, 400)
                .iter()
                .any(|p| p.value.is_none())
        );
        state.receive(batch(
            at + Duration::from_secs(21),
            vec![SampleValue::Gauge(95.0)],
            1,
        ));
        assert!(notified.is_empty());
        state.receive(batch(
            at + Duration::from_secs(22),
            vec![SampleValue::Gauge(95.0)],
            1,
        ));
        assert_eq!(notified.try_recv().unwrap().rule.id, "cpu");
    }

    #[test]
    fn a_failed_sample_followed_by_recovery_in_one_batch_restarts_the_hold() {
        let (mut state, notified) = state();
        let at = Instant::now() - Duration::from_secs(30);
        state.receive(batch(at, vec![SampleValue::Gauge(95.0)], 0));
        let again = at + Duration::from_secs(12);
        state.receive(batch(
            again,
            vec![
                SampleValue::Failed(Reason::Disconnected),
                SampleValue::Gauge(95.0),
            ],
            0,
        ));
        assert_eq!(
            state.view.alerts.states["cpu"],
            AlertState::Pending {
                since: again + Duration::from_millis(1),
            }
        );
        assert!(notified.is_empty());
    }

    #[test]
    fn a_normal_gauge_before_an_exceeded_gauge_in_one_batch_restarts_the_hold() {
        let (mut state, notified) = state();
        let at = Instant::now() - Duration::from_secs(30);
        state.receive(batch(at, vec![SampleValue::Gauge(95.0)], 0));
        let again = at + Duration::from_secs(12);
        state.receive(batch(
            again,
            vec![SampleValue::Gauge(80.0), SampleValue::Gauge(95.0)],
            0,
        ));
        assert_eq!(
            state.view.alerts.states["cpu"],
            AlertState::Pending {
                since: again + Duration::from_millis(1),
            }
        );
        assert!(notified.is_empty());
    }

    #[test]
    fn exceeding_the_hold_then_returning_to_normal_in_one_batch_still_notifies() {
        let (mut state, notified) = state();
        let at = Instant::now() - Duration::from_secs(30);
        state.receive(batch(at, vec![SampleValue::Gauge(95.0)], 0));
        let firing = at + Duration::from_secs(12);
        state.receive(batch(
            firing,
            vec![SampleValue::Gauge(96.0), SampleValue::Gauge(80.0)],
            0,
        ));
        let event = notified.try_recv().unwrap();
        assert_eq!((event.at, event.value), (firing, 96.0));
        assert_eq!(
            state.view.alerts.states["cpu"],
            AlertState::Firing { since: firing }
        );
        // The normal sample at the end of that batch starts the recovery hold.
        state.receive(batch(
            firing + Duration::from_secs(10) + Duration::from_millis(1),
            vec![SampleValue::Gauge(80.0)],
            0,
        ));
        assert_eq!(state.view.alerts.states["cpu"], AlertState::Normal);
        assert!(notified.is_empty());
    }

    #[test]
    fn coalesced_source_failure_and_offline_events_are_seen_before_recovery() {
        for offline in [false, true] {
            let (mut state, notified) = state();
            let at = Instant::now() - Duration::from_secs(30);
            state.receive(batch(at, vec![SampleValue::Gauge(95.0)], 0));
            let mut interruption = batch(at + Duration::from_secs(11), Vec::new(), 0);
            if offline {
                interruption.result.as_mut().unwrap().devices.clear();
            } else {
                interruption.result = Err(Reason::TimedOut);
            }
            state.receive(interruption);
            assert_eq!(state.view.alerts.states["cpu"], AlertState::NoData);
            let again = at + Duration::from_secs(12);
            state.receive(batch(again, vec![SampleValue::Gauge(95.0)], 0));
            assert_eq!(
                state.view.alerts.states["cpu"],
                AlertState::Pending { since: again }
            );
            assert!(notified.is_empty());
        }
    }

    #[test]
    fn receiving_a_batch_keeps_previously_published_readings_immutable() {
        let (mut state, _) = state();
        let at = Instant::now();
        state.receive(batch(at, vec![SampleValue::Gauge(10.0)], 0));
        let earlier = state.view.clone();
        assert!(Arc::ptr_eq(&earlier.metrics, &state.view.metrics));
        state.receive(batch(
            at + Duration::from_secs(1),
            vec![SampleValue::Gauge(20.0)],
            0,
        ));
        assert_eq!(earlier.value("cpu", "usage"), Some(10.0));
        assert_eq!(state.view.value("cpu", "usage"), Some(20.0));
        assert!(!Arc::ptr_eq(&earlier.metrics, &state.view.metrics));
    }
}
