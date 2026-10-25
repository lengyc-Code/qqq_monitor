use std::{
    borrow::Borrow,
    cmp::Ordering,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MetricKey {
    pub device: String,
    pub metric: String,
}

impl MetricKey {
    pub fn new(device: impl Into<String>, metric: impl Into<String>) -> Self {
        Self {
            device: device.into(),
            metric: metric.into(),
        }
    }
}

/// A device and a metric name, owned as in [`MetricKey`] or borrowed as a pair of `&str`.
/// Maps keyed by [`MetricKey`] find entries by borrowed names, without allocating a key:
/// `map.get(&("cpu", "usage") as &dyn MetricKeyView)`.
pub trait MetricKeyView {
    fn device(&self) -> &str;
    fn metric(&self) -> &str;
}

impl MetricKeyView for MetricKey {
    fn device(&self) -> &str {
        &self.device
    }
    fn metric(&self) -> &str {
        &self.metric
    }
}

impl MetricKeyView for (&str, &str) {
    fn device(&self) -> &str {
        self.0
    }
    fn metric(&self) -> &str {
        self.1
    }
}

impl<'a> Borrow<dyn MetricKeyView + 'a> for MetricKey {
    fn borrow(&self) -> &(dyn MetricKeyView + 'a) {
        self
    }
}

// Ordered like the derived order of `MetricKey`: by device, then by metric.
impl Ord for dyn MetricKeyView + '_ {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.device(), self.metric()).cmp(&(other.device(), other.metric()))
    }
}

impl PartialOrd for dyn MetricKeyView + '_ {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for dyn MetricKeyView + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for dyn MetricKeyView + '_ {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Percent,
    Bytes,
    BytesPerSecond,
    Seconds,
    /// A plain number, such as the load average.
    Count,
    /// Frames an application presents per second.
    FramesPerSecond,
    Milliseconds,
}

#[derive(Clone, Debug)]
pub struct MetricDescriptor {
    pub key: MetricKey,
    pub unit: Unit,
}

#[derive(Clone, Debug)]
pub enum SampleValue {
    Gauge(f64),
    Counter(u64),
    WarmingUp,
    Unsupported(Reason),
    Failed(Reason),
}

/// Why a reading or a collector has no valid data. Reasons are language-neutral; the
/// GUI phrases them in the interface language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The system provides no performance counters for the graphics card.
    NoGpuCounters,
    /// Graphics memory counters are unavailable.
    NoGpuMemoryCounters,
    /// Volume capacity could not be read this time.
    VolumeUnavailable,
    /// The system reports no physical memory total.
    NoMemoryTotal,
    /// The value is negative, not finite, or a percentage above 100.
    OutOfRange,
    /// A cumulative counter arrived for a metric that is not a rate.
    CounterUnit,
    /// The device is no longer present or connected.
    Disconnected,
    /// The collector panicked and is being recreated.
    Panicked,
    /// Another collector already reports the same metric key.
    KeyConflict,
    /// Counter instances changed between reads too often to read them.
    InstancesChanging,
    /// A system call failed with a status code.
    CallFailed { api: &'static str, code: u32 },
    /// A system call returned data that failed validation.
    BadData { api: &'static str },
    /// The process has exited, or its PID now belongs to another process.
    ProcessGone,
    /// The system denied access to the process.
    AccessDenied,
    /// The WSL distribution is no longer running.
    HostStopped,
    /// The remote host's name does not resolve to an address.
    AddressNotFound,
    /// The remote host refused the connection.
    ConnectionRefused,
    /// The remote host did not answer in time.
    TimedOut,
    /// The connection to the remote host closed or broke.
    ConnectionLost,
    /// The SSH handshake failed, for example without an algorithm both sides support.
    Handshake,
    /// No trusted host key is known for the remote host yet. `key` is the offered key as
    /// an OpenSSH public key line, `fingerprint` its SHA-256 fingerprint.
    HostKeyUnknown { key: String, fingerprint: String },
    /// The remote host offered a key other than the trusted one.
    HostKeyChanged { key: String, fingerprint: String },
    /// The remote host rejected the sign-in.
    AuthFailed,
    /// No password is saved for the remote host.
    NoPassword,
    /// The private key file is missing, unreadable or not a private key.
    KeyUnreadable,
    /// The private key is encrypted, and the saved passphrase is missing or wrong.
    KeyPassphrase,
    /// No SSH agent is running, or it holds no keys.
    NoAgent,
    /// The remote host has no Linux `/proc` to read.
    NotLinux,
    /// Reading the system's event tracing needs administrator rights or membership in the
    /// Performance Log Users group.
    TracingDenied,
    /// The foreground application has presented no Direct3D frames yet.
    NoFrames,
    /// Notifications of the application are switched off in the system settings.
    NotificationsOff,
    /// The user declined the request, such as the prompt for administrator rights.
    Declined,
    /// Another application already uses the keyboard shortcut.
    HotkeyTaken,
    /// Another application keeps the clipboard open.
    ClipboardBusy,
    /// The file or folder does not exist.
    PathNotFound,
}

#[derive(Clone, Debug)]
pub struct MetricSample {
    pub descriptor: MetricDescriptor,
    pub value: SampleValue,
    pub at: Instant,
}

impl MetricSample {
    pub fn new(device: &str, metric: &str, unit: Unit, value: SampleValue, at: Instant) -> Self {
        Self {
            descriptor: MetricDescriptor {
                key: MetricKey::new(device, metric),
                unit,
            },
            value,
            at,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleState {
    WarmingUp,
    Ready,
    Stale,
    Unsupported,
    Failed,
    Offline,
}

#[derive(Clone, Debug)]
pub struct MetricReading {
    pub descriptor: MetricDescriptor,
    pub value: Option<f64>,
    pub session_bytes: u64,
    pub state: SampleState,
    pub reason: Option<Reason>,
    pub last_success: Option<Instant>,
    pub last_attempt: Instant,
    pub period: Duration,
}

impl MetricReading {
    pub fn effective_state(&self, now: Instant) -> SampleState {
        if self.state == SampleState::Ready
            && self.last_success.is_some_and(|at| {
                now.saturating_duration_since(at) > (self.period * 3).max(Duration::from_secs(3))
            })
        {
            SampleState::Stale
        } else {
            self.state
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn readings_are_found_by_borrowed_names() {
        let keys = [("cpu", "usage"), ("core:1", "usage"), ("cpu", "iowait")];
        let map: BTreeMap<MetricKey, usize> = keys
            .iter()
            .enumerate()
            .map(|(i, (device, metric))| (MetricKey::new(*device, *metric), i))
            .collect();
        for (i, key) in keys.iter().enumerate() {
            assert_eq!(map.get(key as &dyn MetricKeyView), Some(&i));
        }
        assert_eq!(map.get(&("cpu", "missing") as &dyn MetricKeyView), None);
    }
}
