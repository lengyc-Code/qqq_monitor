mod cpu_memory;
mod disk_io;
mod gpu;
mod inventory;
mod native;
mod network;

use qqq_domain::*;
use std::{sync::Arc, time::Duration};

/// Window icons from the icon resource embedded in the executable.
pub use native::set_window_icon;
/// Screen geometry and pointer state for docking the window, in physical pixels.
pub use native::{covers_point, monitor_at, mouse_button_held};

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

pub fn show_startup_error(message: &str) {
    native::show_error(message);
}
