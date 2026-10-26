//! Remote hosts. Every configured host has a worker thread that connects over SSH, reads
//! the host at the sampling interval and reconnects after failures that may pass by
//! themselves. The engine thread starts, stops and restarts the workers as the configured
//! hosts change, and namespaces each host's devices under its own prefix.

use crate::{RemoteHost, delivery::deliver, runtime::Control};
use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};
use qqq_domain::*;
use std::{
    collections::BTreeMap,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Shortest interval between two reads of a remote host.
const SHORTEST_PERIOD: Duration = Duration::from_secs(1);
/// Seconds to wait before reconnecting after successive failures; the last one repeats.
const RETRY_SECS: [u64; 6] = [2, 4, 8, 16, 30, 60];

/// Connection state of a remote host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteState {
    /// Connecting and signing in.
    Connecting,
    Connected,
    /// The latest attempt failed. The worker tries again by itself unless
    /// [`needs_user`] holds for the reason.
    Failed(Reason),
}

#[derive(Clone, Debug)]
pub struct RemoteStatus {
    pub state: RemoteState,
    /// When the host entered the state.
    pub since: Instant,
}

/// Remote hosts for the GUI.
#[derive(Clone, Debug, Default)]
pub struct RemoteView {
    /// Status of every configured host by ID.
    pub hosts: BTreeMap<String, RemoteStatus>,
    /// Hosts of the user's SSH client configuration, read at start.
    pub presets: Arc<Vec<RemotePreset>>,
}

/// A password or passphrase on its way to the credential store; debug output hides it.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(pub String);

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(…)")
    }
}

/// Whether a connection failure waits for the user rather than a retry. Retrying rejected
/// credentials could get the address banned by the host, and an untrusted host key needs
/// the user's decision.
pub fn needs_user(reason: &Reason) -> bool {
    matches!(
        reason,
        Reason::HostKeyUnknown { .. }
            | Reason::HostKeyChanged { .. }
            | Reason::AuthFailed
            | Reason::NoPassword
            | Reason::KeyUnreadable
            | Reason::KeyPassphrase
            | Reason::NoAgent
            | Reason::NotLinux
    )
}

pub(crate) enum RemoteUpdate {
    Connecting,
    Batch {
        batch: SampleBatch,
        period: Duration,
        /// Batches dropped so far because the engine fell behind.
        dropped: u64,
    },
    Failed(Reason),
}

/// Platform without remote access, for runtimes started without it.
pub(crate) struct NoRemote;

impl RemotePlatform for NoRemote {
    fn connect(
        &self,
        _: &RemoteEndpoint,
        _: Arc<AtomicBool>,
    ) -> Result<Box<dyn RemoteSession>, Reason> {
        Err(Reason::ConnectionRefused)
    }
    fn set_secret(&self, _: &str, _: Option<&str>) -> Result<(), Reason> {
        Ok(())
    }
    fn presets(&self) -> Vec<RemotePreset> {
        Vec::new()
    }
}

struct Worker {
    endpoint: RemoteEndpoint,
    updates: Receiver<RemoteUpdate>,
    cancel: Arc<AtomicBool>,
    reconnect: Arc<AtomicU64>,
}

/// Workers of the configured hosts, owned by the engine thread.
pub(crate) struct Remotes {
    pub(crate) platform: Arc<dyn RemotePlatform>,
    control: Arc<Control>,
    workers: BTreeMap<String, Worker>,
    /// Shared with the runtime, which waits for the workers on shutdown.
    threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl Remotes {
    pub(crate) fn new(
        platform: Arc<dyn RemotePlatform>,
        control: Arc<Control>,
        threads: Arc<Mutex<Vec<JoinHandle<()>>>>,
    ) -> Self {
        Self {
            platform,
            control,
            workers: BTreeMap::new(),
            threads,
        }
    }

    /// Matches the workers to `hosts`: starts new hosts, stops removed ones and restarts
    /// hosts whose connection settings changed. Returns the IDs whose earlier readings no
    /// longer apply, which are removed hosts and hosts that now point to another address.
    pub(crate) fn apply(&mut self, hosts: &[RemoteHost]) -> Vec<String> {
        let mut forget = Vec::new();
        let mut cancelled = false;
        self.workers.retain(|id, worker| {
            let host = hosts.iter().find(|h| h.id == *id);
            let endpoint = host.map(RemoteHost::endpoint);
            if endpoint.as_ref() == Some(&worker.endpoint) {
                return true;
            }
            worker.cancel.store(true, Ordering::Release);
            cancelled = true;
            if endpoint.is_none_or(|e| {
                (&e.address, e.port) != (&worker.endpoint.address, worker.endpoint.port)
            }) {
                forget.push(id.clone());
            }
            false
        });
        if cancelled {
            self.control.changed();
        }
        if !self.control.stop.load(Ordering::Acquire) {
            for host in hosts {
                if !self.workers.contains_key(&host.id) {
                    self.start(host.endpoint());
                }
            }
        }
        forget
    }

    fn start(&mut self, endpoint: RemoteEndpoint) {
        let (tx, rx) = bounded(16);
        let cancel = Arc::new(AtomicBool::new(false));
        let reconnect = Arc::new(AtomicU64::new(0));
        let spawned = thread::Builder::new()
            .name(format!("remote-{}", endpoint.id))
            .spawn({
                let platform = Arc::clone(&self.platform);
                let endpoint = endpoint.clone();
                let cancel = Arc::clone(&cancel);
                let reconnect = Arc::clone(&reconnect);
                let control = Arc::clone(&self.control);
                move || worker(platform, endpoint, tx, cancel, reconnect, control)
            });
        match spawned {
            Ok(handle) => {
                let mut threads = self.threads.lock().unwrap();
                threads.retain(|t| !t.is_finished());
                threads.push(handle);
                self.workers.insert(
                    endpoint.id.clone(),
                    Worker {
                        endpoint,
                        updates: rx,
                        cancel,
                        reconnect,
                    },
                );
            }
            Err(error) => tracing::error!(%error, host = endpoint.id, "无法启动远程主机线程"),
        }
    }

    /// Reconnects host `id` at once, also after a failure that waits for the user.
    pub(crate) fn reconnect(&self, id: &str) {
        if let Some(worker) = self.workers.get(id) {
            worker.reconnect.fetch_add(1, Ordering::Release);
            self.control.changed();
        }
    }

    /// Whether a worker has updates waiting.
    pub(crate) fn waiting(&self) -> bool {
        self.workers
            .values()
            .any(|worker| !worker.updates.is_empty())
    }

    /// Waiting updates, at most a few of each host so that none starves the others.
    pub(crate) fn updates(&self) -> Vec<(String, RemoteUpdate)> {
        self.workers
            .iter()
            .flat_map(|(id, worker)| {
                worker
                    .updates
                    .try_iter()
                    .take(8)
                    .map(move |update| (id.clone(), update))
            })
            .collect()
    }

    /// Tells every worker to stop, including connections still being made.
    pub(crate) fn stop(&mut self) {
        for worker in self.workers.values() {
            worker.cancel.store(true, Ordering::Release);
        }
        self.workers.clear();
        self.control.changed();
    }
}

fn guarded<T>(f: impl FnOnce() -> Result<T, Reason>) -> Result<T, Reason> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(Err(Reason::Panicked))
}

/// Connects, reads the host until the connection fails or a reconnect is requested, and
/// waits before the next attempt.
fn worker(
    platform: Arc<dyn RemotePlatform>,
    endpoint: RemoteEndpoint,
    updates: Sender<RemoteUpdate>,
    cancel: Arc<AtomicBool>,
    reconnect: Arc<AtomicU64>,
    control: Arc<Control>,
) {
    let stopped = || cancel.load(Ordering::Acquire) || control.stop.load(Ordering::Acquire);
    let send = |update| deliver(&updates, update, stopped, || control.wake_engine());
    let mut failures = 0;
    let mut dropped = 0_u64;
    while !stopped() {
        let requested = reconnect.load(Ordering::Acquire);
        if !send(RemoteUpdate::Connecting) {
            return;
        }
        let failure = match guarded(|| platform.connect(&endpoint, Arc::clone(&cancel))) {
            Err(reason) => Some(reason),
            Ok(mut session) => {
                let mut next = Instant::now();
                loop {
                    let seen = control.changes();
                    if stopped() {
                        return;
                    }
                    if reconnect.load(Ordering::Acquire) != requested {
                        break None;
                    }
                    let now = Instant::now();
                    if now < next {
                        control.wait(seen, next - now);
                        continue;
                    }
                    let period = Duration::from_millis(control.sampling_ms.load(Ordering::Acquire))
                        .max(SHORTEST_PERIOD);
                    match guarded(|| session.collect()) {
                        Ok(batch) => {
                            failures = 0;
                            match updates.try_send(RemoteUpdate::Batch {
                                batch,
                                period,
                                dropped,
                            }) {
                                Ok(()) => control.wake_engine(),
                                Err(TrySendError::Full(_)) => dropped = dropped.saturating_add(1),
                                Err(TrySendError::Disconnected(_)) => return,
                            }
                        }
                        Err(reason) => break Some(reason),
                    }
                    next += period;
                    if next <= Instant::now() {
                        next = Instant::now() + period;
                    }
                }
            }
        };
        if stopped() {
            return;
        }
        // A requested reconnect connects again at once.
        let Some(reason) = failure else {
            failures = 0;
            continue;
        };
        let deadline = (!needs_user(&reason)).then(|| {
            Instant::now() + Duration::from_secs(RETRY_SECS[failures.min(RETRY_SECS.len() - 1)])
        });
        failures += 1;
        if !send(RemoteUpdate::Failed(reason)) {
            return;
        }
        loop {
            let seen = control.changes();
            let left = deadline.map_or(Duration::MAX, |deadline| {
                deadline.saturating_duration_since(Instant::now())
            });
            if stopped() || reconnect.load(Ordering::Acquire) != requested || left.is_zero() {
                break;
            }
            control.wait(seen, left);
        }
        if reconnect.load(Ordering::Acquire) != requested {
            failures = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppConfig;

    struct Rejected(Sender<()>);
    impl RemotePlatform for Rejected {
        fn connect(
            &self,
            _: &RemoteEndpoint,
            _: Arc<AtomicBool>,
        ) -> Result<Box<dyn RemoteSession>, Reason> {
            self.0.send(()).unwrap();
            Err(Reason::AuthFailed)
        }
        fn set_secret(&self, _: &str, _: Option<&str>) -> Result<(), Reason> {
            Ok(())
        }
        fn presets(&self) -> Vec<RemotePreset> {
            Vec::new()
        }
    }

    #[test]
    fn a_rejected_login_waits_until_its_failure_reaches_a_full_queue() {
        let (updates, received) = bounded(1);
        let (entered, connected) = bounded(1);
        let control = Arc::new(Control::new(&AppConfig::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        let handle = thread::spawn({
            let control = control.clone();
            let cancel = cancel.clone();
            move || {
                worker(
                    Arc::new(Rejected(entered)),
                    RemoteEndpoint {
                        id: "test".into(),
                        address: "unused".into(),
                        port: 22,
                        user: "test".into(),
                        auth: RemoteAuth::Password,
                        host_key: None,
                    },
                    updates,
                    cancel,
                    Arc::new(AtomicU64::new(0)),
                    control,
                )
            }
        });
        // The single queue slot holds Connecting when authentication fails.
        connected.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(control.wait_for_engine_wake());
        let waiting_for_space = control.wait_for_engine_wake();
        assert!(matches!(received.recv().unwrap(), RemoteUpdate::Connecting));
        let failure = received.recv_timeout(Duration::from_secs(2));
        cancel.store(true, Ordering::Release);
        control.changed();
        handle.join().unwrap();
        assert!(waiting_for_space);
        assert!(matches!(
            failure,
            Ok(RemoteUpdate::Failed(Reason::AuthFailed))
        ));
    }
}
