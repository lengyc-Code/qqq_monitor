use qqq_domain::*;
use std::{collections::BTreeMap, time::Instant};

#[derive(Default)]
pub(crate) struct Normalizer {
    baselines: BTreeMap<MetricKey, (u64, Instant)>,
}

impl Normalizer {
    pub fn reset(&mut self, key: &MetricKey) {
        self.baselines.remove(key);
    }
    pub fn apply(&mut self, reading: &mut MetricReading, sample: MetricSample) {
        reading.last_attempt = sample.at;
        reading.reason = None;
        let value = match sample.value {
            SampleValue::Gauge(value)
                if value.is_finite()
                    && value >= 0.0
                    && (reading.descriptor.unit != Unit::Percent || value <= 100.0) =>
            {
                Some(value)
            }
            SampleValue::Gauge(_) => {
                self.reset(&reading.descriptor.key);
                reading.state = SampleState::Failed;
                reading.reason = Some("采集值超出有效范围".into());
                None
            }
            SampleValue::Counter(total) if reading.descriptor.unit == Unit::BytesPerSecond => {
                let previous = self
                    .baselines
                    .insert(reading.descriptor.key.clone(), (total, sample.at));
                previous
                    .and_then(|(old, at)| {
                        let elapsed = sample.at.saturating_duration_since(at);
                        if elapsed.is_zero() || elapsed > reading.period * 3 || total < old {
                            return None;
                        }
                        let delta = total - old;
                        reading.session_bytes = reading.session_bytes.saturating_add(delta);
                        Some(delta as f64 / elapsed.as_secs_f64())
                    })
                    .or_else(|| {
                        reading.state = SampleState::WarmingUp;
                        None
                    })
            }
            SampleValue::Counter(_) => {
                self.reset(&reading.descriptor.key);
                reading.state = SampleState::Failed;
                reading.reason = Some("计数器单位无效".into());
                None
            }
            SampleValue::WarmingUp => {
                self.reset(&reading.descriptor.key);
                reading.state = SampleState::WarmingUp;
                None
            }
            SampleValue::Unsupported(reason) => {
                self.reset(&reading.descriptor.key);
                reading.state = SampleState::Unsupported;
                reading.reason = Some(reason);
                None
            }
            SampleValue::Failed(reason) => {
                self.reset(&reading.descriptor.key);
                reading.state = SampleState::Failed;
                reading.reason = Some(reason);
                None
            }
        };
        if let Some(value) = value {
            reading.value = Some(value);
            reading.last_success = Some(sample.at);
            reading.state = SampleState::Ready;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn reading(unit: Unit, at: Instant) -> MetricReading {
        MetricReading {
            descriptor: MetricDescriptor {
                key: MetricKey::new("device", "rate"),
                unit,
            },
            value: None,
            session_bytes: 0,
            state: SampleState::WarmingUp,
            reason: None,
            last_success: None,
            last_attempt: at,
            period: Duration::from_secs(1),
        }
    }
    fn sample(value: SampleValue, at: Instant, unit: Unit) -> MetricSample {
        MetricSample::new("device", "rate", unit, value, at)
    }
    #[test]
    fn counters_use_actual_elapsed_and_rewarm_after_reset_or_failure() {
        let at = Instant::now();
        let mut normalizer = Normalizer::default();
        let mut r = reading(Unit::BytesPerSecond, at);
        normalizer.apply(
            &mut r,
            sample(SampleValue::Counter(10000), at, Unit::BytesPerSecond),
        );
        assert_eq!(r.state, SampleState::WarmingUp);
        assert_eq!(r.value, None);
        normalizer.apply(
            &mut r,
            sample(
                SampleValue::Counter(11200),
                at + Duration::from_millis(1200),
                Unit::BytesPerSecond,
            ),
        );
        assert_eq!(r.value, Some(1000.0));
        assert_eq!(r.session_bytes, 1200);
        normalizer.apply(
            &mut r,
            sample(
                SampleValue::Counter(100),
                at + Duration::from_millis(2200),
                Unit::BytesPerSecond,
            ),
        );
        assert_eq!(r.state, SampleState::WarmingUp);
        normalizer.apply(
            &mut r,
            sample(
                SampleValue::Counter(1100),
                at + Duration::from_millis(3200),
                Unit::BytesPerSecond,
            ),
        );
        assert_eq!(r.value, Some(1000.0));
        normalizer.apply(
            &mut r,
            sample(
                SampleValue::Failed("断连".into()),
                at + Duration::from_millis(4200),
                Unit::BytesPerSecond,
            ),
        );
        assert_eq!(r.state, SampleState::Failed);
        normalizer.apply(
            &mut r,
            sample(
                SampleValue::Counter(2000),
                at + Duration::from_millis(5200),
                Unit::BytesPerSecond,
            ),
        );
        assert_eq!(r.state, SampleState::WarmingUp);
    }
    #[test]
    fn gauges_are_not_differentiated_and_zero_is_valid() {
        let at = Instant::now();
        let mut normalizer = Normalizer::default();
        let mut r = reading(Unit::BytesPerSecond, at);
        normalizer.apply(
            &mut r,
            sample(SampleValue::Gauge(2500.0), at, Unit::BytesPerSecond),
        );
        assert_eq!(r.value, Some(2500.0));
        normalizer.apply(
            &mut r,
            sample(
                SampleValue::Gauge(0.0),
                at + Duration::from_secs(2),
                Unit::BytesPerSecond,
            ),
        );
        assert_eq!(r.state, SampleState::Ready);
        assert_eq!(r.value, Some(0.0));
        normalizer.apply(
            &mut r,
            sample(
                SampleValue::Gauge(f64::NAN),
                at + Duration::from_secs(3),
                Unit::BytesPerSecond,
            ),
        );
        assert_eq!(r.state, SampleState::Failed);
    }
    #[test]
    fn stale_uses_last_success_and_each_metrics_period() {
        let at = Instant::now();
        let mut normalizer = Normalizer::default();
        let mut r = reading(Unit::Bytes, at);
        r.period = Duration::from_secs(30);
        normalizer.apply(&mut r, sample(SampleValue::Gauge(123.0), at, Unit::Bytes));
        assert_eq!(
            r.effective_state(at + Duration::from_secs(4)),
            SampleState::Ready
        );
        assert_eq!(
            r.effective_state(at + Duration::from_secs(91)),
            SampleState::Stale
        );
    }
}
