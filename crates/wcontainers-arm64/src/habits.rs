//! Compiler habits the translator recognises. A habit only says what to expect: code built on it checks at run
//! time that the guess holds, so a wrong guess costs speed, never correctness.

use crate::Arm64;
use crate::instruction::*;
use wcontainers_guest_cpu::{Guest, JumpTable};

/// Where a jump through a register made from a switch statement can go: a bound check (`cmp` with an
/// immediate, then `b.hi` or `b.hs`), a table of 1, 2 or 4 byte entries loaded from a known address, and a
/// known base plus the shifted entry. `before` is the code ahead of the jump in address order, each with its
/// blob offset; `read(offset, bytes)` reads the blob, little endian. Returns the targets as blob offsets, each
/// once, in table order. Addresses built with `adrp` assume the blob is loaded at a multiple of 4096.
pub fn jump_table(
    before: &[(u64, Instruction)],
    rn: Reg,
    read: impl Fn(u64, u8) -> Option<u64>,
) -> Option<JumpTable> {
    let add = last_def(before, rn, before.len())?;
    let Instruction::AddSub {
        width: Width::X64,
        subtract: false,
        set_flags: false,
        rn: base_register,
        operand,
        ..
    } = before[add].1
    else {
        return None;
    };
    let (entry, shift, extend) = match operand {
        Operand::Shifted {
            rm,
            shift: Shift::Lsl,
            amount,
        } => (rm, amount, None),
        Operand::Extended { rm, extend, amount } => (rm, amount, Some(extend)),
        _ => return None,
    };
    let base = constant(before, base_register, add)?;
    let load = last_def(before, entry, add)?;
    let Instruction::Load {
        bytes: bytes @ (1 | 2 | 4),
        signed,
        address: Address::Indexed { base: table, .. },
        ..
    } = before[load].1
    else {
        return None;
    };
    let table = constant(before, table, load)?;
    let count = bound(&before[..load])?;
    let mut targets = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for k in 0..count {
        let raw = read(table.checked_add(k * u64::from(bytes))?, bytes)?;
        let bits = 8 * u32::from(bytes);
        let sign_extend =
            |value: u64, from: u32| ((value << (64 - from)) as i64 >> (64 - from)) as u64;
        let mut value = match signed {
            None => raw,
            Some(Width::W32) => sign_extend(raw, bits) & 0xffff_ffff,
            Some(Width::X64) => sign_extend(raw, bits),
        };
        if let Some(extend) = extend {
            let from = u32::from(extend.from_bits);
            value = match (extend.signed, from) {
                (_, 64) => value,
                (true, _) => sign_extend(value, from),
                (false, _) => value & ((1 << from) - 1),
            };
        }
        let target = base.wrapping_add(value << shift);
        if seen.insert(target) {
            targets.push(target);
        }
    }
    Some(JumpTable {
        targets,
        read: table..table + count * u64::from(bytes),
    })
}

/// The index of the last instruction before `end` that writes `r`.
fn last_def(before: &[(u64, Instruction)], r: Reg, end: usize) -> Option<usize> {
    (0..end).rev().find(|&i| {
        Arm64::reg_bit(r).is_some_and(|b| crate::guest::effects(&before[i].1).defs.has_bit(b))
    })
}

/// The blob offset a register holds before `end`, when an `adr`, or an `adrp` and an `add`, put it there.
fn constant(before: &[(u64, Instruction)], r: Reg, end: usize) -> Option<u64> {
    let i = last_def(before, r, end)?;
    let (at, instruction) = before[i];
    match instruction {
        Instruction::PcRelative {
            page: false,
            offset,
            ..
        } => Some(at.wrapping_add_signed(offset)),
        Instruction::PcRelative {
            page: true, offset, ..
        } => Some((at & !crate::ADRP_PAGE_MASK).wrapping_add_signed(offset)),
        Instruction::AddSub {
            width: Width::X64,
            subtract: false,
            set_flags: false,
            rn,
            operand: Operand::Imm(value),
            ..
        } => Some(constant(before, rn, i)?.wrapping_add(value)),
        Instruction::Load {
            bytes,
            signed: None,
            address:
                Address::Offset {
                    base: Reg::Sp,
                    offset,
                },
            ..
        } if usize::from(bytes) == size_of::<u64>() => kept_in_frame(before, offset, i),
        _ => None,
    }
}

/// The blob offset a function keeps in the frame slot at `sp + slot` before `end`: what the nearest store to the
/// slot wrote, with `sp` unchanged since.
fn kept_in_frame(before: &[(u64, Instruction)], slot: i64, end: usize) -> Option<u64> {
    let sp = Arm64::reg_bit(Reg::Sp).expect("sp has a bit");
    let word = size_of::<u64>();
    for i in (0..end).rev() {
        let stored = match before[i].1 {
            Instruction::Store {
                bytes,
                rt,
                address:
                    Address::Offset {
                        base: Reg::Sp,
                        offset,
                    },
            } if usize::from(bytes) == word => [Some((rt, offset)), None],
            Instruction::StorePair {
                bytes,
                rt,
                rt2,
                address:
                    Address::Offset {
                        base: Reg::Sp,
                        offset,
                    },
            } if usize::from(bytes) == word => {
                [Some((rt, offset)), Some((rt2, offset + word as i64))]
            }
            _ => [None, None],
        };
        if let Some((rt, _)) = stored.into_iter().flatten().find(|&(_, at)| at == slot) {
            return constant(before, rt, i);
        }
        if crate::guest::effects(&before[i].1).defs.has_bit(sp) {
            return None;
        }
    }
    None
}

/// How many entries the nearest bound check allows: a flag setting subtraction of an immediate (`cmp` or
/// `subs`) followed by `b.hi` (that many plus one) or `b.hs`.
fn bound(before: &[(u64, Instruction)]) -> Option<u64> {
    let branch = before.iter().rposition(|(_, i)| {
        matches!(
            i,
            Instruction::BranchCond {
                cond: Cond(2 | 8),
                ..
            }
        )
    })?;
    let Instruction::BranchCond { cond, .. } = before[branch].1 else {
        unreachable!()
    };
    let compare = before[..branch].iter().rposition(|(_, i)| {
        !crate::guest::effects(i)
            .defs
            .intersect(crate::guest::FLAGS)
            .is_empty()
    })?;
    match before[compare].1 {
        Instruction::AddSub {
            subtract: true,
            set_flags: true,
            operand: Operand::Imm(n),
            ..
        } => Some(if cond.0 == 8 { n.checked_add(1)? } else { n }),
        _ => None,
    }
}
