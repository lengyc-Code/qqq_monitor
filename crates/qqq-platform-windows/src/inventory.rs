use qqq_domain::*;
use std::time::Instant;
use sysinfo::{DiskRefreshKind, Disks};

pub struct Inventory;
impl Collector for Inventory {
    fn collect(&mut self, _: &CollectContext) -> Result<SampleBatch, CollectorError> {
        let mut batch = SampleBatch::default();
        // Discovery without storage reads allows network volumes to be filtered first.
        let mut disks = Disks::new_with_refreshed_list_specifics(DiskRefreshKind::nothing());
        for disk in disks.list_mut() {
            let mount = disk.mount_point().to_string_lossy().to_string();
            let Some(guid) = super::native::local_volume_id(&mount) else {
                continue;
            };
            let id = format!("volume:{guid}");
            let success = disk.refresh_specifics(DiskRefreshKind::nothing().with_storage());
            let at = Instant::now();
            batch.devices.push(
                DeviceDescriptor::new(&id, &mount, DeviceKind::Volume)
                    .detail("卷标", disk.name().to_string_lossy())
                    .detail("文件系统", disk.file_system().to_string_lossy())
                    .detail("卷 GUID", guid),
            );
            let total = disk.total_space();
            let free = disk.available_space();
            for (metric, unit, value) in [
                ("total", Unit::Bytes, total as f64),
                ("available", Unit::Bytes, free as f64),
                (
                    "usage",
                    Unit::Percent,
                    if total > 0 {
                        (total.saturating_sub(free)) as f64 / total as f64 * 100.0
                    } else {
                        0.0
                    },
                ),
            ] {
                let value = if success && total > 0 {
                    SampleValue::Gauge(value)
                } else {
                    SampleValue::Failed("卷容量暂时不可用".into())
                };
                batch
                    .samples
                    .push(MetricSample::new(&id, metric, unit, value, at));
            }
        }
        batch.complete_inventory = true;
        batch.finished_at = Instant::now();
        Ok(batch)
    }
}
