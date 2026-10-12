use std::time::{Duration, Instant};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Percent,
    Bytes,
    BytesPerSecond,
    Seconds,
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
