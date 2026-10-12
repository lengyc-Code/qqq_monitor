use qqq_domain::*;
use std::time::Instant;

pub struct Network;
impl Collector for Network {
    fn collect(&mut self, _: &CollectContext) -> Result<SampleBatch, CollectorError> {
        let mut batch = SampleBatch::default();
        for row in super::native::network_rows()? {
            let at = Instant::now();
            let id = format!("net:{}", row.luid);
            let mut device = DeviceDescriptor::new(&id, row.name, DeviceKind::Network)
                .persistent_id(&row.guid)
                .detail("description", row.description)
                .detail("interface_guid", row.guid)
                .detail(
                    "interface_type",
                    if row.physical { "physical" } else { "virtual" },
                )
                .detail("link_speed", format!("{} Mbps", row.link_speed / 1_000_000));
            device.online = row.connected;
            batch.devices.push(device);
            for (metric, value) in [("receive", row.received), ("send", row.sent)] {
                batch.samples.push(MetricSample::new(
                    &id,
                    metric,
                    Unit::BytesPerSecond,
                    SampleValue::Counter(value),
                    at,
                ));
            }
        }
        batch.complete_inventory = true;
        batch.finished_at = Instant::now();
        Ok(batch)
    }
}
