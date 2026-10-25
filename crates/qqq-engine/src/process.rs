//! Process lists of the host the GUI watches, and the actions it takes on processes.
//! Polling and listing the hosts run only while a host is watched, each on a thread of its
//! own, as do the actions, so a slow `wsl.exe` delays neither the other lists nor the
//! engine.

use crate::{CommandOutcome, CommandResult, Notice, runtime::Control};
use crossbeam_channel::{Receiver, Sender};
use qqq_domain::*;
use std::{
    collections::HashMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

/// How often the hosts are listed again while processes are watched.
const HOSTS_PERIOD: Duration = Duration::from_secs(5);
/// Shortest polling period of each kind of host: Windows lists all processes in one
/// system call, while every WSL poll starts a `wsl.exe`.
const WINDOWS_PERIOD: Duration = Duration::from_secs(1);
const WSL_PERIOD: Duration = Duration::from_secs(2);

/// A process with its CPU usage since the previous snapshot of its host.
#[derive(Clone, Debug)]
pub struct ProcessRow {
    pub info: ProcessInfo,
    /// Share of all logical processors of the host, 0 to 100; `None` in the first
    /// snapshot that lists the process.
    pub cpu: Option<f64>,
}

/// Processes of one host at one moment.
#[derive(Clone, Debug)]
pub struct ProcessTable {
    pub host: ProcessHost,
    pub rows: Vec<ProcessRow>,
}

/// Process state for the GUI.
#[derive(Clone, Debug, Default)]
pub struct ProcessView {
    /// Hosts available to list, Windows first; empty until processes are first watched.
    pub hosts: Vec<ProcessHost>,
    /// Latest table of the watched host.
    pub table: Option<Arc<ProcessTable>>,
    /// Why the latest poll of the watched host failed.
    pub error: Option<Reason>,
}

/// CPU usage from the processor time each process used between two snapshots.
#[derive(Default)]
pub(crate) struct CpuTracker {
    previous: Option<(f64, HashMap<ProcessId, f64>)>,
}

impl CpuTracker {
    pub(crate) fn rows(&mut self, snapshot: ProcessSnapshot) -> Vec<ProcessRow> {
        let capacity = self
            .previous
            .as_ref()
            .map(|(clock, _)| (snapshot.clock - clock) * snapshot.processors.max(1) as f64)
            .filter(|capacity| *capacity > 0.0);
        let times = snapshot
            .processes
            .iter()
            .map(|p| (p.id.clone(), p.cpu_time))
            .collect();
        let previous = self.previous.replace((snapshot.clock, times));
        snapshot
            .processes
            .into_iter()
            .map(|info| {
                let cpu = capacity
                    .zip(previous.as_ref())
                    .and_then(|(capacity, (_, before))| {
                        let used = info.cpu_time - before.get(&info.id)?;
                        Some((used / capacity * 100.0).clamp(0.0, 100.0))
                    });
                ProcessRow { info, cpu }
            })
            .collect()
    }
}

pub(crate) enum ProcessEvent {
    Hosts(Vec<ProcessHost>),
    Table(ProcessHost, Result<Vec<ProcessRow>, Reason>),
}

pub(crate) struct ActionJob {
    pub id: u64,
    pub process: ProcessId,
    pub action: ProcessAction,
}

/// Platform without processes, for runtimes started without process access.
pub(crate) struct NoProcesses;

impl ProcessSource for NoProcesses {
    fn snapshot(&mut self) -> Result<ProcessSnapshot, Reason> {
        Err(Reason::HostStopped)
    }
}

impl ProcessPlatform for NoProcesses {
    fn hosts(&self) -> Vec<ProcessHost> {
        Vec::new()
    }
    fn source(&self, _: &ProcessHost) -> Box<dyn ProcessSource> {
        Box::new(Self)
    }
    fn act(&self, _: &ProcessId, _: ProcessAction) -> Result<(), Reason> {
        Err(Reason::HostStopped)
    }
}

fn guarded<T>(f: impl FnOnce() -> Result<T, Reason>) -> Result<T, Reason> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(Err(Reason::Panicked))
}

/// Lists the hosts every [`HOSTS_PERIOD`] while any host is watched, and at once when
/// asked. Listing WSL distributions starts a `wsl.exe`, which may take seconds.
pub(crate) fn hosts_worker(
    platform: Arc<dyn ProcessPlatform>,
    events: Sender<ProcessEvent>,
    control: Arc<Control>,
) {
    let mut next = Instant::now();
    let mut refresh = control.hosts_refresh.load(Ordering::Acquire);
    while !control.stop.load(Ordering::Acquire) {
        let seen = control.changes();
        if control.process_watch.lock().unwrap().is_none() {
            // The next watch lists the hosts at once.
            next = Instant::now();
            control.wait(seen, Duration::MAX);
            continue;
        }
        let latest = control.hosts_refresh.load(Ordering::Acquire);
        if latest != refresh {
            refresh = latest;
            next = Instant::now();
        }
        let now = Instant::now();
        if now < next {
            control.wait(seen, next - now);
            continue;
        }
        let hosts = guarded(|| Ok(platform.hosts())).unwrap_or_default();
        if events.try_send(ProcessEvent::Hosts(hosts)).is_ok() {
            control.wake_engine();
        }
        next = Instant::now() + HOSTS_PERIOD;
    }
}

/// Polls the watched host while a host is watched.
pub(crate) fn poll_worker(
    platform: Arc<dyn ProcessPlatform>,
    events: Sender<ProcessEvent>,
    control: Arc<Control>,
) {
    let mut source: Option<(ProcessHost, Box<dyn ProcessSource>, CpuTracker)> = None;
    let mut next = Instant::now();
    let mut refresh = control.process_refresh.load(Ordering::Acquire);
    while !control.stop.load(Ordering::Acquire) {
        let seen = control.changes();
        let watched = control.process_watch.lock().unwrap().clone();
        let Some(host) = watched else {
            source = None;
            control.wait(seen, Duration::MAX);
            continue;
        };
        let now = Instant::now();
        let latest = control.process_refresh.load(Ordering::Acquire);
        if latest != refresh {
            refresh = latest;
            next = now;
        }
        if source
            .as_ref()
            .is_none_or(|(watching, ..)| *watching != host)
        {
            source = None;
            next = now;
        }
        if now < next {
            control.wait(seen, next - now);
            continue;
        }
        let (_, poller, tracker) = source
            .get_or_insert_with(|| (host.clone(), platform.source(&host), CpuTracker::default()));
        let result = guarded(|| poller.snapshot()).map(|snapshot| tracker.rows(snapshot));
        if let Err(reason) = &result {
            // The next poll starts over with a new source and CPU baseline.
            source = None;
            if *reason == Reason::HostStopped {
                control.hosts_refresh.fetch_add(1, Ordering::Release);
                control.changed();
            }
        }
        if events
            .try_send(ProcessEvent::Table(host.clone(), result))
            .is_ok()
        {
            control.wake_engine();
        }
        let shortest = match host {
            ProcessHost::Windows => WINDOWS_PERIOD,
            ProcessHost::Wsl(_) => WSL_PERIOD,
        };
        let sampling = Duration::from_millis(control.sampling_ms.load(Ordering::Acquire));
        next = Instant::now() + sampling.max(shortest);
    }
}

/// Carries out process actions and answers each with a command result; a finished action
/// polls the watched host again at once. Ends once the engine, which hands out the
/// actions, has stopped.
pub(crate) fn action_worker(
    platform: Arc<dyn ProcessPlatform>,
    jobs: Receiver<ActionJob>,
    results: Sender<CommandResult>,
    control: Arc<Control>,
) {
    for job in jobs {
        let result = guarded(|| platform.act(&job.process, job.action))
            .map(|()| CommandOutcome::Applied)
            .map_err(Notice::Process);
        let _ = results.try_send(CommandResult { id: job.id, result });
        control.process_refresh.fetch_add(1, Ordering::Release);
        control.changed();
        control.notify_ui();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, start: u64, cpu_time: f64) -> ProcessInfo {
        ProcessInfo {
            id: ProcessId {
                host: ProcessHost::Windows,
                pid,
                start,
            },
            parent: None,
            name: format!("p{pid}"),
            command: String::new(),
            user: String::new(),
            cpu_time,
            memory: 0,
            threads: 1,
            priority: None,
        }
    }

    fn snapshot(clock: f64, processes: Vec<ProcessInfo>) -> ProcessSnapshot {
        ProcessSnapshot {
            clock,
            processors: 4,
            processes,
        }
    }

    #[test]
    fn cpu_usage_needs_a_baseline_of_the_same_instance() {
        let mut tracker = CpuTracker::default();
        let first = tracker.rows(snapshot(10.0, vec![process(1, 7, 1.0), process(2, 7, 0.0)]));
        assert!(first.iter().all(|row| row.cpu.is_none()));
        // Two seconds on four processors: one second of CPU time is 12.5 %.
        let second = tracker.rows(snapshot(
            12.0,
            vec![process(1, 7, 2.0), process(2, 8, 5.0), process(3, 1, 9.0)],
        ));
        assert_eq!(second[0].cpu, Some(12.5));
        // PID 2 now names a newer process, and PID 3 is new.
        assert_eq!(second[1].cpu, None);
        assert_eq!(second[2].cpu, None);
        // A clock that does not advance gives no usage, and usage stays within 100 %.
        assert!(
            tracker.rows(snapshot(12.0, vec![process(1, 7, 3.0)]))[0]
                .cpu
                .is_none()
        );
        let third = tracker.rows(snapshot(13.0, vec![process(1, 7, 50.0)]));
        assert_eq!(third[0].cpu, Some(100.0));
    }
}
