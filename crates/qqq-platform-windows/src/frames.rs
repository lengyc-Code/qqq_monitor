//! Frames of the application in the foreground. A real-time event tracing session receives
//! the Present events of the DXGI and Direct3D 9 runtimes, the source PresentMon and the
//! Xbox Game Bar read as well, without touching the game's process. Each read counts the
//! frames that the foreground window's process presented on its busiest swap chain since
//! the previous read. Starting the session needs administrator rights or membership in the
//! Performance Log Users group.

use crate::process;
use qqq_domain::*;
use std::{
    mem::{offset_of, size_of, size_of_val, take},
    slice,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{
            ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, GetLastError, WIN32_ERROR,
        },
        System::{
            Diagnostics::Etw::*,
            Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
        },
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    },
    core::{GUID, PCWSTR, PWSTR},
};

/// Device of the foreground application's frames.
pub const DEVICE: &str = "frames";
const SESSION: &str = "QQQ Monitor Frames";
/// Microsoft-Windows-DXGI and its Present_Start and PresentMultiplaneOverlay_Start events.
const DXGI: GUID = GUID::from_u128(0xca11c036_0102_4a2d_a6ad_f03cfed5d3c9);
const DXGI_PRESENTS: [u16; 2] = [42, 55];
/// Microsoft-Windows-D3D9 and its Present_Start event.
const D3D9: GUID = GUID::from_u128(0x783aca0a_790e_4d7f_8451_aa850511c6b9);
const D3D9_PRESENTS: [u16; 1] = [1];
/// Longest time the session keeps a buffer before handing it to the consumer; a full
/// buffer goes sooner. Small buffers shared by all processors fill within a fraction of a
/// second while a game presents.
const FLUSH: Duration = Duration::from_secs(1);
const BUFFER_KIB: u32 = 4;
/// `ClientContext` that stamps events with the performance counter.
const QPC_CLOCK: u32 = 1;
/// Most presents kept between two reads.
const BACKLOG: usize = 1 << 16;
/// Shortest pause in presenting that reads as zero frames per second, beyond the time
/// presents may still be on their way.
const IDLE: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Api {
    Dxgi,
    D3d9,
}

impl Api {
    /// Stable token for the `present_api` device detail.
    fn token(self) -> &'static str {
        match self {
            Self::Dxgi => "dxgi",
            Self::D3d9 => "d3d9",
        }
    }
}

/// One Present call, stamped with the performance counter.
#[derive(Clone, Copy, Debug)]
struct Present {
    pid: u32,
    swap_chain: u64,
    api: Api,
    at: i64,
}

/// What the presents since the previous read say about the target.
#[derive(Debug, PartialEq)]
enum Reading {
    /// Frames per second, and the mean and longest frame time in milliseconds.
    Frames { fps: f64, mean: f64, longest: f64 },
    /// The target has stopped presenting.
    Idle,
    /// The target has presented no frame since it came to the foreground.
    NoFrames,
    /// Frames may still be on their way.
    Pending,
    /// Another application came to the foreground; its statistics start over.
    Switched,
}

/// Frame statistics of the foreground application across reads.
struct Counter {
    /// Performance counter ticks per second.
    frequency: f64,
    /// Process whose frames are counted.
    target: Option<u32>,
    /// Swap chain of the target that is counted, its runtime and its last frame.
    chain: Option<(u64, Api, i64)>,
}

impl Counter {
    fn new(frequency: i64) -> Self {
        Self {
            frequency: frequency.max(1) as f64,
            target: None,
            chain: None,
        }
    }

    /// Weighs the presents since the previous read. `foreground` is the process of the
    /// foreground window; this application's own window, `own`, keeps the previous target
    /// so that looking at the statistics does not replace them. `now` is the current tick.
    fn read(
        &mut self,
        presents: &[Present],
        foreground: Option<u32>,
        own: u32,
        now: i64,
        period: Duration,
    ) -> Reading {
        let mut switched = false;
        if let Some(pid) = foreground.filter(|pid| *pid != own && Some(*pid) != self.target) {
            self.target = Some(pid);
            self.chain = None;
            switched = true;
        }
        let Some(target) = self.target else {
            return Reading::NoFrames;
        };
        let mine: Vec<_> = presents.iter().filter(|p| p.pid == target).collect();
        // The busiest swap chain is the game's; ties keep the one already counted.
        let mut chains: Vec<(u64, usize)> = Vec::new();
        for present in &mine {
            match chains
                .iter_mut()
                .find(|(chain, _)| *chain == present.swap_chain)
            {
                Some((_, count)) => *count += 1,
                None => chains.push((present.swap_chain, 1)),
            }
        }
        let current = self.chain.map(|(chain, ..)| chain);
        let busiest = chains
            .iter()
            .max_by_key(|(chain, count)| (*count, Some(*chain) == current))
            .map(|(chain, _)| *chain);
        let mut intervals = Vec::new();
        if let Some(busiest) = busiest {
            if current != Some(busiest) {
                self.chain = None;
            }
            let mut frames: Vec<_> = mine.iter().filter(|p| p.swap_chain == busiest).collect();
            frames.sort_by_key(|p| p.at);
            for frame in frames {
                if let Some((_, _, last)) = self.chain
                    && frame.at > last
                {
                    intervals.push((frame.at - last) as f64 / self.frequency * 1000.0);
                }
                self.chain = Some((busiest, frame.api, frame.at));
            }
        }
        if switched {
            return Reading::Switched;
        }
        if !intervals.is_empty() {
            let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
            return Reading::Frames {
                fps: 1000.0 / mean,
                mean,
                longest: intervals.iter().copied().fold(0.0, f64::max),
            };
        }
        match self.chain {
            None => Reading::NoFrames,
            Some((_, _, last)) => {
                let quiet = (now - last).max(0) as f64 / self.frequency;
                if quiet > (IDLE.max(period) + FLUSH).as_secs_f64() {
                    Reading::Idle
                } else {
                    Reading::Pending
                }
            }
        }
    }
}

/// Presents the consumer thread has received since the last read.
#[derive(Default)]
struct Shared {
    presents: Mutex<Vec<Present>>,
    /// Status `ProcessTrace` returned with, once it has.
    ended: AtomicU32,
}

const RUNNING: u32 = u32::MAX;

/// Session properties followed by room for the session name, as `StartTraceW` expects.
#[repr(C)]
struct Properties {
    base: EVENT_TRACE_PROPERTIES,
    name: [u16; 64],
}

impl Properties {
    fn new() -> Box<Self> {
        let mut properties = Box::new(Self {
            base: EVENT_TRACE_PROPERTIES::default(),
            name: [0; 64],
        });
        let base = &mut properties.base;
        base.Wnode.BufferSize = size_of::<Self>() as u32;
        base.Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        base.Wnode.ClientContext = QPC_CLOCK;
        base.LogFileMode = EVENT_TRACE_REAL_TIME_MODE | EVENT_TRACE_NO_PER_PROCESSOR_BUFFERING;
        base.FlushTimer = FLUSH.as_secs() as u32;
        base.BufferSize = BUFFER_KIB;
        base.MinimumBuffers = 4;
        base.MaximumBuffers = 64;
        base.LoggerNameOffset = offset_of!(Self, name) as u32;
        properties
    }
}

/// Session name as UTF-16 with a terminating NUL.
fn session_name() -> Vec<u16> {
    SESSION.encode_utf16().chain([0]).collect()
}

fn failed(api: &'static str, status: WIN32_ERROR) -> CollectorError {
    CollectorError(Reason::CallFailed {
        api,
        code: status.0,
    })
}

/// Stops the session of that name, if one runs.
fn stop(name: &[u16]) {
    let mut properties = Properties::new();
    // SAFETY: the name is NUL-terminated and the properties buffer is as large as its
    // `Wnode.BufferSize` says; both outlive the call.
    let _ = unsafe {
        ControlTraceW(
            CONTROLTRACE_HANDLE::default(),
            PCWSTR(name.as_ptr()),
            &mut properties.base,
            EVENT_TRACE_CONTROL_STOP,
        )
    };
}

/// Event IDs that a provider's filter lets through, laid out as `EVENT_FILTER_EVENT_ID`.
#[repr(C)]
struct EventIds<const N: usize> {
    filter_in: u8,
    reserved: u8,
    count: u16,
    events: [u16; N],
}

/// Enables the events `ids` of `provider` in the session.
fn enable<const N: usize>(
    session: CONTROLTRACE_HANDLE,
    provider: &GUID,
    ids: [u16; N],
) -> Result<(), CollectorError> {
    let ids = EventIds {
        filter_in: 1,
        reserved: 0,
        count: N as u16,
        events: ids,
    };
    let mut filter = EVENT_FILTER_DESCRIPTOR {
        Ptr: &raw const ids as u64,
        Size: size_of_val(&ids) as u32,
        Type: EVENT_FILTER_TYPE_EVENT_ID,
    };
    let parameters = ENABLE_TRACE_PARAMETERS {
        Version: ENABLE_TRACE_PARAMETERS_VERSION_2,
        EnableFilterDesc: &mut filter,
        FilterDescCount: 1,
        ..Default::default()
    };
    // SAFETY: the provider GUID, the parameters and the filter they point to outlive the
    // call, which copies them; the session handle came from StartTraceW.
    let status = unsafe {
        EnableTraceEx2(
            session,
            provider,
            EVENT_CONTROL_CODE_ENABLE_PROVIDER.0,
            TRACE_LEVEL_INFORMATION as u8,
            u64::MAX,
            0,
            0,
            Some(&parameters),
        )
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(failed("EnableTraceEx2", status))
    }
}

/// Receives each event of the session on the consumer thread.
unsafe extern "system" fn on_event(record: *mut EVENT_RECORD) {
    // SAFETY: ETW passes a record that is valid for the duration of the call; its user
    // context is the `Shared` that the consumer thread keeps alive while ProcessTrace runs.
    let (record, shared) = unsafe { (&*record, &*(*record).UserContext.cast::<Shared>()) };
    let header = &record.EventHeader;
    let id = header.EventDescriptor.Id;
    let api = if header.ProviderId == DXGI && DXGI_PRESENTS.contains(&id) {
        Api::Dxgi
    } else if header.ProviderId == D3D9 && D3D9_PRESENTS.contains(&id) {
        Api::D3d9
    } else {
        return;
    };
    // Every Present event starts with the swap chain pointer of the presenting process.
    let pointer = if u32::from(header.Flags) & EVENT_HEADER_FLAG_32_BIT_HEADER != 0 {
        4
    } else {
        8
    };
    if record.UserData.is_null() || usize::from(record.UserDataLength) < pointer {
        return;
    }
    // SAFETY: the user data holds at least `pointer` bytes, checked above.
    let data = unsafe { slice::from_raw_parts(record.UserData.cast::<u8>(), pointer) };
    let mut bytes = [0_u8; 8];
    bytes[..pointer].copy_from_slice(data);
    let present = Present {
        pid: header.ProcessId,
        swap_chain: u64::from_le_bytes(bytes),
        api,
        at: header.TimeStamp,
    };
    // Unwinding out of the callback would abort, so a poisoned lock is used as it is.
    let mut presents = shared
        .presents
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if presents.len() < BACKLOG {
        presents.push(present);
    }
}

fn ticks() -> i64 {
    let mut now = 0;
    // SAFETY: writes the counter to a valid local; it cannot fail on Windows XP and later.
    let _ = unsafe { QueryPerformanceCounter(&mut now) };
    now
}

/// Process of the foreground window.
fn foreground() -> Option<u32> {
    // SAFETY: plain calls; a null window yields no process.
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return None;
    }
    let mut pid = 0;
    // SAFETY: the window handle is only read, and `pid` is a writable local.
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    (pid != 0).then_some(pid)
}

/// Collector of the foreground application's frames. Dropping it stops the session.
pub struct Frames {
    session: CONTROLTRACE_HANDLE,
    trace: PROCESSTRACE_HANDLE,
    consumer: Option<JoinHandle<()>>,
    shared: Arc<Shared>,
    counter: Counter,
    own: u32,
    /// Executable name of the target, by process ID.
    name: Option<(u32, String)>,
}

impl Frames {
    pub fn new() -> Result<Self, CollectorError> {
        let mut name = session_name();
        let mut session = CONTROLTRACE_HANDLE::default();
        let start = |session: &mut CONTROLTRACE_HANDLE| {
            let mut properties = Properties::new();
            // SAFETY: the name is NUL-terminated and the properties buffer has room for it
            // at `LoggerNameOffset`; both outlive the call.
            unsafe { StartTraceW(session, PCWSTR(name.as_ptr()), &mut properties.base) }
        };
        let mut status = start(&mut session);
        if status == ERROR_ALREADY_EXISTS {
            // A session left behind by an instance that ended without stopping it.
            stop(&name);
            status = start(&mut session);
        }
        match status {
            ERROR_SUCCESS => {}
            ERROR_ACCESS_DENIED => return Err(CollectorError(Reason::TracingDenied)),
            status => return Err(failed("StartTraceW", status)),
        }
        let stop_session = |error| {
            stop(&name);
            error
        };
        enable(session, &DXGI, DXGI_PRESENTS).map_err(stop_session)?;
        enable(session, &D3D9, D3D9_PRESENTS).map_err(stop_session)?;
        let shared = Arc::new(Shared {
            ended: AtomicU32::new(RUNNING),
            ..Default::default()
        });
        let mut logfile = EVENT_TRACE_LOGFILEW {
            LoggerName: PWSTR(name.as_mut_ptr()),
            Anonymous1: EVENT_TRACE_LOGFILEW_0 {
                ProcessTraceMode: PROCESS_TRACE_MODE_REAL_TIME
                    | PROCESS_TRACE_MODE_EVENT_RECORD
                    | PROCESS_TRACE_MODE_RAW_TIMESTAMP,
            },
            Anonymous2: EVENT_TRACE_LOGFILEW_1 {
                EventRecordCallback: Some(on_event),
            },
            Context: Arc::as_ptr(&shared).cast_mut().cast(),
            ..Default::default()
        };
        // SAFETY: the log file structure and the session name it points to outlive the call.
        let trace = unsafe { OpenTraceW(&mut logfile) };
        if trace.Value == u64::MAX {
            // SAFETY: reads the calling thread's last error.
            let error = unsafe { GetLastError() };
            stop(&name);
            return Err(failed("OpenTraceW", error));
        }
        let context = Arc::clone(&shared);
        let consumer = thread::Builder::new()
            .name("frames-trace".into())
            .spawn(move || {
                // SAFETY: the handle came from OpenTraceW and is closed only after this
                // thread is joined; `context`, which the callback reads, lives until the
                // call returns.
                let status = unsafe { ProcessTrace(&[trace], None, None) };
                context.ended.store(status.0, Ordering::Release);
            });
        let consumer = match consumer {
            Ok(consumer) => consumer,
            Err(error) => {
                stop(&name);
                // SAFETY: the handle came from OpenTraceW and no thread processes it.
                let _ = unsafe { CloseTrace(trace) };
                return Err(CollectorError(Reason::CallFailed {
                    api: "CreateThread",
                    code: error.raw_os_error().unwrap_or_default() as u32,
                }));
            }
        };
        let mut frequency = 0;
        // SAFETY: writes the frequency to a valid local.
        let _ = unsafe { QueryPerformanceFrequency(&mut frequency) };
        Ok(Self {
            session,
            trace,
            consumer: Some(consumer),
            shared,
            counter: Counter::new(frequency),
            own: std::process::id(),
            name: None,
        })
    }

    /// Executable file name of a process, read once per target.
    fn target_name(&mut self, pid: u32) -> String {
        if let Some((known, name)) = &self.name
            && *known == pid
        {
            return name.clone();
        }
        let name = process::image_name(pid).unwrap_or_default();
        self.name = Some((pid, name.clone()));
        name
    }
}

impl Drop for Frames {
    fn drop(&mut self) {
        // SAFETY: the session handle came from StartTraceW; stopping it ends ProcessTrace,
        // and closing the trace handle ends it as well should the session be gone already.
        unsafe {
            let mut properties = Properties::new();
            let _ = ControlTraceW(
                self.session,
                PCWSTR::null(),
                &mut properties.base,
                EVENT_TRACE_CONTROL_STOP,
            );
            let _ = CloseTrace(self.trace);
        }
        if let Some(consumer) = self.consumer.take() {
            let _ = consumer.join();
        }
    }
}

impl Collector for Frames {
    fn collect(&mut self, context: &CollectContext) -> Result<SampleBatch, CollectorError> {
        let ended = self.shared.ended.load(Ordering::Acquire);
        if ended != RUNNING {
            // The session was stopped from outside; the next collector starts a new one.
            return Err(failed("ProcessTrace", WIN32_ERROR(ended)));
        }
        let presents = take(
            &mut *self
                .shared
                .presents
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()),
        );
        let reading = self
            .counter
            .read(&presents, foreground(), self.own, ticks(), context.period);
        let at = Instant::now();
        let mut device = DeviceDescriptor::new(DEVICE, "", DeviceKind::Frames);
        if let Some(pid) = self.counter.target {
            device.name = self.target_name(pid);
            device = device.detail("process_id", pid.to_string());
            if let Some((_, api, _)) = self.counter.chain {
                device = device.detail("present_api", api.token());
            }
        }
        let values = match reading {
            Reading::Frames { fps, mean, longest } => Some([
                SampleValue::Gauge(fps),
                SampleValue::Gauge(mean),
                SampleValue::Gauge(longest),
            ]),
            Reading::Idle => Some([
                SampleValue::Gauge(0.0),
                SampleValue::Unsupported(Reason::NoFrames),
                SampleValue::Unsupported(Reason::NoFrames),
            ]),
            Reading::NoFrames => Some([
                SampleValue::Unsupported(Reason::NoFrames),
                SampleValue::Unsupported(Reason::NoFrames),
                SampleValue::Unsupported(Reason::NoFrames),
            ]),
            Reading::Switched => Some([
                SampleValue::WarmingUp,
                SampleValue::WarmingUp,
                SampleValue::WarmingUp,
            ]),
            Reading::Pending => None,
        };
        let mut batch = SampleBatch {
            devices: vec![device],
            complete_inventory: true,
            started_at: at,
            ..Default::default()
        };
        for ((metric, unit), value) in [
            ("fps", Unit::FramesPerSecond),
            ("frame_time", Unit::Milliseconds),
            ("frame_time_max", Unit::Milliseconds),
        ]
        .into_iter()
        .zip(values.into_iter().flatten())
        {
            batch
                .samples
                .push(MetricSample::new(DEVICE, metric, unit, value, at));
        }
        batch.finished_at = Instant::now();
        Ok(batch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One tick per microsecond.
    const FREQUENCY: i64 = 1_000_000;
    const OWN: u32 = 1;
    const GAME: u32 = 7;

    fn present(pid: u32, swap_chain: u64, ms: i64) -> Present {
        Present {
            pid,
            swap_chain,
            api: Api::Dxgi,
            at: ms * 1000,
        }
    }

    fn frames(reading: Reading) -> (f64, f64, f64) {
        match reading {
            Reading::Frames { fps, mean, longest } => (fps, mean, longest),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn counts_the_busiest_swap_chain_of_the_foreground_process() {
        let period = Duration::from_secs(1);
        let mut counter = Counter::new(FREQUENCY);
        assert_eq!(
            counter.read(&[], Some(OWN), OWN, 0, period),
            Reading::NoFrames
        );
        let first = [present(GAME, 0xA, 0), present(OWN, 0xB, 1)];
        assert_eq!(
            counter.read(&first, Some(GAME), OWN, 5000, period),
            Reading::Switched
        );
        // 10 ms frames on the game's chain, a slow overlay chain and our own frames.
        let mut presents: Vec<_> = (1..=10).map(|n| present(GAME, 0xA, n * 10)).collect();
        presents.push(present(GAME, 0xC, 50));
        presents.push(present(OWN, 0xB, 20));
        // The fifth frame arrives 15 ms late: 40 ms, 65 ms and 60 ms in present order.
        presents[4].at += 15_000;
        let (fps, mean, longest) =
            frames(counter.read(&presents, Some(GAME), OWN, 110_000, period));
        assert!((mean - 10.0).abs() < 1e-9);
        assert!((fps - 100.0).abs() < 1e-9);
        assert!((longest - 20.0).abs() < 1e-9);
        assert_eq!(counter.chain.map(|c| c.0), Some(0xA));
        // Looking at this application keeps the game as the target.
        let next = [present(GAME, 0xA, 120), present(GAME, 0xA, 140)];
        let (fps, ..) = frames(counter.read(&next, Some(OWN), OWN, 150_000, period));
        assert!((fps - 50.0).abs() < 1e-9);
        assert_eq!(counter.target, Some(GAME));
    }

    #[test]
    fn a_pause_in_presenting_reads_as_idle_once_late_frames_are_ruled_out() {
        let period = Duration::from_millis(500);
        let mut counter = Counter::new(FREQUENCY);
        counter.read(&[], Some(GAME), OWN, 0, period);
        assert_eq!(
            counter.read(&[], Some(GAME), OWN, 1_000, period),
            Reading::NoFrames
        );
        assert_eq!(
            counter.read(&[present(GAME, 0xA, 10)], Some(GAME), OWN, 20_000, period),
            Reading::Pending
        );
        // A second for the pause and a second for buffers still on their way.
        assert_eq!(
            counter.read(&[], Some(GAME), OWN, 1_900_000, period),
            Reading::Pending
        );
        assert_eq!(
            counter.read(&[], Some(GAME), OWN, 2_100_000, period),
            Reading::Idle
        );
    }

    /// Without administrator rights the session reports the missing rights; with them it
    /// starts and reads.
    #[test]
    fn session_starts_or_reports_missing_rights() {
        match Frames::new() {
            Err(CollectorError(reason)) => assert_eq!(reason, Reason::TracingDenied),
            Ok(mut frames) => {
                let batch = frames
                    .collect(&CollectContext {
                        period: Duration::from_secs(1),
                    })
                    .unwrap();
                assert_eq!(batch.devices[0].id, DEVICE);
            }
        }
    }
}
