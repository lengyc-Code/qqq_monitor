mod autostart;
mod capture;
mod clipboard;
mod cpu_memory;
mod disk_io;
mod frames;
mod gpu;
mod inventory;
mod native;
mod network;
mod notify;
mod process;
mod secret;
mod tray;
mod wsl;

use qqq_domain::*;
use std::{sync::Arc, time::Duration};

/// Whether the executable starts at sign-in, switching that on or off, and the argument the
/// executable receives when started that way.
pub use autostart::{ARGUMENT as AUTOSTART_ARGUMENT, enabled as autostart, set as set_autostart};
/// Screen capture with region selection.
pub use capture::{start as start_capture, take as take_capture};
/// Following, reading and setting the clipboard.
pub use clipboard::{set as set_clipboard, take as take_clipboard, watch as watch_clipboard};
/// Starting this executable again with administrator rights.
pub use native::restart_elevated;
/// Window icons from the icon resource embedded in the executable.
pub use native::set_window_icon;
/// The user's display languages, most preferred first.
pub use native::ui_languages;
/// Screen geometry and pointer state for docking the window, in physical pixels.
pub use native::{covers_point, monitor_at, mouse_button_held};
/// Opening a file or folder with its default program, and the local time.
pub use native::{local_time, open_path};
/// Toast notifications with the application's name and icon.
pub use notify::{notify, prepare as prepare_notifications};
/// Notification-area icon and taskbar button of the application window, and the
/// screenshot shortcut.
pub use tray::{
    attach as attach_tray, cloak as cloak_window, hide as hide_tray, opened as tray_opened,
    screenshot_requested, set_hotkey, set_taskbar_button, show as show_tray,
};

struct BuiltinFactory {
    descriptor: CollectorDescriptor,
    constructor: fn() -> Result<Box<dyn Collector>, CollectorError>,
}
impl CollectorFactory for BuiltinFactory {
    fn descriptor(&self) -> CollectorDescriptor {
        self.descriptor
    }
    fn create(&self) -> Result<Box<dyn Collector>, CollectorError> {
        (self.constructor)()
    }
}

pub fn collectors() -> Vec<Arc<dyn CollectorFactory>> {
    vec![
        Arc::new(BuiltinFactory {
            descriptor: CollectorDescriptor {
                id: "cpu-memory",
                period: SamplingPeriod::Configured,
            },
            constructor: || Ok(Box::new(cpu_memory::CpuMemory::new())),
        }),
        Arc::new(BuiltinFactory {
            descriptor: CollectorDescriptor {
                id: "network",
                period: SamplingPeriod::Configured,
            },
            constructor: || Ok(Box::new(network::Network)),
        }),
        Arc::new(BuiltinFactory {
            descriptor: CollectorDescriptor {
                id: "disk-io",
                period: SamplingPeriod::Configured,
            },
            constructor: || Ok(Box::new(disk_io::DiskIo::new()?)),
        }),
        Arc::new(BuiltinFactory {
            descriptor: CollectorDescriptor {
                id: "gpu",
                period: SamplingPeriod::Configured,
            },
            constructor: || Ok(Box::new(gpu::Gpu::new()?)),
        }),
        Arc::new(BuiltinFactory {
            descriptor: CollectorDescriptor {
                id: "volumes",
                period: SamplingPeriod::Fixed(Duration::from_secs(30)),
            },
            constructor: || Ok(Box::new(inventory::Inventory)),
        }),
    ]
}

/// Frames of the application in the foreground, from the system's event tracing. Tracing
/// needs administrator rights or membership in the Performance Log Users group.
pub fn frames() -> Arc<dyn CollectorFactory> {
    Arc::new(BuiltinFactory {
        descriptor: CollectorDescriptor {
            id: "frames",
            period: SamplingPeriod::Configured,
        },
        constructor: || Ok(Box::new(frames::Frames::new()?)),
    })
}

struct Processes;
impl ProcessPlatform for Processes {
    fn hosts(&self) -> Vec<ProcessHost> {
        std::iter::once(ProcessHost::Windows)
            .chain(wsl::running().into_iter().map(ProcessHost::Wsl))
            .collect()
    }
    fn source(&self, host: &ProcessHost) -> Box<dyn ProcessSource> {
        match host {
            ProcessHost::Windows => Box::new(process::WindowsProcesses::new()),
            ProcessHost::Wsl(distro) => Box::new(wsl::WslProcesses::new(distro)),
        }
    }
    fn act(&self, id: &ProcessId, action: ProcessAction) -> Result<(), Reason> {
        match &id.host {
            ProcessHost::Windows => process::act(id.pid, id.start, action),
            ProcessHost::Wsl(distro) => wsl::act(distro, id.pid, id.start, action),
        }
    }
}

/// Windows processes and those of running WSL distributions.
pub fn processes() -> Arc<dyn ProcessPlatform> {
    Arc::new(Processes)
}

/// Passwords and passphrases in the Windows Credential Manager.
pub fn secrets() -> Arc<dyn SecretStore> {
    Arc::new(secret::Credentials)
}

pub fn show_startup_error(title: &str, message: &str) {
    native::show_error(title, message);
}
