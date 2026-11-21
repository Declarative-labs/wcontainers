//! ARM64's layer 2 code: the model's programs assembled with dynasm. x16 counts loops, x17 recursion, x18
//! holds a moving base or a jump target, x28 the base of a group across a call, x29 the area.

// dynasm's expansion converts register numbers to their own type.
#![allow(clippy::useless_conversion)]

use crate::guest::GuestTest;
use crate::layer2::*;
use crate::programs::Blob;
use dynasmrt::aarch64::Aarch64Relocation;
use dynasmrt::{DynamicLabel, DynasmApi, DynasmLabelApi, VecAssembler, dynasm};
use wcontainers_arm64::Arm64;

const REGISTERS: &[u8] = <Arm64 as GuestTest>::MODEL.registers;
const OUTPUT: u32 = output::<Arm64>();
const AREA_BYTES: usize = area_bytes::<Arm64>();

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
pub(super) fn assemble(program: &Program) -> Blob {
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
    let page = wcontainers_abi::PAGE_SIZE as usize;
    let align = (page - (ops.offset().0 + AREA_BYTES) % page) % page;
    ops.extend(std::iter::repeat_n(0u8, align));
    dynasm!(ops ; .arch aarch64 ; =>area_label);
    ops.extend(std::iter::repeat_n(0u8, AREA_BYTES + page));
    let edge = layout.edge;
    dynasm!(ops ; .arch aarch64 ; =>edge);
    let bytes = ops.finalize().expect("labels resolve");
    Blob {
        name: BLOB_NAME.into(),
        bytes,
        code_bytes,
        load: crate::programs::LOAD,
        link_address: None,
        main: None,
        program_headers: None,
        stack_bytes: crate::programs::STACK_BYTES,
    }
}
