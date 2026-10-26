use crate::{
    AppConfig, ConfigStore, LoadedConfig, MonitorView, Notice, SaveStatus,
    alert::{self, AlertNotify, Alerts},
    delivery::deliver,
    process::{self, ActionJob, NoProcesses, ProcessEvent},
    remote::{NoRemote, Remotes, Secret},
};
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};
use qqq_domain::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

mod state;
use state::EngineState;

type UiWake = Arc<dyn Fn() + Send + Sync>;

/// Prefix of the source of a remote host's readings, followed by the host ID.
const REMOTE_SOURCE: &str = "ssh:";
/// Longest time between two views, so that data ages and alert states move on while no
/// reading arrives.
const IDLE_PUBLISH: Duration = Duration::from_secs(1);
/// Changes that arrive this soon after the first one not yet shown reach the GUI in one
/// view, such as the batches the collectors take in one sampling round.
const COALESCE: Duration = Duration::from_millis(40);

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
    /// Saves, or with `None` removes, the password or key passphrase of a remote host,
    /// then reconnects the host.
    RemoteSecret {
        host: String,
        secret: Option<Secret>,
    },
    /// Connects a remote host again at once.
    Reconnect(String),
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
/// Configuration waiting for the writer thread; a newer one replaces it. The writer wakes
/// for each, and finishes once the engine drops this slot.
struct PendingSave {
    job: Arc<Mutex<Option<SaveJob>>>,
    ready: Sender<()>,
}
struct CollectorEvent {
    source: Arc<str>,
    period: Duration,
    result: Result<SampleBatch, Reason>,
    dropped: u64,
}

pub(crate) struct Control {
    pub(crate) stop: AtomicBool,
    pub(crate) sampling_ms: AtomicU64,
    /// The configuration switches frame monitoring on.
    frames: AtomicBool,
    refresh: AtomicU64,
    /// Host whose processes are polled.
    pub(crate) process_watch: Mutex<Option<ProcessHost>>,
    /// Bumped to poll the watched host at once.
    pub(crate) process_refresh: AtomicU64,
    /// Bumped to list the process hosts at once.
    pub(crate) hosts_refresh: AtomicU64,
    /// Number of changes the workers have been told about; waiting workers wake when it
    /// grows.
    changes: Mutex<u64>,
    signal: Condvar,
    /// Rung when the engine has something to take; holds one ring at most.
    bell: Sender<()>,
    rings: Receiver<()>,
    wake: Mutex<Option<UiWake>>,
}
impl Control {
    pub(crate) fn new(config: &AppConfig) -> Self {
        let (bell, rings) = bounded(1);
        Self {
            stop: AtomicBool::new(false),
            sampling_ms: AtomicU64::new(config.sampling_ms),
            frames: AtomicBool::new(config.frame_monitoring),
            refresh: AtomicU64::new(0),
            process_watch: Mutex::new(None),
            process_refresh: AtomicU64::new(0),
            hosts_refresh: AtomicU64::new(0),
            changes: Mutex::new(0),
            signal: Condvar::new(),
            bell,
            rings,
            wake: Mutex::new(None),
        }
    }
    pub(crate) fn notify_ui(&self) {
        let wake = self.wake.lock().unwrap().clone();
        if let Some(wake) = wake {
            wake();
        }
    }
    /// Number of changes so far. A worker reads it before the settings it acts on and
    /// passes it to [`Self::wait`], so that a change in between cuts the wait short.
    pub(crate) fn changes(&self) -> u64 {
        *self.changes.lock().unwrap()
    }
    /// Tells the waiting workers that something they follow changed, such as the sampling
    /// interval, the watched processes or a reconnect.
    pub(crate) fn changed(&self) {
        *self.changes.lock().unwrap() += 1;
        self.signal.notify_all();
    }
    /// Waits up to `duration`, or until a change after `seen` or the stop.
    pub(crate) fn wait(&self, seen: u64, duration: Duration) {
        let changes = self.changes.lock().unwrap();
        let _ = self
            .signal
            .wait_timeout_while(changes, duration, |changes| {
                *changes == seen && !self.stop.load(Ordering::Acquire)
            })
            .unwrap();
    }
    /// Wakes the engine thread to take a batch, an update, a command or a save result.
    pub(crate) fn wake_engine(&self) {
        let _ = self.bell.try_send(());
    }
    #[cfg(test)]
    pub(crate) fn wait_for_engine_wake(&self) -> bool {
        self.rings.recv_timeout(Duration::from_secs(2)).is_ok()
    }
    fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.changed();
        self.wake_engine();
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
        self.control.wake_engine();
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

/// Platform services the engine uses besides its collectors.
pub struct Services {
    pub processes: Arc<dyn ProcessPlatform>,
    pub remote: Arc<dyn RemotePlatform>,
    /// Frames of the application in the foreground, collected while the configuration
    /// switches frame monitoring on.
    pub frames: Option<Arc<dyn CollectorFactory>>,
    /// Shows the notification of a fired alert, on a thread of its own.
    pub notify: AlertNotify,
}

impl Default for Services {
    /// No process access, no remote hosts, no frames and no notifications.
    fn default() -> Self {
        Self {
            processes: Arc::new(NoProcesses),
            remote: Arc::new(NoRemote),
            frames: None,
            notify: Arc::new(|_| {}),
        }
    }
}

pub struct MonitorRuntime {
    client: MonitorClient,
    threads: Vec<JoinHandle<()>>,
    /// Remote host workers, which the engine thread starts and stops.
    remote_threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
}
impl MonitorRuntime {
    /// Starts the engine without process access.
    pub fn start(
        factories: Vec<Arc<dyn CollectorFactory>>,
        loaded: LoadedConfig,
    ) -> Result<Self, Notice> {
        Self::start_with_processes(factories, Arc::new(NoProcesses), loaded)
    }
    /// Starts the engine without remote access.
    pub fn start_with_processes(
        factories: Vec<Arc<dyn CollectorFactory>>,
        processes: Arc<dyn ProcessPlatform>,
        loaded: LoadedConfig,
    ) -> Result<Self, Notice> {
        Self::start_with_remote(factories, processes, Arc::new(NoRemote), loaded)
    }
    /// Starts the engine without frames and notifications.
    pub fn start_with_remote(
        factories: Vec<Arc<dyn CollectorFactory>>,
        processes: Arc<dyn ProcessPlatform>,
        remote: Arc<dyn RemotePlatform>,
        loaded: LoadedConfig,
    ) -> Result<Self, Notice> {
        Self::start_with_services(
            factories,
            Services {
                processes,
                remote,
                ..Services::default()
            },
            loaded,
        )
    }
    pub fn start_with_services(
        factories: Vec<Arc<dyn CollectorFactory>>,
        services: Services,
        loaded: LoadedConfig,
    ) -> Result<Self, Notice> {
        let Services {
            processes,
            remote,
            frames,
            notify,
        } = services;
        loaded.config.validate()?;
        let (requests_tx, requests_rx) = bounded(32);
        let (results_tx, results_rx) = bounded(32);
        let control = Arc::new(Control::new(&loaded.config));
        let latest = Arc::new(Mutex::new(Arc::new(MonitorView::empty())));
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
            remote_threads: Arc::new(Mutex::new(Vec::new())),
        };
        let mut inputs = Vec::new();
        let gated = frames.map(|factory| (factory, true));
        for (factory, gated) in factories
            .into_iter()
            .map(|factory| (factory, false))
            .chain(gated)
        {
            let (tx, rx) = bounded(8);
            inputs.push(rx);
            let signal = Arc::clone(&control);
            let handle = thread::Builder::new()
                .name(factory.descriptor().id.into())
                .spawn(move || collector_worker(factory, tx, signal, gated))
                .map_err(|e| Notice::System(e.to_string()))?;
            runtime.threads.push(handle);
        }
        let (alert_tx, alert_rx) = bounded(8);
        let notify_control = Arc::clone(&control);
        runtime.threads.push(
            thread::Builder::new()
                .name("alert-notify".into())
                .spawn(move || alert::notify_worker(notify, alert_rx, notify_control))
                .map_err(|e| Notice::System(e.to_string()))?,
        );
        let job = Arc::new(Mutex::new(None));
        let (ready_tx, ready_rx) = bounded(1);
        let pending = PendingSave {
            job: Arc::clone(&job),
            ready: ready_tx,
        };
        let (save_tx, save_rx) = bounded(32);
        let writer_control = Arc::clone(&control);
        runtime.threads.push(
            thread::Builder::new()
                .name("config-writer".into())
                .spawn(move || config_worker(loaded.store, job, ready_rx, save_tx, writer_control))
                .map_err(|e| Notice::System(e.to_string()))?,
        );
        let (process_tx, process_rx) = bounded(8);
        let hosts_platform = Arc::clone(&processes);
        let hosts_tx = process_tx.clone();
        let hosts_control = Arc::clone(&control);
        runtime.threads.push(
            thread::Builder::new()
                .name("process-hosts".into())
                .spawn(move || process::hosts_worker(hosts_platform, hosts_tx, hosts_control))
                .map_err(|e| Notice::System(e.to_string()))?,
        );
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
        let remotes = Remotes::new(
            remote,
            Arc::clone(&control),
            Arc::clone(&runtime.remote_threads),
        );
        let engine_client = runtime.client.clone();
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
                    remotes,
                    Alerts::new(alert_tx),
                    loaded.config,
                    loaded.warning,
                )
            }) {
            Ok(handle) => runtime.threads.push(handle),
            Err(error) => return Err(Notice::System(error.to_string())),
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
        // The engine thread has stopped the remote workers by now, or starts no more.
        let remote: Vec<_> = self.remote_threads.lock().unwrap().drain(..).collect();
        while remote.iter().any(|t| !t.is_finished()) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        for handle in self.threads.drain(..).chain(remote) {
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

/// Collects at the source's period. A `gated` source collects only while frame monitoring
/// is on: switching it off drops the collector and takes the source's devices offline.
fn collector_worker(
    factory: Arc<dyn CollectorFactory>,
    tx: Sender<CollectorEvent>,
    control: Arc<Control>,
    gated: bool,
) {
    let descriptor = factory.descriptor();
    let source: Arc<str> = descriptor.id.into();
    let mut collector = None;
    let mut dropped = 0_u64;
    let mut retry = 1_u64;
    let mut next = Instant::now();
    let mut last = None;
    let mut refresh = control.refresh.load(Ordering::Acquire);
    let mut old_period = Duration::ZERO;
    let mut open = !gated;
    while !control.stop.load(Ordering::Acquire) {
        let seen = control.changes();
        let period = match descriptor.period {
            SamplingPeriod::Configured => {
                Duration::from_millis(control.sampling_ms.load(Ordering::Acquire))
            }
            SamplingPeriod::Fixed(period) => period,
        };
        if gated {
            let enabled = control.frames.load(Ordering::Acquire);
            if enabled != open {
                open = enabled;
                retry = 1;
                next = Instant::now();
                if !enabled {
                    collector = None;
                    // An empty complete inventory takes the devices offline and clears the
                    // source's error.
                    let now = Instant::now();
                    let event = CollectorEvent {
                        source: Arc::clone(&source),
                        period,
                        result: Ok(SampleBatch {
                            complete_inventory: true,
                            started_at: now,
                            finished_at: now,
                            ..Default::default()
                        }),
                        dropped,
                    };
                    if !deliver(
                        &tx,
                        event,
                        || control.stop.load(Ordering::Acquire),
                        || control.wake_engine(),
                    ) {
                        return;
                    }
                }
            }
            if !open {
                control.wait(seen, Duration::MAX);
                continue;
            }
        }
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
            control.wait(seen, next.saturating_duration_since(Instant::now()));
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
            source: Arc::clone(&source),
            period,
            result: outcome,
            dropped,
        }) {
            Ok(()) => control.wake_engine(),
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

/// Writes each configuration the engine leaves in `job`. The engine rings `ready` for each
/// and drops its end on exit, after the last one, which is still written.
fn config_worker(
    mut store: ConfigStore,
    job: Arc<Mutex<Option<SaveJob>>>,
    ready: Receiver<()>,
    tx: Sender<CommandResult>,
    control: Arc<Control>,
) {
    loop {
        let open = ready.recv().is_ok();
        let job = job.lock().unwrap().take();
        if let Some(job) = job {
            let result = store
                .save(&job.config, job.explicit)
                .map(|()| CommandOutcome::Saved);
            let _ = tx.try_send(CommandResult { id: job.id, result });
            control.wake_engine();
        }
        if !open {
            break;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn engine_worker(
    client: MonitorClient,
    inputs: Vec<Receiver<CollectorEvent>>,
    requests: Receiver<Request>,
    results: Sender<CommandResult>,
    pending: PendingSave,
    saves: Receiver<CommandResult>,
    process_events: Receiver<ProcessEvent>,
    actions: Sender<ActionJob>,
    remotes: Remotes,
    alerts: Alerts,
    config: AppConfig,
    warning: Option<Notice>,
) {
    let mut state = EngineState::new(config, warning, remotes, alerts);
    state.view.remote.presets = Arc::new(state.remotes.platform.presets());
    state.apply_remote_hosts();
    let mut last_publish = Instant::now();
    // When the oldest change that no view shows yet arrived.
    let mut unpublished: Option<Instant> = None;
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
        for (host, update) in state.remotes.updates() {
            state.receive_remote(&host, update);
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
                    client.control.changed();
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
                MonitorCommand::RemoteSecret { host, secret } => {
                    let secret = secret.as_ref().map(|s| s.0.as_str());
                    let result = state.remotes.platform.set_secret(&host, secret);
                    state.remotes.reconnect(&host);
                    result
                        .map(|()| CommandOutcome::Applied)
                        .map_err(Notice::SecretUnsaved)
                }
                MonitorCommand::Reconnect(host) => {
                    state.remotes.reconnect(&host);
                    Ok(CommandOutcome::Applied)
                }
                MonitorCommand::RefreshInventory => {
                    client.control.refresh.fetch_add(1, Ordering::Release);
                    client.control.changed();
                    Ok(CommandOutcome::Applied)
                }
                MonitorCommand::SaveConfig { config, explicit } => match config.validate() {
                    Err(error) => Err(error),
                    Ok(()) => {
                        client
                            .control
                            .sampling_ms
                            .store(config.sampling_ms, Ordering::Release);
                        client
                            .control
                            .frames
                            .store(config.frame_monitoring, Ordering::Release);
                        client.control.changed();
                        state.config = config.clone();
                        state.apply_remote_hosts();
                        state.evaluate_alerts();
                        state.latest_save = Some(request.id);
                        state.view.save_status = SaveStatus::Saving;
                        let mut slot = pending.job.lock().unwrap();
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
                        let _ = pending.ready.try_send(());
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
        if dirty {
            unpublished.get_or_insert_with(Instant::now);
        }
        if unpublished.is_some_and(|at| at.elapsed() >= COALESCE)
            || last_publish.elapsed() >= IDLE_PUBLISH
        {
            state.evaluate_alerts();
            state.publish(&client);
            last_publish = Instant::now();
            unpublished = None;
        }
        // What one round left behind is taken at once; anything arriving later rings.
        let waiting = !requests.is_empty()
            || !process_events.is_empty()
            || inputs.iter().any(|input| !input.is_empty())
            || state.remotes.waiting();
        if !waiting {
            let next = unpublished.map_or(last_publish + IDLE_PUBLISH, |at| at + COALESCE);
            let _ = client
                .control
                .rings
                .recv_timeout(next.saturating_duration_since(Instant::now()));
        }
    }
    state.remotes.stop();
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FramesFactory {
        created: Sender<()>,
        dropped: Sender<()>,
    }
    struct FramesCollector(Sender<()>);
    impl Collector for FramesCollector {
        fn collect(&mut self, _: &CollectContext) -> Result<SampleBatch, CollectorError> {
            Ok(SampleBatch::default())
        }
    }
    impl Drop for FramesCollector {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }
    impl CollectorFactory for FramesFactory {
        fn descriptor(&self) -> CollectorDescriptor {
            CollectorDescriptor {
                id: "frames",
                period: SamplingPeriod::Configured,
            }
        }
        fn create(&self) -> Result<Box<dyn Collector>, CollectorError> {
            self.created.send(()).unwrap();
            Ok(Box::new(FramesCollector(self.dropped.clone())))
        }
    }

    #[test]
    fn switching_frames_off_delivers_the_offline_inventory_after_queue_congestion() {
        let config = AppConfig {
            frame_monitoring: true,
            ..AppConfig::default()
        };
        let control = Arc::new(Control::new(&config));
        let (sender, received) = bounded(1);
        sender
            .send(CollectorEvent {
                source: "older".into(),
                period: Duration::from_secs(1),
                result: Ok(SampleBatch::default()),
                dropped: 0,
            })
            .unwrap();
        let (created, ready) = bounded(1);
        let (dropped, released) = bounded(1);
        let handle = thread::spawn({
            let control = control.clone();
            move || {
                collector_worker(
                    Arc::new(FramesFactory { created, dropped }),
                    sender,
                    control,
                    true,
                )
            }
        });
        ready.recv_timeout(Duration::from_secs(2)).unwrap();
        control.frames.store(false, Ordering::Release);
        control.changed();
        released.recv_timeout(Duration::from_secs(2)).unwrap();
        let waiting_for_space = control.wait_for_engine_wake();
        // The marker predates the shutdown; the offline transition must follow it.
        assert_eq!(&*received.recv().unwrap().source, "older");
        let transition = received.recv_timeout(Duration::from_secs(2));
        control.stop();
        handle.join().unwrap();
        assert!(waiting_for_space);
        let transition = transition.unwrap().result.unwrap();
        assert!(transition.complete_inventory && transition.devices.is_empty());
    }
}
