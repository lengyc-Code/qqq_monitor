use super::native::{self, CounterQuery, GpuAdapter, Values};
use qqq_domain::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

/// UMA adapters report only a small carve-out as dedicated memory and work in shared memory.
const DEDICATED_MINIMUM: u64 = 512 * 1024 * 1024;
/// Shortest interval between adapter lists triggered by unknown counter instances.
const RELIST: Duration = Duration::from_secs(10);

pub struct Gpu {
    query: CounterQuery,
    adapters: Vec<GpuAdapter>,
    listed: Instant,
    previous: Option<Instant>,
    /// Engine types seen per adapter; idle types keep reporting zero.
    engine_types: BTreeMap<u64, BTreeSet<String>>,
}
impl Gpu {
    pub fn new() -> Result<Self, CollectorError> {
        Ok(Self {
            query: CounterQuery::new(&[
                r"\GPU Engine(*)\Utilization Percentage",
                r"\GPU Adapter Memory(*)\Dedicated Usage",
                r"\GPU Adapter Memory(*)\Shared Usage",
            ])?,
            adapters: native::gpu_adapters()?,
            listed: Instant::now(),
            previous: None,
            engine_types: BTreeMap::new(),
        })
    }
}

/// Adapter LUID of a counter instance such as
/// `pid_4_luid_0x00000000_0x0000D1A3_phys_0_eng_0_engtype_3D`.
fn instance_luid(name: &str) -> Option<u64> {
    let rest = &name[name.find("luid_0x")? + "luid_0x".len()..];
    let (high, rest) = rest.split_once("_0x")?;
    let low = rest.split('_').next()?;
    Some(
        u64::from(u32::from_str_radix(high, 16).ok()?) << 32
            | u64::from(u32::from_str_radix(low, 16).ok()?),
    )
}

/// Engine type of an engine counter instance, such as `3d` or `videodecode`.
fn engine_type(name: &str) -> Option<String> {
    name.split_once("engtype_")
        .map(|(_, kind)| kind.to_lowercase())
        .filter(|kind| !kind.is_empty())
}

/// Bytes per adapter, summed over its physical nodes.
fn per_adapter(values: &Values) -> BTreeMap<u64, f64> {
    let mut adapters = BTreeMap::new();
    for (name, value) in values {
        if let (Some(luid), Some(value)) = (instance_luid(name), value) {
            *adapters.entry(luid).or_default() += value;
        }
    }
    adapters
}

/// Load of the busiest engine of each type per adapter, after summing each
/// engine over processes. Engine types stay separate, matching Task Manager.
fn engine_load(values: &Values) -> BTreeMap<u64, BTreeMap<String, f64>> {
    let mut engines: BTreeMap<&str, f64> = BTreeMap::new();
    for (name, value) in values {
        if let (Some(at), Some(value)) = (name.find("luid_"), value) {
            *engines.entry(&name[at..]).or_default() += value;
        }
    }
    let mut adapters: BTreeMap<u64, BTreeMap<String, f64>> = BTreeMap::new();
    for (engine, value) in engines {
        if let (Some(luid), Some(kind)) = (instance_luid(engine), engine_type(engine)) {
            let busiest = adapters.entry(luid).or_default().entry(kind).or_default();
            *busiest = busiest.max(value.min(100.0));
        }
    }
    adapters
}

/// Identity of an adapter across restarts, which its LUID is not: the PCI IDs, numbered
/// after the identical adapters `before` it.
fn pci_identity(adapter: &GpuAdapter, before: &[&GpuAdapter]) -> String {
    let ids = |a: &GpuAdapter| (a.vendor, a.device, a.subsystem, a.revision);
    let twins = before.iter().filter(|a| ids(a) == ids(adapter)).count();
    let mut identity = format!(
        "pci:{:04x}:{:04x}:{:08x}:{:02x}",
        adapter.vendor, adapter.device, adapter.subsystem, adapter.revision
    );
    if twins > 0 {
        identity.push_str(&format!("#{twins}"));
    }
    identity
}

fn vendor(id: u32) -> String {
    match id {
        0x10DE => "NVIDIA".into(),
        0x1002 | 0x1022 => "AMD".into(),
        0x8086 => "Intel".into(),
        0x1414 => "Microsoft".into(),
        0x5143 => "Qualcomm".into(),
        _ => format!("0x{id:04X}"),
    }
}

impl Collector for Gpu {
    fn collect(&mut self, context: &CollectContext) -> Result<SampleBatch, CollectorError> {
        let mut batch = SampleBatch::default();
        let at = Instant::now();
        self.query.collect()?;
        let warmed = self
            .previous
            .is_some_and(|old| at.saturating_duration_since(old) <= context.period * 3);
        self.previous = Some(at);
        let engines = self.query.values(0);
        if !warmed && engines.is_err() {
            batch.finished_at = Instant::now();
            return Ok(batch);
        }
        let engines = engine_load(&engines?);
        let dedicated = per_adapter(&self.query.values(1)?);
        let shared = per_adapter(&self.query.values(2)?);
        let unknown = dedicated
            .keys()
            .any(|luid| self.adapters.iter().all(|a| a.luid != *luid));
        if unknown && self.listed.elapsed() >= RELIST {
            self.adapters = native::gpu_adapters()?;
            self.listed = Instant::now();
        }
        let hardware: Vec<_> = self.adapters.iter().filter(|a| !a.software).collect();
        for (index, adapter) in hardware.iter().enumerate() {
            let id = format!("gpu:{}", adapter.luid);
            let uses_dedicated = adapter.dedicated >= DEDICATED_MINIMUM;
            batch.devices.push(
                DeviceDescriptor::new(&id, &adapter.name, DeviceKind::Gpu)
                    .persistent_id(pci_identity(adapter, &hardware[..index]))
                    .detail("vendor", vendor(adapter.vendor))
                    .detail(
                        "memory_type",
                        if uses_dedicated {
                            "dedicated"
                        } else {
                            "shared"
                        },
                    )
                    .detail("adapter_luid", format!("0x{:016X}", adapter.luid)),
            );
            let mut push = |metric: &str, unit, value| {
                batch
                    .samples
                    .push(MetricSample::new(&id, metric, unit, value, at));
            };
            // Counters list every adapter they track in the memory set, busy or idle.
            let counted = dedicated.contains_key(&adapter.luid);
            let load = engines.get(&adapter.luid);
            let usage = if !counted {
                SampleValue::Unsupported(Reason::NoGpuCounters)
            } else if warmed {
                SampleValue::Gauge(
                    load.into_iter()
                        .flat_map(|types| types.values())
                        .fold(0.0, |a: f64, &b| a.max(b)),
                )
            } else {
                SampleValue::WarmingUp
            };
            push("usage", Unit::Percent, usage);
            let types = self.engine_types.entry(adapter.luid).or_default();
            types.extend(load.into_iter().flat_map(|types| types.keys().cloned()));
            for kind in types.iter() {
                let value = if warmed {
                    SampleValue::Gauge(
                        load.and_then(|types| types.get(kind))
                            .copied()
                            .unwrap_or(0.0),
                    )
                } else {
                    SampleValue::WarmingUp
                };
                push(&format!("engine:{kind}"), Unit::Percent, value);
            }

            let bytes = |used: Option<&f64>| match used {
                Some(&used) => SampleValue::Gauge(used),
                None => SampleValue::Unsupported(Reason::NoGpuMemoryCounters),
            };
            let (used, total) = if uses_dedicated {
                (dedicated.get(&adapter.luid), adapter.dedicated)
            } else {
                (shared.get(&adapter.luid), adapter.shared)
            };
            let total = total as f64;
            push(
                "memory_usage",
                Unit::Percent,
                match used {
                    Some(&used) if total > 0.0 => {
                        SampleValue::Gauge((used / total * 100.0).min(100.0))
                    }
                    _ => SampleValue::Unsupported(Reason::NoGpuMemoryCounters),
                },
            );
            push("memory_used", Unit::Bytes, bytes(used));
            push("memory_total", Unit::Bytes, SampleValue::Gauge(total));
            push(
                "dedicated_used",
                Unit::Bytes,
                bytes(dedicated.get(&adapter.luid)),
            );
            push(
                "dedicated_total",
                Unit::Bytes,
                SampleValue::Gauge(adapter.dedicated as f64),
            );
            push("shared_used", Unit::Bytes, bytes(shared.get(&adapter.luid)));
            push(
                "shared_total",
                Unit::Bytes,
                SampleValue::Gauge(adapter.shared as f64),
            );
        }
        batch.complete_inventory = true;
        batch.finished_at = Instant::now();
        Ok(batch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(items: &[(&str, f64)]) -> Values {
        items
            .iter()
            .map(|(name, value)| (name.to_string(), Some(*value)))
            .collect()
    }

    #[test]
    fn luid_and_engine_type_are_read_from_instances() {
        assert_eq!(
            instance_luid("pid_10120_luid_0x00000001_0x000101aa_phys_0_eng_0_engtype_3d"),
            Some(0x1_0001_01AA)
        );
        assert_eq!(
            instance_luid("luid_0x00000000_0x0000D1A3_phys_0"),
            Some(0xD1A3)
        );
        assert_eq!(instance_luid("_Total"), None);
        assert_eq!(
            engine_type("pid_4_luid_0x00000000_0x0000D1A3_phys_0_eng_14_engtype_Security_1"),
            Some("security_1".into())
        );
        assert_eq!(engine_type("luid_0x00000000_0x0000D1A3_phys_0"), None);
    }

    #[test]
    fn engines_sum_processes_and_keep_types_separate() {
        let engines = values(&[
            (
                "pid_1_luid_0x00000000_0x000000aa_phys_0_eng_0_engtype_3d",
                30.0,
            ),
            (
                "pid_2_luid_0x00000000_0x000000aa_phys_0_eng_0_engtype_3d",
                25.0,
            ),
            (
                "pid_2_luid_0x00000000_0x000000aa_phys_0_eng_4_engtype_copy",
                40.0,
            ),
            (
                "pid_2_luid_0x00000000_0x000000aa_phys_0_eng_5_engtype_copy",
                10.0,
            ),
            (
                "pid_3_luid_0x00000000_0x000000bb_phys_0_eng_0_engtype_3d",
                80.0,
            ),
            (
                "pid_4_luid_0x00000000_0x000000bb_phys_0_eng_0_engtype_3d",
                70.0,
            ),
        ]);
        let load = engine_load(&engines);
        assert_eq!(load[&0xAA]["3d"], 55.0);
        // Two copy engines: the busier one represents the type.
        assert_eq!(load[&0xAA]["copy"], 40.0);
        assert_eq!(load[&0xBB]["3d"], 100.0);
    }

    #[test]
    fn memory_is_summed_over_physical_nodes() {
        let memory = values(&[
            ("luid_0x00000000_0x000000aa_phys_0", 1024.0),
            ("luid_0x00000000_0x000000aa_phys_1", 512.0),
        ]);
        assert_eq!(per_adapter(&memory).get(&0xAA), Some(&1536.0));
    }
}
