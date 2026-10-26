//! Measures view-copy cost on synthetic multi-host data. Run in release mode with
//! `cargo run --release -p qqq-engine --example profile_snapshot`.

use qqq_domain::{MetricDescriptor, MetricKey, MetricReading, SampleState, Unit};
use qqq_engine::{HistoryPoint, MonitorView};
use std::{
    hint::black_box,
    sync::Arc,
    time::{Duration, Instant},
};

fn main() {
    let mut view = MonitorView::empty();
    for host in 0..64 {
        for core in 0..128 {
            let key = MetricKey::new(format!("ssh:host-{host}/core:{core}"), "usage");
            Arc::make_mut(&mut view.metrics).insert(
                key.clone(),
                MetricReading {
                    descriptor: MetricDescriptor {
                        key,
                        unit: Unit::Percent,
                    },
                    value: Some(50.0),
                    session_bytes: 0,
                    state: SampleState::Ready,
                    reason: None,
                    last_success: Some(view.now),
                    last_attempt: view.now,
                    period: Duration::from_secs(1),
                },
            );
        }
    }
    for metric in 0..16 {
        Arc::make_mut(&mut view.history).insert(
            MetricKey::new(format!("chart:{metric}"), "usage"),
            (0..400)
                .map(|point| HistoryPoint {
                    elapsed: point as f64,
                    value: Some(50.0),
                })
                .collect(),
        );
    }
    const COPIES: usize = 200;
    let start = Instant::now();
    for _ in 0..COPIES {
        let mut copied = view.clone();
        copied.metrics = Arc::new((*view.metrics).clone());
        copied.history = Arc::new((*view.history).clone());
        black_box(copied);
    }
    let deep = start.elapsed();
    let start = Instant::now();
    for _ in 0..COPIES {
        black_box(view.clone());
    }
    let shared = start.elapsed();
    println!(
        "{} metrics, {} charts, {COPIES} copies: deep={deep:?}, shared={shared:?}",
        view.metrics.len(),
        view.history.len()
    );
}
