//! The inside's heap: `talc` over blocks of guest-physical frames reached through the linear map, taken as
//! the heap grows.

use core::alloc::Layout;
use core::ptr::NonNull;

use talc::base::Talc;
use talc::base::binning::Binning;
use talc::source::Source;

use super::with_frames;
use crate::layout::{BLOCK_SIZE, linear};

/// Takes one block of frames each time the heap runs out.
#[derive(Debug)]
pub struct Blocks;

// SAFETY: it only claims memory that nothing else uses, through the `Talc` it is given.
unsafe impl Source for Blocks {
    fn acquire<B: Binning>(talc: &mut Talc<Self, B>, layout: Layout) -> Result<(), ()> {
        if layout.size() + layout.align() > BLOCK_SIZE as usize {
            return Err(());
        }
        let block = with_frames(|frames| frames.take_block()).map_err(|_| ())?;
        // SAFETY: the block's frames are fresh and reached only through this heap from here on.
        unsafe { talc.claim(linear(block) as *mut u8, BLOCK_SIZE as usize) }
            .map(|_: NonNull<u8>| ())
            .ok_or(())
    }
}

#[global_allocator]
static HEAP: talc::TalcLock<spin::mutex::SpinMutex<()>, Blocks> = talc::TalcLock::new(Blocks);
