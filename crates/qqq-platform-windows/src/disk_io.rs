use qqq_domain::*;
use std::time::Instant;

pub struct DiskIo {
    query: super::native::DiskQuery,
    previous: Option<Instant>,
}
impl DiskIo {
    pub fn new() -> Result<Self, CollectorError> {
        Ok(Self {
            query: super::native::DiskQuery::new()?,
            previous: None,
        })
    }
}
impl Collector for DiskIo {
    fn collect(&mut self, context: &CollectContext) -> Result<SampleBatch, CollectorError> {
        let mut batch = SampleBatch::default();
        let at = Instant::now();
        self.query.collect()?;
        let warmed = self
            .previous
            .is_some_and(|old| at.saturating_duration_since(old) <= context.period * 3);
        self.previous = Some(at);
        let values = self.query.values();
        if !warmed && values.is_err() {
            batch.finished_at = Instant::now();
            return Ok(batch);
        }
        let (read, write) = values?;
        let names: std::collections::BTreeSet<_> =
            read.keys().chain(write.keys()).cloned().collect();
        for name in names {
            let Some(number) = name
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u32>().ok())
            else {
                continue;
            };
            let id = format!("disk:{number}");
            batch.devices.push(
                DeviceDescriptor::new(&id, format!("物理磁盘 {number}"), DeviceKind::Disk)
                    .detail("计数器实例", &name),
            );
            for (metric, values) in [("read", &read), ("write", &write)] {
                let value = if !warmed {
                    SampleValue::WarmingUp
                } else {
                    match values.get(&name).copied().flatten() {
                        Some(value) => SampleValue::Gauge(value),
                        None => SampleValue::WarmingUp,
                    }
                };
                batch.samples.push(MetricSample::new(
                    &id,
                    metric,
                    Unit::BytesPerSecond,
                    value,
                    at,
                ));
            }
        }
        batch.complete_inventory = true;
        batch.finished_at = Instant::now();
        Ok(batch)
    }
}
