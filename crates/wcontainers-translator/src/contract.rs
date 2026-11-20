//! What a runner must provide to, and may expect from, the modules the translator makes.
//!
//! Two kinds of module run per guest thread. The **thread module** ([`crate::thread_module`]) holds the
//! lookup table and the bottom loop. **Code modules** ([`crate::translate`]) hold pieces; they import the
//! thread module's table and put their pieces in it, in order from [`SLOT_BASE`], when instantiated.
//!
//! **Looking up an address.** The lookup table holds one slot per piece. An array of slots in the state
//! memory ([`SLOTS`]), one i32 per `Guest::ALIGN` bytes of guest address from [`TABLE_BASE`], gives the slot of the piece
//! for each address, 0 for none. The runner writes the array: each code module's pieces are
//! [`crate::CodeModule::pieces`]. The array is per process; every thread's table must hold the same piece
//! in the same slot, so a code module is instantiated with the same [`SLOT_BASE`] in every thread.
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

use wcontainers_guest_cpu::Guest;
pub use wcontainers_guest_cpu::{Reason, TrapKind};

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
/// i32: the lookup table slot of the code module's first piece; the others follow in order.
pub const SLOT_BASE: &str = "slot_base";
/// i64: the guest address of the first entry of the array of slots. Entry `i` is for
/// `table_base + Guest::ALIGN * i`.
pub const TABLE_BASE: &str = "table_base";
/// i32: where in the state memory the array of slots starts.
pub const SLOTS: &str = "slots";
/// i64: the bytes of guest address, from [`TABLE_BASE`], the array of slots covers (`Guest::ALIGN` for each
/// entry).
pub const SLOTS_COVER: &str = "slots_cover";
/// i32: where in the state memory this thread's block is (the guest's `Personality::Block` followed by the
/// stop flag), a multiple of 16.
pub const THREAD_BLOCK: &str = "thread_block";
/// i32: where in the state memory the page permission table starts, one byte per page. It may change only
/// while every thread of the process is stopped (see "When the permission table may change" in this module's documentation).
pub const PERMISSIONS: &str = "permissions";
/// i64: how many pages the permission table covers. The byte after the last one must be zero, so addresses at
/// or above `pages * PAGE_SIZE` fault.
pub const PERMISSION_PAGES: &str = "permission_pages";

/// The lookup table, exported by the thread module. Slot 0 is always empty, so an address with no piece finds
/// none. Slot [`EXACT_SLOT`] holds a piece for no address. The runner grows the table for the pieces it adds.
pub const LOOKUP: &str = "lookup";
/// The lookup table slot where [`HOOK_LOAD_CODE`] puts the piece of one instruction a thread must carry on in
/// exactly.
pub const EXACT_SLOT: i32 = 1;
/// The exception tag that unwinds to the bottom loop, exported by the thread module. Its i32 payload is
/// [`UNWIND_GO_TO`], [`UNWIND_GO_TO_EXACT`], [`UNWIND_DEEP`] or [`UNWIND_LEAVE`].
pub const UNWIND: &str = "unwind";
/// Unwind payload: carry on at the address in `pc`.
pub const UNWIND_GO_TO: i32 = 0;
/// Unwind payload: return from `run` to the runner. Registers are saved and `pc` is set.
pub const UNWIND_LEAVE: i32 = 1;
/// Unwind payload: carry on at the address in `pc` from code that checks every access exactly. A check that
/// covers several accesses failed there, so the bottom loop runs that one instruction from the piece of
/// look-ahead none [`HOOK_LOAD_CODE`] puts in [`EXACT_SLOT`], and carries on from the table after it.
pub const UNWIND_GO_TO_EXACT: i32 = 2;
/// Unwind payload: the engine's stack is nearly used up. Carry on at the address in `pc` (a piece's entry)
/// with the WebAssembly stack empty; the functions in progress continue when they return to the bottom loop.
pub const UNWIND_DEEP: i32 = 3;

/// Thread module export: run from the registers in the thread block until a hook asks to leave, or no piece
/// can be found for an address. Takes and returns nothing.
pub const RUN: &str = "run";

/// Hook `(number, a0, a1, a2, a3, a4, a5: i64) -> i64`: a system call, with the registers of
/// `Guest::SYSCALL`. The result goes to its result register. Registers are saved and `pc` is the instruction
/// after. To stop the thread, raise its stop flag and return.
pub const HOOK_SYSCALL: &str = "syscall";
/// Hook `() -> i32`: the stop flag was seen raised. Registers are saved and `pc` is the instruction about
/// to run. Return 0 to carry on, anything else to leave.
pub const HOOK_STOP: &str = "stop";
/// Hook `(address: i64, access: i32)`: an access the page permissions do not allow. Access is 0 for a read
/// and 1 for a write. Registers are saved and `pc` is the instruction. The thread then leaves.
pub const HOOK_MEMORY_FAULT: &str = "memory_fault";
/// Hook `(address: i64, exact: i32) -> i32`: with `exact` 0, the lookup finds no piece for this address:
/// put one there, translated from it (at bounded look-ahead). With `exact` 1, put the piece of the one
/// instruction at the address, of look-ahead none, in [`EXACT_SLOT`], whatever the lookup finds for the
/// address. Return 1 once the piece is in place, or 0 to leave. Registers are saved and `pc` is the
/// address.
pub const HOOK_LOAD_CODE: &str = "load_code";
/// Hook `(reason: i32, detail: i64)`: a slow path was taken; see [`Reason`].
pub const HOOK_WARN: &str = "warn";
/// Hook `(kind: i32, detail: i64)`: the instruction at `pc` (or the one before it, as the guest's trap
/// says) stops the thread; see [`TrapKind`]. Registers are saved. The thread then leaves.
pub const HOOK_TRAP: &str = "trap";

/// The export of a guest's probe module (`Guest::probe_module`) that gives the host answer.
pub const PROBE: &str = "probe";
// After these six, the guest's own hooks follow, as `Guest::HOOKS` names and types them.

/// Bytes a runner gives each thread at its block's place: the block (registers, then the stop flag), then
/// words of the CPU side's own, which start zero when the thread is made and which the kernel never sees.
pub const fn thread_state_size<G: Guest>() -> usize {
    G::BLOCK_SIZE + G::PRIVATE_BYTES
}

/// Permission table byte: the page may be read.
pub const PAGE_READ: u8 = 1;
/// Permission table byte: the page may be written.
pub const PAGE_WRITE: u8 = 2;
/// How far above a page's bits in a permission table byte the next page's are.
pub const NEXT_PAGE_SHIFT: u32 = 2;
/// Permission table byte: the next page may be read.
pub const NEXT_PAGE_READ: u8 = PAGE_READ << NEXT_PAGE_SHIFT;
/// Permission table byte: the next page may be written.
pub const NEXT_PAGE_WRITE: u8 = PAGE_WRITE << NEXT_PAGE_SHIFT;
/// A WebAssembly page (WebAssembly Core Specification, memory instances).
pub const WASM_PAGE_BYTES: u64 = 1 << 16;

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

// A piece's expected return address parameter is the address the caller expects control back at, with
// `Guest::FLAGS_WANTED` set when the caller reads the flags from the thread block after the piece returns;
// without it a piece may return with the flags it set only in its locals. The bottom loop passes
// `Guest::UNREACHABLE_RETURN`, which no return reaches. A return that does not go where its caller expects,
// when that caller is the bottom loop, sets `pc` to its target and returns: the function it returns to was
// in progress without its WebAssembly frame, and carries on from the bottom loop, counted as "continued
// after stack loss".

/// Tuning number: the bytes of the engine's stack the pieces in progress may use before the next call leaves
/// to the bottom loop ([`UNWIND_DEEP`]). The last parameter of every piece is an estimate of what its callers
/// use.
pub const STACK_BUDGET: i64 = 128 * 1024;

/// Tuning number: a slow path warns once per address (or number), when its count there reaches this.
pub const WARNING_THRESHOLD: u64 = 1;

/// Name of the custom section describing a code module: format version (one byte, 3), look-ahead (one byte,
/// 0 for none, 1 for full, 2 for bounded), what it is optimised for (one byte, 0 for speed, 1 for size), the
/// look-ahead's N (unsigned LEB128, 0 unless bounded), the blob's BLAKE3 hash (32 bytes), then the number of
/// pieces and each piece's offset in the blob in slot order, as unsigned LEB128.
pub const DESCRIPTION_SECTION: &str = "wcontainers";
