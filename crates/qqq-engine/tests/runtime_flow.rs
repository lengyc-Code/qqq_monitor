use crossbeam_channel::{Receiver, Sender, bounded};
use qqq_domain::*;
use qqq_engine::*;
use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

static DIRECTORY: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "qqq-test-{}-{}",
            std::process::id(),
            DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn config(&self) -> PathBuf {
        self.0.join("config.toml")
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Factory {
    blocked: bool,
    entered: Sender<()>,
    release: Receiver<()>,
}
struct FakeCollector {
    id: &'static str,
    blocked: bool,
    entered: Sender<()>,
    release: Receiver<()>,
    count: u64,
}
impl CollectorFactory for Factory {
    fn descriptor(&self) -> CollectorDescriptor {
        CollectorDescriptor {
            id: if self.blocked { "slow" } else { "fast" },
            period: SamplingPeriod::Fixed(Duration::from_millis(10)),
        }
    }
    fn create(&self) -> Result<Box<dyn Collector>, CollectorError> {
        Ok(Box::new(FakeCollector {
            id: self.descriptor().id,
            blocked: self.blocked,
            entered: self.entered.clone(),
            release: self.release.clone(),
            count: 0,
        }))
    }
}
impl Collector for FakeCollector {
    fn collect(&mut self, _: &CollectContext) -> Result<SampleBatch, CollectorError> {
        if self.blocked {
            self.entered.send(()).unwrap();
            self.release.recv_timeout(Duration::from_secs(5)).unwrap();
            self.blocked = false;
        }
        self.count += 1;
        let at = Instant::now();
        Ok(SampleBatch {
            devices: vec![DeviceDescriptor::new(self.id, "", DeviceKind::System)],
            samples: vec![MetricSample::new(
                self.id,
                "ticks",
                Unit::Seconds,
                SampleValue::Gauge(self.count as f64),
                at,
            )],
            ..Default::default()
        })
    }
}

#[test]
fn slow_collector_and_unread_ui_do_not_stop_fast_history() {
    let scratch = Scratch::new();
    let (entered_tx, entered_rx) = bounded(1);
    let (release_tx, release_rx) = bounded(1);
    let slow = Arc::new(Factory {
        blocked: true,
        entered: entered_tx.clone(),
        release: release_rx.clone(),
    });
    let fast = Arc::new(Factory {
        blocked: false,
        entered: entered_tx,
        release: release_rx,
    });
    let mut runtime =
        MonitorRuntime::start(vec![slow, fast], ConfigStore::load(scratch.config())).unwrap();
    let client = runtime.client();
    let (wake_tx, wake_rx) = bounded(1);
    client.set_ui_wake(Arc::new(move || {
        let _ = wake_tx.try_send(());
    }));
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    client
        .try_command(MonitorCommand::SetQuery {
            keys: vec![MetricKey::new("fast", "ticks")],
            range_secs: 60,
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        wake_rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        // No UI consumption between notifications: history still originates in the engine.
        let view = client.try_latest_view().unwrap();
        if view
            .history
            .get(&MetricKey::new("fast", "ticks"))
            .is_some_and(|v| v.len() >= 3)
        {
            break;
        }
        assert!(Instant::now() < deadline);
    }
    release_tx.send(()).unwrap();
    runtime.shutdown();
}

#[test]
fn in_flight_limit_is_explicit_and_shutdown_flushes_accepted_save() {
    let scratch = Scratch::new();
    let mut runtime =
        MonitorRuntime::start(Vec::new(), ConfigStore::load(scratch.config())).unwrap();
    let client = runtime.client();
    for _ in 0..32 {
        client
            .try_command(MonitorCommand::RefreshInventory)
            .unwrap();
    }
    assert!(
        client
            .try_command(MonitorCommand::RefreshInventory)
            .is_err()
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut received = 0;
    while received < 32 && Instant::now() < deadline {
        received += client.take_results().len();
        std::thread::yield_now();
    }
    assert_eq!(received, 32);
    let config = AppConfig {
        language: Some(Language::Japanese),
        sampling_ms: 2000,
        history_secs: 300,
        edge_docking: false,
        minimize_to_tray: false,
        autostart_to_tray: false,
        dock: Some(DockEdge::Left),
        dock_position: Some([12.0, 240.0]),
        ..Default::default()
    };
    client
        .try_command(MonitorCommand::SaveConfig {
            config: config.clone(),
            explicit: true,
        })
        .unwrap();
    runtime.shutdown();
    assert_eq!(ConfigStore::load(scratch.config()).config, config);
}

#[test]
fn config_recovers_fields_and_preserves_corrupt_file_until_explicit_save() {
    let scratch = Scratch::new();
    fs::write(
        scratch.config(),
        "sampling_ms = 'bad'\nanimations = false\nhistory_secs = 300\n",
    )
    .unwrap();
    let loaded = ConfigStore::load(scratch.config());
    assert_eq!(loaded.config.sampling_ms, 1000);
    assert!(!loaded.config.animations);
    assert_eq!(loaded.config.history_secs, 300);
    let broken = "this is [broken";
    fs::write(scratch.config(), broken).unwrap();
    let mut loaded = ConfigStore::load(scratch.config());
    assert!(loaded.warning.is_some());
    assert!(loaded.store.save(&loaded.config, false).is_err());
    assert_eq!(fs::read_to_string(scratch.config()).unwrap(), broken);
    loaded.store.save(&loaded.config, true).unwrap();
    assert_eq!(ConfigStore::load(scratch.config()).config, loaded.config);
}

struct FakeProcesses {
    acted: Sender<(ProcessId, ProcessAction)>,
}
struct FakeSource {
    clock: f64,
}
impl ProcessSource for FakeSource {
    fn snapshot(&mut self) -> Result<ProcessSnapshot, Reason> {
        self.clock += 1.0;
        Ok(ProcessSnapshot {
            clock: self.clock,
            processors: 2,
            processes: vec![ProcessInfo {
                id: ProcessId {
                    host: ProcessHost::Windows,
                    pid: 42,
                    start: 7,
                },
                parent: None,
                name: "busy.exe".into(),
                command: String::new(),
                user: String::new(),
                cpu_time: self.clock * 0.5,
                memory: 1024,
                threads: 1,
                priority: Some(Priority::Normal),
            }],
        })
    }
}
impl ProcessPlatform for FakeProcesses {
    fn hosts(&self) -> Vec<ProcessHost> {
        vec![ProcessHost::Windows, ProcessHost::Wsl("Test".into())]
    }
    fn source(&self, _: &ProcessHost) -> Box<dyn ProcessSource> {
        Box::new(FakeSource { clock: 0.0 })
    }
    fn act(&self, id: &ProcessId, action: ProcessAction) -> Result<(), Reason> {
        if id.start != 7 {
            return Err(Reason::ProcessGone);
        }
        self.acted.send((id.clone(), action)).unwrap();
        Ok(())
    }
}

fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn processes_are_polled_while_watched_and_actions_report_back() {
    let scratch = Scratch::new();
    let (acted_tx, acted_rx) = bounded(4);
    let mut runtime = MonitorRuntime::start_with_processes(
        Vec::new(),
        Arc::new(FakeProcesses { acted: acted_tx }),
        ConfigStore::load(scratch.config()),
    )
    .unwrap();
    let client = runtime.client();
    std::thread::sleep(Duration::from_millis(200));
    let view = client.try_latest_view().unwrap();
    assert!(view.processes.hosts.is_empty() && view.processes.table.is_none());

    client
        .try_command(MonitorCommand::WatchProcesses(Some(ProcessHost::Windows)))
        .unwrap();
    // Half a second of CPU time per second on two processors.
    let table = wait_for(|| {
        let table = client.try_latest_view()?.processes.table.clone()?;
        table.rows[0].cpu.is_some().then_some(table)
    });
    assert_eq!(table.host, ProcessHost::Windows);
    assert_eq!(table.rows[0].cpu, Some(25.0));
    let view = client.try_latest_view().unwrap();
    assert_eq!(view.processes.hosts.len(), 2);

    let id = table.rows[0].info.id.clone();
    let end = client
        .try_command(MonitorCommand::Process {
            process: id.clone(),
            action: ProcessAction::End,
        })
        .unwrap();
    let gone = client
        .try_command(MonitorCommand::Process {
            process: ProcessId { start: 8, ..id },
            action: ProcessAction::SetPriority(Priority::High),
        })
        .unwrap();
    let mut results = Vec::new();
    wait_for(|| {
        results.extend(client.take_results());
        (results
            .iter()
            .filter(|r| r.id == end || r.id == gone)
            .count()
            == 2)
            .then_some(())
    });
    let outcome = |id| &results.iter().find(|r| r.id == id).unwrap().result;
    assert!(matches!(outcome(end), Ok(CommandOutcome::Applied)));
    assert_eq!(
        outcome(gone).as_ref().unwrap_err(),
        &Notice::Process(Reason::ProcessGone)
    );
    assert_eq!(acted_rx.try_recv().unwrap().1, ProcessAction::End);

    client
        .try_command(MonitorCommand::WatchProcesses(None))
        .unwrap();
    wait_for(|| {
        client
            .try_latest_view()
            .filter(|view| view.processes.table.is_none())
    });
    runtime.shutdown();
}

struct FakeRemote {
    secrets: std::sync::Mutex<std::collections::HashMap<String, String>>,
    connects: AtomicU64,
}
struct FakeSession;
impl RemoteSession for FakeSession {
    fn collect(&mut self) -> Result<SampleBatch, Reason> {
        Ok(SampleBatch {
            devices: vec![DeviceDescriptor::new("cpu", "Test CPU", DeviceKind::Cpu)],
            samples: vec![MetricSample::new(
                "cpu",
                "usage",
                Unit::Percent,
                SampleValue::Gauge(12.5),
                Instant::now(),
            )],
            complete_inventory: true,
            ..Default::default()
        })
    }
}
impl RemotePlatform for FakeRemote {
    fn connect(
        &self,
        endpoint: &RemoteEndpoint,
        _: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Box<dyn RemoteSession>, Reason> {
        self.connects.fetch_add(1, Ordering::Relaxed);
        match self
            .secrets
            .lock()
            .unwrap()
            .get(&endpoint.id)
            .map(String::as_str)
        {
            None => Err(Reason::NoPassword),
            Some("right") => Ok(Box::new(FakeSession)),
            Some(_) => Err(Reason::AuthFailed),
        }
    }
    fn set_secret(&self, id: &str, secret: Option<&str>) -> Result<(), Reason> {
        let mut secrets = self.secrets.lock().unwrap();
        match secret {
            Some(secret) => secrets.insert(id.into(), secret.into()),
            None => secrets.remove(id),
        };
        Ok(())
    }
    fn presets(&self) -> Vec<RemotePreset> {
        vec![RemotePreset {
            alias: "lab".into(),
            address: "10.0.0.2".into(),
            port: 2222,
            user: "ops".into(),
            key_file: None,
        }]
    }
}

fn remote_host(id: &str) -> RemoteHost {
    RemoteHost {
        id: id.into(),
        name: "Web".into(),
        address: "web.example".into(),
        port: 22,
        user: "ops".into(),
        auth: RemoteAuthKind::Password,
        key_file: None,
        host_key: None,
    }
}

#[test]
fn remote_hosts_wait_for_the_user_and_leave_no_trace_once_removed() {
    let scratch = Scratch::new();
    let mut loaded = ConfigStore::load(scratch.config());
    loaded.config.remote_hosts = vec![remote_host("web")];
    loaded.store.save(&loaded.config, true).unwrap();
    let remote = Arc::new(FakeRemote {
        secrets: Default::default(),
        connects: AtomicU64::new(0),
    });
    let mut runtime = MonitorRuntime::start_with_remote(
        Vec::new(),
        Arc::new(NoProcessAccess),
        remote.clone(),
        ConfigStore::load(scratch.config()),
    )
    .unwrap();
    let client = runtime.client();
    let state = |client: &MonitorClient| {
        client
            .try_latest_view()
            .and_then(|view| Some(view.remote.hosts.get("web")?.state.clone()))
    };
    wait_for(|| (state(&client)? == RemoteState::Failed(Reason::NoPassword)).then_some(()));
    let view = client.try_latest_view().unwrap();
    assert_eq!(view.remote.presets[0].alias, "lab");
    assert!(view.diagnostics.collector_errors.is_empty());
    // A failure that needs the user is not retried by itself.
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(remote.connects.load(Ordering::Relaxed), 1);

    client
        .try_command(MonitorCommand::RemoteSecret {
            host: "web".into(),
            secret: Some(Secret("right".into())),
        })
        .unwrap();
    let view = wait_for(|| {
        let view = client.try_latest_view()?;
        (view.value("ssh:web/cpu", "usage") == Some(12.5)).then_some(view)
    });
    assert_eq!(state(&client), Some(RemoteState::Connected));
    let cpu = &view.devices["ssh:web/cpu"];
    assert_eq!(cpu.host.as_deref(), Some("web"));
    assert_eq!(cpu.kind, DeviceKind::Cpu);

    let mut config = view.config.clone();
    config.remote_hosts.clear();
    client
        .try_command(MonitorCommand::SaveConfig {
            config,
            explicit: true,
        })
        .unwrap();
    wait_for(|| {
        let view = client.try_latest_view()?;
        (view.remote.hosts.is_empty()
            && !view.devices.keys().any(|id| id.starts_with("ssh:"))
            && !view
                .metrics
                .keys()
                .any(|key| key.device.starts_with("ssh:")))
        .then_some(())
    });
    runtime.shutdown();
    assert!(
        ConfigStore::load(scratch.config())
            .config
            .remote_hosts
            .is_empty()
    );
}

struct NoProcessAccess;
impl ProcessPlatform for NoProcessAccess {
    fn hosts(&self) -> Vec<ProcessHost> {
        Vec::new()
    }
    fn source(&self, _: &ProcessHost) -> Box<dyn ProcessSource> {
        unreachable!()
    }
    fn act(&self, _: &ProcessId, _: ProcessAction) -> Result<(), Reason> {
        Err(Reason::HostStopped)
    }
}

#[test]
fn each_remote_host_entry_recovers_on_its_own() {
    let scratch = Scratch::new();
    fs::write(
        scratch.config(),
        "animations = false\n\n[[remote_hosts]]\nid = \"web\"\naddress = \"web.example\"\nuser = \"ops\"\nauth = \"key\"\nkey_file = \"~/.ssh/id_ed25519\"\n\n[[remote_hosts]]\nid = \"bad id\"\naddress = \"db.example\"\nuser = \"ops\"\nauth = \"agent\"\n\n[[remote_hosts]]\nid = \"db\"\naddress = \"db.example\"\nuser = \"ops\"\nauth = \"telepathy\"\n",
    )
    .unwrap();
    let mut loaded = ConfigStore::load(scratch.config());
    assert!(!loaded.config.animations);
    assert_eq!(loaded.config.remote_hosts.len(), 1);
    let web = &loaded.config.remote_hosts[0];
    assert_eq!(web.port, 22);
    assert_eq!(web.label(), "web.example");
    assert_eq!(
        web.endpoint().auth,
        RemoteAuth::Key("~/.ssh/id_ed25519".into())
    );
    assert!(loaded.warning.is_some());
    loaded.config.remote_hosts.push(remote_host("api"));
    loaded.store.save(&loaded.config, true).unwrap();
    assert_eq!(ConfigStore::load(scratch.config()).config, loaded.config);
    let mut twice = loaded.config.clone();
    twice.remote_hosts.push(remote_host("api"));
    assert_eq!(twice.validate(), Err(Notice::InvalidRemoteHost));
}

/// Frames of a fake game at 20 frames per second; `live` counts the collectors in use.
struct FakeFrames {
    live: Arc<AtomicU64>,
}
struct FakeFrameCollector {
    live: Arc<AtomicU64>,
}
impl CollectorFactory for FakeFrames {
    fn descriptor(&self) -> CollectorDescriptor {
        CollectorDescriptor {
            id: "frames",
            period: SamplingPeriod::Fixed(Duration::from_millis(10)),
        }
    }
    fn create(&self) -> Result<Box<dyn Collector>, CollectorError> {
        self.live.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(FakeFrameCollector {
            live: self.live.clone(),
        }))
    }
}
impl Collector for FakeFrameCollector {
    fn collect(&mut self, _: &CollectContext) -> Result<SampleBatch, CollectorError> {
        Ok(SampleBatch {
            devices: vec![DeviceDescriptor::new(
                "frames",
                "game.exe",
                DeviceKind::Frames,
            )],
            samples: vec![MetricSample::new(
                "frames",
                "fps",
                Unit::FramesPerSecond,
                SampleValue::Gauge(20.0),
                Instant::now(),
            )],
            complete_inventory: true,
            ..Default::default()
        })
    }
}
impl Drop for FakeFrameCollector {
    fn drop(&mut self) {
        self.live.fetch_sub(1, Ordering::Relaxed);
    }
}

#[test]
fn frames_run_only_while_switched_on_and_alerts_notify_once() {
    let scratch = Scratch::new();
    let mut loaded = ConfigStore::load(scratch.config());
    loaded.config.alerts = vec![AlertRule {
        id: "low-fps".into(),
        enabled: true,
        device: "frames".into(),
        metric: "fps".into(),
        condition: AlertCondition::Below,
        threshold: 30.0,
        hold_secs: 0,
    }];
    let live = Arc::new(AtomicU64::new(0));
    let (notified_tx, notified_rx) = bounded(8);
    let mut runtime = MonitorRuntime::start_with_services(
        Vec::new(),
        Services {
            frames: Some(Arc::new(FakeFrames { live: live.clone() })),
            notify: Arc::new(move |event: &AlertEvent| {
                let _ = notified_tx.try_send(event.clone());
            }),
            ..Services::default()
        },
        loaded,
    )
    .unwrap();
    let client = runtime.client();
    let view = wait_for(|| {
        let view = client.try_latest_view()?;
        view.alerts.states.contains_key("low-fps").then_some(view)
    });
    assert!(!view.devices.contains_key("frames"));
    assert_eq!(live.load(Ordering::Relaxed), 0);
    assert_eq!(view.alerts.states["low-fps"], AlertState::NoData);

    let mut config = view.config.clone();
    config.frame_monitoring = true;
    client
        .try_command(MonitorCommand::SaveConfig {
            config: config.clone(),
            explicit: true,
        })
        .unwrap();
    let event = notified_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(event.rule.id, "low-fps");
    assert_eq!(event.value, 20.0);
    assert_eq!(event.device.name, "game.exe");
    assert_eq!(event.unit, Unit::FramesPerSecond);
    let view = wait_for(|| {
        let view = client.try_latest_view()?;
        matches!(view.alerts.states["low-fps"], AlertState::Firing { .. }).then_some(view)
    });
    assert_eq!(view.alerts.recent.len(), 1);
    // The reading stays low, so the rule keeps firing without another notification.
    std::thread::sleep(Duration::from_millis(300));
    assert!(notified_rx.try_recv().is_err());
    assert_eq!(live.load(Ordering::Relaxed), 1);

    config.frame_monitoring = false;
    client
        .try_command(MonitorCommand::SaveConfig {
            config,
            explicit: true,
        })
        .unwrap();
    wait_for(|| {
        let view = client.try_latest_view()?;
        (!view.devices.get("frames")?.online
            && view.reading("frames", "fps")?.state == SampleState::Offline
            && view.alerts.states["low-fps"] == AlertState::NoData
            && live.load(Ordering::Relaxed) == 0)
            .then_some(())
    });
    runtime.shutdown();
}

#[test]
fn alert_rules_round_trip_and_recover_one_by_one() {
    let scratch = Scratch::new();
    fs::write(
        scratch.config(),
        "frame_monitoring = true\n\n[[alerts]]\nid = \"cpu\"\ndevice = \"cpu\"\nmetric = \"usage\"\ncondition = \"above\"\nthreshold = 90.0\nhold_secs = 10\n\n[[alerts]]\nid = \"nan\"\ndevice = \"memory\"\nmetric = \"usage\"\ncondition = \"above\"\nthreshold = nan\n\n[[alerts]]\nid = \"odd\"\ndevice = \"memory\"\nmetric = \"usage\"\ncondition = \"sideways\"\nthreshold = 1.0\n",
    )
    .unwrap();
    let mut loaded = ConfigStore::load(scratch.config());
    assert!(loaded.config.frame_monitoring);
    assert_eq!(loaded.config.alerts.len(), 1);
    let cpu = &loaded.config.alerts[0];
    assert!(cpu.enabled);
    assert_eq!(cpu.condition, AlertCondition::Above);
    assert_eq!(cpu.hold_secs, 10);
    assert!(loaded.warning.is_some());
    loaded.config.remote_hosts.push(remote_host("api"));
    loaded.store.save(&loaded.config, true).unwrap();
    assert_eq!(ConfigStore::load(scratch.config()).config, loaded.config);
    let mut twice = loaded.config.clone();
    twice.alerts.push(twice.alerts[0].clone());
    assert_eq!(twice.validate(), Err(Notice::InvalidAlertRule));
}
