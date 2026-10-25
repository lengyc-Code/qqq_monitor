use qqq_domain::MetricKey;
use std::{
    collections::{BTreeMap, VecDeque},
    mem::size_of,
};

#[derive(Clone, Copy, Debug)]
pub struct HistoryPoint {
    pub elapsed: f64,
    pub value: Option<f64>,
}

/// A point as the history keeps it, in 16 bytes: a gap is NaN, which no valid reading is.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stored {
    pub elapsed: f64,
    value: f64,
}

impl From<HistoryPoint> for Stored {
    fn from(point: HistoryPoint) -> Self {
        Self {
            elapsed: point.elapsed,
            value: point.value.unwrap_or(f64::NAN),
        }
    }
}

impl From<Stored> for HistoryPoint {
    fn from(point: Stored) -> Self {
        Self {
            elapsed: point.elapsed,
            value: (!point.value.is_nan()).then_some(point.value),
        }
    }
}

pub(crate) struct History {
    pub series: BTreeMap<MetricKey, VecDeque<Stored>>,
    budget: usize,
}

impl Default for History {
    fn default() -> Self {
        Self {
            series: BTreeMap::new(),
            budget: 16 * 1024 * 1024,
        }
    }
}

impl History {
    pub fn append(&mut self, key: MetricKey, point: HistoryPoint) {
        let values = self.series.entry(key).or_default();
        if values.back().is_some_and(|p| p.elapsed >= point.elapsed) {
            return;
        }
        values.push_back(point.into());
        while values.len() > 1801 {
            values.pop_front();
        }
    }
    pub fn prune(&mut self, now: f64) {
        self.series.retain(|_, values| {
            while values.front().is_some_and(|p| p.elapsed < now - 900.0) {
                values.pop_front();
            }
            if values.capacity() > values.len().max(64) * 2 {
                values.shrink_to_fit();
            }
            !values.is_empty()
        });
        // Trim oldest data fairly, accounting for allocated capacity, not only length.
        while self.bytes() > self.budget {
            let Some(key) = self
                .series
                .iter()
                .filter(|(_, v)| !v.is_empty())
                .min_by(|a, b| {
                    a.1.front()
                        .unwrap()
                        .elapsed
                        .total_cmp(&b.1.front().unwrap().elapsed)
                })
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            let values = self.series.get_mut(&key).unwrap();
            let remove = (values.len() / 4).max(1);
            values.drain(..remove);
            values.shrink_to_fit();
            if values.is_empty() {
                self.series.remove(&key);
            }
        }
    }
    pub fn bytes(&self) -> usize {
        self.series
            .iter()
            .map(|(key, v)| {
                v.capacity() * size_of::<Stored>()
                    + key.device.capacity()
                    + key.metric.capacity()
                    + size_of::<(MetricKey, VecDeque<Stored>)>()
            })
            .sum()
    }
    pub fn project(&self, key: &MetricKey, start: f64, limit: usize) -> Vec<HistoryPoint> {
        let Some(values) = self.series.get(key) else {
            return Vec::new();
        };
        // Points are in time order, so the window starts where the search lands.
        let first = values.partition_point(|p| p.elapsed < start);
        let points: Vec<HistoryPoint> = values.range(first..).map(|&p| p.into()).collect();
        if points.len() <= limit {
            return points;
        }
        let bucket = points.len().div_ceil((limit / 3).max(1));
        let mut result = Vec::with_capacity(limit);
        for chunk in points.chunks(bucket) {
            let mut chosen = Vec::new();
            // A mixed bucket is conservatively a gap: never bridge a hidden failure interval.
            if let Some(gap) = chunk.iter().find(|p| p.value.is_none()) {
                result.push(*gap);
                continue;
            }
            if let Some((index, _)) = chunk
                .iter()
                .enumerate()
                .filter(|(_, p)| p.value.is_some())
                .min_by(|a, b| a.1.value.unwrap().total_cmp(&b.1.value.unwrap()))
            {
                chosen.push(index);
            }
            if let Some((index, _)) = chunk
                .iter()
                .enumerate()
                .filter(|(_, p)| p.value.is_some())
                .max_by(|a, b| a.1.value.unwrap().total_cmp(&b.1.value.unwrap()))
            {
                chosen.push(index);
            }
            chosen.sort_unstable();
            chosen.dedup();
            for index in chosen {
                result.push(chunk[index]);
            }
        }
        result.truncate(limit);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_bounds_use_time_points_and_allocated_budget() {
        let key = MetricKey::new("cpu", "usage");
        let mut history = History::default();
        for n in 0..3000 {
            history.append(
                key.clone(),
                HistoryPoint {
                    elapsed: n as f64 / 2.0,
                    value: Some(n as f64),
                },
            );
        }
        assert_eq!(history.series[&key].len(), 1801);
        history.prune(2000.0);
        assert!(history.series[&key].iter().all(|p| p.elapsed >= 1100.0));
        history.budget = 1024;
        history.prune(2000.0);
        assert!(history.bytes() <= 1024);
        history.prune(4000.0);
        assert!(history.series.is_empty());
    }
    #[test]
    fn stored_points_take_16_bytes_and_keep_gaps() {
        assert_eq!(size_of::<Stored>(), 16);
        let gap = HistoryPoint::from(Stored::from(HistoryPoint {
            elapsed: 1.0,
            value: None,
        }));
        assert_eq!((gap.elapsed, gap.value), (1.0, None));
        let zero = HistoryPoint::from(Stored::from(HistoryPoint {
            elapsed: 2.0,
            value: Some(0.0),
        }));
        assert_eq!(zero.value, Some(0.0));
    }

    #[test]
    fn projection_preserves_extrema_order_and_does_not_bridge_gaps() {
        let key = MetricKey::new("cpu", "usage");
        let mut history = History::default();
        for n in 0..100 {
            history.append(
                key.clone(),
                HistoryPoint {
                    elapsed: n as f64,
                    value: if n == 50 {
                        None
                    } else if n == 80 {
                        Some(1000.0)
                    } else {
                        Some(n as f64)
                    },
                },
            );
        }
        let output = history.project(&key, 0.0, 15);
        assert!(output.len() <= 15);
        assert!(output.windows(2).all(|w| w[0].elapsed < w[1].elapsed));
        assert!(output.iter().any(|p| p.value.is_none()));
        assert!(output.iter().any(|p| p.value == Some(1000.0)));
        assert_eq!(history.series[&key].len(), 100);
    }
}
