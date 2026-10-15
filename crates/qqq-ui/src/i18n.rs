//! Interface text in every supported language. The active language is process-wide:
//! [`set`] switches it and [`t`] returns its strings. Each language fills the whole
//! [`Strings`] table, so a missing translation is a compile error.

mod en;
mod ja;
mod zh;

use crate::components::{alert_device_name, format_value, span};
use qqq_domain::{Priority, Reason};
use qqq_engine::{AlertCondition, AlertEvent, Language, Notice, SaveStatus};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Navigation label, heading and subtitle of a page.
pub struct PageText {
    pub nav: &'static str,
    pub title: &'static str,
    pub hint: &'static str,
}

/// Every interface string of one language. Functions phrase values, such as a duration
/// or a reason, with the word order of the language.
pub struct Strings {
    pub window_title: &'static str,
    pub tagline: &'static str,
    /// Title of the dialog shown when the application cannot start.
    pub startup_failed: &'static str,
    pub overview_page: PageText,
    pub cpu_page: PageText,
    pub memory_page: PageText,
    pub gpu_page: PageText,
    pub frames_page: PageText,
    pub disk_page: PageText,
    pub network_page: PageText,
    pub processes_page: PageText,
    pub remote_page: PageText,
    pub alerts_page: PageText,
    pub settings_page: PageText,

    // Header and status bar.
    pub history_range: &'static str,
    pub seconds: fn(f64) -> String,
    pub minutes: fn(u64) -> String,
    pub running: &'static str,
    pub degraded: &'static str,
    /// Sampling interval and session length, both in seconds.
    pub timing: fn(f64, f64) -> String,
    pub minibar: &'static str,
    /// Number of firing alert rules.
    pub alerts_firing: fn(usize) -> String,
    pub dock_top: &'static str,
    pub dock_left: &'static str,
    pub dock_right: &'static str,
    pub restore_window: &'static str,

    // Notification-area icon menu.
    pub tray_open: &'static str,
    pub tray_exit: &'static str,

    // Readings and shared components.
    pub days: fn(u64) -> String,
    pub state_ready: &'static str,
    pub state_stale: &'static str,
    pub state_failed: &'static str,
    pub state_unsupported: &'static str,
    pub state_offline: &'static str,
    pub state_waiting: &'static str,
    pub seconds_ago: fn(f64) -> String,
    pub minutes_ago: fn(u64) -> String,
    pub last_valid: &'static str,
    pub now: &'static str,
    pub chart_waiting: &'static str,
    pub device_offline: &'static str,
    pub offline_suffix: &'static str,
    pub finding_devices: &'static str,
    pub waiting_samples: &'static str,
    pub click_for_details: &'static str,
    pub list_separator: &'static str,
    /// A label followed by its text, such as a collector and its error.
    pub labeled: fn(&str, &str) -> String,
    /// Used and total amounts.
    pub used_of: fn(&str, &str) -> String,

    // Devices.
    pub processor: &'static str,
    pub physical_memory: &'static str,
    pub this_pc: &'static str,
    pub physical_disk: &'static str,
    pub graphics_card: &'static str,
    /// The application in the foreground, whose frames are counted.
    pub foreground_app: &'static str,
    pub adapter: &'static str,
    pub logical_core: fn(usize) -> String,
    /// Label of a device detail key such as `volume_label`.
    pub detail: fn(&str) -> Option<&'static str>,
    pub physical_adapter: &'static str,
    pub virtual_adapter: &'static str,
    pub dedicated_memory: &'static str,
    pub shared_memory: &'static str,
    pub reason: fn(&Reason) -> String,
    pub notice: fn(&Notice) -> String,
    pub save_status: fn(&SaveStatus) -> String,

    // Metric names.
    pub memory: &'static str,
    pub gpu_memory: &'static str,
    /// Memory and graphics memory in the narrow gauges of the mini bar.
    pub memory_short: &'static str,
    pub gpu_memory_short: &'static str,
    pub read: &'static str,
    pub write: &'static str,
    pub receive: &'static str,
    pub send: &'static str,
    pub logical_cores: &'static str,
    /// Name of a metric that alert rules can watch, such as `usage`.
    pub metric: fn(&str) -> Option<&'static str>,

    // Mascot.
    pub mint_name: &'static str,
    pub mint_hover: &'static str,
    pub mint_waiting: &'static str,
    pub mint_trouble: &'static str,
    /// Speech for a CPU load at or above 85 %.
    pub mint_cpu_hot: fn(f64) -> String,
    /// Speech for a memory load at or above 90 %.
    pub mint_memory_full: fn(f64) -> String,
    /// Speech for moderate CPU and memory load.
    pub mint_busy: fn(f64, f64) -> String,
    /// Speech for a CPU load below 15 %.
    pub mint_relaxed: fn(f64) -> String,
    pub mint_calm: fn(f64, f64) -> String,

    // Overview.
    pub cores_detail: fn(&str) -> String,
    pub disk_read: &'static str,
    pub network_receive: &'static str,
    pub cpu_memory_chart: &'static str,
    pub disk_io: &'static str,
    pub network_io: &'static str,
    pub device_profile: &'static str,
    pub total_memory: &'static str,
    pub current_disk: &'static str,
    pub current_adapter: &'static str,
    pub uptime: &'static str,

    // CPU.
    pub cpu_total: &'static str,
    pub cpu_total_detail: &'static str,
    pub cpu_busiest: &'static str,
    pub cpu_overall: &'static str,
    pub cpu_selected_core: &'static str,
    pub cpu_chart: &'static str,
    pub cpu_cores_hint: &'static str,

    // Memory.
    pub memory_in_use: &'static str,
    pub share_of_total: fn(&str) -> String,
    pub available: &'static str,
    pub allocatable: fn(&str) -> String,
    pub memory_total: &'static str,
    pub memory_total_detail: &'static str,
    pub memory_chart: &'static str,
    pub memory_usage: &'static str,
    pub memory_note: &'static str,

    // GPU.
    pub gpu_usage: &'static str,
    pub engines_waiting: &'static str,
    /// Name and load of the busiest engine.
    pub busiest_engine: fn(&str, &str) -> String,
    /// Memory type, used and total amounts.
    pub gpu_memory_used: fn(&str, &str, &str) -> String,
    pub gpu_chart: &'static str,
    pub engines_title: &'static str,
    pub engines_hint: &'static str,
    pub engines_missing: &'static str,
    pub engine: fn(&str) -> String,
    /// Name of a counter engine type such as `videodecode`, without an index.
    pub engine_kind: fn(&str) -> Option<&'static str>,
    pub gpu_memory_title: &'static str,
    pub gpu_memory_hint: &'static str,
    /// Total amount and share in use.
    pub total_share: fn(&str, &str) -> String,
    pub gpu_note: &'static str,

    // Disk.
    pub disk_read_detail: &'static str,
    pub disk_write_detail: &'static str,
    pub disk_chart: &'static str,
    pub volumes_title: &'static str,
    pub volumes_hint: &'static str,
    pub volumes_waiting: &'static str,
    /// Free and total space.
    pub volume_space: fn(&str, &str) -> String,
    pub no_label: &'static str,
    pub unknown_file_system: &'static str,

    // Network.
    pub session_received: fn(&str) -> String,
    pub session_sent: fn(&str) -> String,
    pub network_chart: &'static str,
    pub network_note: &'static str,

    // Settings.
    pub preferences: &'static str,
    pub language: &'static str,
    pub follow_system: &'static str,
    pub sampling_interval: &'static str,
    pub default_range: &'static str,
    pub animations: &'static str,
    pub animations_option: &'static str,
    pub edge_docking: &'static str,
    pub edge_docking_option: &'static str,
    pub minimize_to_tray: &'static str,
    pub minimize_to_tray_option: &'static str,
    pub autostart: &'static str,
    pub autostart_option: &'static str,
    pub autostart_to_tray_option: &'static str,
    pub frame_monitoring: &'static str,
    pub frame_monitoring_option: &'static str,
    pub apply_save: &'static str,
    pub runtime_status: &'static str,
    /// Received and dropped sample batches.
    pub batches: fn(u64, u64) -> String,
    pub history_usage: fn(&str) -> String,
    pub collectors_ok: &'static str,
    pub refresh_devices: &'static str,
    pub about_mint: &'static str,
    pub about_animations: &'static str,

    // Processes.
    pub process_name: &'static str,
    pub process_user: &'static str,
    pub process_memory: &'static str,
    pub process_threads: &'static str,
    pub process_priority: &'static str,
    pub process_command: &'static str,
    pub process_search: &'static str,
    /// Shown and total process counts.
    pub process_count: fn(usize, usize) -> String,
    pub processes_loading: &'static str,
    pub processes_none: &'static str,
    /// The latest list could not be read, with the reason.
    pub processes_failed: fn(&str) -> String,
    pub wsl_hint: &'static str,
    pub end_process: &'static str,
    pub kill_process: &'static str,
    /// Confirmation question for a process name and PID.
    pub end_confirm: fn(&str, u32) -> String,
    pub end_warning: &'static str,
    pub kill_warning: &'static str,
    pub cancel: &'static str,
    /// Priority classes from idle to realtime, in the order of `Priority`.
    pub priorities: [&'static str; 6],

    // Remote hosts.
    pub remote_add: &'static str,
    pub remote_empty: &'static str,
    pub remote_connecting: &'static str,
    pub remote_connected: &'static str,
    /// A failed connection, with the reason.
    pub remote_failed: fn(&str) -> String,
    pub remote_retrying: &'static str,
    /// First connection to a host, with the fingerprint of the key it offers.
    pub remote_key_unknown: fn(&str) -> String,
    /// A host that offers another key than the trusted one, with the new fingerprint.
    pub remote_key_changed: fn(&str) -> String,
    pub remote_trust: &'static str,
    pub remote_trust_new: &'static str,
    pub remote_edit: &'static str,
    pub remote_reconnect: &'static str,
    pub remote_remove: &'static str,
    /// Removal question for a host name.
    pub remote_remove_confirm: fn(&str) -> String,
    pub remote_remove_warning: &'static str,
    pub remote_auth_password: &'static str,
    pub remote_auth_key: &'static str,
    pub remote_auth_agent: &'static str,
    pub remote_add_title: &'static str,
    pub remote_edit_title: &'static str,
    pub remote_import: &'static str,
    pub remote_name: &'static str,
    pub remote_name_hint: &'static str,
    pub remote_address: &'static str,
    pub remote_address_hint: &'static str,
    pub remote_port: &'static str,
    pub remote_user: &'static str,
    pub remote_auth: &'static str,
    pub remote_password: &'static str,
    pub remote_password_keep: &'static str,
    pub remote_key_file: &'static str,
    pub remote_key_hint: &'static str,
    pub remote_passphrase: &'static str,
    pub remote_passphrase_hint: &'static str,
    pub remote_passphrase_keep: &'static str,
    pub remote_agent_hint: &'static str,
    pub remote_secret_note: &'static str,
    pub remote_save: &'static str,
    /// A required field left empty, by its label.
    pub remote_missing: fn(&str) -> String,
    pub remote_bad_port: &'static str,
    pub remote_hostname: &'static str,
    pub remote_load: &'static str,
    /// Share of processor time waiting for I/O, and the number of processes.
    pub remote_cpu_detail: fn(&str, &str) -> String,
    pub remote_swap: &'static str,
    pub remote_no_swap: &'static str,
    pub remote_volumes_title: &'static str,
    pub remote_volumes_hint: &'static str,
    pub remote_cores_hint: &'static str,
    pub yes: &'static str,

    // Frames.
    pub fps: &'static str,
    pub frame_time: &'static str,
    pub frame_time_max: &'static str,
    /// Mean frame time under the frame rate.
    pub fps_detail: fn(&str) -> String,
    /// Longest frame time under the mean.
    pub frame_time_detail: fn(&str) -> String,
    pub frames_target: &'static str,
    pub frames_off: &'static str,
    pub frames_denied: &'static str,
    pub frames_restart: &'static str,
    pub frames_waiting: &'static str,
    pub fps_chart: &'static str,
    pub frame_time_chart: &'static str,
    pub frames_note: &'static str,

    // Alerts.
    pub alert_add: &'static str,
    pub alert_test: &'static str,
    pub alert_test_body: &'static str,
    pub alert_empty: &'static str,
    pub alert_rules: &'static str,
    pub alert_recent: &'static str,
    pub alert_recent_empty: &'static str,
    pub alert_normal: &'static str,
    pub alert_no_data: &'static str,
    pub alert_disabled: &'static str,
    /// Beyond the threshold, with the time left until the rule fires.
    pub alert_pending: fn(&str) -> String,
    /// Firing, for how long.
    pub alert_firing: fn(&str) -> String,
    /// Current value of the watched metric.
    pub alert_now: fn(&str) -> String,
    pub alert_above: &'static str,
    pub alert_below: &'static str,
    /// Condition: above or below, the threshold, and the hold time when there is one.
    pub alert_condition: fn(bool, &str, Option<&str>) -> String,
    pub alert_hold_none: &'static str,
    pub alert_add_title: &'static str,
    pub alert_edit_title: &'static str,
    pub alert_device: &'static str,
    pub alert_metric: &'static str,
    pub alert_when: &'static str,
    pub alert_threshold: &'static str,
    pub alert_hold: &'static str,
    pub alert_enabled: &'static str,
    pub alert_pick_device: &'static str,
    pub alert_device_missing: &'static str,
    pub alert_missing: &'static str,
    pub alert_bad_threshold: &'static str,
    pub alert_save: &'static str,
    pub alert_edit: &'static str,
    pub alert_remove: &'static str,
    /// Notification title: device, metric, and whether it is above the threshold.
    pub alert_title: fn(&str, &str, bool) -> String,
    /// Notification text: value, threshold, whether above, and the hold time if any.
    pub alert_body: fn(&str, &str, bool, Option<&str>) -> String,
    pub alert_note: &'static str,

    // Screenshots.
    pub screenshots_page: PageText,
    /// Screenshot item of the notification-area icon menu.
    pub tray_screenshot: &'static str,
    pub hours_ago: fn(u64) -> String,
    pub shot_take: &'static str,
    /// Shown over the frozen screen while the user selects a region.
    pub shot_hint: &'static str,
    pub shot_folder: &'static str,
    pub shot_hotkey: &'static str,
    /// Option that registers the shortcut, named by its keys.
    pub shot_hotkey_option: fn(&str) -> String,
    pub shot_hotkey_change: &'static str,
    pub shot_hotkey_press: &'static str,
    pub shot_hotkey_invalid: &'static str,
    /// Label of the number of screenshots or clipboard entries kept.
    pub keep: &'static str,
    /// Number of screenshots.
    pub shot_count: fn(usize) -> String,
    pub shot_recent: &'static str,
    pub shot_empty: &'static str,
    pub shot_loading: &'static str,
    /// What happens to a selected region, with the folder that keeps the files.
    pub shot_note: fn(&str) -> String,
    pub copy: &'static str,
    pub open: &'static str,
    pub delete: &'static str,

    // Clipboard.
    pub clipboard_page: PageText,
    pub clip_option: &'static str,
    pub clip_off: &'static str,
    pub clip_clear: &'static str,
    /// Number of clipboard entries.
    pub clip_count: fn(usize) -> String,
    pub clip_recent: &'static str,
    pub clip_empty: &'static str,
    /// A text entry with its number of characters.
    pub clip_text: fn(usize) -> String,
    /// A picture entry with its width and height.
    pub clip_picture: fn(u32, u32) -> String,
    /// A files entry with its number of files.
    pub clip_files: fn(usize) -> String,
    /// Files beyond those listed.
    pub clip_more: fn(usize) -> String,
    pub clip_note: &'static str,
}

/// Index of the active language in [`Language::ALL`].
static ACTIVE: AtomicUsize = AtomicUsize::new(0);

pub fn set(language: Language) {
    let index = Language::ALL.iter().position(|l| *l == language);
    ACTIVE.store(index.unwrap_or(0), Ordering::Relaxed);
}

pub fn language() -> Language {
    Language::ALL[ACTIVE.load(Ordering::Relaxed)]
}

/// Strings of the active language.
pub fn t() -> &'static Strings {
    strings(language())
}

pub fn strings(language: Language) -> &'static Strings {
    match language {
        Language::Chinese => &zh::STRINGS,
        Language::English => &en::STRINGS,
        Language::Japanese => &ja::STRINGS,
    }
}

/// Name of a language in that language, as listed in the language setting.
pub fn native_name(language: Language) -> &'static str {
    match language {
        Language::Chinese => "简体中文",
        Language::English => "English",
        Language::Japanese => "日本語",
    }
}

pub fn reason(reason: &Reason) -> String {
    (t().reason)(reason)
}

pub fn notice(notice: &Notice) -> String {
    (t().notice)(notice)
}

/// Name of a priority class.
pub fn priority(priority: Priority) -> &'static str {
    t().priorities[priority as usize]
}

pub fn save_status(status: &SaveStatus) -> String {
    (t().save_status)(status)
}

/// Name of a metric that alert rules can watch, or the metric's own name.
pub fn metric(name: &str) -> &str {
    (t().metric)(name).unwrap_or(name)
}

/// Hold time of an alert rule, or `None` when it fires at once.
fn hold(secs: u64) -> Option<String> {
    (secs > 0).then(|| span(secs))
}

/// Condition of an alert rule in words, with the threshold in the metric's unit.
pub fn alert_condition(rule: &qqq_engine::AlertRule, unit: qqq_domain::Unit) -> String {
    (t().alert_condition)(
        rule.condition == AlertCondition::Above,
        &format_value(Some(rule.threshold), unit),
        hold(rule.hold_secs).as_deref(),
    )
}

/// Title and text of the notification of a fired alert.
pub fn alert(event: &AlertEvent) -> (String, String) {
    let t = t();
    let above = event.rule.condition == AlertCondition::Above;
    let title = (t.alert_title)(
        &alert_device_name(&event.device, event.host.as_deref()),
        metric(&event.rule.metric),
        above,
    );
    let body = (t.alert_body)(
        &format_value(Some(event.value), event.unit),
        &format_value(Some(event.rule.threshold), event.unit),
        above,
        hold(event.rule.hold_secs).as_deref(),
    );
    (title, body)
}

/// Value of a device detail with its enumerated tokens phrased, such as `physical` for
/// `interface_type`.
pub fn detail_value<'a>(key: &str, value: &'a str) -> &'a str {
    let t = t();
    match (key, value) {
        ("interface_type", "physical") => t.physical_adapter,
        ("interface_type", "virtual") => t.virtual_adapter,
        ("memory_type", "dedicated") => t.dedicated_memory,
        ("memory_type", "shared") => t.shared_memory,
        ("default_route", "yes") => t.yes,
        ("present_api", "dxgi") => "DXGI",
        ("present_api", "d3d9") => "Direct3D 9",
        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_can_be_activated() {
        for each in Language::ALL {
            set(each);
            assert_eq!(language(), each);
            assert!(t().window_title.starts_with("QQQ Monitor"));
        }
        set(Language::Chinese);
    }

    #[test]
    fn every_language_names_known_details() {
        for strings in [&zh::STRINGS, &en::STRINGS, &ja::STRINGS] {
            for key in [
                "logical_cores",
                "os",
                "description",
                "interface_guid",
                "interface_type",
                "link_speed",
                "vendor",
                "memory_type",
                "adapter_luid",
                "counter_instance",
                "volume_label",
                "file_system",
                "volume_guid",
                "kernel",
                "architecture",
                "default_route",
                "device",
                "process_id",
                "present_api",
            ] {
                assert!((strings.detail)(key).is_some(), "{key}");
            }
        }
    }

    #[test]
    fn every_language_names_alert_metrics() {
        for strings in [&zh::STRINGS, &en::STRINGS, &ja::STRINGS] {
            for metric in crate::components::ALERT_METRICS {
                assert!((strings.metric)(metric).is_some(), "{metric}");
            }
        }
    }
}
