//! What each buffer of a batch must still hold: every byte outside the ranges its calls were allowed to change.
//!
//! So the canaries of a call are the bytes of its own buffer on both sides of the range it may touch, and every other
//! byte of every live buffer.

use std::cell::RefCell;
use std::rc::Rc;

/// A buffer's expected bytes, and which of them a call was allowed to change.
pub struct Shadow {
    expected: Vec<u8>,
    free: Vec<bool>,
    /// The ranges written since the last submission, which a device lost before it may drop.
    pending: Vec<(u64, u64)>,
}

/// One buffer's shadow, shared by every handle the batch keeps of it.
pub type Tracked = Rc<RefCell<Shadow>>;

/// Returns the bytes of a range that lie inside a buffer of `length`, without overflow.
fn inside(length: usize, offset: u64, size: u64) -> std::ops::Range<usize> {
    let start = usize::try_from(offset).unwrap_or(usize::MAX).min(length);
    let end = usize::try_from(offset.saturating_add(size))
        .unwrap_or(usize::MAX)
        .min(length);
    start..end.max(start)
}

impl Shadow {
    /// Returns the shadow of a new buffer of `length` bytes, all zero.
    pub fn new(length: u64) -> Tracked {
        let length = usize::try_from(length).expect("a batch's buffer fits in memory");
        Rc::new(RefCell::new(Shadow {
            expected: vec![0; length],
            free: vec![false; length],
            pending: Vec::new(),
        }))
    }

    /// Returns the buffer's length.
    pub fn length(&self) -> u64 {
        self.expected.len() as u64
    }

    /// Notes bytes the CPU wrote, in order with the GPU's work: they are known again.
    pub fn wrote(&mut self, offset: u64, bytes: &[u8]) {
        let range = inside(self.expected.len(), offset, bytes.len() as u64);
        let count = range.len();
        self.expected[range.clone()].copy_from_slice(&bytes[..count]);
        self.free[range].fill(false);
    }

    /// Notes bytes written as `wrote` does, which a device lost before the next submission may drop.
    pub fn wrote_pending(&mut self, offset: u64, bytes: &[u8]) {
        self.wrote(offset, bytes);
        self.pending.push((offset, bytes.len() as u64));
    }

    /// Notes that the work so far was submitted, so that its writes stand.
    pub fn submitted(&mut self) {
        self.pending.clear();
    }

    /// Notes that the device was lost: the writes since the last submission may have been dropped.
    pub fn lost(&mut self) {
        for (offset, size) in std::mem::take(&mut self.pending) {
            self.allow(offset, size);
        }
    }

    /// Notes a range a call may change.
    pub fn allow(&mut self, offset: u64, size: u64) {
        let range = inside(self.free.len(), offset, size);
        self.free[range].fill(true);
    }

    /// Notes that every byte may have changed.
    pub fn allow_all(&mut self) {
        self.free.fill(true);
    }

    /// Returns how many of its bytes calls were allowed to change, which are no canaries.
    pub fn free(&self) -> u64 {
        self.free.iter().filter(|free| **free).count() as u64
    }

    /// Returns how many bytes read back changed where no call was allowed to change them.
    ///
    /// It prints the first and last of them, and the bytes from the first.
    pub fn changed(&self, read: &[u8]) -> u64 {
        let mut changed = 0u64;
        let mut first: Option<(usize, usize)> = None;
        for (at, ((byte, expected), free)) in
            read.iter().zip(&self.expected).zip(&self.free).enumerate()
        {
            if !free && byte != expected {
                changed = changed.saturating_add(1);
                first = Some(first.map_or((at, at), |(start, _)| (start, at)));
            }
        }
        if let Some((start, end)) = first {
            let shown = start..end.min(start.saturating_add(16)).saturating_add(1);
            eprintln!(
                "CANARY {changed} bytes of a buffer of {} changed outside every allowed range, from byte {start} to \
                 {end}: {:?} where {:?} was",
                self.expected.len(),
                &read[shown.clone()],
                &self.expected[shown]
            );
        }
        changed
    }
}
