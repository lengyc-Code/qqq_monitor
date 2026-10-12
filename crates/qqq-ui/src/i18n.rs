//! Interface text in every supported language. The active language is process-wide:
//! [`set`] switches it and [`t`] returns its strings. Each language fills the whole
//! [`Strings`] table, so a missing translation is a compile error.

mod en;
mod ja;
mod zh;

use qqq_domain::Reason;
use qqq_engine::{Language, Notice, SaveStatus};
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
    pub disk_page: PageText,
    pub network_page: PageText,
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
    pub apply_save: &'static str,
    pub runtime_status: &'static str,
    /// Received and dropped sample batches.
    pub batches: fn(u64, u64) -> String,
    pub history_usage: fn(&str) -> String,
    pub collectors_ok: &'static str,
    pub refresh_devices: &'static str,
    pub about_mint: &'static str,
    pub about_animations: &'static str,
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

pub fn save_status(status: &SaveStatus) -> String {
    (t().save_status)(status)
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
            ] {
                assert!((strings.detail)(key).is_some(), "{key}");
            }
        }
    }
}
