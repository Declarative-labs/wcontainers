//! Compiler habits the translator recognises. A habit only says what to expect: code built on it checks at run
//! time that the guess holds, so a wrong guess costs speed, never correctness.

use crate::instruction::*;

/// Where a jump through a register made from a switch statement can go: a bound check (`cmp` with an
/// immediate, then `b.hi` or `b.hs`), a table of 1, 2 or 4 byte entries loaded from a known address, and a
/// known base plus the shifted entry. `before` is the code ahead of the jump in address order, each with its
/// blob offset; `read(offset, bytes)` reads the blob, little endian. Returns the targets as blob offsets, each
/// once, in table order. Addresses built with `adrp` assume the blob is loaded at a multiple of 4096.
pub fn jump_table(
    before: &[(u64, Instruction)],
    rn: Reg,
    read: impl Fn(u64, u8) -> Option<u64>,
) -> Option<Vec<u64>> {
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
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    Some(targets)
}

/// The index of the last instruction before `end` that writes `r`.
fn last_def(before: &[(u64, Instruction)], r: Reg, end: usize) -> Option<usize> {
    (0..end).rev().find(|&i| before[i].1.effects().defs.has(r))
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
        } => Some((at & !0xfff).wrapping_add_signed(offset)),
        Instruction::AddSub {
            width: Width::X64,
            subtract: false,
            set_flags: false,
            rn,
            operand: Operand::Imm(value),
            ..
        } => Some(constant(before, rn, i)?.wrapping_add(value)),
        _ => None,
    }
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
        i.effects().defs.intersect(crate::Registers::FLAGS) != crate::Registers::NONE
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
