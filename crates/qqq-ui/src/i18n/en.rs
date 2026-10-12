//! English.

use super::{PageText, Strings};
use qqq_domain::Reason;
use qqq_engine::{Notice, SaveStatus};

pub const STRINGS: Strings = Strings {
    window_title: "QQQ Monitor · System Performance",
    tagline: "System Performance",
    startup_failed: "QQQ Monitor could not start",
    overview_page: PageText {
        nav: "Overview",
        title: "System Overview",
        hint: "Everything your PC is doing, live",
    },
    cpu_page: PageText {
        nav: "CPU",
        title: "Processor",
        hint: "Overall load and every logical core",
    },
    memory_page: PageText {
        nav: "Memory",
        title: "Physical Memory",
        hint: "Available physical memory and its usage over time",
    },
    gpu_page: PageText {
        nav: "GPU",
        title: "Graphics Card",
        hint: "Utilization, engine load and GPU memory",
    },
    disk_page: PageText {
        nav: "Disk",
        title: "Disks & Storage",
        hint: "Physical disk I/O and local volume capacity",
    },
    network_page: PageText {
        nav: "Network",
        title: "Network Traffic",
        hint: "Live traffic of the selected adapter",
    },
    settings_page: PageText {
        nav: "Settings",
        title: "Settings",
        hint: "Tune the monitor to the way you work",
    },

    history_range: "History",
    seconds: |v| format!("{v} s"),
    minutes: |v| format!("{v} min"),
    running: "Monitoring",
    degraded: "Some sources unavailable",
    timing: |sampling, session| format!("Sampling {sampling:.1}s  ·  Session {session:.0}s"),
    minibar: "Mini bar",
    dock_top: "Dock to the top edge",
    dock_left: "Dock to the left edge",
    dock_right: "Dock to the right edge",
    restore_window: "Restore the full window",

    days: |v| match v {
        1 => "1 day".into(),
        _ => format!("{v} days"),
    },
    state_ready: "Live",
    state_stale: "Stale",
    state_failed: "Failed",
    state_unsupported: "Unsupported",
    state_offline: "Disconnected",
    state_waiting: "Collecting",
    seconds_ago: |v| format!("{v:.0} s ago"),
    last_valid: "last valid data",
    now: "now",
    chart_waiting: "Waiting for valid samples; the curve leaves gaps where data is missing",
    device_offline: "Device disconnected",
    offline_suffix: " (disconnected)",
    finding_devices: "Looking for devices",
    waiting_samples: "Waiting for samples",
    click_for_details: "Click for details",
    list_separator: ", ",
    labeled: |label, text| format!("{label}: {text}"),
    used_of: |used, total| format!("{used} / {total} used"),

    processor: "Processor",
    physical_memory: "Physical memory",
    this_pc: "This PC",
    physical_disk: "Physical disk",
    graphics_card: "Graphics card",
    adapter: "Adapter",
    logical_core: |n| format!("Logical core {n}"),
    detail,
    physical_adapter: "Physical adapter",
    virtual_adapter: "Virtual or software interface",
    dedicated_memory: "Dedicated GPU memory",
    shared_memory: "Shared GPU memory",
    reason,
    notice,
    save_status,

    memory: "Memory",
    gpu_memory: "GPU memory",
    memory_short: "RAM",
    gpu_memory_short: "VRAM",
    read: "Read",
    write: "Write",
    receive: "Receive",
    send: "Send",
    logical_cores: "Logical cores",

    mint_name: "Mascot · MINT",
    mint_hover: "MINT changes her expression with CPU and memory load",
    mint_waiting: "Waking up sensors…\nJust a moment~",
    mint_trouble: "Missing some data…\nCheck Settings?",
    mint_cpu_hot: |cpu| format!("CPU {cpu:.0}%!\nOverheating >_<"),
    mint_memory_full: |memory| format!("Memory at {memory:.0}%\nI'm almost full!"),
    mint_busy: |cpu, memory| format!("Getting busy~\nCPU {cpu:.0}%  RAM {memory:.0}%"),
    mint_relaxed: |cpu| format!("Relaxed~ CPU {cpu:.0}%\nTea time?"),
    mint_calm: |cpu, memory| format!("All good!\nCPU {cpu:.0}%  RAM {memory:.0}%"),

    cores_detail: |cores| format!("{cores} logical cores"),
    disk_read: "Disk read",
    network_receive: "Network receive",
    cpu_memory_chart: "CPU and memory usage",
    disk_io: "Disk read & write",
    network_io: "Network traffic",
    device_profile: "Device profile",
    total_memory: "Memory",
    current_disk: "Disk",
    current_adapter: "Adapter",
    uptime: "Uptime",

    cpu_total: "Total usage",
    cpu_total_detail: "Average load of all logical cores",
    cpu_busiest: "Busiest core",
    cpu_overall: "Overall CPU",
    cpu_selected_core: "Selected core",
    cpu_chart: "CPU usage",
    cpu_cores_hint: "Click a core to see its history",

    memory_in_use: "In use",
    share_of_total: |share| format!("{share} of total"),
    available: "Available",
    allocatable: |share| format!("{share} still free to allocate"),
    memory_total: "Physical memory",
    memory_total_detail: "Physical memory usable by the system",
    memory_chart: "Physical memory usage",
    memory_usage: "Memory usage",
    memory_note: "Counts physical memory usable by the system; memory in use and committed memory are different metrics.",

    gpu_usage: "GPU usage",
    engines_waiting: "Waiting for engine data",
    busiest_engine: |engine, load| format!("Busiest engine {engine} · {load}"),
    gpu_memory_used: |kind, used, total| format!("{kind} · {used} / {total} used"),
    gpu_chart: "GPU and GPU memory usage",
    engines_title: "Engine load",
    engines_hint: "The busiest engine of each type; click one to see its history",
    engines_missing: "Waiting for engine data; cards without engine counters show memory only",
    engine: |name| format!("{name} engine"),
    engine_kind,
    gpu_memory_title: "GPU memory",
    gpu_memory_hint: "Dedicated memory sits on the card; shared GPU memory comes from system memory",
    total_share: |total, share| format!("Total {total}  ·  {share} used"),
    gpu_note: "GPU usage matches Task Manager: the load of each engine is summed over processes and the busiest engine counts; engines of different types are not added together.",

    disk_read_detail: "Read rate of the selected physical disk",
    disk_write_detail: "Write rate of the selected physical disk",
    disk_chart: "Physical disk read and write rate",
    volumes_title: "Local volumes",
    volumes_hint: "Volume capacity is counted apart from physical disk I/O",
    volumes_waiting: "Looking for local volumes",
    volume_space: |free, total| format!("{free} free  ·  {total} total"),
    no_label: "No label",
    unknown_file_system: "Unknown file system",

    session_received: |bytes| format!("{bytes} received this session"),
    session_sent: |bytes| format!("{bytes} sent this session"),
    network_chart: "Network send and receive rate",
    network_note: "Only the selected interface is counted, so physical adapter and VPN traffic is never added twice; session traffic is the valid counter growth seen while the app runs.",

    preferences: "Preferences",
    language: "Language",
    follow_system: "System default",
    sampling_interval: "Sampling interval",
    default_range: "Default history",
    animations: "Animations",
    animations_option: "Light animations and a bobbing mascot",
    edge_docking: "Edge docking",
    edge_docking_option: "Dragging to the top or a side edge shows the mini bar",
    apply_save: "Apply & save",
    runtime_status: "Status",
    batches: |received, dropped| format!("Batches received {received}  ·  dropped {dropped}"),
    history_usage: |used| format!("History memory {used} / 16 MiB"),
    collectors_ok: "All sources are working",
    refresh_devices: "Refresh devices",
    about_mint: "MINT, the mascot, changes her expression with CPU and memory load: relaxed, calm, busy and overloaded.",
    about_animations: "With animations off, her expression still follows the load, but she stops bobbing and the window does not redraw for her.",
};

fn detail(key: &str) -> Option<&'static str> {
    Some(match key {
        "logical_cores" => "Logical cores",
        "os" => "Operating system",
        "description" => "Description",
        "interface_guid" => "Interface GUID",
        "interface_type" => "Interface type",
        "link_speed" => "Link speed",
        "vendor" => "Vendor",
        "memory_type" => "Memory type",
        "adapter_luid" => "Adapter LUID",
        "counter_instance" => "Counter instance",
        "volume_label" => "Volume label",
        "file_system" => "File system",
        "volume_guid" => "Volume GUID",
        _ => return None,
    })
}

fn engine_kind(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "copy" => "Copy",
        "videodecode" => "Video Decode",
        "videoencode" => "Video Encode",
        "videoprocessing" => "Video Processing",
        "compute" => "Compute",
        "graphics" => "Graphics",
        "legacyoverlay" => "Legacy Overlay",
        "overlay" => "Overlay",
        "security" => "Security",
        "ofa" => "Optical Flow",
        _ => return None,
    })
}

fn reason(reason: &Reason) -> String {
    match reason {
        Reason::NoGpuCounters => "The system provides no performance counters for this card".into(),
        Reason::NoGpuMemoryCounters => "GPU memory counters are unavailable".into(),
        Reason::VolumeUnavailable => "Volume capacity is temporarily unavailable".into(),
        Reason::NoMemoryTotal => "Total physical memory is unavailable".into(),
        Reason::OutOfRange => "The value is out of range".into(),
        Reason::CounterUnit => "Invalid counter unit".into(),
        Reason::Disconnected => "Device disconnected".into(),
        Reason::Panicked => "The collector crashed and is being restarted".into(),
        Reason::KeyConflict => "The metric key conflicts with another collector".into(),
        Reason::InstancesChanging => "Counter instances change too often to read".into(),
        Reason::CallFailed { api, code } => format!("{api} failed with error 0x{code:08X}"),
        Reason::BadData { api } => format!("{api} returned invalid data"),
    }
}

fn notice(notice: &Notice) -> String {
    match notice {
        Notice::Stopped => "The monitor service has stopped".into(),
        Notice::Busy => "Busy, please try again in a moment".into(),
        Notice::InvalidQuery => "Invalid history query".into(),
        Notice::InvalidSampling => "The sampling interval must be 0.5, 1 or 2 seconds".into(),
        Notice::InvalidHistory => "The history range must be 60, 300 or 900 seconds".into(),
        Notice::InvalidWindowSize => "Invalid window size".into(),
        Notice::InvalidWindowPosition => "Invalid window position".into(),
        Notice::InvalidValue => "Some settings were invalid and are back to their defaults".into(),
        Notice::InvalidType => {
            "Some settings had the wrong type and are back to their defaults".into()
        }
        Notice::CorruptFile => {
            "The settings file is damaged and defaults are in use; it stays until you save settings"
                .into()
        }
        Notice::Unreadable(error) => format!("Cannot read settings: {error}"),
        Notice::CorruptKept => {
            "The damaged settings file is kept; save from the Settings page to replace it".into()
        }
        Notice::InvalidDirectory => "Invalid settings folder".into(),
        Notice::System(error) => error.clone(),
    }
}

fn save_status(status: &SaveStatus) -> String {
    match status {
        SaveStatus::Loaded => "Settings loaded".into(),
        SaveStatus::Recovered(warning) => notice(warning),
        SaveStatus::Saving => "Settings applied, saving…".into(),
        SaveStatus::Saved => "Settings saved".into(),
        SaveStatus::Unsaved(error) => format!("Settings applied but not saved: {}", notice(error)),
    }
}
