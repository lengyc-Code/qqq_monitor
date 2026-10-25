//! English.

use super::{PageText, Strings};
use qqq_domain::{Reason, Shortcut};
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
    frames_page: PageText {
        nav: "Frames",
        title: "Frame Rate & Frame Time",
        hint: "How smoothly the foreground app runs while you play",
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
    processes_page: PageText {
        nav: "Processes",
        title: "Processes",
        hint: "Manage the processes of Windows and of running WSL distributions",
    },
    remote_page: PageText {
        nav: "Remote",
        title: "Remote Hosts",
        hint: "System information and key metrics of Linux hosts over SSH",
    },
    alerts_page: PageText {
        nav: "Alerts",
        title: "Alerts",
        hint: "Windows notifications when a metric crosses its threshold",
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
    alerts_firing: |n| {
        if n == 1 {
            "1 alert firing".into()
        } else {
            format!("{n} alerts firing")
        }
    },
    dock_top: "Dock to the top edge",
    dock_left: "Dock to the left edge",
    dock_right: "Dock to the right edge",
    restore_window: "Restore the full window",

    tray_open: "Show main window",
    tray_exit: "Exit",

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
    minutes_ago: |v| format!("{v} min ago"),
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
    foreground_app: "Foreground app",
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
    metric,

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
    minimize_to_tray: "Minimize to tray",
    minimize_to_tray_option: "Minimizing leaves only an icon in the notification area",
    autostart: "Autostart",
    autostart_option: "Start QQQ Monitor when you sign in to Windows",
    autostart_to_tray_option: "Start hidden in the notification area",
    frame_monitoring: "Frame monitoring",
    frame_monitoring_option: "Count the frame rate and frame time of the foreground app and show FPS on the mini bar",
    apply_save: "Apply & save",
    runtime_status: "Status",
    batches: |received, dropped| format!("Batches received {received}  ·  dropped {dropped}"),
    history_usage: |used| format!("History memory {used} / 16 MiB"),
    collectors_ok: "All sources are working",
    refresh_devices: "Refresh devices",
    about: "About",
    about_intro: "An anime pixel-art multi-purpose task manager for Windows, where MINT, the mascot, keeps an eye on your PC with you: real-time performance monitoring with history charts, process management, Linux hosts monitored over SSH, frame counting for the foreground app, alerts, and tools such as screenshots, clipboard history and notes.",
    about_github: "GitHub page",

    process_name: "Name",
    process_user: "User",
    process_memory: "Memory",
    process_threads: "Threads",
    process_priority: "Priority",
    process_command: "Command line",
    process_search: "Search name, PID, user or command line",
    process_count: |shown, total| {
        if shown == total {
            format!("{total} processes")
        } else {
            format!("{shown} of {total} processes")
        }
    },
    processes_loading: "Reading processes…",
    processes_none: "No matching processes",
    processes_failed: |reason| format!("Cannot read processes: {reason}"),
    wsl_hint: "Running WSL distributions appear here",
    end_process: "End process",
    kill_process: "Force end",
    end_confirm: |name, pid| format!("End \u{201c}{name}\u{201d} (PID {pid})?"),
    end_warning: "Unsaved data in the process will be lost.",
    kill_warning: "Force end stops the process at once; unsaved data will be lost.",
    cancel: "Cancel",
    priorities: [
        "Low",
        "Below normal",
        "Normal",
        "Above normal",
        "High",
        "Realtime",
    ],

    remote_add: "Add host",
    remote_empty: "No remote hosts yet. Add a Linux host you can sign in to over SSH to see its system information, CPU, memory, disks, network and file systems here.",
    remote_connecting: "Connecting…",
    remote_connected: "Connected",
    remote_failed: |reason| format!("Connection failed: {reason}"),
    remote_retrying: "Retrying shortly",
    remote_key_unknown: |fingerprint| {
        format!(
            "First connection to this host. Check the host key fingerprint with the server's administrator and trust it only if it matches:\n{fingerprint}"
        )
    },
    remote_key_changed: |fingerprint| {
        format!(
            "The host key differs from the trusted one: the server may have been reinstalled, or someone may be impersonating it. Trust the new key only once you are sure:\n{fingerprint}"
        )
    },
    remote_trust: "Trust and connect",
    remote_trust_new: "Trust new key",
    remote_edit: "Edit",
    remote_reconnect: "Reconnect",
    remote_remove: "Remove",
    remote_remove_confirm: |name| format!("Remove remote host \u{201c}{name}\u{201d}?"),
    remote_remove_warning: "Its password or passphrase in Credential Manager and the collected data are removed too.",
    remote_auth_password: "Password",
    remote_auth_key: "Key file",
    remote_auth_agent: "SSH agent",
    remote_add_title: "Add Remote Host",
    remote_edit_title: "Edit Remote Host",
    remote_import: "Import from SSH config",
    remote_name: "Name",
    remote_name_hint: "Optional; the address shows when empty",
    remote_address: "Address",
    remote_address_hint: "Host name or IP address",
    remote_port: "Port",
    remote_user: "User",
    remote_auth: "Sign-in",
    remote_password: "Password",
    remote_password_keep: "Leave empty to keep the saved password",
    remote_key_file: "Key file",
    remote_key_hint: "e.g. ~/.ssh/id_ed25519",
    remote_passphrase: "Passphrase",
    remote_passphrase_hint: "Leave empty for an unencrypted key",
    remote_passphrase_keep: "Leave empty to keep the saved passphrase",
    remote_agent_hint: "Uses the keys loaded in the Windows OpenSSH Authentication Agent service or Pageant.",
    remote_secret_note: "Passwords and passphrases are kept in Windows Credential Manager, not in the settings file.",
    remote_save: "Save and connect",
    remote_missing: |field| format!("{field} is required"),
    remote_bad_port: "The port must be a number from 1 to 65535",
    remote_hostname: "Host name",
    remote_load: "Load average",
    remote_cpu_detail: |iowait, processes| format!("I/O wait {iowait} · {processes} processes"),
    remote_swap: "Swap",
    remote_no_swap: "Off",
    remote_volumes_title: "File Systems",
    remote_volumes_hint: "Capacity of local file systems; usage matches df",
    remote_cores_hint: "Current load of each logical core",
    yes: "Yes",

    fps: "Frame rate",
    frame_time: "Frame time",
    frame_time_max: "Longest frame",
    fps_detail: |mean| format!("Mean frame time {mean}"),
    frame_time_detail: |longest| format!("Longest {longest}"),
    frames_target: "Counting",
    frames_off: "Frame monitoring is off. Switch it on to count, in the background, the frames the foreground app presents per second and the time between two of them; the mini bar then shows FPS as well.",
    frames_denied: "Reading the system's graphics events needs administrator rights. Restart as administrator, or add your account to the Performance Log Users group and sign in to Windows again.",
    frames_restart: "Restart as administrator",
    frames_waiting: "Switch to a game or another app to start counting its frames",
    fps_chart: "Frame rate",
    frame_time_chart: "Frame time",
    frames_note: "The frame rate is the number of frames the foreground app presents per second, and the frame time is the gap between two presents. Both come from the Windows event tracing of DXGI and Direct3D 9, without touching the game's process. While this app is in front, the previous app stays counted; Vulkan and OpenGL apps that do not present through DXGI may show no data.",

    alert_add: "Add alert",
    alert_test: "Send test notification",
    alert_test_body: "If you can read this, alerts reach you.",
    alert_empty: "No alert rules yet. Pick a device and a metric and set a threshold, and a Windows notification arrives whenever the reading crosses it.",
    alert_rules: "Alert rules",
    alert_recent: "Recent alerts",
    alert_recent_empty: "No alert has fired in this session",
    alert_normal: "Normal",
    alert_no_data: "Waiting for data",
    alert_disabled: "Off",
    alert_pending: |left| format!("Past the threshold, alerting in {left}"),
    alert_firing: |held| format!("Firing for {held}"),
    alert_now: |value| format!("Now {value}"),
    alert_above: "Above",
    alert_below: "Below",
    alert_condition: |above, threshold, hold| {
        let side = if above { "Above" } else { "Below" };
        match hold {
            Some(hold) => format!("{side} {threshold} for {hold}"),
            None => format!("{side} {threshold}"),
        }
    },
    alert_hold_none: "At once",
    alert_add_title: "Add alert rule",
    alert_edit_title: "Edit alert rule",
    alert_device: "Device",
    alert_metric: "Metric",
    alert_when: "When",
    alert_threshold: "Threshold",
    alert_hold: "Hold for",
    alert_enabled: "Enable this rule",
    alert_pick_device: "Choose a device",
    alert_device_missing: "not connected",
    alert_missing: "Choose a device and a metric",
    alert_bad_threshold: "The threshold must be a number",
    alert_save: "Save",
    alert_edit: "Edit",
    alert_remove: "Remove",
    alert_title: |device, metric, above| {
        format!(
            "{device}: {metric} too {}",
            if above { "high" } else { "low" }
        )
    },
    alert_body: |value, threshold, above, hold| {
        let side = if above { "above" } else { "below" };
        match hold {
            Some(hold) => format!("Now {value}, {side} the {threshold} threshold for {hold}"),
            None => format!("Now {value}, {side} the {threshold} threshold"),
        }
    },
    alert_note: "Rules read the latest valid reading of each source. Once a reading stays past its threshold for the hold time, a Windows notification arrives; once it stays back on the other side for the same time, the rule is normal again. A rule notifies at most once a minute. Notifications also arrive while the window sits in the tray or docks as the mini bar, and clicking one opens the main window.",

    tools_page: PageText {
        nav: "Tools",
        title: "Tools",
        hint: "Handy tools such as screenshots, clipboard history, notes and quick commands",
    },
    tool_screenshots: "Screenshots",
    tool_clipboard: "Clipboard",
    tool_notes: "Notes",
    tool_commands: "Commands",
    tray_screenshot: "Take screenshot",
    hours_ago: |v| format!("{v} h ago"),
    shot_take: "Take screenshot",
    shot_hint: "Drag to select a region · Click to capture the whole screen · Right-click or Esc to cancel",
    shot_folder: "Open folder",
    shot_hotkey: "Shortcut",
    shot_hotkey_option: |key| format!("Press {key} to take a screenshot"),
    shot_hotkey_change: "Change",
    shot_hotkey_press: "Press the new shortcut, or Esc to cancel",
    shot_hotkey_invalid: "Use Ctrl or Alt with a letter or digit, or one of F1 to F24",
    keep: "Keep",
    shot_count: |n| match n {
        1 => "1 screenshot".into(),
        _ => format!("{n} screenshots"),
    },
    shot_recent: "Recent screenshots",
    shot_empty: "No screenshots yet. Press the shortcut or click Take screenshot.",
    shot_loading: "Reading screenshots…",
    shot_note: |folder| {
        format!(
            "When you release the mouse, the selected region is copied to the clipboard and saved as a PNG in {folder}. Beyond the number to keep, the oldest screenshots are removed first. Take screenshot hides the main window before the capture."
        )
    },
    copy: "Copy",
    open: "Open",
    delete: "Delete",

    clip_option: "Record text, pictures and files copied to the clipboard",
    clip_off: "Clipboard history is off. Once on, it records what you copy from then on.",
    clip_clear: "Clear history",
    clip_count: |n| match n {
        1 => "1 entry".into(),
        _ => format!("{n} entries"),
    },
    clip_recent: "Clipboard history",
    clip_empty: "Nothing recorded yet. Text, pictures and files you copy appear here.",
    clip_text: |n| match n {
        1 => "Text · 1 character".into(),
        _ => format!("Text · {n} characters"),
    },
    clip_picture: |w, h| format!("Picture · {w} × {h}"),
    clip_files: |n| match n {
        1 => "1 file".into(),
        _ => format!("{n} files"),
    },
    clip_more: |n| format!("and {n} more"),
    clip_note: "Click an entry to copy it back to the clipboard; the newest entry comes first. History stays in memory and clears when the program exits. Content that password managers mark as private is skipped, and pictures take at most 256 MiB of memory together.",

    tray_note: "Notes",
    note_title: "QQQ Notes",
    note_hotkey_option: |key| format!("Press {key} to show or hide the notes"),
    hotkey_taken: "Another shortcut already uses these keys",
    hotkey_none: "No shortcut",
    hotkey_set: "Set",
    note_open: "Open notes",
    note_new: "New note",
    note_list: "All notes",
    note_write: "Edit",
    note_read: "Preview",
    note_delete: "Delete note",
    note_hide: "Hide notes",
    note_untitled: "Untitled note",
    note_hint: "Write here; Markdown works",
    note_read_empty: "This note is empty. Double-click here or press Ctrl+E to start writing.",
    note_delete_confirm: "Delete this note?",
    note_delete_confirm_short: "Delete for good",
    note_chars: |n| match n {
        1 => "1 character".into(),
        _ => format!("{n} characters"),
    },
    note_saving: "Saving…",
    note_saved: "Saved",
    note_save_failed: |error| format!("Could not save: {error}"),
    note_count: |n| match n {
        1 => "1 note".into(),
        _ => format!("{n} notes"),
    },
    note_empty: "No notes yet. Press the shortcut or click Open notes to start one.",
    note_loading: "Reading notes…",
    note_about: |folder| {
        format!(
            "The note window stays above other windows. The shortcut shows it and, pressed again in the note, hides it; drag its title bar to move it and its lower right corner to resize it. Notes are Markdown: Ctrl+E switches between editing and the preview, and Ctrl+N starts a new note; in the preview, tick tasks and open web links. Each note is a .md file in {folder}, saved a moment after you stop typing."
        )
    },

    command_hotkey_option: |key| format!("Press {key} to run"),
    command_new: "New command",
    command_list: "All commands",
    command_count: |n| match n {
        1 => "1 command".into(),
        _ => format!("{n} commands"),
    },
    command_empty: "No quick commands yet. Click New command and enter the PowerShell commands to run.",
    command_run: "Run",
    command_stop: "Stop",
    command_running: |n| match n {
        1 => "Running · 1 session".into(),
        _ => format!("Running · {n} sessions"),
    },
    command_delete_ends: |n| match n {
        1 => "Deleting also ends its session running in the background".into(),
        _ => format!("Deleting also ends its {n} sessions running in the background"),
    },
    command_sound_option: "Play a sound when a quick command runs",
    command_balloon: |n| match n {
        1 => "Started in 1 session".into(),
        _ => format!("Started in {n} sessions"),
    },
    command_edit: "Edit",
    command_add_title: "New quick command",
    command_edit_title: "Edit quick command",
    command_name: "Name",
    command_lines: "Commands",
    command_lines_hint: "One PowerShell command per line",
    command_sessions: "Sessions",
    command_shared: "Run in order in one session",
    command_separate: "One session per command",
    command_window: "Window",
    command_window_option: "Show the PowerShell window and keep it open afterwards",
    command_directory: "Working folder",
    command_directory_hint: "Empty starts in your user folder",
    command_save: "Save",
    command_missing_name: "Enter a name",
    command_missing_lines: "Enter at least one command",
    command_too_long: |lines, bytes| {
        format!("Too many or too long commands: at most {lines} lines and about {bytes} characters")
    },
    command_bad_directory: "This folder was not found; enter its full path",
    command_summary: |shared, lines, window| {
        format!(
            "{} · {} · {}",
            if shared {
                "One session"
            } else {
                "One session per command"
            },
            match lines {
                1 => "1 command".into(),
                _ => format!("{lines} commands"),
            },
            if window {
                "Window shown"
            } else {
                "In the background"
            }
        )
    },
    command_about: "Quick commands run in Windows PowerShell. Run in order in one session runs every command in a single session, so later commands keep the variables and location that earlier ones set; One session per command starts each command in a session of its own, all at once. A shown window stays open after the commands end, to read the output or keep typing; without a window, the commands run in the background and the session ends with them. While sessions run, the command shows how many and a Stop button, which ends all of them along with the programs they started. Each run plays a sound. Shortcuts work from any application, also while the window sits in the tray or docks as the mini bar, where a tray balloon then shows how the run went; click Set on a command and press the keys to give it one.",
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
        "kernel" => "Kernel",
        "architecture" => "Architecture",
        "default_route" => "Default route",
        "device" => "Device",
        "process_id" => "Process ID",
        "present_api" => "Present API",
        _ => return None,
    })
}

fn metric(name: &str) -> Option<&'static str> {
    Some(match name {
        "usage" => "Usage",
        "iowait" => "I/O wait",
        "used" => "Used",
        "available" => "Available",
        "swap_usage" => "Swap usage",
        "memory_usage" => "GPU memory usage",
        "memory_used" => "GPU memory used",
        "read" => "Read rate",
        "write" => "Write rate",
        "receive" => "Receive rate",
        "send" => "Send rate",
        "load1" => "1-minute load",
        "load5" => "5-minute load",
        "load15" => "15-minute load",
        "processes" => "Processes",
        "fps" => "Frame rate",
        "frame_time" => "Mean frame time",
        "frame_time_max" => "Longest frame",
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
        Reason::ProcessGone => "the process has exited".into(),
        Reason::AccessDenied => "access denied; run as administrator and try again".into(),
        Reason::HostStopped => "the WSL distribution has stopped".into(),
        Reason::AddressNotFound => "the host name does not resolve".into(),
        Reason::ConnectionRefused => {
            "connection refused; check the address, port and SSH service".into()
        }
        Reason::TimedOut => "the connection timed out".into(),
        Reason::ConnectionLost => "the connection was lost".into(),
        Reason::Handshake => "SSH handshake failed: no algorithm both sides support".into(),
        Reason::HostKeyUnknown { .. } => "the host key is not trusted yet".into(),
        Reason::HostKeyChanged { .. } => "the host key differs from the trusted one".into(),
        Reason::AuthFailed => "the user name, password or key was rejected".into(),
        Reason::NoPassword => "no password is saved".into(),
        Reason::KeyUnreadable => "the key file cannot be read".into(),
        Reason::KeyPassphrase => {
            "the key is encrypted and its passphrase is missing or wrong".into()
        }
        Reason::NoAgent => "no SSH agent is running, or it holds no keys".into(),
        Reason::NotLinux => "the host offers no POSIX shell or Linux /proc".into(),
        Reason::TracingDenied => {
            "reading event tracing needs administrator rights or the Performance Log Users group"
                .into()
        }
        Reason::NoFrames => "the foreground app presents no new Direct3D frames".into(),
        Reason::NotificationsOff => {
            "notifications of this app are off in the system settings".into()
        }
        Reason::Declined => "the request was declined".into(),
        Reason::HotkeyTaken => "another application already uses the shortcut".into(),
        Reason::ClipboardBusy => "another application is using the clipboard".into(),
        Reason::PathNotFound => "the file or folder was not found".into(),
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
        Notice::Process(reason) => format!("Process action failed: {}", self::reason(reason)),
        Notice::InvalidRemoteHost => "Invalid remote host settings".into(),
        Notice::SecretUnsaved(reason) => {
            format!("The password could not be saved: {}", self::reason(reason))
        }
        Notice::AutostartUnchanged(reason) => {
            format!("Autostart could not be changed: {}", self::reason(reason))
        }
        Notice::InvalidAlertRule => "Invalid alert rule".into(),
        Notice::NotificationFailed(reason) => {
            format!(
                "The notification could not be shown: {}",
                self::reason(reason)
            )
        }
        Notice::ElevationFailed(reason) => {
            format!(
                "Could not restart as administrator: {}",
                self::reason(reason)
            )
        }
        Notice::InvalidHotkey => "A shortcut is invalid".into(),
        Notice::InvalidQuickCommand => "A quick command is invalid".into(),
        Notice::CommandFailed(reason) => {
            format!("Could not run the quick command: {}", self::reason(reason))
        }
        Notice::CommandStopFailed(reason) => {
            format!("Could not stop the quick command: {}", self::reason(reason))
        }
        Notice::InvalidRecordLimit => "The number to keep is not one of the choices".into(),
        Notice::HotkeyUnavailable(shortcut, reason) => {
            let name = match shortcut {
                Shortcut::Screenshot => "screenshot",
                Shortcut::Note => "notes",
                Shortcut::Command(_) => "quick command",
            };
            format!(
                "The {name} shortcut is unavailable: {}",
                self::reason(reason)
            )
        }
        Notice::ScreenshotFailed(reason) => {
            format!("The screenshot failed: {}", self::reason(reason))
        }
        Notice::ScreenshotFile(error) => format!("Screenshot file error: {error}"),
        Notice::ClipboardFailed(reason) => {
            format!("The clipboard is unavailable: {}", self::reason(reason))
        }
        Notice::OpenFailed(reason) => format!("Could not open: {}", self::reason(reason)),
        Notice::NoteFile(error) => format!("Note file error: {error}"),
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
