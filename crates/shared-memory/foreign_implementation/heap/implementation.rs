use alloc::vec::Vec;
use core::sync::atomic::AtomicU64;
#[cfg(target_os = "linux")]
use core::time::Duration;

#[cfg(target_os = "linux")]
use crate::WaitError;
use crate::foreign_implementation::region::Region;
use crate::{OutsideError, SharedMemory, Word, WordError};

/// Words on this program's heap, zero when made, for Miri and Kani, which cannot run a host's mapping; built only with
/// `--cfg miri` or `--cfg kani`, which any build can set.
pub struct HeapMapping {
    region: Region,
    #[expect(
        dead_code,
        reason = "held so that the words stay allocated while the region lives; only the region reaches them"
    )]
    words: Vec<AtomicU64>,
}

impl HeapMapping {
    pub fn new(count: usize) -> HeapMapping {
        let words: Vec<AtomicU64> = (0..count).map(|_| AtomicU64::new(0)).collect();
        let base = words.as_ptr().cast::<u8>().cast_mut();
        #[expect(
            unsafe_code,
            reason = "the heap's words, which only the region reaches"
        )]
        // SAFETY: the vector is this value's own, freed only when it drops, after the region, and never grown; `AtomicU64`
        // is aligned to 8; nothing but the region reaches the words, through a pointer `Vec::as_ptr` gives without a
        // reference to them, into cells that allow writes, and no reference to them is formed after but the region's
        // one-word borrows. Unlike a host's mapping these bytes are a Rust allocation, so two threads racing on them with
        // the region's copies or volatile accesses is a data race, which Miri reports.
        let region = unsafe { Region::new(base, words.len() * size_of::<u64>()) };
        HeapMapping { region, words }
    }
}

impl SharedMemory for HeapMapping {
    fn len(&self) -> u64 {
        self.region.len()
    }

    fn read(&self, at: u64, into: &mut [u8]) -> Result<(), OutsideError> {
        self.region.read(at, into)
    }

    fn write(&self, at: u64, from: &[u8]) -> Result<(), OutsideError> {
        self.region.write(at, from)
    }

    fn load<W: Word>(&self, at: u64) -> Result<W, WordError> {
        self.region.load(at)
    }

    fn store<W: Word>(&self, at: u64, value: W) -> Result<(), WordError> {
        self.region.store(at, value)
    }

    fn swap(&self, at: u64, value: u64) -> Result<u64, WordError> {
        self.region.swap(at, value)
    }

    fn compare_exchange(&self, at: u64, current: u32, new: u32) -> Result<u32, WordError> {
        self.region.compare_exchange(at, current, new)
    }

    #[cfg(target_os = "linux")]
    fn wait(&self, at: u64, expected: u32, timeout: Option<Duration>) -> Result<(), WaitError> {
        self.region.wait(at, expected, timeout)
    }

    #[cfg(target_os = "linux")]
    fn wake(&self, at: u64, limit: u32) -> Result<u32, WaitError> {
        self.region.wake(at, limit)
    }
}
