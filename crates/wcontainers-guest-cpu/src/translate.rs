//! The translation half of the interface: what the translator, its analyses and the runners need of a guest
//! CPU, and the [`Emitter`] its meanings write to.

use crate::{Bit, Effects, MemoryFault, Personality, Registers, TrapKind};
use core::fmt::Debug;
use core::hash::Hash;
use core::ops::Range;
use wasm_encoder::{InstructionSink, ValType};

/// Everything the translator needs that is true of one guest CPU only.
pub trait Guest: Personality {
    // --- identity ---
    /// Names the guest in the description section and the cache key.
    const ID: GuestId;

    // --- the instruction unit ---
    /// Every instruction starts at a multiple of this many bytes: ARM64 4, x86-64 1, RV64GC 2.
    const ALIGN: u64;
    /// The longest instruction in bytes: ARM64 4, x86-64 15, RV64GC 8 (a far call's two halves read as one).
    const MAX_LENGTH: u64;
    type Instruction: Copy + Debug + Send + Sync;
    /// The instruction at the start of `code` (which runs to the end of the code range), with its length in
    /// bytes; `None` when it would run past the end. An encoding of no instruction is an instruction whose
    /// meaning traps.
    fn decode(code: &[u8]) -> Option<(Self::Instruction, u8)>;
    /// The detail a trap reports for the instruction at the start of `code`: the 32-bit word on ARM64.
    fn trap_detail(code: &[u8]) -> i64;

    // --- registers ---
    /// A general register as meanings name it.
    type Reg: Copy + Eq + Debug + Send + Sync;
    /// A register of the second file (SIMD and floating point).
    type Vector: Copy + Eq + Debug + Send + Sync;
    /// One entry per register bit used, sorted by `Bit::bit`; bits not listed are unused.
    const BITS: &'static [Bit];
    /// The register's bit; `None` for a hard zero register (ARM64 xzr, RISC-V x0).
    fn reg_bit(r: Self::Reg) -> Option<u32>;
    fn vector_bit(v: Self::Vector) -> u32;
    /// The register a general-register bit names.
    fn bit_reg(bit: u32) -> Self::Reg;
    /// The stack pointer's bit.
    const SP_BIT: u32;
    /// Registers passed into pieces as their first parameters, in order, and out of them as their results,
    /// each list including the stack pointer.
    const PASSED_IN: &'static [u32];
    const PASSED_OUT: &'static [u32];

    // --- flags (all empty on a CPU without flags) ---
    type Flag: Copy + Eq + Debug + Send + Sync;
    /// The flags the translator tracks lazily, each with its bit in the flags word and its register bit, in
    /// the order the flags word is put together. The flags word holds these and nothing else.
    const FLAGS: &'static [(Self::Flag, u32, u32)];
    /// Where the i32 flags word sits in the block.
    const FLAGS_WORD_OFFSET: usize;
    /// How flags were last set: a shape with the locals holding its operands.
    type Flags: Copy + Debug + Send + Sync;
    /// A shape without its locals: the key of the out-of-line flags-word functions.
    type Shape: Copy + Eq + Hash + Debug + Send + Sync;
    /// The shapes worth an out-of-line flags-word function, in function order.
    const SHAPES: &'static [Self::Shape];
    /// The shape of a flags value, `None` for one that keeps no shape (its flags are worked out at once).
    fn shape(flags: Self::Flags) -> Option<Self::Shape>;
    /// The types of the locals a shape keeps, and the shape rebuilt over locals of those types.
    fn shape_locals(shape: Self::Shape) -> &'static [ValType];
    fn with_locals(shape: Self::Shape, locals: &[u32]) -> Self::Flags;
    /// The inverse of `with_locals`: the locals holding a shape's operands, in `shape_locals` order.
    fn locals_of(flags: Self::Flags) -> Locals;
    /// The type of the two temporaries `complete` works in, `None` when this shape keeps everything.
    fn temporaries(flags: Self::Flags) -> Option<ValType>;
    /// Work out into `temporaries` what a shape keeps only implicitly, and return it with every local filled.
    fn complete(
        sink: &mut InstructionSink<'_>,
        flags: Self::Flags,
        temporaries: [u32; 2],
    ) -> Self::Flags;
    /// The flags a value sets (register bits); the others keep their earlier value.
    fn sets(flags: Self::Flags) -> Registers;
    /// Push one flag, as an i32 0 or 1, for flags set this way.
    fn flag_value(sink: &mut InstructionSink<'_>, flags: Self::Flags, flag: Self::Flag);
    type Cond: Copy + Debug + Send + Sync;
    /// Push whether a condition holds as one comparison of the shape's operands, or return false.
    fn condition_direct(
        sink: &mut InstructionSink<'_>,
        flags: Self::Flags,
        cond: Self::Cond,
    ) -> bool;
    /// Push whether a condition holds, from the flags one by one.
    fn condition_from_flags<E: Emitter<Self> + ?Sized>(e: &mut E, cond: Self::Cond);
    /// Marks "the caller reads the flags" in a piece's expected-return parameter: a bit no return address
    /// can have. 0 for a CPU without flags.
    const FLAGS_WANTED: i64;

    // --- registers kept only in the thread's state ---
    type Control: Copy + Eq + Debug + Send + Sync;
    fn control_ty(c: Self::Control) -> ValType;
    /// Byte offset from the start of the block (into the block or the private bytes).
    fn control_offset(c: Self::Control) -> usize;

    // --- floating point modes ---
    /// Push an i32 not zero when floating point must take the exact path now.
    fn float_slow<E: Emitter<Self> + ?Sized>(e: &mut E);
    /// Controls whose write makes an earlier `float_slow` answer stale.
    const FLOAT_CONTROLS: &'static [Self::Control];

    // --- hooks the meanings call ---
    type Hook: Copy + Eq + Debug + Send + Sync;
    /// Each hook's name and type and who does its work. Function indices follow the translator's own hooks,
    /// in this order.
    const HOOKS: &'static [HookType];
    fn hook_index(h: Self::Hook) -> usize;
    /// The host side of a hook whose `HookType::body` is `Cpu`: the work, on the bytes of the thread's state
    /// its `HookType::state` names (`state[0]` is the byte at `state.at`), with the hook's arguments. `hook`
    /// is the hook's index in `HOOKS`. Every runner calls this and nothing CPU-named.
    fn run_hook(hook: usize, args: &[i64], state: &mut [u8]) -> HookDone;

    // --- traps ---
    type Trap: Copy + Debug + Send + Sync;
    /// The generic stop a trap is, where its detail comes from and where it leaves pc.
    fn trap_kind(t: Self::Trap) -> (TrapKind, TrapDetail, TrapPc);
    /// Slow paths of this CPU's own, numbered from `GUEST_REASONS` in this order, with their fixed names.
    type Reason: Copy + Eq + Debug + Send + Sync;
    const REASONS: &'static [(Self::Reason, &'static str)];

    // --- what the host's engine does where WebAssembly leaves it open ---
    type Host: Copy + Eq + Hash + Debug + Send + Sync;
    /// A module whose `probe` export (the contract's `PROBE`), an i32 function, gives the answer
    /// `host_from_probe` reads.
    fn probe_module() -> Vec<u8>;
    fn host_from_probe(answer: i32) -> Self::Host;
    /// The host answer as a number, for the cache key.
    fn host_key(host: Self::Host) -> u32;

    // --- what the container offers beyond the default claims ---
    /// A container setting that changes what the CPU claims, given with every translation (`()` when there
    /// is none); its default is what the CPU claims when the container asks for nothing more.
    type Offer: Copy + Eq + Hash + Debug + Default + Send + Sync;
    /// The offer as a number, for the cache key and the description section (0 for `()`).
    fn offer_key(offer: Self::Offer) -> u32;

    // --- what a memory fault is on this CPU ---
    /// The memory fault hook reported `address` (the first byte not allowed) for the instruction at the
    /// start of `code`, with the thread's registers exact in `block`: the fault the CPU gives for it. The
    /// runner asks this before it tells the kernel.
    fn memory_fault(address: u64, code: &[u8], block: &Self::Block) -> MemoryFault;

    // --- single stepping by a trap flag the program sets ---
    /// Whether a program can turn single stepping on (x86's TF); when false the runners never call `step`.
    const STEPS: bool = false;
    /// What the stop hook does about single stepping, worked out on the thread's state (`BLOCK_SIZE +
    /// PRIVATE_BYTES` bytes from the block). The runner asks it first at every stop hook call, with
    /// `StepAt::Stop(pc)`, and with `StepAt::Kernel` whenever the kernel has had the thread.
    fn step(state: &mut [u8], at: StepAt) -> Step {
        let _ = (state, at);
        Step::Off
    }

    // --- meanings ---
    fn emit<E: Emitter<Self> + ?Sized>(e: &mut E, i: &Self::Instruction);

    // --- facts about one instruction, for the analyses ---
    fn flow(i: &Self::Instruction) -> Flow<Self::Reg>;
    /// Up to two memory accesses, in the order the instruction makes them.
    fn accesses(i: &Self::Instruction) -> [Option<MemoryAccess<Self::Reg>>; 2];
    /// How the stack pointer moves, where the CPU knows; any other write of it is an unknown move.
    fn stack_move(i: &Self::Instruction) -> StackMove;
    /// The blob offset of an address the instruction `i` at blob offset `at` builds into a register, for
    /// code in `code`. `before` as for `jump_table`.
    fn built_address(
        at: u64,
        i: &Self::Instruction,
        blob: &[u8],
        code: Range<u64>,
        before: impl Fn(u64) -> Option<(u64, Self::Instruction)>,
    ) -> Option<u64>;
    /// The jump table of the jump through `reg` at blob offset `at`, made from a switch statement: its
    /// targets as blob offsets, each once, in table order, and the blob bytes it was read from. `before(o)` is the function's instruction that falls through
    /// into offset `o`, with its offset, or `None` at the start of a block more than one edge enters: the
    /// way back for a CPU whose code cannot be decoded backwards.
    fn jump_table(
        at: u64,
        reg: Self::Reg,
        blob: &[u8],
        code: Range<u64>,
        before: impl Fn(u64) -> Option<(u64, Self::Instruction)>,
    ) -> Option<JumpTable>;
    /// Plain computation, which the flags' liveness may look past: registers from registers only (no memory,
    /// hook, control, trap or leaving).
    fn plain(i: &Self::Instruction) -> bool;
    /// Whether a real CPU of this architecture may give a different result from the meaning's for this
    /// instruction: the meaning fixes what the architecture leaves open to what the reference CPU does
    /// (undefined flags), answers by design (cpuid, the time stamp counter), or estimates. A native output may
    /// copy an instruction through only where this is false.
    fn may_vary(i: &Self::Instruction) -> bool;

    // --- calls, returns and the stack ---
    /// Where a call leaves the return address.
    const RETURN_ADDRESS: ReturnAddress;
    /// Bytes below the stack pointer a function may use without moving it: x86-64 128, else 0.
    const RED_ZONE: u64;
    /// The CPU checks the stack pointer's alignment when it is the base of an access.
    const SP_ALIGNMENT_CHECKED: bool;
    /// Push the address in this i64 local as data accesses use it (ARM64 untags it).
    fn data_address(sink: &mut InstructionSink<'_>, local: u32);
    /// Push the target a jump through a register goes to, from the address in this i64 local, on the
    /// translator's own slow paths for such a jump (ARM64 untags it).
    fn branch_target<E: Emitter<Self> + ?Sized>(e: &mut E, local: u32);
    /// The return address the bottom loop passes. No target a meaning hands to `Emitter::ret` equals
    /// `UNREACHABLE_RETURN & !FLAGS_WANTED`.
    const UNREACHABLE_RETURN: i64;

    // --- system calls ---
    const SYSCALL: SyscallRegisters<Self::Reg>;

    // --- code changes ---
    const CODE_CHANGES: CodeChanges;

    // --- loop proofs ---
    /// This instruction adds a step to a register (by arithmetic or by an access's write-back).
    fn induction(i: &Self::Instruction) -> Option<Induction<Self::Reg>>;
    /// What the conditional branch `branch` tests, as one comparison that holds on its taken edge, given
    /// the instructions of its block before it (oldest first, each with its blob offset and effects);
    /// `None` when it is not one comparison of a register with a register or a constant.
    fn branch_comparison(
        path: &[(u64, Self::Instruction, Effects<Self>)],
        branch: (u64, &Self::Instruction),
    ) -> Option<Comparison<Self::Reg>>;
    /// When this instruction is a load that zero-extends what it loads (at most 4 bytes): the register it
    /// loads and the largest value it leaves there.
    fn loaded_max(i: &Self::Instruction) -> Option<(Self::Reg, i64)>;
}

/// The export of `Guest::probe_module`'s module.
pub const PROBE: &str = "probe";

/// The first number of a CPU's own slow paths (`Guest::REASONS`); the translator's are below it.
pub const GUEST_REASONS: u32 = 64;

/// Which guest CPU, in the description section and the cache key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GuestId {
    Arm64 = 1,
    X86_64 = 2,
    Rv64 = 3,
}

/// Where a meaning's registers, flags and control flow go. Each look-ahead implements it, and so does the
/// recorder of [`crate::effects`].
///
/// Meanings keep two rules an emitter may rely on: `set` and `set_vector` are never called inside a part of
/// the meaning that runs only sometimes, and an instruction that calls `set_flags` once does so
/// unconditionally. Locals a meaning gets from `local` are only used within that instruction, except those
/// named in the flags it passes to `set_flags`.
pub trait Emitter<G: Guest> {
    /// The body being written.
    fn sink(&mut self) -> InstructionSink<'_>;
    /// A fresh local of this type.
    fn local(&mut self, ty: ValType) -> u32;
    /// Push a general register as an i64. A hard zero register reads as zero.
    fn get(&mut self, r: G::Reg);
    /// Pop an i64 into a general register. Writes to a hard zero register are dropped.
    fn set(&mut self, r: G::Reg);
    /// Push the guest address of this instruction plus `by`.
    fn this_address(&mut self, by: i64);
    /// Whether anything may read the flags this instruction sets. When not, `set_flags` writes nothing and
    /// a meaning need work out nothing only for them.
    fn flags_live(&self) -> bool {
        true
    }
    fn set_flags(&mut self, flags: G::Flags);
    /// Push one flag as an i32 0 or 1.
    fn flag(&mut self, flag: G::Flag);
    /// Push a register of the second file, in its `Bit::ty`.
    fn get_vector(&mut self, v: G::Vector);
    /// Pop a value into a register of the second file.
    fn set_vector(&mut self, v: G::Vector);
    /// Push a register kept only in the thread's state, as a value of `Guest::control_ty`.
    fn get_control(&mut self, control: G::Control);
    /// Pop a value of `Guest::control_ty` into a register kept only in the thread's state.
    fn set_control(&mut self, control: G::Control);
    /// Call a hook with its arguments pushed; it leaves its results.
    fn hook(&mut self, hook: G::Hook);
    /// What the host's engine does where WebAssembly leaves it open.
    fn host(&self) -> G::Host;
    /// What the container offers beyond the default claims.
    fn offer(&self) -> G::Offer;
    /// Push whether floating point takes the slow path (`Guest::float_slow`), as an i32 not zero when it
    /// does, and return true: the meaning then writes both paths. Or, as nothing of the instruction is
    /// written yet, carry on from this instruction in other code when it does, and return false: the meaning
    /// then writes only the fast path.
    fn float_mode(&mut self) -> bool {
        G::float_slow(self);
        true
    }
    /// Whether the stack pointer is known here to be a multiple of 16, so its alignment needs no check.
    fn stack_aligned(&mut self) -> bool {
        false
    }
    /// Make the address in this i64 local what data accesses use (`Guest::data_address`), in place, then
    /// stop the thread with a memory fault unless `bytes` bytes at it may be accessed this way. `which` is
    /// the instruction's access (0 or 1, as `Guest::accesses` lists them) this check is for.
    fn check(&mut self, address: u32, bytes: u32, access: Access, which: u8);
    /// Whether the i64 run-time `length` (in this local) of bytes from the address in `address` may all be
    /// accessed this way, as an i32 local 1 or 0, without stopping.
    fn check_range(&mut self, address: u32, length: u32, access: Access) -> u32;
    /// Count and warn this CPU's own slow path at this instruction.
    fn warn(&mut self, reason: G::Reason);
    /// Pop an i32; when it is not zero, raise the thread's stop flag, so the stop hook is called before
    /// the next instruction runs.
    fn raise_stop(&mut self);
    /// Go to the target. Nothing after it in this instruction runs.
    fn jump(&mut self, target: Target);
    /// Pop an i32 and go to the target if it is not zero.
    fn jump_if(&mut self, target: Target);
    /// Call the target with the next instruction as the return address, already written where the CPU
    /// keeps it (`Guest::RETURN_ADDRESS`; on the stack the meaning has also moved the stack pointer).
    fn call(&mut self, target: Target);
    /// Return to the address in this i64 local. With return addresses on the stack, the meaning has read
    /// it from there, and this moves the stack pointer up past it.
    fn ret(&mut self, target: u32);
    /// The system call instruction, with its registers as `Guest::SYSCALL` says.
    fn syscall(&mut self);
    fn trap(&mut self, trap: G::Trap);
    /// Carry on with the next instruction. Ends every instruction that does not leave by itself.
    fn next(&mut self);
    /// Push whether a condition holds, as an i32.
    fn condition(&mut self, cond: G::Cond) {
        G::condition_from_flags(self, cond);
    }
}

/// The locals of a shape (`Guest::locals_of`): the first `len` of `at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Locals {
    pub len: u8,
    pub at: [u32; 9],
}

impl Locals {
    pub fn new(locals: &[u32]) -> Self {
        let mut at = [0; 9];
        at[..locals.len()].copy_from_slice(locals);
        Locals {
            len: locals.len() as u8,
            at,
        }
    }

    pub fn as_slice(&self) -> &[u32] {
        &self.at[..usize::from(self.len)]
    }
}

/// Where control goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// This many bytes from the current instruction.
    Relative(i64),
    /// The guest address in this i64 local.
    Address(u32),
}

/// What a memory access does, for the permission check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
}

/// Where control can go from an instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flow<R> {
    /// The next instruction can run after this one (including after a call returns).
    pub falls_through: bool,
    /// A direct jump or call target, in bytes from this instruction.
    pub target: Option<i64>,
    pub kind: FlowKind<R>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowKind<R> {
    Plain,
    /// A direct call, to the target above.
    Call,
    /// A call through a register or memory.
    CallThrough(Through<R>),
    Return,
    /// A jump through a register or memory.
    JumpThrough(Through<R>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Through<R> {
    Register(R),
    Memory,
}

/// One memory access: address `segment + base + index + displacement`, worked out modulo 2^`address_bits`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryAccess<R> {
    /// A segment base added to the address (x86 `fs:`, `gs:`); `None` on ARM64 and RISC-V.
    pub segment: Option<R>,
    pub base: Option<Base<R>>,
    pub index: Option<Index<R>>,
    pub displacement: i64,
    /// 64, except 32 for x86's 0x67 prefix.
    pub address_bits: u8,
    pub write_back: WriteBack,
    pub bytes: AccessBytes,
    pub kind: AccessKind,
}

impl<R> MemoryAccess<R> {
    /// Base plus a 32- or 64-bit index plus displacement, from a segment base or not, a fixed size and 64-bit
    /// addresses: the accesses the check analysis reasons about (the loop proofs only those without a
    /// segment). Every other access is checked by its own meaning each time it runs.
    pub fn plain(&self) -> bool {
        self.address_bits == 64
            && matches!(self.bytes, AccessBytes::Fixed(_))
            && match &self.index {
                None => true,
                Some(Index::Register { extend, .. }) => matches!(extend.from_bits, 32 | 64),
                Some(Index::BitOffset { .. } | Index::Lanes { .. }) => false,
            }
    }

    /// The bytes of a fixed-size access.
    pub fn fixed_bytes(&self) -> Option<u32> {
        match self.bytes {
            AccessBytes::Fixed(n) => Some(u32::from(n)),
            AccessBytes::Counted { .. } | AccessBytes::Masked { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base<R> {
    Register(R),
    /// The instruction's own address.
    Pc,
}

/// What the index adds to the address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Index<R> {
    /// A general register, read as `extend` says, shifted left by `shift`.
    Register { reg: R, shift: u8, extend: Extend },
    /// x86 `bt`, `bts`, `btr`, `btc` with a register bit offset: whole operands of `width` bits from the
    /// operand's address, the register read signed at `width` and shifted right arithmetically by
    /// log2(width), then left by log2(width / 8). The operand's own index, when it has one, is `operand`:
    /// a register shifted left.
    BitOffset {
        reg: R,
        width: u8,
        operand: Option<(R, u8)>,
    },
    /// x86 VSIB (the gathers): each element's address takes one lane of this vector register, `lane_bits`
    /// (32 or 64) read signed, shifted left by `shift`.
    Lanes { reg: R, lane_bits: u8, shift: u8 },
}

/// How a register is read as an index: its low `from_bits` (8, 16, 32 or 64), zero- or sign-extended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extend {
    pub from_bits: u8,
    pub signed: bool,
}

/// How the base register changes with the access: `Before(by)` adds `by` to it and the access uses the new
/// value; `After(by)` adds `by` after the access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteBack {
    None,
    Before(i64),
    After(i64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessBytes {
    Fixed(u16),
    /// A rep string instruction: rcx elements of this size.
    Counted {
        element: u8,
    },
    /// `count` elements of `element` bytes from the address (or one per lane of a `Lanes` index), of which
    /// only those a mask selects at run time are accessed; masked-off elements never fault.
    Masked {
        element: u8,
        count: u8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessKind {
    Read,
    Write,
    /// Reads and writes the same bytes: checked once for writing.
    ReadWrite,
}

/// A constant or a register, as the loop-proof recognisers name operands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value<R> {
    Imm(i64),
    Reg(R),
}

/// `reg` becomes `reg + step` (`reg - step` when `negate`) at `bits` (32 or 64), all 64 bits written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Induction<R> {
    pub reg: R,
    pub bits: u8,
    pub step: Value<R>,
    pub negate: bool,
}

/// On a branch's taken edge, `left relation right` holds, compared at `bits`, signed or not, with `left`
/// and `right` read at the blob offset `at` (the flag setter, or the branch itself).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Comparison<R> {
    pub left: R,
    pub right: Value<R>,
    pub relation: Relation,
    pub signed: bool,
    pub bits: u8,
    pub at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Eq,
    Ne,
    Below { inclusive: bool },
    Above { inclusive: bool },
}

impl Relation {
    /// The relation that holds where this one does not.
    pub fn negated(self) -> Self {
        match self {
            Relation::Eq => Relation::Ne,
            Relation::Ne => Relation::Eq,
            Relation::Below { inclusive } => Relation::Above {
                inclusive: !inclusive,
            },
            Relation::Above { inclusive } => Relation::Below {
                inclusive: !inclusive,
            },
        }
    }

    /// The relation with its two sides swapped.
    pub fn swapped(self) -> Self {
        match self {
            Relation::Below { inclusive } => Relation::Above { inclusive },
            Relation::Above { inclusive } => Relation::Below { inclusive },
            other => other,
        }
    }
}

/// A switch's jump table: the targets, as blob offsets, each once in table order, and the blob bytes read
/// for them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JumpTable {
    pub targets: Vec<u64>,
    pub read: Range<u64>,
}

/// How the stack pointer moves at an instruction: by a known amount (before or after its access), or to
/// something unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackMove {
    None,
    By { delta: i64, before_access: bool },
    Unknown,
}

/// Where a call leaves the return address: in a register, which the call's meaning writes like any other,
/// or written on the stack below the stack pointer, `bytes` long.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReturnAddress {
    Register,
    Stack { bytes: u8 },
}

/// How the translator learns that code changed: the program announces it (ARM64 `ic ivau`, RISC-V
/// `fence.i`), or a write to a page holding translated code is detected (x86-64).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeChanges {
    Announced,
    Detected,
}

/// When `Guest::step` is asked: at a stop hook call with pc, or after the kernel has had the thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepAt {
    Stop(u64),
    Kernel,
}

/// What the runner then does. `Off`: carry on, the stop flag lowered unless another reason holds it. `Run`:
/// carry on with the stop flag kept raised. `Exact`: run the instruction at pc alone, the flag kept raised.
/// `Trap`: stop the thread with a single step at pc (`SIGTRAP`, `TRAP_TRACE`), before any other reason the
/// flag was raised for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Off,
    Run,
    Exact,
    Trap,
}

/// The registers of the system call instruction: the number, the six arguments and the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyscallRegisters<R> {
    pub number: R,
    pub args: [R; 6],
    pub result: R,
}

/// Where a trap's detail comes from: the instruction's bytes (`Guest::trap_detail`), an i64 local, or a
/// constant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapDetail {
    Instruction,
    Local(u32),
    Const(i64),
}

/// Where a trap leaves pc: at the instruction (a fault), or after it (x86's `int3` and other traps).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapPc {
    At,
    After,
}

/// A hook a CPU's meanings call: its import name, its parameter and result types, and who does its work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HookType {
    pub name: &'static str,
    pub params: &'static [ValType],
    pub results: &'static [ValType],
    /// The register bits the hook reads or writes in the thread block: written back before the call and
    /// loaded after it.
    pub registers: Registers,
    /// For a `Cpu` hook, the bytes of the thread's state its work reads and writes, which the runner hands
    /// to `Guest::run_hook`.
    pub state: StateBytes,
    pub body: HookBody,
}

impl HookType {
    /// The most parameters a hook may have; the runners check every hook against it at compile time. The
    /// browser passes each as an argument of its own, so every one more costs every call of every hook.
    pub const MAX_PARAMS: usize = 4;
    /// The most results, as many as `HookDone` holds.
    pub const MAX_RESULTS: usize = 2;
}

/// `len` bytes of a thread's state from `at`, an offset from the start of the block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateBytes {
    pub at: u16,
    pub len: u16,
}

impl StateBytes {
    pub const NONE: Self = Self { at: 0, len: 0 };

    pub const fn range(self) -> Range<usize> {
        self.at as usize..self.at as usize + self.len as usize
    }
}

/// Who does a hook's work on the host side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookBody {
    /// The runner's clock, in nanoseconds.
    Clock,
    /// The runner drops the pieces of the `line` bytes (a power of two) holding the address the hook is
    /// given, its one `i64` parameter.
    CodeChanged { line: u32 },
    /// `Guest::run_hook`.
    Cpu,
}

/// What `Guest::run_hook` did: its results (as many as `HookType::results` says), and the slow path it
/// was, by reason number, for the runner to count and warn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HookDone {
    pub results: [i64; 2],
    pub reason: Option<u32>,
}

/// Every slow path of the translated code and its runners, with the fixed name its counter and warning
/// carry.
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
    /// The engine's stack was nearly used up, so the thread left to the bottom loop (the unwind payload for it).
    StackNearlyFull = 13,
    /// A page holding translated code was written without an announcement, so its pieces were dropped.
    CodeWrittenWithoutAnnouncement = 14,
}

impl Reason {
    pub const ALL: [Reason; 15] = [
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
        Reason::CodeWrittenWithoutAnnouncement,
    ];

    pub fn from_i32(value: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|r| *r as i32 == value)
    }

    /// Its number, as the warn hook and the runners' counters have it.
    pub const fn number(self) -> u32 {
        self as u32
    }

    /// The name of the slow path with this number, the translator's or the guest's own.
    pub fn name_of<G: Guest>(number: u32) -> Option<&'static str> {
        match number.checked_sub(GUEST_REASONS) {
            Some(i) => G::REASONS.get(i as usize).map(|r| r.1),
            None => Self::from_i32(number as i32).map(Self::name),
        }
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
            Reason::CodeWrittenWithoutAnnouncement => "code written without announcement",
        }
    }
}
