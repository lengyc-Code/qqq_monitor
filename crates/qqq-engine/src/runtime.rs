use crate::{
    AppConfig, ConfigStore, LoadedConfig, MonitorView, Notice, ProcessTable, SaveStatus,
    history::{History, HistoryPoint},
    normalize::Normalizer,
    process::{self, ActionJob, NoProcesses, ProcessEvent},
};
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};
use qqq_domain::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

type UiWake = Arc<dyn Fn() + Send + Sync>;

#[derive(Debug)]
pub enum MonitorCommand {
    SetQuery {
        keys: Vec<MetricKey>,
        range_secs: u64,
    },
    SaveConfig {
        config: AppConfig,
        explicit: bool,
    },
    RefreshInventory,
    /// Lists the processes of a host until another host, or `None`, is watched.
    WatchProcesses(Option<ProcessHost>),
    /// Acts on a process; the result arrives once the platform has carried it out.
    Process {
        process: ProcessId,
        action: ProcessAction,
    },
}

#[derive(Debug)]
pub enum CommandOutcome {
    Applied,
    Saved,
    Superseded,
}
#[derive(Debug)]
pub struct CommandResult {
    pub id: u64,
    pub result: Result<CommandOutcome, Notice>,
}
struct Request {
    id: u64,
    command: MonitorCommand,
}
struct SaveJob {
    id: u64,
    config: AppConfig,
    explicit: bool,
}
struct CollectorEvent {
    source: &'static str,
    period: Duration,
    result: Result<SampleBatch, Reason>,
    dropped: u64,
}

pub(crate) struct Control {
    pub(crate) stop: AtomicBool,
    engine_done: AtomicBool,
    pub(crate) sampling_ms: AtomicU64,
    refresh: AtomicU64,
    /// Host whose processes are polled.
    pub(crate) process_watch: Mutex<Option<ProcessHost>>,
    /// Bumped to poll the watched host at once.
    pub(crate) process_refresh: AtomicU64,
    sleep_lock: Mutex<()>,
    pub(crate) sleep: Condvar,
    wake: Mutex<Option<UiWake>>,
}
impl Control {
    pub(crate) fn notify_ui(&self) {
        let wake = self.wake.lock().unwrap().clone();
        if let Some(wake) = wake {
            wake();
        }
    }
    pub(crate) fn wait(&self, duration: Duration) {
        let lock = self.sleep_lock.lock().unwrap();
        if !self.stop.load(Ordering::Acquire) {
            let _ = self
                .sleep
                .wait_timeout(lock, duration.min(Duration::from_millis(100)))
                .unwrap();
        }
    }
    fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.sleep.notify_all();
    }
}

#[derive(Clone)]
pub struct MonitorClient {
    latest: Arc<Mutex<Arc<MonitorView>>>,
    requests: Sender<Request>,
    results: Receiver<CommandResult>,
    in_flight: Arc<AtomicUsize>,
    next_id: Arc<AtomicU64>,
    control: Arc<Control>,
}
impl MonitorClient {
    pub fn try_latest_view(&self) -> Option<Arc<MonitorView>> {
        self.latest.try_lock().ok().map(|v| Arc::clone(&v))
    }
    pub fn set_ui_wake(&self, wake: UiWake) {
        *self.control.wake.lock().unwrap() = Some(wake);
    }
    pub fn try_command(&self, command: MonitorCommand) -> Result<u64, Notice> {
        if self.control.stop.load(Ordering::Acquire) {
            return Err(Notice::Stopped);
        }
        self.in_flight
            .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < 32).then_some(n + 1)
            })
            .map_err(|_| Notice::Busy)?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if let Err(error) = self.requests.try_send(Request { id, command }) {
            self.in_flight.fetch_sub(1, Ordering::Release);
            return Err(match error {
                TrySendError::Full(_) => Notice::Busy,
                TrySendError::Disconnected(_) => Notice::Stopped,
            });
        }
        Ok(id)
    }
    pub fn take_results(&self) -> Vec<CommandResult> {
        let values: Vec<_> = self.results.try_iter().collect();
        self.in_flight.fetch_sub(values.len(), Ordering::Release);
        values
    }
    pub fn request_shutdown(&self) {
        self.control.stop();
    }
}

pub struct MonitorRuntime {
    client: MonitorClient,
    threads: Vec<JoinHandle<()>>,
}
impl MonitorRuntime {
    /// Starts the engine without process access.
    pub fn start(
        factories: Vec<Arc<dyn CollectorFactory>>,
        loaded: LoadedConfig,
    ) -> Result<Self, Notice> {
        Self::start_with_processes(factories, Arc::new(NoProcesses), loaded)
    }
    pub fn start_with_processes(
        factories: Vec<Arc<dyn CollectorFactory>>,
        processes: Arc<dyn ProcessPlatform>,
        loaded: LoadedConfig,
    ) -> Result<Self, Notice> {
        loaded.config.validate()?;
        let (requests_tx, requests_rx) = bounded(32);
        let (results_tx, results_rx) = bounded(32);
        let control = Arc::new(Control {
            stop: AtomicBool::new(false),
            engine_done: AtomicBool::new(true),
            sampling_ms: AtomicU64::new(loaded.config.sampling_ms),
            refresh: AtomicU64::new(0),
            process_watch: Mutex::new(None),
            process_refresh: AtomicU64::new(0),
            sleep_lock: Mutex::new(()),
            sleep: Condvar::new(),
            wake: Mutex::new(None),
        });
        let latest = Arc::new(Mutex::new(Arc::new(MonitorView::empty(
            loaded.config.clone(),
        ))));
        let client = MonitorClient {
            latest,
            requests: requests_tx,
            results: results_rx,
            in_flight: Arc::new(AtomicUsize::new(0)),
            next_id: Arc::new(AtomicU64::new(1)),
            control: Arc::clone(&control),
        };
        let mut runtime = Self {
            client,
            threads: Vec::new(),
        };
        let mut inputs = Vec::new();
        for factory in factories {
            let (tx, rx) = bounded(8);
            inputs.push(rx);
            let signal = Arc::clone(&control);
            let handle = thread::Builder::new()
                .name(factory.descriptor().id.into())
                .spawn(move || collector_worker(factory, tx, signal))
                .map_err(|e| Notice::System(e.to_string()))?;
            runtime.threads.push(handle);
        }
        let pending = Arc::new(Mutex::new(None));
        let (save_tx, save_rx) = bounded(32);
        let writer_pending = Arc::clone(&pending);
        let writer_control = Arc::clone(&control);
        runtime.threads.push(
            thread::Builder::new()
                .name("config-writer".into())
                .spawn(move || config_worker(loaded.store, writer_pending, save_tx, writer_control))
                .map_err(|e| Notice::System(e.to_string()))?,
        );
        let (process_tx, process_rx) = bounded(8);
        let poll_platform = Arc::clone(&processes);
        let poll_control = Arc::clone(&control);
        runtime.threads.push(
            thread::Builder::new()
                .name("process-poll".into())
                .spawn(move || process::poll_worker(poll_platform, process_tx, poll_control))
                .map_err(|e| Notice::System(e.to_string()))?,
        );
        let (action_tx, action_rx) = bounded(8);
        let action_results = results_tx.clone();
        let action_control = Arc::clone(&control);
        runtime.threads.push(
            thread::Builder::new()
                .name("process-actions".into())
                .spawn(move || {
                    process::action_worker(processes, action_rx, action_results, action_control)
                })
                .map_err(|e| Notice::System(e.to_string()))?,
        );
        let engine_client = runtime.client.clone();
        control.engine_done.store(false, Ordering::Release);
        match thread::Builder::new()
            .name("monitor-engine".into())
            .spawn(move || {
                engine_worker(
                    engine_client,
                    inputs,
                    requests_rx,
                    results_tx,
                    pending,
                    save_rx,
                    process_rx,
                    action_tx,
                    loaded.config,
                    loaded.warning,
                )
            }) {
            Ok(handle) => runtime.threads.push(handle),
            Err(error) => {
                control.engine_done.store(true, Ordering::Release);
                return Err(Notice::System(error.to_string()));
            }
        }
        Ok(runtime)
    }
    pub fn client(&self) -> MonitorClient {
        self.client.clone()
    }
    pub fn shutdown(&mut self) {
        self.client.control.stop();
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.threads.iter().any(|t| !t.is_finished()) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        for handle in self.threads.drain(..) {
            if handle.is_finished() {
                if handle.join().is_err() {
                    tracing::error!("后台线程退出异常");
                }
            } else {
                tracing::warn!("系统调用尚未返回，退出时由操作系统回收线程");
            }
        }
    }
}
impl Drop for MonitorRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn collector_worker(
    factory: Arc<dyn CollectorFactory>,
    tx: Sender<CollectorEvent>,
    control: Arc<Control>,
) {
    let descriptor = factory.descriptor();
    let mut collector = None;
    let mut dropped = 0_u64;
    let mut retry = 1_u64;
    let mut next = Instant::now();
    let mut last = None;
    let mut refresh = control.refresh.load(Ordering::Acquire);
    let mut old_period = Duration::ZERO;
    while !control.stop.load(Ordering::Acquire) {
        let period = match descriptor.period {
            SamplingPeriod::Configured => {
                Duration::from_millis(control.sampling_ms.load(Ordering::Acquire))
            }
            SamplingPeriod::Fixed(period) => period,
        };
        if old_period != period {
            if let Some(at) = last {
                next = at + period;
            }
            old_period = period;
        }
        let new_refresh = control.refresh.load(Ordering::Acquire);
        if new_refresh != refresh {
            refresh = new_refresh;
            if matches!(descriptor.period, SamplingPeriod::Fixed(_)) {
                next = Instant::now();
            }
        }
        if Instant::now() < next {
            control.wait(next.saturating_duration_since(Instant::now()));
            continue;
        }
        let now = Instant::now();
        last = Some(now);
        let result = catch_unwind(AssertUnwindSafe(|| {
            if collector.is_none() {
                collector = Some(factory.create()?);
            }
            collector
                .as_mut()
                .unwrap()
                .collect(&CollectContext { period })
        }));
        let outcome = match result {
            Ok(result) => result.map_err(|e| e.0),
            Err(_) => {
                collector = None;
                Err(Reason::Panicked)
            }
        };
        let failed = outcome.is_err();
        if failed {
            collector = None;
        }
        match tx.try_send(CollectorEvent {
            source: descriptor.id,
            period,
            result: outcome,
            dropped,
        }) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => dropped = dropped.saturating_add(1),
            Err(TrySendError::Disconnected(_)) => return,
        }
        let delay = if failed {
            let delay = Duration::from_secs(retry);
            retry = (retry * 2).min(30);
            delay
        } else {
            retry = 1;
            period
        };
        next += delay;
        if next <= Instant::now() {
            next = Instant::now() + delay;
        }
    }
}

fn config_worker(
    mut store: ConfigStore,
    pending: Arc<Mutex<Option<SaveJob>>>,
    tx: Sender<CommandResult>,
    control: Arc<Control>,
) {
    loop {
        let job = pending.lock().unwrap().take();
        if let Some(job) = job {
            let result = store
                .save(&job.config, job.explicit)
                .map(|()| CommandOutcome::Saved);
            let _ = tx.try_send(CommandResult { id: job.id, result });
            control.notify_ui();
        } else if control.stop.load(Ordering::Acquire)
            && control.engine_done.load(Ordering::Acquire)
        {
            break;
        } else {
            thread::sleep(Duration::from_millis(20));
        }
    }
}

struct EngineState {
    view: MonitorView,
    history: History,
    normalizer: Normalizer,
    metric_sources: BTreeMap<MetricKey, &'static str>,
    device_sources: BTreeMap<String, &'static str>,
    dropped: BTreeMap<&'static str, u64>,
    query: Vec<MetricKey>,
    range: u64,
    latest_save: Option<u64>,
    start: Instant,
    /// Host whose processes the GUI watches.
    watched: Option<ProcessHost>,
}
impl EngineState {
    fn receive_processes(&mut self, event: ProcessEvent) {
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
    fn receive(&mut self, event: CollectorEvent) {
        let old_dropped = self
            .dropped
            .insert(event.source, event.dropped)
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
                self.view
                    .diagnostics
                    .collector_errors
                    .insert(event.source.into(), error.clone());
                for (key, reading) in &mut self.view.metrics {
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
                self.view.diagnostics.collector_errors.remove(event.source);
                if batch.complete_inventory {
                    let present: BTreeSet<_> =
                        batch.devices.iter().map(|d| d.id.as_str()).collect();
                    for (id, device) in &mut self.view.devices {
                        if self.device_sources.get(id) == Some(&event.source)
                            && !present.contains(id.as_str())
                        {
                            device.online = false;
                        }
                    }
                }
                for device in batch.devices {
                    self.device_sources.insert(device.id.clone(), event.source);
                    self.view.devices.insert(device.id.clone(), device);
                }
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
                            .insert(event.source.into(), Reason::KeyConflict);
                        continue;
                    }
                    self.metric_sources.insert(key.clone(), event.source);
                    let reading =
                        self.view
                            .metrics
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
                    reading.period = event.period;
                    let elapsed = sample
                        .at
                        .saturating_duration_since(self.start)
                        .as_secs_f64();
                    self.normalizer.apply(reading, sample);
                    self.history.append(
                        key,
                        HistoryPoint {
                            elapsed,
                            value: (reading.state == SampleState::Ready)
                                .then_some(reading.value)
                                .flatten(),
                        },
                    );
                }
                for (key, reading) in &mut self.view.metrics {
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
    }
    fn publish(&mut self, client: &MonitorClient) {
        self.view.now = Instant::now();
        self.view.elapsed = self.start.elapsed().as_secs_f64();
        self.view.sequence += 1;
        self.history.prune(self.view.elapsed);
        self.view.history = self
            .query
            .iter()
            .map(|key| {
                (
                    key.clone(),
                    self.history
                        .project(key, self.view.elapsed - self.range as f64, 400),
                )
            })
            .collect();
        self.view.diagnostics.history_bytes = self.history.bytes();
        let obsolete: Vec<_> = self
            .view
            .devices
            .iter()
            .filter(|(id, d)| !d.online && !self.history.series.keys().any(|k| k.device == **id))
            .map(|(id, _)| id.clone())
            .collect();
        for id in obsolete {
            self.view.devices.remove(&id);
            self.device_sources.remove(&id);
            self.view.metrics.retain(|k, _| k.device != id);
            self.metric_sources.retain(|k, _| k.device != id);
        }
        let snapshot = Arc::new(self.view.clone());
        *client.latest.lock().unwrap() = snapshot;
        client.control.notify_ui();
    }
}

#[allow(clippy::too_many_arguments)]
fn engine_worker(
    client: MonitorClient,
    inputs: Vec<Receiver<CollectorEvent>>,
    requests: Receiver<Request>,
    results: Sender<CommandResult>,
    pending: Arc<Mutex<Option<SaveJob>>>,
    saves: Receiver<CommandResult>,
    process_events: Receiver<ProcessEvent>,
    actions: Sender<ActionJob>,
    config: AppConfig,
    warning: Option<Notice>,
) {
    let range = config.history_secs;
    let mut view = MonitorView::empty(config);
    view.save_status = warning.map_or(SaveStatus::Loaded, SaveStatus::Recovered);
    let mut state = EngineState {
        view,
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
    };
    let mut last_publish = Instant::now();
    while !client.control.stop.load(Ordering::Acquire) || !requests.is_empty() {
        let mut dirty = false;
        for input in &inputs {
            for event in input.try_iter().take(8) {
                state.receive(event);
                dirty = true;
            }
        }
        for event in process_events.try_iter().take(8) {
            state.receive_processes(event);
            dirty = true;
        }
        for saved in saves.try_iter() {
            if state.latest_save == Some(saved.id) {
                state.view.save_status = match &saved.result {
                    Ok(_) => SaveStatus::Saved,
                    Err(e) => SaveStatus::Unsaved(e.clone()),
                };
            }
            let _ = results.try_send(saved);
            dirty = true;
        }
        for request in requests.try_iter().take(32) {
            let result = match request.command {
                MonitorCommand::SetQuery { keys, range_secs } => {
                    if keys.len() > 16 || ![60, 300, 900].contains(&range_secs) {
                        Err(Notice::InvalidQuery)
                    } else {
                        state.query = keys;
                        state.range = range_secs;
                        Ok(CommandOutcome::Applied)
                    }
                }
                MonitorCommand::WatchProcesses(host) => {
                    if state.watched != host {
                        state.view.processes.table = None;
                        state.view.processes.error = None;
                    }
                    state.watched.clone_from(&host);
                    *client.control.process_watch.lock().unwrap() = host;
                    client
                        .control
                        .process_refresh
                        .fetch_add(1, Ordering::Release);
                    client.control.sleep.notify_all();
                    Ok(CommandOutcome::Applied)
                }
                MonitorCommand::Process { process, action } => match actions.try_send(ActionJob {
                    id: request.id,
                    process,
                    action,
                }) {
                    // The action thread answers once the platform has acted.
                    Ok(()) => continue,
                    Err(TrySendError::Full(_)) => Err(Notice::Busy),
                    Err(TrySendError::Disconnected(_)) => Err(Notice::Stopped),
                },
                MonitorCommand::RefreshInventory => {
                    client.control.refresh.fetch_add(1, Ordering::Release);
                    client.control.sleep.notify_all();
                    Ok(CommandOutcome::Applied)
                }
                MonitorCommand::SaveConfig { config, explicit } => match config.validate() {
                    Err(error) => Err(error),
                    Ok(()) => {
                        client
                            .control
                            .sampling_ms
                            .store(config.sampling_ms, Ordering::Release);
                        client.control.sleep.notify_all();
                        state.view.config = config.clone();
                        state.latest_save = Some(request.id);
                        state.view.save_status = SaveStatus::Saving;
                        let mut slot = pending.lock().unwrap();
                        let mut job = SaveJob {
                            id: request.id,
                            config,
                            explicit,
                        };
                        if let Some(previous) = slot.take() {
                            job.explicit |= previous.explicit;
                            let _ = results.try_send(CommandResult {
                                id: previous.id,
                                result: Ok(CommandOutcome::Superseded),
                            });
                        }
                        *slot = Some(job);
                        dirty = true;
                        continue;
                    }
                },
            };
            let _ = results.try_send(CommandResult {
                id: request.id,
                result,
            });
            dirty = true;
        }
        if dirty || last_publish.elapsed() >= Duration::from_secs(1) {
            state.publish(&client);
            last_publish = Instant::now();
        }
        client.control.wait(Duration::from_millis(25));
    }
    client.control.engine_done.store(true, Ordering::Release);
}
