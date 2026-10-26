//! Delivery of state transitions that a worker reports once before waiting. Samples may
//! be dropped under load; these transitions wait for queue space while remaining
//! cancellable.

use crossbeam_channel::{SendTimeoutError, Sender};
use std::time::Duration;

pub(crate) fn deliver<T>(
    sender: &Sender<T>,
    mut value: T,
    stopped: impl Fn() -> bool,
    wake: impl Fn(),
) -> bool {
    loop {
        if stopped() {
            return false;
        }
        match sender.send_timeout(value, Duration::from_millis(25)) {
            Ok(()) => {
                wake();
                return true;
            }
            Err(SendTimeoutError::Timeout(pending)) => {
                value = pending;
                wake();
            }
            Err(SendTimeoutError::Disconnected(_)) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::bounded;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
    };

    #[test]
    fn a_full_queue_keeps_the_transition_until_space_is_available() {
        let (sender, receiver) = bounded(1);
        sender.send(1).unwrap();
        let (blocked, waiting) = bounded(1);
        let worker = thread::spawn(move || {
            deliver(
                &sender,
                2,
                || false,
                || {
                    let _ = blocked.try_send(());
                },
            )
        });
        waiting.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(receiver.recv().unwrap(), 1);
        assert_eq!(receiver.recv_timeout(Duration::from_secs(2)).unwrap(), 2);
        assert!(worker.join().unwrap());
    }

    #[test]
    fn cancellation_ends_a_transition_waiting_for_a_full_queue() {
        let (sender, _receiver) = bounded(1);
        sender.send(1).unwrap();
        let stopped = Arc::new(AtomicBool::new(false));
        let (blocked, waiting) = bounded(1);
        let worker = thread::spawn({
            let stopped = stopped.clone();
            move || {
                deliver(
                    &sender,
                    2,
                    || stopped.load(Ordering::Acquire),
                    || {
                        let _ = blocked.try_send(());
                    },
                )
            }
        });
        waiting.recv_timeout(Duration::from_secs(2)).unwrap();
        stopped.store(true, Ordering::Release);
        assert!(!worker.join().unwrap());
    }
}
