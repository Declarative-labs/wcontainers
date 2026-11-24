use alloc::boxed::Box;
use alloc::sync::Arc;
use core::ptr::NonNull;
use core::sync::atomic::AtomicU64;

use crate::Mapped;
use crate::seal::Sealed;

/// Words on this program's heap, zero when made, built only for Miri and Kani, which cannot run a host's mapping.
pub struct HeapMapping {
    words: Box<[AtomicU64]>,
}

impl HeapMapping {
    pub fn new(count: usize) -> Arc<HeapMapping> {
        Arc::new(HeapMapping {
            words: (0..count).map(|_| AtomicU64::new(0)).collect(),
        })
    }
}

impl Sealed for HeapMapping {}

#[expect(unsafe_code, reason = "the heap's words, which only `Words` reaches")]
// SAFETY: the box is this value's own and is freed only when it drops; `[AtomicU64]` is aligned to 8, and its size in
// bytes is a multiple of 8 and never changes. Nothing but `base` hands out its address, and nothing writes it except
// through that address. The pointer comes from a shared reference to the whole slice, whose cells allow writes.
unsafe impl Mapped for HeapMapping {
    fn base(&self) -> NonNull<u64> {
        NonNull::from(&*self.words).cast()
    }

    fn len(&self) -> usize {
        size_of_val(&*self.words)
    }
}
