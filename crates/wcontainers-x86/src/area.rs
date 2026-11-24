//! The thread area translated code reaches on its own segment (docs/x86-native-output.md section 2): the words
//! guest code reads and writes, the words only the way writes, and the lookup's first level, at fixed offsets
//! from the segment's base.

use wcontainers_abi::PAGE_SIZE;
use wcontainers_guest_cpu::InsideCall;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// The register area, the thread area's first page, which guest code reads and writes.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, FromBytes, IntoBytes, Immutable, KnownLayout,
)]
#[repr(C)]
pub struct RegisterArea {
    /// The guest's GS base, which the hardware's never holds in any way of running.
    pub gs_base: u64,
    /// The guest's values of the withheld registers, in `crate::carry::WITHHELD`'s order.
    pub withheld: [u64; WITHHELD],
    /// The guest's values of the registers standing in for withheld ones around one instruction, in the order
    /// the stand-ins are taken.
    pub stand_ins: [u64; WITHHELD],
    /// The guest address of the last x87 instruction that is not a control one (FIP), which the hardware's
    /// never holds.
    pub x87_ip: u64,
    /// The x87 control word the hardware holds, stored there by the check after a copied `fldcw`.
    pub x87_control: u64,
    /// The guest's values of the registers that hold withheld ones throughout a loop, in the order they are taken.
    pub homes: [u64; WITHHELD],
    /// The guest's FS base in the in-process way, where the hardware's FS holds this area.
    pub fs_base: u64,
    /// A guest address t holds while a carrying sequence borrows t (the in-process way's 16-bit bit test).
    pub kept_t: u64,
}

/// The words of the thread area's second page, which only the way writes: read-only to guest code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
pub struct FixedPart {
    /// The host address of the inside's routine for each call, by its code (P and I).
    pub entries: [u64; InsideCall::ALL.len()],
    /// The thread's stop word: the count at which a replay check stops it, a value no count reaches on every
    /// other thread (docs/native.md 5.7).
    pub stop: u64,
    /// The in-process way's window base B, where the guest's memory starts on the host (0 in the other ways).
    pub window: u64,
    /// -B modulo 2^64, which takes a rebased register back without writing a flag.
    pub window_negated: u64,
}

/// How many registers translated code keeps for itself: the count and two scratch registers (section 3.4).
pub const WITHHELD: usize = 3;

/// The guest's GS base, as an offset from the thread area's segment base.
pub const GUEST_GS_BASE: usize = core::mem::offset_of!(RegisterArea, gs_base);
/// The guest's FS base in the in-process way, as an offset from the thread area's segment base.
pub const GUEST_FS_BASE: usize = core::mem::offset_of!(RegisterArea, fs_base);
/// Where t is kept while borrowed, as an offset from the thread area's segment base.
pub const KEPT_T: usize = core::mem::offset_of!(RegisterArea, kept_t);
/// The in-process way's page of zeros, this far below the window's base B: the exact form's probe reads it for an
/// address below 2^32 and, for one above, a place up to 2^35 bytes lower, where nothing is mapped (section 4).
pub const PROBE_PAGE: u64 = 2 * PAGE_SIZE;
/// The guest value of withheld register k is at `WITHHELD_VALUES + 8 * k`.
pub const WITHHELD_VALUES: usize = core::mem::offset_of!(RegisterArea, withheld);
/// The guest value of the k-th stand-in is at `STAND_IN_VALUES + 8 * k`.
pub const STAND_IN_VALUES: usize = core::mem::offset_of!(RegisterArea, stand_ins);
/// The guest's FIP, as an offset from the thread area's segment base.
pub const X87_IP: usize = core::mem::offset_of!(RegisterArea, x87_ip);
/// The word the check after a copied `fldcw` reads back.
pub const X87_CONTROL: usize = core::mem::offset_of!(RegisterArea, x87_control);
/// The guest value of the k-th register holding a withheld one throughout a loop is at `HOME_VALUES + 8 * k`.
pub const HOME_VALUES: usize = core::mem::offset_of!(RegisterArea, homes);
/// The fixed part's offset from the segment base.
pub const FIXED: usize = PAGE_SIZE as usize;
/// The inside's routine for call code k is at `ENTRIES + 8 * k`.
pub const ENTRIES: usize = FIXED + core::mem::offset_of!(FixedPart, entries);
/// The stop word of a thread no replay stops: no count reaches it.
pub const NO_STOP: u64 = u64::MAX;
/// The stop word, as an offset from the segment base.
pub const STOP_WORD: usize = FIXED + core::mem::offset_of!(FixedPart, stop);
/// The window's base B, as an offset from the segment base.
pub const WINDOW: usize = FIXED + core::mem::offset_of!(FixedPart, window);
/// -B, as an offset from the segment base.
pub const WINDOW_NEGATED: usize = FIXED + core::mem::offset_of!(FixedPart, window_negated);
/// The lookup's first level (section 3.3): 2^32 words, word m the second-level table of guest region m (bits 32
/// to 63 of a target) less m << 35.
pub const FIRST_LEVEL: usize = FIXED + PAGE_SIZE as usize;

/// The interrupt vectors the architecture keeps for its exceptions (Intel SDM Vol. 3A 6.2): the machine's calls
/// into the inside take the vectors after them, `int FIRST_CALL_VECTOR + code`.
pub const ARCHITECTURE_VECTORS: u8 = 32;
pub const FIRST_CALL_VECTOR: u8 = ARCHITECTURE_VECTORS;

const _: () = assert!(size_of::<RegisterArea>() <= FIXED);
const _: () = assert!(size_of::<FixedPart>() <= FIRST_LEVEL - FIXED);
