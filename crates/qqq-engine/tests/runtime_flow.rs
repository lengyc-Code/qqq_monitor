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
            devices: vec![DeviceDescriptor::new(self.id, "测试", DeviceKind::System)],
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
        sampling_ms: 2000,
        history_secs: 300,
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
