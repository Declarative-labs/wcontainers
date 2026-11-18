//! The link: the one way the inside asks the host for something. The inside writes a [`Link`] page and rings
//! the doorbell; the monitor answers in the same page and resumes the virtual CPU.

use wcontainers_abi::PAGE_SIZE;

/// What the monitor tells a virtual CPU when it starts it, in memory the monitor wrote. Its inside address is
/// in x0 at entry.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Start {
    /// Bytes of guest-physical memory from [`crate::layout::MEMORY_BASE`].
    pub memory: u64,
    /// The first guest-physical address the inside may take frames from; everything below is the image, the
    /// boot tables, the stacks and the link pages.
    pub free: u64,
    /// The guest-physical address of this virtual CPU's link page.
    pub link: u64,
    /// This virtual CPU's index, which is also its redistributor's.
    pub cpu: u64,
    /// Virtual CPUs the machine can have, so redistributors the interrupt controller has.
    pub cpus: u64,
    /// The host's page size: memory is given back in whole host pages.
    pub host_page: u64,
    /// The guest-physical address of the top-level table of the boot map of the inside's half.
    pub boot_root: u64,
    /// The interrupt number of the virtual timer.
    pub timer_interrupt: u64,
    /// The interrupt number of the performance counters' overflow.
    pub counter_interrupt: u64,
    /// The first interrupt number the host may raise.
    pub first_raised_interrupt: u64,
    /// 1 when the machine gives its virtual CPUs EL2 (they still enter at EL1).
    pub el2: u64,
}

/// Words of arguments and answers in a [`Link`].
pub const LINK_WORDS: usize = 8;

/// Bytes of text or data a [`Link`] carries.
pub const LINK_BYTES: usize = PAGE_SIZE as usize - (LINK_WORDS + 2) * size_of::<u64>();

/// One request and its answer.
#[repr(C)]
pub struct Link {
    /// A [`Request`].
    pub request: u64,
    /// Arguments in, answers out.
    pub words: [u64; LINK_WORDS],
    /// Bytes used of `bytes`.
    pub length: u64,
    /// Text or data.
    pub bytes: [u8; LINK_BYTES],
}

const _: () = assert!(size_of::<Link>() as u64 == PAGE_SIZE);

macro_rules! requests {
    ($($(#[$doc:meta])* $name:ident,)*) => {
        /// What the inside asks.
        #[repr(u64)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Request { $($(#[$doc])* $name,)* }

        impl Request {
            /// The request a [`Link::request`] word holds.
            pub fn from_word(word: u64) -> Option<Request> {
                [$(Request::$name,)*].into_iter().find(|r| *r as u64 == word)
            }
        }
    };
}

requests! {
    /// Nothing; answered at once (the round trip's cost).
    Nothing,
    /// Text for the host's log, in `bytes`.
    Print,
    /// The inside has finished.
    Done,
    /// The inside faulted itself: words 0 to 2 are ESR_EL1, ELR_EL1 and FAR_EL1.
    Fatal,
    /// Write `bytes` at guest-physical address word 0 (code into a frame the inside gave).
    Write,
    /// Give back the host pages of word 1 bytes of guest-physical memory at word 0; they read as zeros after.
    GiveBack,
    /// A test case's result: word 0 is 1 when it passed, `bytes` its name and what it saw.
    Report,
    /// The host process's memory footprint in bytes, into word 0.
    Footprint,
    /// Raise interrupt word 0 after word 1 microseconds, from another host thread; answered at once.
    RaiseLater,
    /// Lower interrupt word 0; word 0 becomes the guest's counter value at the moment it was raised.
    Lower,
    /// Start virtual CPU word 0 on a new host thread and answer when it is done.
    StartCpu,
    /// Stop this virtual CPU from another host thread word 0 times while it runs at the guest's level in the
    /// loop from word 2 to word 3 counting in general register word 4, then raise interrupt word 1.
    StopTest,
}
