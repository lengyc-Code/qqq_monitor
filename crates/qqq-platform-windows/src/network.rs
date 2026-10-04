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
                .detail("描述", row.description)
                .detail("接口 GUID", row.guid)
                .detail(
                    "接口类型",
                    if row.physical {
                        "物理网卡"
                    } else {
                        "虚拟或软件接口"
                    },
                )
                .detail("链路速率", format!("{} Mbps", row.link_speed / 1_000_000));
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
