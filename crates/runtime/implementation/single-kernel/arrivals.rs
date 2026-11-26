//! What the system's threads hand the kernel: the network's changes and the standard input's bytes.
//! Each arrival wakes the system's delivery thread and ends a free turn.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Duration;

use crate::StopFlag;

/// What arrived from outside for the kernel.
pub(crate) enum Arrival {
    /// A socket can be read or written, or a connection finished connecting; `key` is its key in
    /// the network's poller.
    Ready {
        key: usize,
        readable: bool,
        writable: bool,
    },
    /// A name lookup has its answer.
    Answered,
    /// Bytes read from the standard input, within the credit the kernel gave.
    Input(Vec<u8>),
    /// The standard input ended, or failed with this error.
    InputEnded(Option<std::io::Error>),
}

/// Arrivals from the network's threads and the input's reader, which wake the delivery thread and end a free turn.
#[derive(Default)]
pub(crate) struct Arrivals {
    queue: Mutex<Vec<Arrival>>,
    /// Whether the queue may hold something: read at every exit without the lock.
    pending: AtomicBool,
    changed: Condvar,
    flag: Mutex<Option<StopFlag>>,
    /// Whether the system they arrive for is gone.
    closed: AtomicBool,
}

impl Arrivals {
    pub(crate) fn push(&self, arrival: Arrival) {
        self.queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(arrival);
        self.pending.store(true, Ordering::SeqCst);
        self.changed.notify_all();
        if let Some(flag) = &*self.flag.lock().unwrap_or_else(PoisonError::into_inner) {
            flag.raise();
        }
    }

    pub(crate) fn set_flag(&self, flag: StopFlag) {
        *self.flag.lock().unwrap_or_else(PoisonError::into_inner) = Some(flag);
    }

    /// What arrived, if anything did.
    pub(crate) fn take(&self) -> Vec<Arrival> {
        if !self.pending.swap(false, Ordering::SeqCst) {
            return Vec::new();
        }
        std::mem::take(&mut *self.queue.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Wakes every wait for good: the system they arrive for is gone.
    pub(crate) fn close(&self) {
        let _queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        self.closed.store(true, Ordering::SeqCst);
        self.changed.notify_all();
    }

    pub(crate) fn closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Waits until something arrives, the arrivals close, or `timeout` passes (for ever when none).
    pub(crate) fn wait(&self, timeout: Option<Duration>) {
        let queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
        if !queue.is_empty() || self.closed() {
            return;
        }
        let _queue = match timeout {
            Some(timeout) => self
                .changed
                .wait_timeout(queue, timeout)
                .map(|(guard, _)| guard)
                .unwrap_or_else(|poisoned| poisoned.into_inner().0),
            None => self
                .changed
                .wait(queue)
                .unwrap_or_else(PoisonError::into_inner),
        };
    }
}
