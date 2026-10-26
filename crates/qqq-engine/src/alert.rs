//! Alerts. A rule watches one metric of one device and fires once the reading has stayed
//! beyond its threshold for the hold time; it clears once the reading has stayed back on
//! the normal side for the same time. Rules work on normalized readings, so stale, failed
//! and offline readings never count as values. A fired alert reaches the user as a system
//! notification, shown on a thread of its own.

use crate::{AppConfig, MonitorView, Notice, RemoteHost, runtime::Control};
use crossbeam_channel::{Receiver, Sender, TrySendError};
use qqq_domain::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

/// Most alert rules the configuration holds.
pub const MAX_ALERT_RULES: usize = 64;
/// Longest hold time of a rule, in seconds.
pub const MAX_ALERT_HOLD_SECS: u64 = 3600;
/// Shortest time between two notifications of the same rule.
const QUIET: Duration = Duration::from_secs(60);
/// Fired alerts the view keeps, newest first.
const RECENT: usize = 20;

/// Side of the threshold on which a rule fires.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AlertCondition {
    Above,
    Below,
}

/// A metric of a device to watch, and when to alert.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AlertRule {
    /// Stable identity: letters, digits, `-` and `_`, at most 32 characters.
    pub id: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    /// The device's identity across sessions: its persistent ID when it has one, such as
    /// the interface GUID of a network adapter, else its device ID.
    pub device: String,
    /// Metric name, such as `usage`.
    pub metric: String,
    pub condition: AlertCondition,
    /// Threshold in the metric's unit: percent, bytes, bytes per second, frames per second
    /// or milliseconds.
    pub threshold: f64,
    /// Seconds the reading stays beyond the threshold before the rule fires, and back on
    /// the normal side before it clears.
    #[serde(default)]
    pub hold_secs: u64,
}

fn enabled() -> bool {
    true
}

impl AlertRule {
    pub fn validate(&self) -> Result<(), Notice> {
        let text = |value: &str, limit: usize| {
            !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
        };
        let valid = !self.id.is_empty()
            && self.id.len() <= 32
            && self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            && text(&self.device, 256)
            && text(&self.metric, 64)
            && self.threshold.is_finite()
            && self.hold_secs <= MAX_ALERT_HOLD_SECS;
        if valid {
            Ok(())
        } else {
            Err(Notice::InvalidAlertRule)
        }
    }

    /// Whether `value` lies beyond the threshold.
    pub fn exceeded(&self, value: f64) -> bool {
        match self.condition {
            AlertCondition::Above => value > self.threshold,
            AlertCondition::Below => value < self.threshold,
        }
    }

    fn hold(&self) -> Duration {
        Duration::from_secs(self.hold_secs)
    }
}

/// A rule that fired, with what the notification shows.
#[derive(Clone, Debug)]
pub struct AlertEvent {
    pub rule: AlertRule,
    pub device: DeviceDescriptor,
    /// Display name of the remote host the device belongs to.
    pub host: Option<String>,
    pub unit: Unit,
    pub value: f64,
    pub at: Instant,
}

/// Where a rule stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertState {
    /// The device or a current reading of the metric is missing.
    NoData,
    Normal,
    /// The reading is beyond the threshold and the rule fires once the hold time passes.
    Pending {
        since: Instant,
    },
    Firing {
        since: Instant,
    },
}

/// Alerts for the GUI.
#[derive(Clone, Debug, Default)]
pub struct AlertView {
    /// State of every enabled rule by ID.
    pub states: BTreeMap<String, AlertState>,
    /// Latest fired alerts, newest first.
    pub recent: Arc<Vec<AlertEvent>>,
}

/// Shows the notification of a fired alert.
pub type AlertNotify = Arc<dyn Fn(&AlertEvent) + Send + Sync>;

/// Progress of one rule.
struct Tracker {
    /// Definition the progress belongs to; an edited rule starts over.
    rule: AlertRule,
    /// Resolved device and metric of the current device directory.
    key: Option<MetricKey>,
    /// Sample time of the last reading weighed.
    sample: Option<Instant>,
    /// Since when the reading is beyond the threshold.
    beyond: Option<Instant>,
    /// Since when a firing rule's reading is back on the normal side.
    back: Option<Instant>,
    firing: Option<Instant>,
    notified: Option<Instant>,
}

impl Tracker {
    fn new(rule: AlertRule) -> Self {
        Self {
            rule,
            key: None,
            sample: None,
            beyond: None,
            back: None,
            firing: None,
            notified: None,
        }
    }

    /// Weighs a reading taken at `at`; returns whether the rule should notify now.
    fn weigh(&mut self, value: f64, at: Instant) -> bool {
        if self.sample == Some(at) {
            return false;
        }
        self.sample = Some(at);
        let hold = self.rule.hold();
        if self.rule.exceeded(value) {
            self.back = None;
            let since = *self.beyond.get_or_insert(at);
            if self.firing.is_none() && at.saturating_duration_since(since) >= hold {
                self.firing = Some(at);
                if self
                    .notified
                    .is_none_or(|last| at.saturating_duration_since(last) >= QUIET)
                {
                    self.notified = Some(at);
                    return true;
                }
            }
        } else {
            self.beyond = None;
            if self.firing.is_some() {
                let since = *self.back.get_or_insert(at);
                if at.saturating_duration_since(since) >= hold {
                    self.firing = None;
                    self.back = None;
                }
            }
        }
        false
    }

    /// Without a current reading the hold times start over; a firing rule stays firing
    /// until a reading clears it.
    fn interrupt(&mut self) {
        self.beyond = None;
        self.back = None;
    }

    fn state(&self, current: bool) -> AlertState {
        match (current, self.firing, self.beyond) {
            (false, _, _) => AlertState::NoData,
            (true, Some(since), _) => AlertState::Firing { since },
            (true, None, Some(since)) => AlertState::Pending { since },
            (true, None, None) => AlertState::Normal,
        }
    }
}

/// The device a rule watches, preferring one that is online.
pub fn alert_device<'a>(view: &'a MonitorView, rule: &AlertRule) -> Option<&'a DeviceDescriptor> {
    view.devices
        .values()
        .filter(|d| d.identity() == rule.device || d.id == rule.device)
        .max_by_key(|d| d.online)
}

/// Rule progress, owned by the engine thread.
pub(crate) struct Alerts {
    trackers: BTreeMap<String, Tracker>,
    recent: Vec<AlertEvent>,
    notify: Sender<AlertEvent>,
    /// Rules resolved by metric key, refreshed when their definitions or devices change.
    targets: BTreeMap<MetricKey, Vec<String>>,
    devices: Arc<BTreeMap<String, DeviceDescriptor>>,
}

impl Alerts {
    pub(crate) fn new(notify: Sender<AlertEvent>) -> Self {
        Self {
            trackers: BTreeMap::new(),
            recent: Vec::new(),
            notify,
            targets: BTreeMap::new(),
            devices: Arc::default(),
        }
    }

    /// Resolves enabled rules once per directory or rule change. Samples use this index
    /// rather than scanning the device directory or unrelated rules.
    pub(crate) fn prepare(&mut self, view: &MonitorView, config: &AppConfig) {
        let rules = config.alerts.iter().filter(|rule| rule.enabled);
        let changed = rules.clone().count() != self.trackers.len()
            || rules.clone().any(|rule| {
                self.trackers
                    .get(&rule.id)
                    .is_none_or(|tracker| tracker.rule != *rule)
            });
        if changed {
            self.trackers.retain(|id, tracker| {
                rules
                    .clone()
                    .any(|rule| rule.id == *id && *rule == tracker.rule)
            });
            for rule in rules {
                self.trackers
                    .entry(rule.id.clone())
                    .or_insert_with(|| Tracker::new(rule.clone()));
            }
        }
        if changed || !Arc::ptr_eq(&self.devices, &view.devices) {
            self.devices = Arc::clone(&view.devices);
            self.targets.clear();
            for (id, tracker) in &mut self.trackers {
                tracker.key = alert_device(view, &tracker.rule)
                    .map(|device| MetricKey::new(&device.id, &tracker.rule.metric));
                if let Some(key) = &tracker.key {
                    self.targets
                        .entry(key.clone())
                        .or_default()
                        .push(id.clone());
                }
            }
        }
    }

    /// Starts the hold time over before a new valid sample can hide a missing interval.
    pub(crate) fn interrupt(&mut self, key: &MetricKey) {
        if let Some(rules) = self.targets.get(key) {
            for id in rules {
                self.trackers.get_mut(id).unwrap().interrupt();
            }
        }
    }

    /// Advances only the rules indexed for this normalized sample. `prepare` runs after
    /// inventory updates and before a batch's samples are applied.
    pub(crate) fn evaluate_sample(
        &mut self,
        view: &mut MonitorView,
        config: &AppConfig,
        key: &MetricKey,
    ) {
        let Some(rules) = self.targets.get(key) else {
            return;
        };
        let device = view.devices.get(&key.device);
        let reading = view.metrics.get(key);
        let mut states = Vec::with_capacity(rules.len());
        let mut fired = Vec::new();
        for id in rules {
            let tracker = self.trackers.get_mut(id).unwrap();
            let (state, event) = advance(tracker, device, reading, view.now, config);
            states.push((id.clone(), state));
            fired.extend(event);
        }
        view.alerts.states.extend(states);
        self.record(view, fired);
    }

    /// Updates rule visibility and expiry, including rules with no device or reading.
    pub(crate) fn evaluate(&mut self, view: &mut MonitorView, config: &AppConfig) {
        self.prepare(view, config);
        let mut states = BTreeMap::new();
        let mut fired = Vec::new();
        for (id, tracker) in &mut self.trackers {
            let device = tracker
                .key
                .as_ref()
                .and_then(|key| view.devices.get(&key.device));
            let reading = tracker.key.as_ref().and_then(|key| view.metrics.get(key));
            let (state, event) = advance(tracker, device, reading, view.now, config);
            states.insert(id.clone(), state);
            fired.extend(event);
        }
        view.alerts.states = states;
        self.record(view, fired);
    }

    fn record(&mut self, view: &mut MonitorView, fired: Vec<AlertEvent>) {
        if fired.is_empty() {
            return;
        }
        for event in fired {
            match self.notify.try_send(event.clone()) {
                Ok(()) | Err(TrySendError::Disconnected(_)) => {}
                Err(TrySendError::Full(_)) => {
                    tracing::warn!(rule = event.rule.id, "通知队列已满，跳过本次告警通知")
                }
            }
            self.recent.insert(0, event);
        }
        self.recent.truncate(RECENT);
        view.alerts.recent = Arc::new(self.recent.clone());
    }
}

/// Applies one rule's timing and formats a notification when it first fires.
fn advance(
    tracker: &mut Tracker,
    device: Option<&DeviceDescriptor>,
    reading: Option<&MetricReading>,
    now: Instant,
    config: &AppConfig,
) -> (AlertState, Option<AlertEvent>) {
    let current = device
        .filter(|device| device.online)
        .and(reading)
        .and_then(|reading| {
            (reading.effective_state(now) == SampleState::Ready)
                .then(|| reading.value.zip(reading.last_success))
                .flatten()
        });
    let event = match (current, device, reading) {
        (Some((value, at)), Some(device), Some(reading)) => {
            tracker.weigh(value, at).then(|| AlertEvent {
                host: device.host.as_ref().and_then(|host| {
                    config
                        .remote_hosts
                        .iter()
                        .find(|entry| entry.id == *host)
                        .map(|entry| RemoteHost::label(entry).to_owned())
                }),
                device: device.clone(),
                unit: reading.descriptor.unit,
                value,
                at,
                rule: tracker.rule.clone(),
            })
        }
        _ => {
            tracker.interrupt();
            None
        }
    };
    (tracker.state(current.is_some()), event)
}

/// Shows the notifications of fired alerts until the runtime stops.
pub(crate) fn notify_worker(
    notify: AlertNotify,
    events: Receiver<AlertEvent>,
    control: Arc<Control>,
) {
    // The engine holds the sender, so the loop ends once the engine has stopped.
    for event in events {
        if control.stop.load(Ordering::Acquire) {
            break;
        }
        if catch_unwind(AssertUnwindSafe(|| notify(&event))).is_err() {
            tracing::error!(rule = event.rule.id, "显示告警通知时发生异常");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(condition: AlertCondition, threshold: f64, hold_secs: u64) -> AlertRule {
        AlertRule {
            id: "cpu".into(),
            enabled: true,
            device: "cpu".into(),
            metric: "usage".into(),
            condition,
            threshold,
            hold_secs,
        }
    }

    #[test]
    fn fires_after_the_hold_time_and_clears_after_the_same_time_back() {
        let start = Instant::now();
        let at = |secs: u64| start + Duration::from_secs(secs);
        let mut tracker = Tracker::new(rule(AlertCondition::Above, 90.0, 10));
        assert!(!tracker.weigh(95.0, at(0)));
        assert_eq!(tracker.state(true), AlertState::Pending { since: at(0) });
        // A dip below the threshold starts the hold time over.
        assert!(!tracker.weigh(80.0, at(5)));
        assert_eq!(tracker.state(true), AlertState::Normal);
        assert!(!tracker.weigh(95.0, at(6)));
        assert!(!tracker.weigh(95.0, at(15)));
        assert!(tracker.weigh(96.0, at(16)));
        assert_eq!(tracker.state(true), AlertState::Firing { since: at(16) });
        // The same reading weighed twice counts once.
        assert!(!tracker.weigh(96.0, at(16)));
        assert!(!tracker.weigh(50.0, at(20)));
        assert!(!tracker.weigh(50.0, at(29)));
        assert!(matches!(tracker.state(true), AlertState::Firing { .. }));
        assert!(!tracker.weigh(50.0, at(30)));
        assert_eq!(tracker.state(true), AlertState::Normal);
    }

    #[test]
    fn notifies_at_most_once_a_minute_and_waits_out_missing_data() {
        let start = Instant::now();
        let at = |secs: u64| start + Duration::from_secs(secs);
        let mut tracker = Tracker::new(rule(AlertCondition::Below, 30.0, 0));
        assert!(!tracker.weigh(30.0, at(0)));
        assert!(tracker.weigh(29.0, at(1)));
        assert!(!tracker.weigh(60.0, at(2)));
        assert_eq!(tracker.state(true), AlertState::Normal);
        // Fires again within the quiet minute without a second notification.
        assert!(!tracker.weigh(10.0, at(3)));
        assert!(matches!(tracker.state(true), AlertState::Firing { .. }));
        assert!(!tracker.weigh(60.0, at(4)));
        tracker.interrupt();
        assert_eq!(tracker.state(false), AlertState::NoData);
        assert!(tracker.weigh(10.0, at(62)));
    }

    #[test]
    fn rules_validate_their_fields() {
        assert!(rule(AlertCondition::Above, 90.0, 10).validate().is_ok());
        for broken in [
            AlertRule {
                id: "bad id".into(),
                ..rule(AlertCondition::Above, 90.0, 10)
            },
            AlertRule {
                metric: String::new(),
                ..rule(AlertCondition::Above, 90.0, 10)
            },
            rule(AlertCondition::Above, f64::NAN, 10),
            rule(AlertCondition::Above, 90.0, MAX_ALERT_HOLD_SECS + 1),
        ] {
            assert_eq!(broken.validate(), Err(Notice::InvalidAlertRule));
        }
    }
}
