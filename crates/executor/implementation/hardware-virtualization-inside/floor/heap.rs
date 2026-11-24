//! The inside's heap: `talc` over blocks of guest-physical frames reached through the linear map, taken as
//! the heap grows.

use core::alloc::Layout;
use core::ptr::NonNull;

use talc::base::binning::Binning;
use talc::base::{CHUNK_UNIT, Talc};
use talc::min_first_heap_size;
use talc::source::Source;

use super::with_frames;
use crate::layout::{BLOCK_SIZE, linear};

/// Takes a run of blocks of frames each time the heap runs out, as many as the allocation needs.
#[derive(Debug)]
pub struct Blocks;

// SAFETY: it only claims memory that nothing else uses, through the `Talc` it is given.
#[expect(
    unsafe_code,
    reason = "promises talc that acquiring takes no talc lock and no global allocation: acquire only takes the frames' spin lock, which does not allocate; code that allocates while it holds that lock, or a panic in acquire, would deadlock, and only convention prevents it; stays a one-off"
)]
unsafe impl Source for Blocks {
    #[expect(
        unsafe_code,
        reason = "UNSOUND: hands talc a run of frames take_blocks gave, never given before, as memory only the heap reaches, which safe code can break: tables.rs give and clear, Table (its root private now, but Copy, so a copy walks tables free gave back), native.rs frame_bytes and write_page, and floor write_frame take any frame number; Words over the linear map (GuestFrame) replaces those, and this line then stays a one-off"
    )]
    fn acquire<B: Binning>(talc: &mut Talc<Self, B>, layout: Layout) -> Result<(), ()> {
        // The allocation with its worst alignment, and talc's own chunk on either side of a claim.
        let mut needed = layout.size() + layout.align() + CHUNK_UNIT + CHUNK_UNIT;
        if !talc.is_metadata_established() {
            needed += min_first_heap_size::<B>();
        }
        let length = (needed as u64).next_multiple_of(BLOCK_SIZE);
        let run = with_frames(|frames| frames.take_blocks(length / BLOCK_SIZE)).map_err(|_| ())?;
        // SAFETY: the run's frames are fresh; that only this heap reaches them from here on is not enforced
        // (see the reason above).
        unsafe { talc.claim(linear(run) as *mut u8, length as usize) }
            .map(|_: NonNull<u8>| ())
            .ok_or(())
    }
}

#[global_allocator]
static HEAP: talc::TalcLock<spin::mutex::SpinMutex<()>, Blocks> = talc::TalcLock::new(Blocks);
