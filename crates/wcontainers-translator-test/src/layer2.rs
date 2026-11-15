//! Layer 2: random small ARM64 programs built to always finish, assembled with dynasm. Each has loops with
//! counters, calls, bounded recursion, tail calls, calls and jumps through registers, compiler-style jump
//! tables, flags carried across instructions and memory use inside a fixed area. At the end it stores its
//! registers and flags after the area and writes the whole area out.

// dynasm's expansion converts register numbers to their own type.
#![allow(clippy::useless_conversion)]

use crate::programs::Blob;
use dynasmrt::aarch64::Aarch64Relocation;
use dynasmrt::{DynamicLabel, DynasmApi, DynasmLabelApi, VecAssembler, dynasm};
use proptest::prelude::*;

/// Bytes of the area random loads and stores use, from x29.
const AREA: u32 = 512;
/// Bytes written out at the end: the area, then x0 to x15 and NZCV.
pub const OUTPUT: u32 = AREA + 17 * 8;

/// One straight-line instruction on x0 to x15.
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
}

#[derive(Clone, Debug)]
pub enum Chunk {
    Ops(Vec<Op>),
    /// Skip the ops when a condition holds on the flags as they are.
    Skip {
        cond: u8,
        ops: Vec<Op>,
    },
    /// Run the ops `count` times, counting in x16.
    Loop {
        count: u8,
        ops: Vec<Op>,
    },
    /// Call a function by number, directly or through x18.
    Call {
        function: usize,
        through_register: bool,
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

/// A function may call only functions with lower numbers, so calls always end.
#[derive(Clone, Debug)]
pub struct Function {
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
    0u8..16
}

fn op() -> impl Strategy<Value = Op> {
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
    ]
}

fn ops() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(op(), 0..8)
}

/// A chunk that may call functions numbered below `callable`.
fn chunk(callable: usize) -> BoxedStrategy<Chunk> {
    let mut choices = vec![
        ops().prop_map(Chunk::Ops).boxed(),
        (0u8..14, ops())
            .prop_map(|(cond, ops)| Chunk::Skip { cond, ops })
            .boxed(),
        (1u8..6, ops())
            .prop_map(|(count, ops)| Chunk::Loop { count, ops })
            .boxed(),
        (register(), prop::collection::vec(ops(), 1..6))
            .prop_map(|(index, cases)| Chunk::Switch { index, cases })
            .boxed(),
        (any::<bool>(), ops())
            .prop_map(|(through_register, ops)| Chunk::Jump {
                through_register,
                ops,
            })
            .boxed(),
    ];
    if callable > 0 {
        choices.push(
            (0..callable, any::<bool>())
                .prop_map(|(function, through_register)| Chunk::Call {
                    function,
                    through_register,
                })
                .boxed(),
        );
    }
    prop::strategy::Union::new(choices).boxed()
}

pub fn program() -> impl Strategy<Value = Program> {
    (0usize..5).prop_flat_map(|count| {
        let functions: Vec<BoxedStrategy<Function>> = (0..count)
            .map(|i| {
                (
                    prop::collection::vec(chunk(i), 0..4),
                    0u8..4,
                    prop::option::of(0..i.max(1)),
                )
                    .prop_map(move |(body, recursion, tail)| Function {
                        body,
                        recursion,
                        tail: tail.filter(|&t| t < i),
                    })
                    .boxed()
            })
            .collect();
        (
            prop::collection::vec(any::<u64>(), 16),
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

fn emit_op(ops: &mut Ops, op: &Op) {
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
    tables: Vec<(DynamicLabel, Vec<u8>)>,
}

fn emit_chunks(ops: &mut Ops, layout: &mut Layout, chunks: &[Chunk]) {
    for chunk in chunks {
        match chunk {
            Chunk::Ops(list) => list.iter().for_each(|op| emit_op(ops, op)),
            Chunk::Skip { cond, ops: list } => {
                let after = ops.new_dynamic_label();
                branch_if(ops, *cond, after);
                list.iter().for_each(|op| emit_op(ops, op));
                dynasm!(ops ; .arch aarch64 ; =>after);
            }
            Chunk::Loop { count, ops: list } => {
                let top = ops.new_dynamic_label();
                let count = u32::from(*count);
                dynasm!(ops ; .arch aarch64 ; movz x16, #count ; =>top);
                list.iter().for_each(|op| emit_op(ops, op));
                dynasm!(ops ; .arch aarch64 ; subs x16, x16, #1 ; b.ne =>top);
            }
            Chunk::Call {
                function,
                through_register,
            } => {
                let target = layout.functions[*function];
                if *through_register {
                    dynasm!(ops ; .arch aarch64 ; adr x18, =>target ; blr x18);
                } else {
                    dynasm!(ops ; .arch aarch64 ; bl =>target);
                }
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
                    case.iter().for_each(|op| emit_op(ops, op));
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
                list.iter().for_each(|op| emit_op(ops, op));
                dynasm!(ops ; .arch aarch64 ; =>after);
            }
        }
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
        tables: Vec::new(),
    };
    let area = (AREA + 17 * 8 + 15) & !15;
    // The area sits on the stack; x0 to x15 start from the program's values.
    dynasm!(ops ; .arch aarch64 ; sub sp, sp, #area ; mov x29, sp);
    for (r, &value) in program.registers.iter().enumerate() {
        let r = r as u8;
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
    // Store x0 to x15 and the flags after the area, write area and registers to stdout, exit 0.
    for r in 0..16u8 {
        let at = AREA + 8 * u32::from(r);
        dynasm!(ops ; .arch aarch64 ; str X(r), [x29, #at]);
    }
    let flags_at = AREA + 16 * 8;
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
            ; str x30, [sp, #-16]!
        );
        emit_chunks(&mut ops, &mut layout, &function.body);
        // Bounded recursion: call itself again while x17 counts down.
        dynasm!(ops ; .arch aarch64
            ; cbz x17, =>done
            ; sub x17, x17, #1
            ; bl =>again
            ; =>done
            ; ldr x30, [sp], #16
        );
        match function.tail {
            Some(t) => {
                let target = layout.functions[t];
                dynasm!(ops ; .arch aarch64 ; b =>target);
            }
            None => dynasm!(ops ; .arch aarch64 ; ret),
        }
    }
    let code_bytes = ops.offset().0 as u64;
    for (label, entries) in &layout.tables {
        dynasm!(ops ; .arch aarch64 ; =>*label);
        for &e in entries {
            dynasm!(ops ; .arch aarch64 ; .u8 e as _);
        }
    }
    let bytes = ops.finalize().expect("labels resolve");
    Blob {
        name: "layer2".into(),
        bytes,
        code_bytes,
    }
}
