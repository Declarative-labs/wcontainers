//! Layer 2: random small ARM64 programs built to always end, assembled with dynasm. Each has loops with
//! counters, calls (some to a function that makes a system call), bounded recursion, tail calls, calls and
//! jumps through registers, compiler-style jump tables, flags carried across instructions, stack frames of
//! every size (some running off the stack), and memory use inside a fixed area, through the stack pointer,
//! and in groups through other registers, some near a page that allows nothing, groups and loops across
//! calls, accesses below sp once a frame is popped, and system calls that write nothing. At the end it
//! stores its registers and flags after the area and writes the whole area out; a program that faults ends
//! with SIGSEGV instead.

// dynasm's expansion converts register numbers to their own type.
#![allow(clippy::useless_conversion)]

use crate::programs::Blob;
use dynasmrt::aarch64::Aarch64Relocation;
use dynasmrt::{DynamicLabel, DynasmApi, DynasmLabelApi, VecAssembler, dynasm};
use proptest::prelude::*;

/// Bytes of the area random loads and stores use, from x29.
const AREA: u32 = 512;
/// The registers random instructions use: everything but x16 to x18 (scratch for loops, recursion and
/// jumps), x29 (the area) and x30.
const REGISTERS: [u8; 26] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28,
];
/// Bytes written out at the end: the area, then `REGISTERS` and NZCV.
pub const OUTPUT: u32 = AREA + (REGISTERS.len() as u32 + 1) * 8;
/// Bytes the area and what is written out after it take in the blob.
const AREA_BYTES: usize = OUTPUT.next_multiple_of(16) as usize;

/// Where a program's area starts in its blob: no access reaches below it, into the code and tables.
pub fn area_offset(blob: &Blob) -> usize {
    blob.bytes.len() - 4096 - AREA_BYTES
}
/// Bytes of the stack every function frame keeps below what random accesses through sp reach: x30, x28, x16
/// and x18, which every function saves and restores, so a caller's loop or group across a call keeps them.
const FRAME_FLOOR: u32 = 544;
/// Where random accesses through sp start.
const STACK_SLOTS: u32 = 32;

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

fn register() -> impl Strategy<Value = u8> {
    prop::sample::select(REGISTERS.as_slice())
}

fn group_access() -> impl Strategy<Value = GroupAccess> {
    (
        any::<bool>(),
        prop::sample::select(ACCESS_KINDS.as_slice()),
        register(),
        register(),
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

/// `r`, or the next of `REGISTERS` after it that is not in `not`.
fn other(r: u8, not: &[u8]) -> u8 {
    let mut r = r;
    while not.contains(&r) {
        r = REGISTERS[(REGISTERS.iter().position(|&x| x == r).unwrap() + 1) % REGISTERS.len()];
    }
    r
}

fn group() -> impl Strategy<Value = Op> {
    (
        register(),
        group_start(),
        prop::collection::vec(group_access(), 1..5),
        prop::option::weighted(0.5, 0usize..4),
    )
        .prop_map(|(base, start, accesses, syscall)| {
            // Inside the group or just after it, where a loop goes back to its first access.
            let syscall = syscall.map(|at| 1 + at % accesses.len());
            let base = if syscall.is_some() {
                other(base, &SYSCALL_SETS)
            } else {
                base
            };
            let accesses = through(base, accesses, false);
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
/// loads into the base either.
fn through(base: u8, accesses: Vec<GroupAccess>, keep: bool) -> Vec<GroupAccess> {
    accesses
        .into_iter()
        .map(|mut a| {
            let writes_back = !matches!(a.indexing, Indexing::Offset);
            if writes_back || (keep && a.load) {
                a.rt = other(a.rt, &[base]);
            }
            if a.bytes == 16 && !a.vector {
                let base = (writes_back || (keep && a.load)).then_some(base);
                let first = a.load.then_some(a.rt);
                let not: Vec<u8> = base.into_iter().chain(first).collect();
                a.rt2 = other(a.rt2, &not);
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

fn around() -> impl Strategy<Value = Around> {
    (
        group_start(),
        prop::collection::vec(group_access(), 1..4),
        prop::collection::vec(group_access(), 1..4),
    )
        .prop_map(|(start, before, after)| {
            let before = through(28, before, true);
            let after = through(28, after, true);
            let all: Vec<GroupAccess> = before.iter().chain(&after).copied().collect();
            Around {
                start: above(start, &all),
                before,
                after,
            }
        })
}

fn below() -> impl Strategy<Value = Chunk> {
    (
        1u16..17,
        any::<u16>(),
        any::<bool>(),
        register(),
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

fn walk() -> impl Strategy<Value = Walk> {
    let moves = prop_oneof![
        prop_oneof![
            Just(Indexing::Offset),
            Just(Indexing::Pre),
            Just(Indexing::Post)
        ]
        .prop_map(WalkMove::Base),
        (
            register(),
            0u8..3,
            any::<bool>(),
            any::<bool>(),
            prop::option::of(register()),
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
            register(),
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
                        *reg = other(*reg, &SYSCALL_SETS);
                        *step_in = step_in
                            .map(|r| other(r, &SYSCALL_SETS))
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

fn lookup() -> impl Strategy<Value = Lookup> {
    (
        (1u8..24, register(), register(), register(), register()),
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
                    *REGISTERS
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

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![10 => plain_op(), 1 => Just(Op::Syscall)]
}

fn plain_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0u8..13, any::<bool>(), register(), register(), register()).prop_map(
            |(kind, wide, rd, rn, rm)| Op::Arithmetic {
                kind,
                wide,
                rd,
                rn,
                rm
            }
        ),
        (
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            register(),
            register(),
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
        (any::<bool>(), register(), any::<u16>(), 0u8..4)
            .prop_map(|(keep, rd, imm, hw)| Op::Move { keep, rd, imm, hw }),
        (
            0u8..4,
            any::<bool>(),
            register(),
            register(),
            register(),
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
        (any::<bool>(), register(), register()).prop_map(|(wide, rn, rm)| Op::Compare {
            wide,
            rn,
            rm
        }),
        (any::<bool>(), register(), register(), 0u8..16, 0u8..14).prop_map(
            |(wide, rn, rm, nzcv, cond)| Op::CondCompare {
                wide,
                rn,
                rm,
                nzcv,
                cond
            }
        ),
        (
            any::<bool>(),
            any::<bool>(),
            register(),
            register(),
            register()
        )
            .prop_map(|(signed, wide, rd, rn, rm)| Op::Divide {
                signed,
                wide,
                rd,
                rn,
                rm
            }),
        (any::<bool>(), register()).prop_map(|(write, r)| Op::Flags { write, r }),
        (
            prop::sample::select(vec![1u8, 2, 4, 8]),
            register(),
            0u8..64
        )
            .prop_map(|(bytes, rt, slot)| Op::Load { bytes, rt, slot }),
        (
            prop::sample::select(vec![1u8, 2, 4, 8]),
            register(),
            0u8..64
        )
            .prop_map(|(bytes, rt, slot)| Op::Store { bytes, rt, slot }),
        (any::<bool>(), register(), register(), 0u8..63).prop_map(|(load, rt, rt2, slot)| {
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
        }),
        (
            any::<bool>(),
            register(),
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
        group(),
    ]
}

fn ops() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(op(), 0..8)
}

/// A chunk that may call functions numbered below `callable`.
fn chunk(callable: usize) -> BoxedStrategy<Chunk> {
    let choices = vec![
        ops().prop_map(Chunk::Ops).boxed(),
        (0u8..14, ops())
            .prop_map(|(cond, ops)| Chunk::Skip { cond, ops })
            .boxed(),
        (1u8..6, ops(), prop::option::weighted(0.4, callee(callable)))
            .prop_map(|(count, ops, call)| Chunk::Loop { count, ops, call })
            .boxed(),
        (walk(), prop::option::weighted(0.4, callee(callable)))
            .prop_map(|(mut walk, call)| {
                walk.call = call.filter(|_| matches!(walk.moves, WalkMove::Base(_)));
                Chunk::Walk(walk)
            })
            .boxed(),
        below().boxed(),
        lookup().prop_map(Chunk::Lookup).boxed(),
        (register(), prop::collection::vec(ops(), 1..6))
            .prop_map(|(index, cases)| Chunk::Switch { index, cases })
            .boxed(),
        (any::<bool>(), ops())
            .prop_map(|(through_register, ops)| Chunk::Jump {
                through_register,
                ops,
            })
            .boxed(),
        (
            callee(callable),
            any::<bool>(),
            prop::option::weighted(0.5, around()),
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

pub fn program() -> impl Strategy<Value = Program> {
    (0usize..5).prop_flat_map(|count| {
        let functions: Vec<BoxedStrategy<Function>> = (0..count)
            .map(|i| {
                (
                    prop_oneof![
                        8 => (0u32..64).prop_map(|k| FRAME_FLOOR.next_multiple_of(16) + 16 * k),
                        // From a few pages to more than the whole stack.
                        1 => (1u32..48).prop_map(|k| k << 15),
                    ],
                    prop::collection::vec(chunk(i), 0..4),
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
            prop::collection::vec(any::<u64>(), REGISTERS.len()),
            prop::collection::vec(chunk(count), 1..8),
            functions,
        )
            .prop_map(|(registers, main, functions)| Program {
                registers,
                main,
                functions,
            })
    })
}

macro_rules! conditional {
    ($ops:expr, $cond:expr, $op:tt, $($rest:tt)*) => {
        match $cond {
            0 => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, eq),
            1 => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, ne),
            2 => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, hs),
            3 => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, lo),
            4 => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, mi),
            5 => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, pl),
            6 => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, vs),
            7 => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, vc),
            8 => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, hi),
            9 => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, ls),
            10 => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, ge),
            11 => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, lt),
            12 => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, gt),
            _ => dynasm!($ops ; .arch aarch64 ; $op $($rest)*, le),
        }
    };
}

type Ops = VecAssembler<Aarch64Relocation>;

fn branch_if(ops: &mut Ops, cond: u8, to: DynamicLabel) {
    match cond {
        0 => dynasm!(ops ; .arch aarch64 ; b.eq =>to),
        1 => dynasm!(ops ; .arch aarch64 ; b.ne =>to),
        2 => dynasm!(ops ; .arch aarch64 ; b.hs =>to),
        3 => dynasm!(ops ; .arch aarch64 ; b.lo =>to),
        4 => dynasm!(ops ; .arch aarch64 ; b.mi =>to),
        5 => dynasm!(ops ; .arch aarch64 ; b.pl =>to),
        6 => dynasm!(ops ; .arch aarch64 ; b.vs =>to),
        7 => dynasm!(ops ; .arch aarch64 ; b.vc =>to),
        8 => dynasm!(ops ; .arch aarch64 ; b.hi =>to),
        9 => dynasm!(ops ; .arch aarch64 ; b.ls =>to),
        10 => dynasm!(ops ; .arch aarch64 ; b.ge =>to),
        11 => dynasm!(ops ; .arch aarch64 ; b.lt =>to),
        12 => dynasm!(ops ; .arch aarch64 ; b.gt =>to),
        _ => dynasm!(ops ; .arch aarch64 ; b.le =>to),
    }
}

fn emit_op(ops: &mut Ops, edge: DynamicLabel, op: &Op) {
    match *op {
        Op::Arithmetic {
            kind,
            wide: true,
            rd,
            rn,
            rm,
        } => match kind {
            0 => dynasm!(ops ; .arch aarch64 ; add X(rd), X(rn), X(rm)),
            1 => dynasm!(ops ; .arch aarch64 ; sub X(rd), X(rn), X(rm)),
            2 => dynasm!(ops ; .arch aarch64 ; and X(rd), X(rn), X(rm)),
            3 => dynasm!(ops ; .arch aarch64 ; orr X(rd), X(rn), X(rm)),
            4 => dynasm!(ops ; .arch aarch64 ; eor X(rd), X(rn), X(rm)),
            5 => dynasm!(ops ; .arch aarch64 ; adds X(rd), X(rn), X(rm)),
            6 => dynasm!(ops ; .arch aarch64 ; subs X(rd), X(rn), X(rm)),
            7 => dynasm!(ops ; .arch aarch64 ; ands X(rd), X(rn), X(rm)),
            8 => dynasm!(ops ; .arch aarch64 ; lslv X(rd), X(rn), X(rm)),
            9 => dynasm!(ops ; .arch aarch64 ; lsrv X(rd), X(rn), X(rm)),
            10 => dynasm!(ops ; .arch aarch64 ; asrv X(rd), X(rn), X(rm)),
            11 => dynasm!(ops ; .arch aarch64 ; rorv X(rd), X(rn), X(rm)),
            _ => dynasm!(ops ; .arch aarch64 ; mul X(rd), X(rn), X(rm)),
        },
        Op::Arithmetic {
            kind,
            wide: false,
            rd,
            rn,
            rm,
        } => match kind {
            0 => dynasm!(ops ; .arch aarch64 ; add W(rd), W(rn), W(rm)),
            1 => dynasm!(ops ; .arch aarch64 ; sub W(rd), W(rn), W(rm)),
            2 => dynasm!(ops ; .arch aarch64 ; and W(rd), W(rn), W(rm)),
            3 => dynasm!(ops ; .arch aarch64 ; orr W(rd), W(rn), W(rm)),
            4 => dynasm!(ops ; .arch aarch64 ; eor W(rd), W(rn), W(rm)),
            5 => dynasm!(ops ; .arch aarch64 ; adds W(rd), W(rn), W(rm)),
            6 => dynasm!(ops ; .arch aarch64 ; subs W(rd), W(rn), W(rm)),
            7 => dynasm!(ops ; .arch aarch64 ; ands W(rd), W(rn), W(rm)),
            8 => dynasm!(ops ; .arch aarch64 ; lslv W(rd), W(rn), W(rm)),
            9 => dynasm!(ops ; .arch aarch64 ; lsrv W(rd), W(rn), W(rm)),
            10 => dynasm!(ops ; .arch aarch64 ; asrv W(rd), W(rn), W(rm)),
            11 => dynasm!(ops ; .arch aarch64 ; rorv W(rd), W(rn), W(rm)),
            _ => dynasm!(ops ; .arch aarch64 ; mul W(rd), W(rn), W(rm)),
        },
        Op::AddImmediate {
            subtract,
            flags,
            wide,
            rd,
            rn,
            imm,
        } => {
            let imm = u32::from(imm);
            match (subtract, flags, wide) {
                (false, false, true) => dynasm!(ops ; .arch aarch64 ; add XSP(rd), XSP(rn), #imm),
                (false, true, true) => dynasm!(ops ; .arch aarch64 ; adds X(rd), XSP(rn), #imm),
                (true, false, true) => dynasm!(ops ; .arch aarch64 ; sub XSP(rd), XSP(rn), #imm),
                (true, true, true) => dynasm!(ops ; .arch aarch64 ; subs X(rd), XSP(rn), #imm),
                (false, false, false) => dynasm!(ops ; .arch aarch64 ; add WSP(rd), WSP(rn), #imm),
                (false, true, false) => dynasm!(ops ; .arch aarch64 ; adds W(rd), WSP(rn), #imm),
                (true, false, false) => dynasm!(ops ; .arch aarch64 ; sub WSP(rd), WSP(rn), #imm),
                (true, true, false) => dynasm!(ops ; .arch aarch64 ; subs W(rd), WSP(rn), #imm),
            }
        }
        Op::Move { keep, rd, imm, hw } => {
            let imm = u32::from(imm);
            let shift = u32::from(hw) * 16;
            if keep {
                dynasm!(ops ; .arch aarch64 ; movk X(rd), #imm, lsl #shift);
            } else {
                dynasm!(ops ; .arch aarch64 ; movz X(rd), #imm, lsl #shift);
            }
        }
        Op::Select {
            kind,
            wide: true,
            rd,
            rn,
            rm,
            cond,
        } => match kind {
            0 => conditional!(ops, cond, csel, X(rd), X(rn), X(rm)),
            1 => conditional!(ops, cond, csinc, X(rd), X(rn), X(rm)),
            2 => conditional!(ops, cond, csinv, X(rd), X(rn), X(rm)),
            _ => conditional!(ops, cond, csneg, X(rd), X(rn), X(rm)),
        },
        Op::Select {
            kind,
            wide: false,
            rd,
            rn,
            rm,
            cond,
        } => match kind {
            0 => conditional!(ops, cond, csel, W(rd), W(rn), W(rm)),
            1 => conditional!(ops, cond, csinc, W(rd), W(rn), W(rm)),
            2 => conditional!(ops, cond, csinv, W(rd), W(rn), W(rm)),
            _ => conditional!(ops, cond, csneg, W(rd), W(rn), W(rm)),
        },
        Op::Compare { wide: true, rn, rm } => dynasm!(ops ; .arch aarch64 ; cmp X(rn), X(rm)),
        Op::Compare {
            wide: false,
            rn,
            rm,
        } => dynasm!(ops ; .arch aarch64 ; cmp W(rn), W(rm)),
        Op::CondCompare {
            wide,
            rn,
            rm,
            nzcv,
            cond,
        } => {
            let nzcv = u32::from(nzcv);
            // ccmp Xn, Xm, #nzcv, cond: the condition comes last.
            if wide {
                ccmp_wide(ops, rn, rm, nzcv, cond);
            } else {
                ccmp_narrow(ops, rn, rm, nzcv, cond);
            }
        }
        Op::Divide {
            signed,
            wide,
            rd,
            rn,
            rm,
        } => match (signed, wide) {
            (false, true) => dynasm!(ops ; .arch aarch64 ; udiv X(rd), X(rn), X(rm)),
            (true, true) => dynasm!(ops ; .arch aarch64 ; sdiv X(rd), X(rn), X(rm)),
            (false, false) => dynasm!(ops ; .arch aarch64 ; udiv W(rd), W(rn), W(rm)),
            (true, false) => dynasm!(ops ; .arch aarch64 ; sdiv W(rd), W(rn), W(rm)),
        },
        // mrs Xr, nzcv and msr nzcv, Xr, as words.
        Op::Flags { write: false, r } => {
            dynasm!(ops ; .arch aarch64 ; .u32 0xd53b_4200 | u32::from(r))
        }
        Op::Flags { write: true, r } => {
            dynasm!(ops ; .arch aarch64 ; .u32 0xd51b_4200 | u32::from(r))
        }
        Op::Load { bytes, rt, slot } => {
            let at = u32::from(slot) * 8;
            match bytes {
                1 => dynasm!(ops ; .arch aarch64 ; ldrb W(rt), [x29, #at]),
                2 => dynasm!(ops ; .arch aarch64 ; ldrh W(rt), [x29, #at]),
                4 => dynasm!(ops ; .arch aarch64 ; ldr W(rt), [x29, #at]),
                _ => dynasm!(ops ; .arch aarch64 ; ldr X(rt), [x29, #at]),
            }
        }
        Op::Store { bytes, rt, slot } => {
            let at = u32::from(slot) * 8;
            match bytes {
                1 => dynasm!(ops ; .arch aarch64 ; strb W(rt), [x29, #at]),
                2 => dynasm!(ops ; .arch aarch64 ; strh W(rt), [x29, #at]),
                4 => dynasm!(ops ; .arch aarch64 ; str W(rt), [x29, #at]),
                _ => dynasm!(ops ; .arch aarch64 ; str X(rt), [x29, #at]),
            }
        }
        Op::Pair {
            load,
            rt,
            rt2,
            slot,
        } => {
            let at = i32::from(slot) * 8;
            if load {
                dynasm!(ops ; .arch aarch64 ; ldp X(rt), X(rt2), [x29, #at]);
            } else {
                dynasm!(ops ; .arch aarch64 ; stp X(rt), X(rt2), [x29, #at]);
            }
        }
        Op::Stack {
            load,
            rt,
            slot,
            vector,
        } => {
            let at = STACK_SLOTS + u32::from(slot) * 8;
            let v = u32::from(rt);
            match vector {
                StackVector::No if load => dynasm!(ops ; .arch aarch64 ; ldr X(rt), [sp, #at]),
                StackVector::No => dynasm!(ops ; .arch aarch64 ; str X(rt), [sp, #at]),
                StackVector::Pair => {
                    // ldp or stp of q registers rt and the next, 16 bytes up per slot from `STACK_SLOTS`.
                    let units = (STACK_SLOTS + u32::from(slot % 31) * 16) / 16;
                    let word = 0xad00_0000 | u32::from(load) << 22 | units << 15;
                    dynasm!(ops ; .arch aarch64 ; .u32 word | ((v + 1) % 32) << 10 | 31 << 5 | v);
                }
                StackVector::Structure => {
                    // ld4 {vN.2d-...}, [sp]: the frame's first 64 bytes, read only (they hold saved registers).
                    dynasm!(ops ; .arch aarch64 ; .u32 0x4c40_0c00 | 31 << 5 | v);
                }
            }
        }
        Op::Group {
            base,
            start,
            ref accesses,
            syscall,
        } => {
            point(ops, edge, base, start);
            for (i, access) in accesses.iter().enumerate() {
                if syscall == Some(i) {
                    emit_syscall(ops);
                }
                emit_group_access(ops, base, access);
            }
            if syscall == Some(accesses.len()) {
                emit_syscall(ops);
            }
        }
        Op::Syscall => emit_syscall(ops),
    }
}

/// Point a group's base at its start.
fn point(ops: &mut Ops, edge: DynamicLabel, base: u8, start: GroupBase) {
    match start {
        GroupBase::Area(at) => {
            let at = u32::from(at);
            dynasm!(ops ; .arch aarch64 ; add XSP(base), x29, #at);
        }
        GroupBase::Edge(by) => {
            let by = u32::from(by);
            dynasm!(ops ; .arch aarch64 ; adr X(base), =>edge ; sub XSP(base), XSP(base), #by);
        }
    }
}

/// The registers `Op::Syscall` sets.
const SYSCALL_SETS: [u8; 4] = [0, 1, 2, 8];

fn emit_syscall(ops: &mut Ops) {
    dynasm!(ops ; .arch aarch64 ; movz x0, #1 ; mov x1, x29 ; movz x2, #0 ; movz x8, #64 ; svc #0);
}

/// Vector loads and stores, as words: `rt`, `rt2` and `b` are put in by the caller.
fn emit_vector_access(ops: &mut Ops, b: u8, a: &GroupAccess) {
    let load = u32::from(a.load);
    let by = i32::from(a.offset);
    let (rt, rt2, rn) = (u32::from(a.rt), u32::from(a.rt2), u32::from(b) << 5);
    let word = match (a.bytes, a.indexing) {
        (16, Indexing::Offset) => 0x3d80_0000 | load << 22 | (by as u32) << 10,
        (16, Indexing::Pre) => 0x3c80_0c00 | load << 22 | (by as u32 & 0x1ff) << 12,
        (16, Indexing::Post) => 0x3c80_0400 | load << 22 | (by as u32 & 0x1ff) << 12,
        (32, indexing) => {
            let units = match indexing {
                Indexing::Offset => by,
                _ => by / 16,
            } as u32
                & 0x7f;
            let kind = match indexing {
                Indexing::Offset => 0xad00_0000,
                Indexing::Pre => 0xad80_0000,
                Indexing::Post => 0xac80_0000,
            };
            kind | load << 22 | units << 15 | rt2 << 10
        }
        (bytes, indexing) => {
            // ld1 or st1 of several registers, or ld3, st3, ld4 and st4 (by `rt2`'s low bit, with its next
            // two bits as the element size), base only or post-indexed by the size.
            let opcode = match (bytes, rt2 & 1) {
                (48, 0) => 0b0110,
                (48, _) => 0b0100,
                (_, 0) => 0b0010,
                _ => 0b0000,
            };
            let post = match indexing {
                Indexing::Post => 0x0080_0000 | 31 << 16,
                _ => 0,
            };
            0x4c00_0000 | post | load << 22 | opcode << 12 | (rt2 >> 1 & 3) << 10
        }
    };
    dynasm!(ops ; .arch aarch64 ; .u32 word | rn | rt);
}

fn emit_group_access(ops: &mut Ops, b: u8, a: &GroupAccess) {
    if a.vector {
        emit_vector_access(ops, b, a);
        return;
    }
    let GroupAccess {
        load,
        bytes,
        rt,
        rt2,
        indexing,
        offset,
        ..
    } = *a;
    let by = i32::from(offset);
    match (bytes, indexing) {
        (16, Indexing::Offset) => {
            let at = by * 8;
            if load {
                dynasm!(ops ; .arch aarch64 ; ldp X(rt), X(rt2), [X(b), #at]);
            } else {
                dynasm!(ops ; .arch aarch64 ; stp X(rt), X(rt2), [X(b), #at]);
            }
        }
        (16, Indexing::Pre) => {
            if load {
                dynasm!(ops ; .arch aarch64 ; ldp X(rt), X(rt2), [X(b), #by]!);
            } else {
                dynasm!(ops ; .arch aarch64 ; stp X(rt), X(rt2), [X(b), #by]!);
            }
        }
        (16, Indexing::Post) => {
            if load {
                dynasm!(ops ; .arch aarch64 ; ldp X(rt), X(rt2), [X(b)], #by);
            } else {
                dynasm!(ops ; .arch aarch64 ; stp X(rt), X(rt2), [X(b)], #by);
            }
        }
        (_, Indexing::Offset) => {
            let at = by as u32 * u32::from(bytes);
            match (bytes, load) {
                (1, true) => dynasm!(ops ; .arch aarch64 ; ldrb W(rt), [X(b), #at]),
                (1, false) => dynasm!(ops ; .arch aarch64 ; strb W(rt), [X(b), #at]),
                (2, true) => dynasm!(ops ; .arch aarch64 ; ldrh W(rt), [X(b), #at]),
                (2, false) => dynasm!(ops ; .arch aarch64 ; strh W(rt), [X(b), #at]),
                (4, true) => dynasm!(ops ; .arch aarch64 ; ldr W(rt), [X(b), #at]),
                (4, false) => dynasm!(ops ; .arch aarch64 ; str W(rt), [X(b), #at]),
                (_, true) => dynasm!(ops ; .arch aarch64 ; ldr X(rt), [X(b), #at]),
                (_, false) => dynasm!(ops ; .arch aarch64 ; str X(rt), [X(b), #at]),
            }
        }
        (_, Indexing::Pre) => match (bytes, load) {
            (1, true) => dynasm!(ops ; .arch aarch64 ; ldrb W(rt), [X(b), #by]!),
            (1, false) => dynasm!(ops ; .arch aarch64 ; strb W(rt), [X(b), #by]!),
            (2, true) => dynasm!(ops ; .arch aarch64 ; ldrh W(rt), [X(b), #by]!),
            (2, false) => dynasm!(ops ; .arch aarch64 ; strh W(rt), [X(b), #by]!),
            (4, true) => dynasm!(ops ; .arch aarch64 ; ldr W(rt), [X(b), #by]!),
            (4, false) => dynasm!(ops ; .arch aarch64 ; str W(rt), [X(b), #by]!),
            (_, true) => dynasm!(ops ; .arch aarch64 ; ldr X(rt), [X(b), #by]!),
            (_, false) => dynasm!(ops ; .arch aarch64 ; str X(rt), [X(b), #by]!),
        },
        (_, Indexing::Post) => match (bytes, load) {
            (1, true) => dynasm!(ops ; .arch aarch64 ; ldrb W(rt), [X(b)], #by),
            (1, false) => dynasm!(ops ; .arch aarch64 ; strb W(rt), [X(b)], #by),
            (2, true) => dynasm!(ops ; .arch aarch64 ; ldrh W(rt), [X(b)], #by),
            (2, false) => dynasm!(ops ; .arch aarch64 ; strh W(rt), [X(b)], #by),
            (4, true) => dynasm!(ops ; .arch aarch64 ; ldr W(rt), [X(b)], #by),
            (4, false) => dynasm!(ops ; .arch aarch64 ; str W(rt), [X(b)], #by),
            (_, true) => dynasm!(ops ; .arch aarch64 ; ldr X(rt), [X(b)], #by),
            (_, false) => dynasm!(ops ; .arch aarch64 ; str X(rt), [X(b)], #by),
        },
    }
}

fn ccmp_wide(ops: &mut Ops, rn: u8, rm: u8, nzcv: u32, cond: u8) {
    match cond {
        0 => dynasm!(ops ; .arch aarch64 ; ccmp X(rn), X(rm), #nzcv, eq),
        1 => dynasm!(ops ; .arch aarch64 ; ccmp X(rn), X(rm), #nzcv, ne),
        2 => dynasm!(ops ; .arch aarch64 ; ccmp X(rn), X(rm), #nzcv, hs),
        3 => dynasm!(ops ; .arch aarch64 ; ccmp X(rn), X(rm), #nzcv, lo),
        8 => dynasm!(ops ; .arch aarch64 ; ccmp X(rn), X(rm), #nzcv, hi),
        10 => dynasm!(ops ; .arch aarch64 ; ccmp X(rn), X(rm), #nzcv, ge),
        11 => dynasm!(ops ; .arch aarch64 ; ccmp X(rn), X(rm), #nzcv, lt),
        _ => dynasm!(ops ; .arch aarch64 ; ccmp X(rn), X(rm), #nzcv, le),
    }
}

fn ccmp_narrow(ops: &mut Ops, rn: u8, rm: u8, nzcv: u32, cond: u8) {
    match cond {
        0 => dynasm!(ops ; .arch aarch64 ; ccmp W(rn), W(rm), #nzcv, eq),
        1 => dynasm!(ops ; .arch aarch64 ; ccmp W(rn), W(rm), #nzcv, ne),
        4 => dynasm!(ops ; .arch aarch64 ; ccmp W(rn), W(rm), #nzcv, mi),
        9 => dynasm!(ops ; .arch aarch64 ; ccmp W(rn), W(rm), #nzcv, ls),
        12 => dynasm!(ops ; .arch aarch64 ; ccmp W(rn), W(rm), #nzcv, gt),
        _ => dynasm!(ops ; .arch aarch64 ; ccmp W(rn), W(rm), #nzcv, vs),
    }
}

/// Labels the whole program refers to, and jump tables to place after the code.
struct Layout {
    functions: Vec<DynamicLabel>,
    writer: DynamicLabel,
    /// The end of the blob.
    edge: DynamicLabel,
    tables: Vec<(DynamicLabel, Vec<u8>)>,
}

impl Layout {
    fn target(&self, callee: Callee) -> DynamicLabel {
        match callee {
            Callee::Function(f) => self.functions[f],
            Callee::Writer => self.writer,
        }
    }
}

fn emit_chunks(ops: &mut Ops, layout: &mut Layout, chunks: &[Chunk]) {
    for chunk in chunks {
        match chunk {
            Chunk::Ops(list) => list.iter().for_each(|op| emit_op(ops, layout.edge, op)),
            Chunk::Skip { cond, ops: list } => {
                let after = ops.new_dynamic_label();
                branch_if(ops, *cond, after);
                list.iter().for_each(|op| emit_op(ops, layout.edge, op));
                dynasm!(ops ; .arch aarch64 ; =>after);
            }
            Chunk::Loop {
                count,
                ops: list,
                call,
            } => {
                let top = ops.new_dynamic_label();
                let count = u32::from(*count);
                dynasm!(ops ; .arch aarch64 ; movz x16, #count ; =>top);
                list.iter().for_each(|op| emit_op(ops, layout.edge, op));
                if let Some(callee) = *call {
                    let target = layout.target(callee);
                    dynasm!(ops ; .arch aarch64 ; bl =>target);
                }
                dynasm!(ops ; .arch aarch64 ; subs x16, x16, #1 ; b.ne =>top);
            }
            Chunk::Walk(w) => emit_walk(ops, layout, w),
            Chunk::Lookup(l) => emit_lookup(ops, layout, l),
            Chunk::Call {
                callee,
                through_register,
                around,
            } => {
                if let Some(a) = around {
                    point(ops, layout.edge, 28, a.start);
                    a.before.iter().for_each(|a| emit_group_access(ops, 28, a));
                }
                let target = layout.target(*callee);
                if *through_register {
                    dynasm!(ops ; .arch aarch64 ; adr x18, =>target ; blr x18);
                } else {
                    dynasm!(ops ; .arch aarch64 ; bl =>target);
                }
                if let Some(a) = around {
                    a.after.iter().for_each(|a| emit_group_access(ops, 28, a));
                }
            }
            &Chunk::Below {
                frame,
                below,
                load,
                rt,
                between,
            } => {
                let frame = u32::from(frame);
                let below = -i32::from(below);
                dynasm!(ops ; .arch aarch64
                    ; mov x18, sp
                    ; and sp, x18, #0xffff_ffff_ffff_f000
                    ; sub sp, sp, #frame
                    ; str x18, [sp]
                    ; add sp, sp, #frame
                );
                let access = |ops: &mut Ops| {
                    if load {
                        dynasm!(ops ; .arch aarch64 ; ldur X(rt), [sp, #below]);
                    } else {
                        dynasm!(ops ; .arch aarch64 ; stur X(rt), [sp, #below]);
                    }
                };
                match between {
                    Between::Nothing => access(ops),
                    Between::Syscall => {
                        emit_syscall(ops);
                        access(ops);
                    }
                    Between::Loop(turns) => {
                        let top = ops.new_dynamic_label();
                        let turns = u32::from(turns);
                        dynasm!(ops ; .arch aarch64 ; movz x16, #turns ; =>top);
                        access(ops);
                        dynasm!(ops ; .arch aarch64 ; subs x16, x16, #1 ; b.ne =>top);
                    }
                }
                dynasm!(ops ; .arch aarch64 ; mov sp, x18);
            }
            Chunk::Switch { index, cases } => {
                let n = cases.len() as u32;
                let default = ops.new_dynamic_label();
                let table = ops.new_dynamic_label();
                let base = ops.new_dynamic_label();
                let end = ops.new_dynamic_label();
                let index = *index;
                dynasm!(ops ; .arch aarch64
                    ; and w16, W(index), #7
                    ; cmp w16, #n - 1
                    ; b.hi =>default
                    ; adr x18, =>table
                    ; ldrb w16, [x18, x16]
                    ; adr x18, =>base
                    ; add x18, x18, x16, lsl #2
                    ; br x18
                    ; =>base
                );
                let labels: Vec<DynamicLabel> =
                    cases.iter().map(|_| ops.new_dynamic_label()).collect();
                for &label in &labels {
                    dynasm!(ops ; .arch aarch64 ; b =>label);
                }
                for (label, case) in labels.iter().zip(cases) {
                    dynasm!(ops ; .arch aarch64 ; =>*label);
                    case.iter().for_each(|op| emit_op(ops, layout.edge, op));
                    dynasm!(ops ; .arch aarch64 ; b =>end);
                }
                dynasm!(ops ; .arch aarch64 ; =>default ; =>end);
                // Entry k goes to slot k, reversed so the table is not just the index.
                layout.tables.push((table, (0..n as u8).rev().collect()));
            }
            Chunk::Jump {
                through_register,
                ops: list,
            } => {
                let after = ops.new_dynamic_label();
                if *through_register {
                    dynasm!(ops ; .arch aarch64 ; adr x18, =>after ; br x18);
                } else {
                    dynasm!(ops ; .arch aarch64 ; b =>after);
                }
                list.iter().for_each(|op| emit_op(ops, layout.edge, op));
                dynasm!(ops ; .arch aarch64 ; =>after);
            }
        }
    }
}

/// Set a register to a 64 bit value.
fn set(ops: &mut Ops, r: u8, value: u64) {
    for hw in 0..4u32 {
        let part = (value >> (16 * hw)) as u32 & 0xffff;
        let shift = hw * 16;
        if hw == 0 {
            dynasm!(ops ; .arch aarch64 ; movz X(r), #part);
        } else {
            dynasm!(ops ; .arch aarch64 ; movk X(r), #part, lsl #shift);
        }
    }
}

fn emit_walk(ops: &mut Ops, layout: &Layout, w: &Walk) {
    let edge = layout.edge;
    let top = ops.new_dynamic_label();
    match w.at {
        GroupBase::Area(at) => {
            let at = u32::from(at);
            dynasm!(ops ; .arch aarch64 ; add x18, x29, #at);
        }
        GroupBase::Edge(by) => {
            let by = u32::from(by);
            dynasm!(ops ; .arch aarch64 ; adr x18, =>edge ; sub x18, x18, #by);
        }
    }
    let step = i32::from(w.step);
    let magnitude = step.unsigned_abs();
    if let WalkMove::Index {
        reg,
        extend,
        shift,
        start,
        step_in,
        ..
    } = w.moves
    {
        // The base moves back by the index's first value, so the first access is at the chosen place.
        set(
            ops,
            reg,
            if extend == 2 {
                start as u32 as i32 as u64
            } else {
                start
            },
        );
        match (extend, shift) {
            (0, false) => dynasm!(ops ; .arch aarch64 ; sub x18, x18, W(reg), uxtw),
            (0, true) => dynasm!(ops ; .arch aarch64 ; sub x18, x18, W(reg), uxtw #3),
            (1, false) => dynasm!(ops ; .arch aarch64 ; sub x18, x18, W(reg), sxtw),
            (1, true) => dynasm!(ops ; .arch aarch64 ; sub x18, x18, W(reg), sxtw #3),
            (_, false) => dynasm!(ops ; .arch aarch64 ; sub x18, x18, X(reg)),
            (_, true) => dynasm!(ops ; .arch aarch64 ; sub x18, x18, X(reg), lsl #3),
        }
        if let Some(r) = step_in {
            set(ops, r, i64::from(step) as u64);
        }
    }
    // The counter's first value, and the bound for the ordered forms.
    let turns = u32::from(w.turns);
    let by = u32::from(w.by);
    let (first, bound) = match w.counter {
        0 | 4 => (turns, 0),
        1 => (0, turns),
        2 => (3, 3 + turns * by),
        _ => (5 + turns * by, 5),
    };
    if w.wide {
        dynasm!(ops ; .arch aarch64 ; movz x16, #first ; =>top);
    } else {
        dynasm!(ops ; .arch aarch64 ; movz w16, #first ; =>top);
    }
    let index_step = |ops: &mut Ops| {
        if let WalkMove::Index {
            reg, wide, step_in, ..
        } = w.moves
        {
            match (step_in, wide) {
                (Some(r), true) => dynasm!(ops ; .arch aarch64 ; add X(reg), X(reg), X(r)),
                (Some(r), false) => dynasm!(ops ; .arch aarch64 ; add W(reg), W(reg), W(r)),
                (None, true) if step < 0 => {
                    dynasm!(ops ; .arch aarch64 ; sub XSP(reg), XSP(reg), #magnitude)
                }
                (None, true) => dynasm!(ops ; .arch aarch64 ; add XSP(reg), XSP(reg), #magnitude),
                (None, false) if step < 0 => {
                    dynasm!(ops ; .arch aarch64 ; sub WSP(reg), WSP(reg), #magnitude)
                }
                (None, false) => dynasm!(ops ; .arch aarch64 ; add WSP(reg), WSP(reg), #magnitude),
            }
        }
    };
    // A loaded value must not replace the index or the step.
    let rt = match w.moves {
        WalkMove::Index { reg, step_in, .. } if w.load => *REGISTERS
            .iter()
            .cycle()
            .skip_while(|&&r| r != w.rt)
            .find(|&&r| r != reg && Some(r) != step_in)
            .unwrap(),
        _ => w.rt,
    };
    match w.moves {
        WalkMove::Base(indexing) => {
            let access = GroupAccess {
                load: w.load,
                bytes: w.bytes,
                vector: w.vector,
                rt,
                rt2: (rt + 1) % 32,
                indexing,
                offset: if matches!(indexing, Indexing::Offset) {
                    0
                } else {
                    i16::from(w.step)
                },
            };
            emit_group_access(ops, 18, &access);
            if let Indexing::Offset = indexing {
                if step < 0 {
                    dynasm!(ops ; .arch aarch64 ; sub x18, x18, #magnitude);
                } else {
                    dynasm!(ops ; .arch aarch64 ; add x18, x18, #magnitude);
                }
            }
        }
        WalkMove::Index {
            reg,
            extend,
            shift,
            first,
            ..
        } => {
            if first {
                index_step(ops);
            }
            if w.vector {
                // ldr or str of a Q register, the index extended as `extend` says, not shifted.
                let option = [0b010, 0b110, 0b011][usize::from(extend.min(2))];
                let word =
                    0x3ca0_0800 | u32::from(w.load) << 22 | u32::from(reg) << 16 | option << 13;
                dynasm!(ops ; .arch aarch64 ; .u32 word | 18 << 5 | u32::from(rt));
            } else {
                match (w.bytes, w.load, extend, shift) {
                    (1, true, 0, _) => {
                        dynasm!(ops ; .arch aarch64 ; ldrb W(rt), [x18, W(reg), uxtw])
                    }
                    (1, true, 1, _) => {
                        dynasm!(ops ; .arch aarch64 ; ldrb W(rt), [x18, W(reg), sxtw])
                    }
                    (1, true, _, _) => dynasm!(ops ; .arch aarch64 ; ldrb W(rt), [x18, X(reg)]),
                    (1, false, 0, _) => {
                        dynasm!(ops ; .arch aarch64 ; strb W(rt), [x18, W(reg), uxtw])
                    }
                    (1, false, 1, _) => {
                        dynasm!(ops ; .arch aarch64 ; strb W(rt), [x18, W(reg), sxtw])
                    }
                    (1, false, _, _) => dynasm!(ops ; .arch aarch64 ; strb W(rt), [x18, X(reg)]),
                    (_, true, 0, false) => {
                        dynasm!(ops ; .arch aarch64 ; ldr X(rt), [x18, W(reg), uxtw])
                    }
                    (_, true, 0, true) => {
                        dynasm!(ops ; .arch aarch64 ; ldr X(rt), [x18, W(reg), uxtw #3])
                    }
                    (_, true, 1, false) => {
                        dynasm!(ops ; .arch aarch64 ; ldr X(rt), [x18, W(reg), sxtw])
                    }
                    (_, true, 1, true) => {
                        dynasm!(ops ; .arch aarch64 ; ldr X(rt), [x18, W(reg), sxtw #3])
                    }
                    (_, true, _, false) => dynasm!(ops ; .arch aarch64 ; ldr X(rt), [x18, X(reg)]),
                    (_, true, _, true) => {
                        dynasm!(ops ; .arch aarch64 ; ldr X(rt), [x18, X(reg), lsl #3])
                    }
                    (_, false, 0, false) => {
                        dynasm!(ops ; .arch aarch64 ; str X(rt), [x18, W(reg), uxtw])
                    }
                    (_, false, 0, true) => {
                        dynasm!(ops ; .arch aarch64 ; str X(rt), [x18, W(reg), uxtw #3])
                    }
                    (_, false, 1, false) => {
                        dynasm!(ops ; .arch aarch64 ; str X(rt), [x18, W(reg), sxtw])
                    }
                    (_, false, 1, true) => {
                        dynasm!(ops ; .arch aarch64 ; str X(rt), [x18, W(reg), sxtw #3])
                    }
                    (_, false, _, false) => dynasm!(ops ; .arch aarch64 ; str X(rt), [x18, X(reg)]),
                    (_, false, _, true) => {
                        dynasm!(ops ; .arch aarch64 ; str X(rt), [x18, X(reg), lsl #3])
                    }
                }
            }
            if !first {
                index_step(ops);
            }
        }
    }
    if w.syscall {
        emit_syscall(ops);
    }
    if let Some(callee) = w.call {
        let target = layout.target(callee);
        dynasm!(ops ; .arch aarch64 ; bl =>target);
    }
    // The counter: changed, compared and branched on in the form chosen.
    let wide = w.wide;
    let change = |ops: &mut Ops| match (w.counter, wide) {
        (0, true) => dynasm!(ops ; .arch aarch64 ; subs x16, x16, #1),
        (0, false) => dynasm!(ops ; .arch aarch64 ; subs w16, w16, #1),
        (1, true) => dynasm!(ops ; .arch aarch64 ; add x16, x16, #1),
        (1, false) => dynasm!(ops ; .arch aarch64 ; add w16, w16, #1),
        (2, true) => dynasm!(ops ; .arch aarch64 ; add x16, x16, #by),
        (2, false) => dynasm!(ops ; .arch aarch64 ; add w16, w16, #by),
        (3, true) => dynasm!(ops ; .arch aarch64 ; sub x16, x16, #by),
        (3, false) => dynasm!(ops ; .arch aarch64 ; sub w16, w16, #by),
        (_, true) => dynasm!(ops ; .arch aarch64 ; sub x16, x16, #1),
        (_, false) => dynasm!(ops ; .arch aarch64 ; sub w16, w16, #1),
    };
    let compare = |ops: &mut Ops| {
        if wide {
            dynasm!(ops ; .arch aarch64 ; cmp x16, #bound);
        } else {
            dynasm!(ops ; .arch aarch64 ; cmp w16, #bound);
        }
    };
    match w.counter {
        0 => {
            change(ops);
            dynasm!(ops ; .arch aarch64 ; b.ne =>top);
        }
        1 => {
            change(ops);
            compare(ops);
            dynasm!(ops ; .arch aarch64 ; b.ne =>top);
        }
        2 | 3 => {
            if w.compare_first {
                compare(ops);
                change(ops);
            } else {
                change(ops);
                compare(ops);
            }
            // lo, ls, lt, le going up; hi, hs, gt, ge going down.
            let cond =
                [3, 9, 11, 13, 8, 2, 12, 10][usize::from(w.cond) + 4 * usize::from(w.counter == 3)];
            branch_if(ops, cond, top);
        }
        _ => {
            change(ops);
            if wide {
                dynasm!(ops ; .arch aarch64 ; cbnz x16, =>top);
            } else {
                dynasm!(ops ; .arch aarch64 ; cbnz w16, =>top);
            }
        }
    }
}

fn emit_lookup(ops: &mut Ops, layout: &mut Layout, l: &Lookup) {
    let (base, index, rt, tab) = (l.base, l.index, l.rt, l.table);
    match l.at {
        GroupBase::Area(at) => {
            let at = u32::from(at);
            dynasm!(ops ; .arch aarch64 ; add XSP(base), x29, #at);
        }
        GroupBase::Edge(by) => {
            let by = u32::from(by);
            dynasm!(ops ; .arch aarch64 ; adr X(base), =>layout.edge ; sub XSP(base), XSP(base), #by);
        }
    }
    let top = ops.new_dynamic_label();
    let skip = ops.new_dynamic_label();
    let table = ops.new_dynamic_label();
    let turns = u32::from(l.turns);
    let start = u32::from(l.start);
    dynasm!(ops ; .arch aarch64 ; adr X(tab), =>table ; movz W(index), #start ; movz x16, #turns ; =>top);
    let top_at = ops.offset().0;
    if l.loaded {
        dynasm!(ops ; .arch aarch64 ; add x18, XSP(tab), #25 ; ldrb W(index), [x18, x16]);
    } else {
        dynasm!(ops ; .arch aarch64 ; eor W(index), W(index), w16);
    }
    if let Some((bound, inclusive, wide)) = l.guard {
        let bound = u32::from(bound);
        if wide {
            dynasm!(ops ; .arch aarch64 ; cmp XSP(index), #bound);
        } else {
            dynasm!(ops ; .arch aarch64 ; cmp WSP(index), #bound);
        }
        branch_if(ops, if inclusive { 8 } else { 2 }, skip);
    }
    match (l.bytes, l.load, l.extend, l.shift) {
        (1, true, 0, _) => dynasm!(ops ; .arch aarch64 ; ldrb W(rt), [X(base), W(index), uxtw]),
        (1, true, 1, _) => dynasm!(ops ; .arch aarch64 ; ldrb W(rt), [X(base), W(index), sxtw]),
        (1, true, _, _) => dynasm!(ops ; .arch aarch64 ; ldrb W(rt), [X(base), X(index)]),
        (1, false, 0, _) => dynasm!(ops ; .arch aarch64 ; strb W(rt), [X(base), W(index), uxtw]),
        (1, false, 1, _) => dynasm!(ops ; .arch aarch64 ; strb W(rt), [X(base), W(index), sxtw]),
        (1, false, _, _) => dynasm!(ops ; .arch aarch64 ; strb W(rt), [X(base), X(index)]),
        (_, true, 0, false) => dynasm!(ops ; .arch aarch64 ; ldr X(rt), [X(base), W(index), uxtw]),
        (_, true, 0, true) => {
            dynasm!(ops ; .arch aarch64 ; ldr X(rt), [X(base), W(index), uxtw #3])
        }
        (_, true, 1, false) => dynasm!(ops ; .arch aarch64 ; ldr X(rt), [X(base), W(index), sxtw]),
        (_, true, 1, true) => {
            dynasm!(ops ; .arch aarch64 ; ldr X(rt), [X(base), W(index), sxtw #3])
        }
        (_, true, _, false) => dynasm!(ops ; .arch aarch64 ; ldr X(rt), [X(base), X(index)]),
        (_, true, _, true) => dynasm!(ops ; .arch aarch64 ; ldr X(rt), [X(base), X(index), lsl #3]),
        (_, false, 0, false) => dynasm!(ops ; .arch aarch64 ; str X(rt), [X(base), W(index), uxtw]),
        (_, false, 0, true) => {
            dynasm!(ops ; .arch aarch64 ; str X(rt), [X(base), W(index), uxtw #3])
        }
        (_, false, 1, false) => dynasm!(ops ; .arch aarch64 ; str X(rt), [X(base), W(index), sxtw]),
        (_, false, 1, true) => {
            dynasm!(ops ; .arch aarch64 ; str X(rt), [X(base), W(index), sxtw #3])
        }
        (_, false, _, false) => dynasm!(ops ; .arch aarch64 ; str X(rt), [X(base), X(index)]),
        (_, false, _, true) => {
            dynasm!(ops ; .arch aarch64 ; str X(rt), [X(base), X(index), lsl #3])
        }
    }
    if l.switch {
        // Two cases chosen by the turn's choice, through the table's first two bytes.
        let cases = ops.new_dynamic_label();
        let (one, two) = (ops.new_dynamic_label(), ops.new_dynamic_label());
        dynasm!(ops ; .arch aarch64
            ; add x18, XSP(tab), #1
            ; ldrb w18, [x18, x16]
            ; cmp w18, #1
            ; b.hi =>skip
            ; ldrb w18, [X(tab), x18]
            ; adr X(index), =>cases
            ; add X(index), X(index), x18, lsl #2
            ; br X(index)
            ; =>cases
            ; b =>one
            ; b =>two
            ; =>one
            ; add XSP(rt), XSP(rt), #1
            ; b =>skip
            ; =>two
            ; eor X(rt), X(rt), x16
        );
    }
    if l.back {
        // Entry 0 is the head, entry 1 the way out, chosen by whether the count has reached 0.
        let (back, out) = (ops.new_dynamic_label(), ops.new_dynamic_label());
        dynasm!(ops ; .arch aarch64
            ; =>skip
            ; subs x16, x16, #1
            ; cset w18, eq
            ; cmp w18, #1
            ; b.hi =>out
            ; adr X(index), =>back
            ; ldrb w18, [X(index), x18]
            ; adr X(index), =>top
            ; add X(index), X(index), x18, lsl #2
            ; br X(index)
            ; =>out
        );
        let entry = u8::try_from((ops.offset().0 - top_at) / 4).expect("a short loop");
        layout.tables.push((back, vec![0, entry]));
    } else {
        dynasm!(ops ; .arch aarch64 ; =>skip ; subs x16, x16, #1 ; b.ne =>top);
    }
    // Turn t (counting down from `turns` to 1) reads byte t + 1 for its choice and t + 25 for its index.
    let mut bytes = vec![1, 0];
    bytes.extend(&l.choices);
    bytes.extend(&l.values);
    layout.tables.push((table, bytes));
}

/// Move sp down (or back up) by a frame size: a multiple of 4096, or less than 4096.
fn stack_pointer(ops: &mut Ops, bytes: u32, down: bool) {
    match (bytes >= 4096, down) {
        (true, true) => dynasm!(ops ; .arch aarch64 ; sub sp, sp, #bytes >> 12, lsl #12),
        (true, false) => dynasm!(ops ; .arch aarch64 ; add sp, sp, #bytes >> 12, lsl #12),
        (false, true) => dynasm!(ops ; .arch aarch64 ; sub sp, sp, #bytes),
        (false, false) => dynasm!(ops ; .arch aarch64 ; add sp, sp, #bytes),
    }
}

/// Assemble a program into a flat blob that starts at its first byte.
pub fn assemble(program: &Program) -> Blob {
    let mut ops: Ops = VecAssembler::new(0);
    let mut layout = Layout {
        functions: program
            .functions
            .iter()
            .map(|_| ops.new_dynamic_label())
            .collect(),
        writer: ops.new_dynamic_label(),
        edge: ops.new_dynamic_label(),
        tables: Vec::new(),
    };
    let area_label = ops.new_dynamic_label();
    // The area is in the blob, so it is no function's frame and its pages may change under any access; main
    // still makes a frame of that size for its accesses through sp. The registers start from the program's
    // values.
    let area = AREA_BYTES as u32;
    dynasm!(ops ; .arch aarch64 ; sub sp, sp, #area ; adr x29, =>area_label);
    for (&r, &value) in REGISTERS.iter().zip(&program.registers) {
        for hw in 0..4u32 {
            let part = (value >> (16 * hw)) as u32 & 0xffff;
            let shift = hw * 16;
            if hw == 0 {
                dynasm!(ops ; .arch aarch64 ; movz X(r), #part);
            } else {
                dynasm!(ops ; .arch aarch64 ; movk X(r), #part, lsl #shift);
            }
        }
    }
    emit_chunks(&mut ops, &mut layout, &program.main);
    // Store the registers and the flags after the area, write area and registers to stdout, exit 0.
    for (i, &r) in REGISTERS.iter().enumerate() {
        let at = AREA + 8 * i as u32;
        dynasm!(ops ; .arch aarch64 ; str X(r), [x29, #at]);
    }
    let flags_at = AREA + 8 * REGISTERS.len() as u32;
    dynasm!(ops ; .arch aarch64
        ; .u32 0xd53b_4210
        ; str x16, [x29, #flags_at]
        ; movz x0, #1
        ; mov x1, x29
        ; movz x2, #OUTPUT
        ; movz x8, #64
        ; svc #0
        ; movz x0, #0
        ; movz x8, #94
        ; svc #0
    );
    for (i, function) in program.functions.iter().enumerate() {
        let label = layout.functions[i];
        let again = ops.new_dynamic_label();
        let done = ops.new_dynamic_label();
        let recursion = u32::from(function.recursion);
        dynasm!(ops ; .arch aarch64
            ; =>label
            ; movz x17, #recursion
            ; =>again
        );
        // Each call, recursive ones too, makes its own frame with x30, x28, x16 and x18 at the bottom.
        stack_pointer(&mut ops, function.frame, true);
        dynasm!(ops ; .arch aarch64 ; stp x30, x28, [sp] ; stp x16, x18, [sp, #16]);
        emit_chunks(&mut ops, &mut layout, &function.body);
        // Bounded recursion: call itself again while x17 counts down.
        dynasm!(ops ; .arch aarch64
            ; cbz x17, =>done
            ; sub x17, x17, #1
            ; bl =>again
            ; =>done
            ; ldp x30, x28, [sp]
            ; ldp x16, x18, [sp, #16]
        );
        stack_pointer(&mut ops, function.frame, false);
        match function.tail {
            Some(t) => {
                let target = layout.functions[t];
                dynasm!(ops ; .arch aarch64 ; b =>target);
            }
            None => dynasm!(ops ; .arch aarch64 ; ret),
        }
    }
    let writer = layout.writer;
    dynasm!(ops ; .arch aarch64 ; =>writer);
    emit_syscall(&mut ops);
    dynasm!(ops ; .arch aarch64 ; ret);
    let code_bytes = ops.offset().0 as u64;
    for (label, entries) in &layout.tables {
        dynasm!(ops ; .arch aarch64 ; =>*label);
        for &e in entries {
            dynasm!(ops ; .arch aarch64 ; .u8 e as _);
        }
    }
    // Zeros up to where the area ends on a page boundary, then a page of zeros, so accesses near the edge reach
    // no code, table or area.
    let align = (4096 - (ops.offset().0 + AREA_BYTES) % 4096) % 4096;
    ops.extend(std::iter::repeat_n(0u8, align));
    dynasm!(ops ; .arch aarch64 ; =>area_label);
    ops.extend(std::iter::repeat_n(0u8, AREA_BYTES + 4096));
    let edge = layout.edge;
    dynasm!(ops ; .arch aarch64 ; =>edge);
    let bytes = ops.finalize().expect("labels resolve");
    Blob {
        name: "layer2".into(),
        bytes,
        code_bytes,
        load: crate::programs::LOAD,
        program_headers: None,
        stack_bytes: crate::programs::STACK_BYTES,
    }
}
