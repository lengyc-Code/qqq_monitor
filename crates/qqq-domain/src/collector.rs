use crate::{DeviceDescriptor, MetricSample};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub enum SamplingPeriod {
    Configured,
    Fixed(Duration),
}

#[derive(Clone, Copy, Debug)]
pub struct CollectorDescriptor {
    pub id: &'static str,
    pub period: SamplingPeriod,
}

pub struct CollectContext {
    pub period: Duration,
}

#[derive(Debug)]
pub struct SampleBatch {
    pub devices: Vec<DeviceDescriptor>,
    pub samples: Vec<MetricSample>,
    /// Only authoritative complete inventories may mark absent devices offline.
    pub complete_inventory: bool,
    pub started_at: Instant,
    pub finished_at: Instant,
}

impl Default for SampleBatch {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            devices: Vec::new(),
            samples: Vec::new(),
            complete_inventory: false,
            started_at: now,
            finished_at: now,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct CollectorError(pub String);

pub trait Collector {
    fn collect(&mut self, context: &CollectContext) -> Result<SampleBatch, CollectorError>;
}

/// Factories cross threads; actual collectors and thread-bound native handles do not.
pub trait CollectorFactory: Send + Sync {
    fn descriptor(&self) -> CollectorDescriptor;
    fn create(&self) -> Result<Box<dyn Collector>, CollectorError>;
}
