//! What a runner must provide to, and may expect from, the modules the translator makes.
//!
//! Two kinds of module run per guest thread. The **thread module** ([`crate::thread_module`]) holds the
//! lookup table and the bottom loop. **Code modules** ([`crate::translate`]) hold pieces; they import the
//! thread module's table and add their pieces to the table when instantiated.
//!
//! Registers that are not in a piece's locals live in the thread block in the state memory, so whenever a hook
//! runs or `run` has returned, the block holds them all ("registers are saved"), and a runner sets registers
//! by writing the block before `run`.
//!
//! **When the permission table may change.** What a piece has found out about page permissions (checks one
//! check covers, kept runs of allowed pages, proofs over a loop's range, bounded indices) holds only until
//! the next point where its thread can stop: a stop check whose hook runs, a system call, or leaving `run`.
//! After any of those the piece checks again. The one exception is a stack frame checked when the stack
//! pointer moved down: a change to a page of a running function's own frame, at or above the stack pointer,
//! is noticed at the next function entry, not at once. Memory below the stack pointer is no frame and is
//! checked at every access. So whoever changes a process's page permissions must first bring every other
//! running thread of that process to a stop (raise its stop flag and wait until its hook runs or it has
//! left `run`), as a real kernel interrupts the other processors before it changes a page table. The
//! thread making the change is already stopped, in its system call or hook. With one host thread this
//! happens by itself, because only a stopped thread lets the kernel run.

/// Import module name for what the runner provides.
pub const ENV: &str = "env";
/// Import module name for the hooks the runner provides.
pub const HOOKS: &str = "hooks";
/// Import module name for what the thread module exports.
pub const THREAD: &str = "thread";

/// The guest memory: a 64 bit memory holding the guest's whole address space, unshared. Its base must never
/// move, so the engine keeps the base and bound in registers instead of reloading them at every access: in
/// wasmtime the engine needs `Config::memory_may_move(false)` and a memory reservation that covers the
/// memory (V8 keeps them in registers either way).
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
/// i32: where in the state memory the page permission table starts, one byte per page. It may change only
/// while every thread of the process is stopped (see "When the permission table may change" in this module's documentation).
pub const PERMISSIONS: &str = "permissions";
/// i64: how many pages the permission table covers. The byte after the last one must be zero, so addresses at
/// or above `pages * PAGE_SIZE` fault.
pub const PERMISSION_PAGES: &str = "permission_pages";

/// The lookup table, exported by the thread module. Its last slot holds a piece for no address: it is where
/// [`HOOK_LOAD_CODE`] puts the piece of one instruction a thread must carry on in exactly.
pub const LOOKUP: &str = "lookup";
/// The exception tag that unwinds to the bottom loop, exported by the thread module. Its i32 payload is
/// [`UNWIND_GO_TO`], [`UNWIND_GO_TO_EXACT`], [`UNWIND_DEEP`] or [`UNWIND_LEAVE`].
pub const UNWIND: &str = "unwind";
/// Unwind payload: carry on at the address in `pc`.
pub const UNWIND_GO_TO: i32 = 0;
/// Unwind payload: return from `run` to the runner. Registers are saved and `pc` is set.
pub const UNWIND_LEAVE: i32 = 1;
/// Unwind payload: carry on at the address in `pc` from code that checks every access exactly. A check that
/// covers several accesses failed there, so the bottom loop runs that one instruction from the piece of
/// look-ahead none [`HOOK_LOAD_CODE`] puts in the table's last slot, and carries on from the table after it.
pub const UNWIND_GO_TO_EXACT: i32 = 2;
/// Unwind payload: the engine's stack is nearly used up. Carry on at the address in `pc` (a piece's entry)
/// with the WebAssembly stack empty; the functions in progress continue when they return to the bottom loop.
pub const UNWIND_DEEP: i32 = 3;

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
/// Hook `(address: i64, exact: i32) -> i32`: with `exact` 0, the lookup table holds no piece for this
/// address: put one there, translated from it (at bounded look-ahead). With `exact` 1, put the piece of the
/// one instruction at the address, of look-ahead none, in the table's last slot, whatever the table holds
/// for the address. Return 1 once the piece is in place, or 0 to leave. Registers are saved and `pc` is the
/// address.
pub const HOOK_LOAD_CODE: &str = "load_code";
/// Hook `(reason: i32, detail: i64)`: a slow path was taken; see [`Reason`].
pub const HOOK_WARN: &str = "warn";
/// Hook `(kind: i32, detail: i64)`: the instruction at `pc` stops the thread; see [`TrapKind`]. Registers
/// are saved. The thread then leaves.
pub const HOOK_TRAP: &str = "trap";
/// Hook `(operation: i32, a: i64, b: i64, c: i64) -> i64`: one floating point operation in software,
/// [`wcontainers_arm64::exact::run`] with this thread's FPCR, adding to its FPSR. It is a slow path: counted as
/// "software fused multiply-add" when FPCR is zero and the flags are not kept (the host's engine does not
/// fuse), else as "exact floating point requested". Registers are not saved.
pub const HOOK_FLOAT: &str = "float";
/// Hook `() -> i64`: the monotonic clock in nanoseconds, for the counter.
pub const HOOK_CLOCK: &str = "clock";
/// Hook `(address: i64)`: `ic ivau` says the code at the address may have changed. Registers are not saved.
pub const HOOK_CODE_CHANGED: &str = "code_changed";

/// Bytes a runner gives each thread at its block's place: the abi's block (registers, then the stop flag),
/// then words of the CPU side's own, which start zero when the thread is made and which the kernel never
/// sees.
pub const THREAD_STATE_SIZE: usize = wcontainers_abi::THREAD_BLOCK_SIZE + 32;
/// i32: not zero once the program wrote FPSR, so its flags are kept exact.
pub const FLOAT_TRACKING: usize = wcontainers_abi::THREAD_BLOCK_SIZE;
/// i32: the size of the access a load exclusive remembered, 0 for none.
pub const EXCLUSIVE_BYTES: usize = FLOAT_TRACKING + 4;
/// i64: the address it remembered.
pub const EXCLUSIVE_ADDRESS: usize = FLOAT_TRACKING + 8;
/// Two i64: the value it loaded.
pub const EXCLUSIVE_VALUE: usize = FLOAT_TRACKING + 16;

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

/// The bit of a piece's expected return address parameter that says the caller reads the flags from the
/// thread block after the piece returns; without it a piece may return with the flags it set only in its
/// locals. The rest of the parameter is the address the caller expects control back at.
pub const FLAGS_WANTED: i64 = 1;

/// The return address the bottom loop passes to the piece it calls (with [`FLAGS_WANTED`]). No return reaches
/// it, because return targets are untagged and an untagged address with bit 55 clear has a top byte of zero.
/// A return that does not go where its caller expects, when that caller is the bottom loop, sets `pc` to its
/// target and returns: the function it returns to was in progress without its WebAssembly frame, and
/// carries on from the bottom loop, counted as "continued after stack loss".
pub const BOTTOM_RETURN: i64 = i64::MIN;

/// Tuning number: the bytes of the engine's stack the pieces in progress may use before the next call leaves
/// to the bottom loop ([`UNWIND_DEEP`]). The last parameter of every piece is an estimate of what its callers
/// use.
pub const STACK_BUDGET: i64 = 128 * 1024;

/// Why the instruction at `pc` stopped the thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum TrapKind {
    /// Not an instruction of the CPU we present. Detail: the instruction word.
    Undefined = 0,
    /// An instruction of a feature we do not offer: illegal instruction, counted and warned. Detail: the
    /// instruction word.
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
    /// A loop's accesses were not proven allowed at its entry, so it runs its copy with every check.
    LoopNotProven = 12,
    /// The engine's stack was nearly used up, so the thread left to the bottom loop ([`UNWIND_DEEP`]).
    StackNearlyFull = 13,
}

impl Reason {
    pub const ALL: [Reason; 14] = [
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
        Reason::LoopNotProven,
        Reason::StackNearlyFull,
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
            Reason::LoopNotProven => "loop not proven",
            Reason::StackNearlyFull => "engine stack nearly full",
        }
    }
}

/// Name of the custom section describing a code module: format version (one byte, 2), look-ahead (one byte,
/// 0 for none, 1 for full, 2 for bounded) and its N (unsigned LEB128, 0 unless bounded), the blob's BLAKE3
/// hash (32 bytes), then the number of pieces and each piece's offset in the blob, as unsigned LEB128.
pub const DESCRIPTION_SECTION: &str = "wcontainers";
