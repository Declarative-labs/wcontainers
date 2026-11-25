//! Layer 2: random small programs built to always end, the same model for every guest, which each CPU's
//! `GuestTest::assemble` turns into its own code. Each has loops with counters, calls (some to a function
//! that makes a system call), bounded recursion, tail calls, calls and jumps through registers,
//! compiler-style jump tables, flags carried across instructions, stack frames of every size (some running
//! off the stack), and memory use inside a fixed area, through the stack pointer, and in groups through
//! other registers, some near a page that allows nothing, groups and loops across calls, accesses below sp
//! once a frame is popped, and system calls that write nothing. At the end it stores its registers and flags
//! after the area and writes the whole area out; a program that faults ends with SIGSEGV instead.
//!
//! The model's operations and their ranges are those ARM64 can encode in one instruction; register numbers
//! are the guest's own, drawn from `G::MODEL`. The names of the scratch registers in the comments (x16 the
//! loop counter, x17 recursion's, x18 a moving base or jump target, x28 a base kept across calls, x29 the
//! area) are ARM64's, standing for the registers another CPU's `assemble` keeps for the same roles.

use crate::guest::GuestTest;
use crate::programs::Blob;
use proptest::prelude::*;
use wcontainers_guest_cpu::ReturnAddress;

/// Bytes of the area random loads and stores use.
pub const AREA: u32 = 512;

/// The registers a guest gives the model.
pub struct Model {
    /// The registers random instructions use, in a fixed order (the draws pick by position).
    pub registers: &'static [u8],
    /// Those of them a system call sets.
    pub syscall_sets: &'static [u8],
    /// One of them every function keeps, the base of a group across a call.
    pub kept: u8,
}

/// Bytes written out at the end: the area, then the model's registers and a word of flags.
pub const fn output<G: GuestTest>() -> u32 {
    AREA + (G::MODEL.registers.len() as u32 + 1) * 8
}

/// Bytes the area and what is written out after it take in the blob.
pub const fn area_bytes<G: GuestTest>() -> usize {
    output::<G>().next_multiple_of(16) as usize
}

/// Where a program's area starts in its blob: no access reaches below it, into the code and tables.
pub fn area_offset<G: GuestTest>(blob: &Blob) -> usize {
    blob.bytes.len() - wcontainers_abi::PAGE_SIZE as usize - area_bytes::<G>()
}
/// Bytes of the stack every function frame keeps below what random accesses through sp reach, for the
/// registers every function saves and restores, so a caller's loop or group across a call keeps them.
pub const FRAME_FLOOR: u32 = 544;
/// Where random accesses through sp start.
pub const STACK_SLOTS: u32 = 32;

/// Vector registers in an access through sp.
#[derive(Clone, Copy, Debug)]
pub enum StackVector {
    No,
    /// ldp or stp of two Q registers.
    Pair,
    /// ld4 of four Q registers at sp itself.
    Structure,
}

/// Where a group of accesses through one register starts.
#[derive(Clone, Copy, Debug)]
pub enum GroupBase {
    /// This many bytes into the area.
    Area(u16),
    /// This many bytes before the end of the blob, which is followed by a page that allows nothing.
    Edge(u16),
}

/// How a group access uses its base: an offset, or pre- or post-indexing that moves the base.
#[derive(Clone, Copy, Debug)]
pub enum Indexing {
    Offset,
    Pre,
    Post,
}

#[derive(Clone, Copy, Debug)]
pub struct GroupAccess {
    pub load: bool,
    /// 1, 2, 4 or 8, or 16 for a pair of X registers; with `vector`, 16 for one Q register, 32 for a pair
    /// of them, and 48 and 64 for a structure of three or four (ld1 to ld4, st1 to st4).
    pub bytes: u8,
    /// The transfer registers are vector registers, from `rt` on.
    pub vector: bool,
    pub rt: u8,
    pub rt2: u8,
    pub indexing: Indexing,
    /// In units of `bytes` (of 8 for a pair of X registers, of 16 for Q registers) for an offset, which is
    /// 0 for a structure; in bytes (-64 to 63) when indexing, `bytes` for a structure, which has only
    /// post-indexing.
    pub offset: i16,
}

/// The sizes of a group access, and whether its registers are vector registers.
const ACCESS_KINDS: [(u8, bool); 9] = [
    (1, false),
    (2, false),
    (4, false),
    (8, false),
    (16, false),
    (16, true),
    (32, true),
    (48, true),
    (64, true),
];

/// One straight-line instruction.
#[derive(Clone, Debug)]
pub enum Op {
    /// add, sub, and, orr, eor, adds, subs, ands, lslv, lsrv, asrv, rorv, mul by kind.
    Arithmetic {
        kind: u8,
        wide: bool,
        rd: u8,
        rn: u8,
        rm: u8,
    },
    AddImmediate {
        subtract: bool,
        flags: bool,
        wide: bool,
        rd: u8,
        rn: u8,
        imm: u16,
    },
    Move {
        keep: bool,
        rd: u8,
        imm: u16,
        hw: u8,
    },
    /// csel, csinc, csinv, csneg by kind.
    Select {
        kind: u8,
        wide: bool,
        rd: u8,
        rn: u8,
        rm: u8,
        cond: u8,
    },
    Compare {
        wide: bool,
        rn: u8,
        rm: u8,
    },
    CondCompare {
        wide: bool,
        rn: u8,
        rm: u8,
        nzcv: u8,
        cond: u8,
    },
    Divide {
        signed: bool,
        wide: bool,
        rd: u8,
        rn: u8,
        rm: u8,
    },
    /// Read the flags into a register, or set them from one.
    Flags {
        write: bool,
        r: u8,
    },
    Load {
        bytes: u8,
        rt: u8,
        slot: u8,
    },
    Store {
        bytes: u8,
        rt: u8,
        slot: u8,
    },
    Pair {
        load: bool,
        rt: u8,
        rt2: u8,
        slot: u8,
    },
    /// An access through sp, `STACK_SLOTS` bytes up plus 8 per slot.
    Stack {
        load: bool,
        rt: u8,
        slot: u8,
        vector: StackVector,
    },
    /// Point a register at `start`, then access memory through it, with a system call like `Syscall`'s
    /// before the access numbered `syscall` (from 1; one past the last puts it after them), and then the
    /// base is not one the call sets.
    Group {
        base: u8,
        start: GroupBase,
        accesses: Vec<GroupAccess>,
        syscall: Option<usize>,
    },
    /// A system call that writes nothing to standard output: x0 = 1, x1 = the area, x2 = 0, x8 = 64 (write),
    /// leaving x0 = 0.
    Syscall,
}

#[derive(Clone, Debug)]
pub enum Chunk {
    Ops(Vec<Op>),
    /// Skip the ops when a condition holds on the flags as they are.
    Skip {
        cond: u8,
        ops: Vec<Op>,
    },
    /// Run the ops `count` times, counting in x16, with a call after them each turn when `call`.
    Loop {
        count: u8,
        ops: Vec<Op>,
        call: Option<Callee>,
    },
    /// A loop counting in x16 that accesses memory through x18 at an address moving by a fixed step.
    Walk(Walk),
    /// A loop counting in x16 that accesses memory at a base plus an index read from the area or worked
    /// out each turn, and bounded by a comparison or by the load that read it.
    Lookup(Lookup),
    /// A call, directly or through x18, inside a group through x28 when `around`.
    Call {
        callee: Callee,
        through_register: bool,
        around: Option<Around>,
    },
    /// Move sp down to its page's start, make and pop a frame of `frame` bytes there, then access memory
    /// `below` bytes below sp (in the page below), after `between`, and put sp back.
    Below {
        frame: u16,
        below: u16,
        load: bool,
        rt: u8,
        between: Between,
    },
    /// A switch on the low bits of a register, through a table of byte entries.
    Switch {
        index: u8,
        cases: Vec<Vec<Op>>,
    },
    /// Jump over the ops, directly or through x18.
    Jump {
        through_register: bool,
        ops: Vec<Op>,
    },
}

/// What a call calls.
#[derive(Clone, Copy, Debug)]
pub enum Callee {
    /// A function by number.
    Function(usize),
    /// A function that makes a system call like `Op::Syscall`'s and returns, leaving x16, x18 and x28.
    Writer,
}

/// A group through x28 with a call between its accesses `before` and `after`.
#[derive(Clone, Debug)]
pub struct Around {
    pub start: GroupBase,
    pub before: Vec<GroupAccess>,
    pub after: Vec<GroupAccess>,
}

/// What comes between popping a frame and accessing below sp.
#[derive(Clone, Copy, Debug)]
pub enum Between {
    Nothing,
    /// A system call like `Op::Syscall`'s.
    Syscall,
    /// The access is made this many times in a loop counting in x16.
    Loop(u8),
}

/// How a walk's address moves: its base (x18) by indexing or by an add after the access, or an index
/// register read as `extend` (0 uxtw, 1 sxtw, 2 lsl) and shifted by the access size when `shift`.
#[derive(Clone, Copy, Debug)]
pub enum WalkMove {
    Base(Indexing),
    Index {
        reg: u8,
        extend: u8,
        shift: bool,
        /// The index changes by a 64 bit add (otherwise 32 bits).
        wide: bool,
        /// The step is in this register, set before the loop.
        step_in: Option<u8>,
        /// The index's first value.
        start: u64,
        /// The index changes before the access in each turn.
        first: bool,
    },
}

/// The name of a layer 2 program's blob.
pub const BLOB_NAME: &str = "layer2";

/// A walk's counter kinds (`Walk::counter`).
pub const COUNT_DOWN: u8 = 0;
pub const COUNT_UP: u8 = 1;
pub const UP_TO_BOUND: u8 = 2;
pub const DOWN_TO_BOUND: u8 = 3;
pub const DECREMENT: u8 = 4;

/// A loop over memory: its counter and the access it makes each turn.
#[derive(Clone, Copy, Debug)]
pub struct Walk {
    /// 0: subs and b.ne; 1: add, cmp and b.ne; 2: up to a bound by an ordered condition; 3: down to one;
    /// 4: sub and cbnz.
    pub counter: u8,
    pub wide: bool,
    pub turns: u8,
    /// The counter's step for kinds 2 and 3.
    pub by: u8,
    /// The ordered condition for kinds 2 (lo, ls, lt, le) and 3 (hi, hs, gt, ge), by number.
    pub cond: u8,
    /// The comparison comes before the counter changes.
    pub compare_first: bool,
    pub at: GroupBase,
    pub load: bool,
    /// 1 or 8, or with `vector` 16, 32, 48 or 64 (as in [`GroupAccess`]).
    pub bytes: u8,
    pub vector: bool,
    pub rt: u8,
    pub moves: WalkMove,
    /// Bytes the address moves by each turn, before any shift by the access size.
    pub step: i8,
    /// A system call like `Op::Syscall`'s after the access each turn (then the index and the step are not
    /// in registers the call sets).
    pub syscall: bool,
    /// A direct call after the access each turn (only when the base moves, as functions keep x16 and x18
    /// but not an index).
    pub call: Option<Callee>,
}

/// A loop whose access goes through a table: `base` plus `index` (read as `extend`: 0 uxtw, 1 sxtw, 2 lsl,
/// shifted by the access size when `shift`).
#[derive(Clone, Debug)]
pub struct Lookup {
    pub turns: u8,
    pub base: u8,
    pub index: u8,
    pub rt: u8,
    /// Holds the place of the loop's byte table: the switch's two entries, then a switch choice for each
    /// turn, then an index for each turn.
    pub table: u8,
    /// The index is loaded from the table each turn; otherwise it starts at `start` and has the counter
    /// exclusive-ored into it each turn.
    pub loaded: bool,
    pub start: u16,
    pub values: Vec<u8>,
    /// The access is skipped when the index is above `bound` (or at or above it when not `inclusive`),
    /// compared in 64 bits when `wide`.
    pub guard: Option<(u16, bool, bool)>,
    pub extend: u8,
    pub shift: bool,
    pub load: bool,
    /// 1 or 8.
    pub bytes: u8,
    pub at: GroupBase,
    /// After the access, a switch through the table's first two bytes on the turn's choice (2 skips it).
    pub switch: bool,
    pub choices: Vec<u8>,
    /// The loop goes back to its head through a switch whose cases are the head and the way out, as a
    /// compiler may make of a `continue` in a switch. Only with a loaded index (the switch uses its register).
    pub back: bool,
}

/// A function may call only functions with lower numbers, so calls always end.
#[derive(Clone, Debug)]
pub struct Function {
    /// Bytes of stack each call (and each recursive call) moves sp down by, a multiple of 16 and at least
    /// `FRAME_FLOOR`.
    pub frame: u32,
    pub body: Vec<Chunk>,
    /// Call itself this many more times, counting in x17.
    pub recursion: u8,
    /// End by jumping to a lower numbered function instead of returning.
    pub tail: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub registers: Vec<u64>,
    pub main: Vec<Chunk>,
    pub functions: Vec<Function>,
}

fn register<G: GuestTest>() -> impl Strategy<Value = u8> {
    prop::sample::select(G::MODEL.registers)
}

fn group_access<G: GuestTest>() -> impl Strategy<Value = GroupAccess> {
    (
        any::<bool>(),
        prop::sample::select(ACCESS_KINDS.as_slice()),
        register::<G>(),
        register::<G>(),
        prop_oneof![
            2 => Just(Indexing::Offset),
            1 => Just(Indexing::Pre),
            1 => Just(Indexing::Post)
        ],
        -64i16..64,
    )
        .prop_map(|(load, (bytes, vector), rt, rt2, indexing, offset)| {
            fitted(GroupAccess {
                load,
                bytes,
                vector,
                rt,
                rt2,
                indexing,
                offset,
            })
        })
}

/// An access with its offset and indexing made encodable: `offset` is taken as a random number from -64 to
/// 63, or as the step in bytes when indexing.
fn fitted(mut a: GroupAccess) -> GroupAccess {
    let structure = a.vector && a.bytes >= 48;
    if structure && !matches!(a.indexing, Indexing::Offset) {
        a.indexing = Indexing::Post;
    }
    a.offset = match (a.indexing, a.bytes, a.vector) {
        (_, _, true) if structure => match a.indexing {
            Indexing::Offset => 0,
            _ => a.bytes.into(),
        },
        (Indexing::Offset, 16, false) => a.offset.clamp(-8, 7),
        (Indexing::Offset, 16, true) => a.offset.rem_euclid(16),
        (Indexing::Offset, 32, true) => a.offset.clamp(-4, 3),
        (Indexing::Offset, ..) => a.offset.rem_euclid(32),
        (_, 16, false) => a.offset & !7,
        (_, 32, true) => a.offset & !15,
        _ => a.offset,
    };
    if a.vector {
        // A pair of Q registers loads into two different ones.
        a.rt2 = (a.rt + 1) % 32;
    }
    a
}

/// The lowest byte accesses made one after another through one base reach, from where the base starts.
fn lowest(accesses: &[GroupAccess]) -> i32 {
    let mut base = 0;
    let mut low = 0;
    for a in accesses {
        let by = i32::from(a.offset);
        match a.indexing {
            Indexing::Offset => {
                let unit = match (a.bytes, a.vector) {
                    (16, false) => 8,
                    (16 | 32, true) => 16,
                    (_, true) => 0,
                    (bytes, false) => i32::from(bytes),
                };
                low = low.min(base + by * unit);
            }
            Indexing::Pre => {
                base += by;
                low = low.min(base);
            }
            Indexing::Post => {
                low = low.min(base);
                base += by;
            }
        }
    }
    low
}

/// A start in the area moved up as far as the accesses from it reach below it, so they stay in the area.
fn above(start: GroupBase, accesses: &[GroupAccess]) -> GroupBase {
    match start {
        GroupBase::Area(at) => {
            let at = (i32::from(at).max(-lowest(accesses)) as u32).next_multiple_of(8);
            GroupBase::Area(at as u16)
        }
        edge => edge,
    }
}

/// `r`, or the next of the model's registers after it that is not in `not`.
fn other<G: GuestTest>(r: u8, not: &[u8]) -> u8 {
    let mut r = r;
    while not.contains(&r) {
        let registers = G::MODEL.registers;
        r = registers[(registers.iter().position(|&x| x == r).unwrap() + 1) % registers.len()];
    }
    r
}

fn group<G: GuestTest>() -> impl Strategy<Value = Op> {
    (
        register::<G>(),
        group_start(),
        prop::collection::vec(group_access::<G>(), 1..5),
        prop::option::weighted(0.5, 0usize..4),
    )
        .prop_map(|(base, start, accesses, syscall)| {
            // Inside the group or just after it, where a loop goes back to its first access.
            let syscall = syscall.map(|at| 1 + at % accesses.len());
            let base = if syscall.is_some() {
                other::<G>(base, G::MODEL.syscall_sets)
            } else {
                base
            };
            let accesses = through::<G>(base, accesses, false);
            Op::Group {
                base,
                start: above(start, &accesses),
                accesses,
                syscall,
            }
        })
}

/// Accesses fitted to their base: Arm leaves a transfer register that is also a base written back, or a pair
/// loaded into one register, unpredictable (the CPU may raise illegal instruction). With `keep`, no access
/// loads into the base either. Where calls write return addresses into memory (x86), only the last access
/// may load into the base: an access through a value loaded from the area may follow a code address and
/// write over the program's code, which then runs bytes no case defines. Vector registers are never the
/// base, and `fitted` already keeps a vector pair's two apart.
fn through<G: GuestTest>(base: u8, accesses: Vec<GroupAccess>, keep: bool) -> Vec<GroupAccess> {
    let code_addresses_in_memory = matches!(G::RETURN_ADDRESS, ReturnAddress::Stack { .. });
    let last = accesses.len() - 1;
    accesses
        .into_iter()
        .enumerate()
        .map(|(n, mut a)| {
            if a.vector {
                return a;
            }
            let writes_back = !matches!(a.indexing, Indexing::Offset);
            let followed = code_addresses_in_memory && n < last;
            let base_kept = writes_back || (a.load && (keep || followed));
            if base_kept {
                a.rt = other::<G>(a.rt, &[base]);
            }
            if a.bytes == 16 {
                let base = base_kept.then_some(base);
                let first = a.load.then_some(a.rt);
                let not: Vec<u8> = base.into_iter().chain(first).collect();
                a.rt2 = other::<G>(a.rt2, &not);
            }
            a
        })
        .collect()
}

fn group_start() -> impl Strategy<Value = GroupBase> {
    prop_oneof![
        (0u16..24).prop_map(|slot| GroupBase::Area(slot * 8)),
        // Near the edge often enough to cross it now and then.
        (0u16..96).prop_map(GroupBase::Edge),
        (96u16..2048).prop_map(GroupBase::Edge),
    ]
}

fn around<G: GuestTest>() -> impl Strategy<Value = Around> {
    (
        group_start(),
        prop::collection::vec(group_access::<G>(), 1..4),
        prop::collection::vec(group_access::<G>(), 1..4),
    )
        .prop_map(|(start, before, after)| {
            let before = through::<G>(G::MODEL.kept, before, true);
            let after = through::<G>(G::MODEL.kept, after, true);
            let all: Vec<GroupAccess> = before.iter().chain(&after).copied().collect();
            Around {
                start: above(start, &all),
                before,
                after,
            }
        })
}

fn below<G: GuestTest>() -> impl Strategy<Value = Chunk> {
    (
        1u16..17,
        any::<u16>(),
        any::<bool>(),
        register::<G>(),
        prop_oneof![
            Just(Between::Nothing),
            Just(Between::Syscall),
            (1u8..6).prop_map(Between::Loop)
        ],
    )
        .prop_map(|(frame, below, load, rt, between)| {
            let frame = frame * 16;
            Chunk::Below {
                frame,
                below: 8 + below % (frame - 7) / 8 * 8,
                load,
                rt,
                between,
            }
        })
}

/// A function numbered below `callable`, or the writer.
fn callee(callable: usize) -> BoxedStrategy<Callee> {
    if callable == 0 {
        Just(Callee::Writer).boxed()
    } else {
        prop_oneof![
            (0..callable).prop_map(Callee::Function),
            Just(Callee::Writer)
        ]
        .boxed()
    }
}

fn walk<G: GuestTest>() -> impl Strategy<Value = Walk> {
    let moves = prop_oneof![
        prop_oneof![
            Just(Indexing::Offset),
            Just(Indexing::Pre),
            Just(Indexing::Post)
        ]
        .prop_map(WalkMove::Base),
        (
            register::<G>(),
            0u8..3,
            any::<bool>(),
            any::<bool>(),
            prop::option::of(register::<G>()),
            prop_oneof![
                (0u64..64),
                // Just below 2^32, so a 32 bit index wraps, or below 2^31, so a signed one does.
                (1u64..64).prop_map(|k| (1 << 32) - k),
                (1u64..64).prop_map(|k| (1 << 31) - k),
            ],
            any::<bool>(),
        )
            .prop_map(|(reg, extend, shift, wide, step_in, start, first)| {
                WalkMove::Index {
                    reg,
                    extend,
                    shift,
                    wide,
                    step_in: step_in.filter(|&r| r != reg),
                    start,
                    first,
                }
            }),
    ];
    (
        (
            0u8..5,
            any::<bool>(),
            1u8..40,
            1u8..5,
            0u8..4,
            any::<bool>(),
        ),
        prop_oneof![
            (0u16..512).prop_map(GroupBase::Area),
            (0u16..64).prop_map(GroupBase::Edge),
            (64u16..2048).prop_map(GroupBase::Edge),
        ],
        (
            any::<bool>(),
            prop_oneof![
                3 => prop::sample::select(vec![(1u8, false), (8, false)]),
                1 => prop::sample::select(vec![(16u8, true), (32, true), (48, true), (64, true)]),
            ],
            register::<G>(),
        ),
        moves,
        -32i8..33,
        prop::bool::weighted(0.25),
    )
        .prop_map(
            |(
                (counter, wide, turns, by, cond, compare_first),
                at,
                (load, (bytes, vector), rt),
                moves,
                step,
                syscall,
            )| {
                let mut moves = moves;
                let mut step = step;
                // Pairs and structures of vector registers have no register offset form.
                if vector && bytes > 16 && matches!(moves, WalkMove::Index { .. }) {
                    moves = WalkMove::Base(Indexing::Offset);
                }
                // A structure moves its base only by its size, after the access.
                let structure = vector && bytes >= 48;
                if let WalkMove::Base(indexing) = &mut moves
                    && structure
                    && !matches!(indexing, Indexing::Offset)
                {
                    *indexing = Indexing::Post;
                    step = bytes as i8;
                }
                if let WalkMove::Index {
                    shift,
                    reg,
                    step_in,
                    ..
                } = &mut moves
                {
                    *shift &= bytes == 8 && !vector;
                    if syscall {
                        *reg = other::<G>(*reg, G::MODEL.syscall_sets);
                        *step_in = step_in
                            .map(|r| other::<G>(r, G::MODEL.syscall_sets))
                            .filter(|&r| r != *reg);
                    }
                }
                let scale = match moves {
                    WalkMove::Index { shift: true, .. } => 8,
                    _ => 1,
                };
                let mut at = at;
                // The most bytes a walk may cover: the turns, two more for the counter's forms, and the
                // access itself.
                let longest = i32::from(bytes.max(8));
                let span =
                    |step: i8| (i32::from(turns) + 2) * i32::from(step).abs() * scale + longest;
                match at {
                    // Forward only near the edge, onto the page that allows nothing.
                    GroupBase::Edge(_) => step = step.abs(),
                    GroupBase::Area(start) => {
                        if structure && span(step) > AREA as i32 {
                            moves = WalkMove::Base(Indexing::Offset);
                        }
                        while span(step) > AREA as i32 {
                            step /= 2;
                        }
                        let room = AREA as i32 - span(step);
                        let first = i32::from(start) % (room + 1);
                        let first = if step < 0 {
                            AREA as i32 - longest - first
                        } else {
                            first
                        };
                        at = GroupBase::Area(first as u16);
                    }
                }
                if let WalkMove::Base(indexing) = moves
                    && !matches!(indexing, Indexing::Offset)
                    && !structure
                {
                    // Pre and post indexing move by -64 to 63 bytes, a pair of Q registers by multiples of 16;
                    // both only shorten the step, so the walk stays where it was fitted.
                    step = step.clamp(-32, 31);
                    if vector && bytes == 32 {
                        step = step / 16 * 16;
                    }
                }
                Walk {
                    counter,
                    wide,
                    turns,
                    by,
                    cond,
                    compare_first,
                    at,
                    load,
                    bytes,
                    vector,
                    rt,
                    moves,
                    step,
                    syscall,
                    call: None,
                }
            },
        )
}

fn lookup<G: GuestTest>() -> impl Strategy<Value = Lookup> {
    (
        (
            1u8..24,
            register::<G>(),
            register::<G>(),
            register::<G>(),
            register::<G>(),
        ),
        (
            any::<bool>(),
            0u16..600,
            prop::collection::vec(prop_oneof![3 => 0u8..16, 1 => any::<u8>()], 24),
        ),
        prop::option::of((
            prop_oneof![3 => 0u16..64, 1 => 64u16..4096],
            any::<bool>(),
            any::<bool>(),
        )),
        (
            0u8..3,
            any::<bool>(),
            any::<bool>(),
            prop::sample::select(vec![1u8, 8]),
        ),
        prop_oneof![
            (0u16..512).prop_map(GroupBase::Area),
            (0u16..2048).prop_map(GroupBase::Edge),
        ],
        (
            any::<bool>(),
            prop::collection::vec(0u8..3, 24),
            prop::bool::weighted(0.3),
        ),
        prop::bool::weighted(0.3),
    )
        .prop_map(
            |(
                (turns, base, index, rt, table),
                (loaded, start, values),
                guard,
                (extend, shift, load, bytes),
                at,
                (switch, choices, back),
                tight,
            )| {
                // Four different registers.
                let next = |r: u8, not: &[u8]| {
                    *G::MODEL
                        .registers
                        .iter()
                        .cycle()
                        .skip_while(|&&x| x != r)
                        .find(|x| !not.contains(x))
                        .unwrap()
                };
                let index = next(index, &[base]);
                let rt = next(rt, &[base, index]);
                let table = next(table, &[base, index, rt]);
                let shift = shift && bytes == 8;
                let scale: u32 = if shift { 8 } else { 1 };
                // A worked out index, a store and an access in the area need the guard. Every access stays
                // within the area, or within 2 KiB of a base near the end of the blob, so it runs onto the
                // page after it and no further.
                let mut guard = guard;
                let mut at = at;
                if guard.is_none() && (!loaded || !load || matches!(at, GroupBase::Area(_))) {
                    guard = Some((255, true, false));
                }
                // Tight: the bound is the largest index the turns load, and an access there starts at the
                // end of the blob.
                if let (true, true, Some((bound, inclusive, _))) = (tight, loaded, &mut guard) {
                    *bound = u16::from(*values[..usize::from(turns)].iter().max().unwrap());
                    *inclusive = true;
                    if let GroupBase::Edge(by) = &mut at {
                        *by = (u32::from(*bound) * scale).min(2040) as u16;
                    }
                }
                if let Some((bound, inclusive, _)) = &mut guard {
                    *bound = (*bound).min((2040 / scale) as u16);
                    if let GroupBase::Area(first) = &mut at {
                        *inclusive = true;
                        *bound = (*bound).min(((AREA - 8) / scale) as u16 - 1);
                        *first = (*first).min((AREA - 8 - u32::from(*bound + 1) * scale) as u16);
                    }
                }
                Lookup {
                    turns,
                    base,
                    index,
                    rt,
                    table,
                    loaded,
                    start,
                    values,
                    guard,
                    extend,
                    shift,
                    load,
                    bytes,
                    at,
                    switch,
                    choices,
                    back: back && loaded,
                }
            },
        )
}

fn op<G: GuestTest>() -> impl Strategy<Value = Op> {
    prop_oneof![10 => plain_op::<G>(), 1 => Just(Op::Syscall)]
}

fn plain_op<G: GuestTest>() -> impl Strategy<Value = Op> {
    prop_oneof![
        (
            0u8..13,
            any::<bool>(),
            register::<G>(),
            register::<G>(),
            register::<G>()
        )
            .prop_map(|(kind, wide, rd, rn, rm)| Op::Arithmetic {
                kind,
                wide,
                rd,
                rn,
                rm
            }),
        (
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            register::<G>(),
            register::<G>(),
            0u16..4096
        )
            .prop_map(|(subtract, flags, wide, rd, rn, imm)| Op::AddImmediate {
                subtract,
                flags,
                wide,
                rd,
                rn,
                imm
            }),
        (any::<bool>(), register::<G>(), any::<u16>(), 0u8..4)
            .prop_map(|(keep, rd, imm, hw)| Op::Move { keep, rd, imm, hw }),
        (
            0u8..4,
            any::<bool>(),
            register::<G>(),
            register::<G>(),
            register::<G>(),
            0u8..14
        )
            .prop_map(|(kind, wide, rd, rn, rm, cond)| Op::Select {
                kind,
                wide,
                rd,
                rn,
                rm,
                cond
            }),
        (any::<bool>(), register::<G>(), register::<G>()).prop_map(|(wide, rn, rm)| Op::Compare {
            wide,
            rn,
            rm
        }),
        (
            any::<bool>(),
            register::<G>(),
            register::<G>(),
            0u8..16,
            0u8..14
        )
            .prop_map(|(wide, rn, rm, nzcv, cond)| Op::CondCompare {
                wide,
                rn,
                rm,
                nzcv,
                cond
            }),
        (
            any::<bool>(),
            any::<bool>(),
            register::<G>(),
            register::<G>(),
            register::<G>()
        )
            .prop_map(|(signed, wide, rd, rn, rm)| Op::Divide {
                signed,
                wide,
                rd,
                rn,
                rm
            }),
        (any::<bool>(), register::<G>()).prop_map(|(write, r)| Op::Flags { write, r }),
        (
            prop::sample::select(vec![1u8, 2, 4, 8]),
            register::<G>(),
            0u8..64
        )
            .prop_map(|(bytes, rt, slot)| Op::Load { bytes, rt, slot }),
        (
            prop::sample::select(vec![1u8, 2, 4, 8]),
            register::<G>(),
            0u8..64
        )
            .prop_map(|(bytes, rt, slot)| Op::Store { bytes, rt, slot }),
        (any::<bool>(), register::<G>(), register::<G>(), 0u8..63).prop_map(
            |(load, rt, rt2, slot)| {
                Op::Pair {
                    load,
                    rt,
                    rt2: if load && rt2 == rt {
                        (rt + 1) % 16
                    } else {
                        rt2
                    },
                    slot,
                }
            }
        ),
        (
            any::<bool>(),
            register::<G>(),
            0u8..64,
            prop_oneof![
                4 => Just(StackVector::No),
                1 => Just(StackVector::Pair),
                1 => Just(StackVector::Structure),
            ]
        )
            .prop_map(|(load, rt, slot, vector)| Op::Stack {
                load,
                rt,
                slot,
                vector
            }),
        group::<G>(),
    ]
}

fn ops<G: GuestTest>() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(op::<G>(), 0..8)
}

/// A chunk that may call functions numbered below `callable`.
fn chunk<G: GuestTest>(callable: usize) -> BoxedStrategy<Chunk> {
    let choices = vec![
        ops::<G>().prop_map(Chunk::Ops).boxed(),
        (0u8..14, ops::<G>())
            .prop_map(|(cond, ops)| Chunk::Skip { cond, ops })
            .boxed(),
        (
            1u8..6,
            ops::<G>(),
            prop::option::weighted(0.4, callee(callable)),
        )
            .prop_map(|(count, ops, call)| Chunk::Loop { count, ops, call })
            .boxed(),
        (walk::<G>(), prop::option::weighted(0.4, callee(callable)))
            .prop_map(|(mut walk, call)| {
                walk.call = call.filter(|_| matches!(walk.moves, WalkMove::Base(_)));
                Chunk::Walk(walk)
            })
            .boxed(),
        below::<G>().boxed(),
        lookup::<G>().prop_map(Chunk::Lookup).boxed(),
        (register::<G>(), prop::collection::vec(ops::<G>(), 1..6))
            .prop_map(|(index, cases)| Chunk::Switch { index, cases })
            .boxed(),
        (any::<bool>(), ops::<G>())
            .prop_map(|(through_register, ops)| Chunk::Jump {
                through_register,
                ops,
            })
            .boxed(),
        (
            callee(callable),
            any::<bool>(),
            prop::option::weighted(0.5, around::<G>()),
        )
            .prop_map(|(callee, through_register, around)| Chunk::Call {
                callee,
                through_register,
                around,
            })
            .boxed(),
    ];
    prop::strategy::Union::new(choices).boxed()
}

pub fn program<G: GuestTest>() -> impl Strategy<Value = Program> {
    (0usize..5).prop_flat_map(|count| {
        let functions: Vec<BoxedStrategy<Function>> = (0..count)
            .map(|i| {
                (
                    prop_oneof![
                        8 => (0u32..64).prop_map(|k| FRAME_FLOOR.next_multiple_of(16) + 16 * k),
                        // From a few pages to more than the whole stack.
                        1 => (1u32..48).prop_map(|k| k << 15),
                    ],
                    prop::collection::vec(chunk::<G>(i), 0..4),
                    0u8..4,
                    prop::option::of(0..i.max(1)),
                )
                    .prop_map(move |(frame, body, recursion, tail)| Function {
                        frame,
                        body,
                        recursion,
                        tail: tail.filter(|&t| t < i),
                    })
                    .boxed()
            })
            .collect();
        (
            prop::collection::vec(any::<u64>(), G::MODEL.registers.len()),
            prop::collection::vec(chunk::<G>(count), 1..8),
            functions,
        )
            .prop_map(|(registers, main, functions)| Program {
                registers,
                main,
                functions,
            })
    })
}
