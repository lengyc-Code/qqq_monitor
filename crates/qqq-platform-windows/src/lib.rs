mod cpu_memory;
mod disk_io;
mod gpu;
mod inventory;
mod native;
mod network;
mod process;
mod tray;
mod wsl;

use qqq_domain::*;
use std::{sync::Arc, time::Duration};

/// Window icons from the icon resource embedded in the executable.
pub use native::set_window_icon;
/// The user's display languages, most preferred first.
pub use native::ui_languages;
/// Screen geometry and pointer state for docking the window, in physical pixels.
pub use native::{covers_point, monitor_at, mouse_button_held};
/// Notification-area icon and taskbar button of the application window.
pub use tray::{
    attach as attach_tray, hide as hide_tray, opened as tray_opened, set_taskbar_button,
    show as show_tray,
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

pub fn show_startup_error(title: &str, message: &str) {
    native::show_error(title, message);
}
