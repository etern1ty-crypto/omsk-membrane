//! Bounded, in-process single-producer/single-consumer channels.
//!
//! The implementation deliberately uses the standard library rather than an
//! unaudited custom unsafe ring. It can block and make system calls. It is not
//! shared-memory IPC and makes no lock-free or throughput claim.
//!
//! ```
//! let (mut producer, mut consumer) = synapse::bounded(4).unwrap();
//! producer.send(42).unwrap();
//! drop(producer);
//! assert_eq!(consumer.recv().unwrap(), 42);
//! assert!(consumer.recv().is_err());
//! ```

#![forbid(unsafe_code)]

use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::Duration;

pub use std::sync::mpsc::{RecvError, RecvTimeoutError, SendError, TryRecvError, TrySendError};

/// Deliberately not Clone or Sync: ownership identifies the single producer.
pub struct Producer<T> {
    inner: SyncSender<T>,
    not_sync: PhantomData<Cell<()>>,
}

/// Deliberately not Clone or Sync: ownership identifies the single consumer.
pub struct Consumer<T> {
    inner: Receiver<T>,
    not_sync: PhantomData<Cell<()>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityError;

impl fmt::Display for CapacityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("channel capacity must be between 1 and 65536")
    }
}

impl std::error::Error for CapacityError {}

pub fn bounded<T>(capacity: usize) -> Result<(Producer<T>, Consumer<T>), CapacityError> {
    if !(1..=65_536).contains(&capacity) {
        return Err(CapacityError);
    }
    let (sender, receiver) = mpsc::sync_channel(capacity);
    Ok((
        Producer {
            inner: sender,
            not_sync: PhantomData,
        },
        Consumer {
            inner: receiver,
            not_sync: PhantomData,
        },
    ))
}

impl<T> Producer<T> {
    /// Block under backpressure; receiver drop wakes a blocked producer.
    pub fn send(&mut self, value: T) -> Result<(), SendError<T>> {
        self.inner.send(value)
    }

    pub fn try_send(&mut self, value: T) -> Result<(), TrySendError<T>> {
        self.inner.try_send(value)
    }
}

impl<T> Consumer<T> {
    /// Buffered values are drained before disconnection is reported.
    pub fn recv(&mut self) -> Result<T, RecvError> {
        self.inner.recv()
    }

    pub fn recv_timeout(&mut self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        self.inner.recv_timeout(timeout)
    }

    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        self.inner.try_recv()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn rejects_zero_and_excessive_capacity() {
        assert!(bounded::<u8>(0).is_err());
        assert!(bounded::<u8>(65_537).is_err());
    }

    #[test]
    fn full_queue_returns_original_value() {
        let (mut producer, mut consumer) = bounded(1).unwrap();
        producer.try_send(7).unwrap();
        assert!(matches!(producer.try_send(8), Err(TrySendError::Full(8))));
        assert_eq!(consumer.recv().unwrap(), 7);
        producer.send(8).unwrap();
        assert_eq!(consumer.recv().unwrap(), 8);
    }

    #[test]
    fn closes_after_draining() {
        let (mut producer, mut consumer) = bounded(2).unwrap();
        producer.send("a").unwrap();
        producer.send("b").unwrap();
        drop(producer);
        assert_eq!(consumer.recv().unwrap(), "a");
        assert_eq!(consumer.recv().unwrap(), "b");
        assert!(consumer.recv().is_err());
    }

    #[test]
    fn receiver_drop_releases_blocked_sender() {
        let (mut producer, consumer) = bounded(1).unwrap();
        producer.send(1).unwrap();
        let worker = thread::spawn(move || producer.send(2));
        drop(consumer);
        assert!(worker.join().unwrap().is_err());
    }

    #[test]
    fn timed_receive_does_not_spin() {
        let (_producer, mut consumer) = bounded::<u8>(1).unwrap();
        assert_eq!(
            consumer.recv_timeout(Duration::from_millis(2)),
            Err(RecvTimeoutError::Timeout)
        );
    }

    #[test]
    fn threaded_fifo_stress() {
        let (mut producer, mut consumer) = bounded(3).unwrap();
        let worker = thread::spawn(move || {
            for value in 0..100_000 {
                producer.send(value).unwrap();
            }
        });
        for expected in 0..100_000 {
            assert_eq!(consumer.recv().unwrap(), expected);
        }
        assert!(consumer.recv().is_err());
        worker.join().unwrap();
    }
}
