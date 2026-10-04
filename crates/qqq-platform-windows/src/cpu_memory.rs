use qqq_domain::*;
use std::time::Instant;
use sysinfo::System;

pub struct CpuMemory {
    system: System,
    previous: Option<Instant>,
}
impl CpuMemory {
    pub fn new() -> Self {
        Self {
            system: System::new(),
            previous: None,
        }
    }
}
impl Collector for CpuMemory {
    fn collect(&mut self, context: &CollectContext) -> Result<SampleBatch, CollectorError> {
        let mut batch = SampleBatch::default();
        let at = Instant::now();
        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        let warmed = self
            .previous
            .is_some_and(|old| at.saturating_duration_since(old) <= context.period * 3);
        self.previous = Some(at);
        let brand = self
            .system
            .cpus()
            .first()
            .map(|cpu| cpu.brand())
            .unwrap_or("CPU");
        batch.devices.push(
            DeviceDescriptor::new("cpu", brand, DeviceKind::Cpu)
                .detail("逻辑核心", self.system.cpus().len().to_string()),
        );
        batch.devices.push(DeviceDescriptor::new(
            "memory",
            "物理内存",
            DeviceKind::Memory,
        ));
        batch.devices.push(
            DeviceDescriptor::new("system", "本机", DeviceKind::System).detail(
                "操作系统",
                System::long_os_version().unwrap_or_else(|| "Windows".into()),
            ),
        );
        let usage = |value| {
            if warmed {
                SampleValue::Gauge(value)
            } else {
                SampleValue::WarmingUp
            }
        };
        batch.samples.push(MetricSample::new(
            "cpu",
            "usage",
            Unit::Percent,
            usage(self.system.global_cpu_usage() as f64),
            at,
        ));
        for (index, cpu) in self.system.cpus().iter().enumerate() {
            let id = format!("core:{index}");
            batch.devices.push(DeviceDescriptor::new(
                &id,
                format!("逻辑核心 {}", index + 1),
                DeviceKind::Core,
            ));
            batch.samples.push(MetricSample::new(
                &id,
                "usage",
                Unit::Percent,
                usage(cpu.cpu_usage() as f64),
                at,
            ));
        }
        for (metric, value) in [
            ("total", self.system.total_memory()),
            ("used", self.system.used_memory()),
            ("available", self.system.available_memory()),
        ] {
            batch.samples.push(MetricSample::new(
                "memory",
                metric,
                Unit::Bytes,
                SampleValue::Gauge(value as f64),
                at,
            ));
        }
        let total = self.system.total_memory();
        batch.samples.push(MetricSample::new(
            "memory",
            "usage",
            Unit::Percent,
            if total > 0 {
                SampleValue::Gauge(self.system.used_memory() as f64 / total as f64 * 100.0)
            } else {
                SampleValue::Failed("物理内存总量不可用".into())
            },
            at,
        ));
        batch.samples.push(MetricSample::new(
            "system",
            "uptime",
            Unit::Seconds,
            SampleValue::Gauge(System::uptime() as f64),
            at,
        ));
        batch.complete_inventory = true;
        batch.finished_at = Instant::now();
        Ok(batch)
    }
}
