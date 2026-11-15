//! What a runner must provide to, and may expect from, the modules the translator makes.
//!
//! Two kinds of module run per guest thread. The **thread module** ([`crate::thread_module`]) holds the
//! lookup table and the bottom loop. **Code modules** ([`crate::translate`]) hold pieces; they import the
//! thread module's table and add their pieces to the table when instantiated.
//!
//! Registers that are not in a piece's locals live in the thread block in the state memory, so whenever a hook
//! runs or `run` has returned, the block holds them all ("registers are saved"), and a runner sets registers
//! by writing the block before `run`.

/// Import module name for what the runner provides.
pub const ENV: &str = "env";
/// Import module name for the hooks the runner provides.
pub const HOOKS: &str = "hooks";
/// Import module name for what the thread module exports.
pub const THREAD: &str = "thread";

/// The guest memory: a 64 bit memory holding the guest's whole address space, unshared.
pub const MEMORY: &str = "memory";
/// The runtime's own memory, which the guest cannot reach: thread blocks and the page permission table.
/// A shared 32 bit memory, at most 4 GiB.
pub const STATE: &str = "state";
/// i64: the guest address where the blob's first byte sits.
pub const LOAD_ADDRESS: &str = "load_address";
/// i32: the lookup table slot of the blob's first byte, `(load_address - table_base) / 4`.
pub const SLOT_BASE: &str = "slot_base";
/// i64: the guest address of lookup table slot zero. Slot `i` holds the piece for `table_base + 4 * i`.
pub const TABLE_BASE: &str = "table_base";
/// i32: where in the state memory this thread's block is (a [`wcontainers_abi::RegisterBlock`] followed by
/// the stop flag), a multiple of 16.
pub const THREAD_BLOCK: &str = "thread_block";
/// i32: where in the state memory the page permission table starts, one byte per page.
pub const PERMISSIONS: &str = "permissions";
/// i64: how many pages the permission table covers. The byte after the last one must be zero, so addresses at
/// or above `pages * PAGE_SIZE` fault.
pub const PERMISSION_PAGES: &str = "permission_pages";

/// The lookup table, exported by the thread module.
pub const LOOKUP: &str = "lookup";
/// The exception tag that unwinds to the bottom loop, exported by the thread module. Its i32 payload is
/// [`UNWIND_GO_TO`], [`UNWIND_GO_TO_EXACT`] or [`UNWIND_LEAVE`].
pub const UNWIND: &str = "unwind";
/// Unwind payload: carry on at the address in `pc`.
pub const UNWIND_GO_TO: i32 = 0;
/// Unwind payload: return from `run` to the runner. Registers are saved and `pc` is set.
pub const UNWIND_LEAVE: i32 = 1;
/// Unwind payload: carry on at the address in `pc` from code that checks every access exactly. A check that
/// covers several accesses failed there, so the piece the table holds for `pc` may be the one that made it;
/// the bottom loop asks [`HOOK_LOAD_CODE`] for code at `pc` first.
pub const UNWIND_GO_TO_EXACT: i32 = 2;

/// Thread module export: run from the registers in the thread block until a hook asks to leave, or no piece
/// can be found for an address. Takes and returns nothing.
pub const RUN: &str = "run";

/// Hook `(number, a0, a1, a2, a3, a4, a5: i64) -> i64`: a system call. The result goes to x0. Registers
/// are saved and `pc` is the instruction after. To stop the thread, raise its stop flag and return.
pub const HOOK_SYSCALL: &str = "syscall";
/// Hook `() -> i32`: the stop flag was seen raised. Registers are saved and `pc` is the instruction about
/// to run. Return 0 to carry on, anything else to leave.
pub const HOOK_STOP: &str = "stop";
/// Hook `(address: i64, access: i32)`: an access the page permissions do not allow. Access is 0 for a read
/// and 1 for a write. Registers are saved and `pc` is the instruction. The thread then leaves.
pub const HOOK_MEMORY_FAULT: &str = "memory_fault";
/// Hook `(address: i64) -> i32`: no piece is known for this address, or (after [`UNWIND_GO_TO_EXACT`]) the
/// one there must be replaced by one translated at look-ahead none, unless it already is one. Return 1 once
/// such a piece is in the lookup table, or 0 to leave. Registers are saved and `pc` is the address.
pub const HOOK_LOAD_CODE: &str = "load_code";
/// Hook `(reason: i32, detail: i64)`: a slow path was taken; see [`Reason`].
pub const HOOK_WARN: &str = "warn";
/// Hook `(kind: i32, detail: i64)`: the instruction at `pc` stops the thread; see [`TrapKind`]. Registers
/// are saved. The thread then leaves.
pub const HOOK_TRAP: &str = "trap";

/// Permission table byte: the page may be read.
pub const PAGE_READ: u8 = 1;
/// Permission table byte: the page may be written.
pub const PAGE_WRITE: u8 = 2;
/// Permission table byte: the next page may be read.
pub const NEXT_PAGE_READ: u8 = 4;
/// Permission table byte: the next page may be written.
pub const NEXT_PAGE_WRITE: u8 = 8;

/// The permission table byte for a page, given what it and the page after it allow.
pub fn permission_byte(
    this: wcontainers_abi::Permissions,
    next: wcontainers_abi::Permissions,
) -> u8 {
    use wcontainers_abi::Permissions as P;
    let bits = |p: P, read: u8, write: u8| {
        (u8::from(p.contains(P::READ)) * read) | (u8::from(p.contains(P::WRITE)) * write)
    };
    bits(this, PAGE_READ, PAGE_WRITE) | bits(next, NEXT_PAGE_READ, NEXT_PAGE_WRITE)
}

/// The return address the bottom loop passes to the first piece it calls. Any value is correct: a piece
/// returns only when the guest returns to exactly this address, and the loop then carries on there.
pub const BOTTOM_RETURN: i64 = -1;

/// Why the instruction at `pc` stopped the thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum TrapKind {
    /// Not an instruction of the CPU we present. Detail: the instruction word.
    Undefined = 0,
    /// An instruction we present but do not translate yet. Detail: the instruction word.
    Unsupported = 1,
    /// `brk`. Detail: its comment field.
    Breakpoint = 2,
    /// A load or store through a stack pointer that is not a multiple of 16, or a jump to an address that
    /// is not a multiple of 4. Detail: that stack pointer or address.
    Alignment = 3,
}

impl TrapKind {
    pub fn from_i32(value: i32) -> Option<Self> {
        [
            Self::Undefined,
            Self::Unsupported,
            Self::Breakpoint,
            Self::Alignment,
        ]
        .into_iter()
        .find(|kind| *kind as i32 == value)
    }
}

/// Tuning number: a slow path warns once per address (or number), when its count there reaches this.
pub const WARNING_THRESHOLD: u64 = 1;

/// Every slow path, with the fixed name its counter and warning carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Reason {
    TranslatedAtRunTime = 0,
    LeftToAddress = 1,
    ContinuedAfterStackLoss = 2,
    JumpThroughRegisterNotRecognised = 3,
    SoftwareFusedMultiplyAdd = 4,
    UnsupportedInstructionReached = 5,
    SystemCallNotImplemented = 6,
    ExactFloatingPointRequested = 7,
    AddressAboveHostCap = 8,
    StoppedByFlag = 9,
    MemoryFault = 10,
    FaultWithoutExactRegisters = 11,
}

impl Reason {
    pub const ALL: [Reason; 12] = [
        Reason::TranslatedAtRunTime,
        Reason::LeftToAddress,
        Reason::ContinuedAfterStackLoss,
        Reason::JumpThroughRegisterNotRecognised,
        Reason::SoftwareFusedMultiplyAdd,
        Reason::UnsupportedInstructionReached,
        Reason::SystemCallNotImplemented,
        Reason::ExactFloatingPointRequested,
        Reason::AddressAboveHostCap,
        Reason::StoppedByFlag,
        Reason::MemoryFault,
        Reason::FaultWithoutExactRegisters,
    ];

    pub fn from_i32(value: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|r| *r as i32 == value)
    }

    pub fn name(self) -> &'static str {
        match self {
            Reason::TranslatedAtRunTime => "translated at run time",
            Reason::LeftToAddress => "left to an address",
            Reason::ContinuedAfterStackLoss => "continued after stack loss",
            Reason::JumpThroughRegisterNotRecognised => "jump through a register not recognised",
            Reason::SoftwareFusedMultiplyAdd => "software fused multiply-add",
            Reason::UnsupportedInstructionReached => "unsupported instruction reached",
            Reason::SystemCallNotImplemented => "system call not implemented",
            Reason::ExactFloatingPointRequested => "exact floating point requested",
            Reason::AddressAboveHostCap => "address above the host's cap",
            Reason::StoppedByFlag => "stopped by the flag",
            Reason::MemoryFault => "memory fault",
            Reason::FaultWithoutExactRegisters => "fault without exact registers",
        }
    }
}

/// Name of the custom section describing a code module: format version (one byte), look-ahead (one byte,
/// 0 for none, 1 for full), the blob's BLAKE3 hash (32 bytes), then the number of pieces and each piece's offset in the
/// blob, as unsigned LEB128.
pub const DESCRIPTION_SECTION: &str = "wcontainers";
