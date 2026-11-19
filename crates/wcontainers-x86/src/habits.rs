//! What compilers make of switches and loops, recognised for the translator's analyses: switch tables, the
//! comparison a conditional branch tests, and loads whose value is bounded.

use crate::X86_64;
use crate::decode::Instruction;
use crate::flags::{BELOW, BELOW_OR_EQUAL, EQUAL, LESS, LESS_OR_EQUAL};
use crate::guest::ALL_FLAGS;
use crate::operand::{General, general, jcc, memory_bytes};
use iced_x86::{Mnemonic, OpKind, Register};
use wcontainers_guest_cpu::{Comparison, Effects, Relation, Value};

/// How far back a switch's instructions are looked for.
const LOOK_BACK: usize = 64;
/// Bytes of an entry of a position-independent switch table.
const ENTRY: u64 = size_of::<i32>() as u64;

/// A general register of 32 or 64 bits as operand `n`.
fn wide_register(i: &Instruction, n: u32) -> Option<General> {
    (i.iced.op_kind(n) == OpKind::Register)
        .then(|| general(i.iced.op_register(n)))
        .flatten()
        .filter(|g| g.width >= u32::BITS as u8 && !g.high)
}

/// Whether the instruction writes general register `r`: its first operand, or a register it changes besides.
fn writes(i: &Instruction, r: u8) -> bool {
    let ins = &i.iced;
    let first = ins.op_count() > 0
        && ins.op_kind(0) == OpKind::Register
        && general(ins.op_register(0)).is_some_and(|g| g.number == r)
        && !matches!(ins.mnemonic(), Mnemonic::Cmp | Mnemonic::Test);
    let both = ins.mnemonic() == Mnemonic::Xchg
        && ins.op_kind(1) == OpKind::Register
        && general(ins.op_register(1)).is_some_and(|g| g.number == r);
    // Anything else that changes registers it does not name (calls, string instructions, divisions, ...)
    // ends the search: it may have changed `r`.
    let implicit = crate::meaning::is_string(i)
        || matches!(
            ins.mnemonic(),
            Mnemonic::Call
                | Mnemonic::Div
                | Mnemonic::Idiv
                | Mnemonic::Mul
                | Mnemonic::Syscall
                | Mnemonic::Cqo
                | Mnemonic::Cdq
                | Mnemonic::Cwd
        );
    first || both || implicit
}

/// The source of `mov r32, s32`, which zero-extends `s32` into `r`.
fn zero_extended(i: &Instruction) -> Option<General> {
    let word = u32::BITS as u8;
    (i.iced.mnemonic() == Mnemonic::Mov && wide_register(i, 0)?.width == word)
        .then(|| wide_register(i, 1).filter(|s| s.width == word))
        .flatten()
}

/// The blob offset `lea r, [rip + d]` at `at` builds.
fn rip_relative(at: u64, i: &Instruction) -> Option<u64> {
    (i.iced.mnemonic() == Mnemonic::Lea && i.iced.memory_base() == Register::RIP)
        .then(|| at.wrapping_add(i.iced.memory_displacement64()))
}

/// See `Guest::jump_table`.
pub fn jump_table(
    at: u64,
    reg: u8,
    blob: &[u8],
    before: impl Fn(u64) -> Option<(u64, Instruction)>,
) -> Option<Vec<u64>> {
    let mut walk = Vec::new();
    let mut o = at;
    while walk.len() < LOOK_BACK {
        let Some((p, i)) = before(o) else { break };
        walk.push((p, i));
        o = p;
    }
    // The sum: `add reg, other`.
    let (_, add) = walk.first()?;
    if add.iced.mnemonic() != Mnemonic::Add
        || wide_register(add, 0)?.number != reg
        || wide_register(add, 0)?.width != u64::BITS as u8
    {
        return None;
    }
    let other = wide_register(add, 1)?.number;
    let writer = |r: u8| {
        walk[1..]
            .iter()
            .position(|(_, i)| writes(i, r))
            .map(|k| k + 1)
    };
    let (first, second) = (writer(reg)?, writer(other)?);
    // One is the entry's load, the other the table's address, built before the load.
    let entry = |k: usize| {
        let (_, i) = &walk[k];
        let ins = &i.iced;
        (ins.mnemonic() == Mnemonic::Movsxd
            && ins.op_kind(1) == OpKind::Memory
            && u64::from(memory_bytes(i)) == ENTRY
            && u64::from(ins.memory_index_scale()) == ENTRY
            && ins.memory_displacement64() == 0)
            .then(|| Some((general(ins.memory_base())?, general(ins.memory_index())?)))
            .flatten()
    };
    let (load, table_at) = match (entry(first), entry(second)) {
        (Some(_), None) => (first, second),
        (None, Some(_)) => (second, first),
        _ => return None,
    };
    let (base, index) = entry(load)?;
    let (lea_at, lea) = &walk[table_at];
    if table_at < load || wide_register(lea, 0)?.number != base.number {
        return None;
    }
    let table = rip_relative(*lea_at, lea)?;
    // The guard: `cmp index, k` and `ja` (k + 1 cases) or `jae` (k), with the index unchanged since, or
    // only zero-extended into the register the load reads (`mov r32, index32`, both compilers' form for a
    // 32-bit index).
    let mut index = index;
    let mut count = None;
    for (k, (_, i)) in walk.iter().enumerate().skip(load + 1) {
        let ins = &i.iced;
        if let Some(cond) = jcc(ins.mnemonic()) {
            // Taken above the bound: `ja` (not below or equal) or `jae` (not below).
            let inclusive = match cond {
                c if c == BELOW_OR_EQUAL | 1 => true,
                c if c == BELOW | 1 => false,
                _ => return None,
            };
            let (_, compare) = walk.get(k + 1)?;
            let c = &compare.iced;
            if c.mnemonic() != Mnemonic::Cmp
                || wide_register(compare, 0)?.number != index.number
                || matches!(c.op_kind(1), OpKind::Register | OpKind::Memory)
            {
                return None;
            }
            let bound = c.immediate(1);
            count = Some(if inclusive {
                bound.checked_add(1)?
            } else {
                bound
            });
            break;
        }
        if writes(i, index.number) {
            index = zero_extended(i)?;
        }
    }
    let count = count.filter(|&n| n > 0)?;
    let end = table.checked_add(count.checked_mul(ENTRY)?)?;
    if end > blob.len() as u64 {
        return None;
    }
    let mut targets = Vec::new();
    for k in 0..count {
        let at = (table + k * ENTRY) as usize;
        let bytes = blob[at..at + ENTRY as usize].try_into().ok()?;
        let target = table.wrapping_add_signed(i64::from(i32::from_le_bytes(bytes)));
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    Some(targets)
}

/// See `Guest::branch_comparison`.
pub fn branch_comparison(
    path: &[(u64, Instruction, Effects<X86_64>)],
    branch: &Instruction,
) -> Option<Comparison<u8>> {
    // The even condition of each pair holds as the relation, the odd one is its negation.
    let cond = jcc(branch.iced.mnemonic())?;
    let (relation, signed) = match cond & !1 {
        EQUAL => (Relation::Eq, false),
        BELOW => (Relation::Below { inclusive: false }, false),
        BELOW_OR_EQUAL => (Relation::Below { inclusive: true }, false),
        LESS => (Relation::Below { inclusive: false }, true),
        LESS_OR_EQUAL => (Relation::Below { inclusive: true }, true),
        _ => return None,
    };
    let relation = if cond & 1 == 1 {
        relation.negated()
    } else {
        relation
    };
    let &(at, setter, _) = path
        .iter()
        .rev()
        .find(|(_, _, effects)| !effects.defs.intersect(ALL_FLAGS).is_empty())?;
    let left = wide_register(&setter, 0)?;
    let ins = &setter.iced;
    // An immediate is taken only when it means the same read signed or not, at either width.
    let immediate = || {
        let k = ins.immediate(1) as i64;
        (0..=i64::from(i32::MAX))
            .contains(&k)
            .then_some(Value::Imm(k))
    };
    let (right, relation, signed) = match ins.mnemonic() {
        Mnemonic::Cmp | Mnemonic::Sub => {
            let right = match ins.op_kind(1) {
                OpKind::Register => {
                    let r = wide_register(&setter, 1).filter(|r| r.width == left.width)?;
                    Value::Reg(r.number)
                }
                OpKind::Memory => return None,
                _ => immediate()?,
            };
            (right, relation, signed)
        }
        // CF and OF clear: below or equal is equal, above is not equal, below and above or equal never test.
        Mnemonic::Test if wide_register(&setter, 1) == Some(left) => {
            let (relation, signed) = match (relation, signed) {
                (Relation::Below { inclusive: true }, false) => (Relation::Eq, false),
                (Relation::Above { inclusive: false }, false) => (Relation::Ne, false),
                (Relation::Below { .. } | Relation::Above { .. }, false) => return None,
                other => other,
            };
            (Value::Imm(0), relation, signed)
        }
        // ZF from the register minus 1.
        Mnemonic::Dec if matches!(relation, Relation::Eq | Relation::Ne) => {
            (Value::Imm(1), relation, false)
        }
        _ => return None,
    };
    Some(Comparison {
        left: left.number,
        right,
        relation,
        signed,
        bits: left.width,
        at,
    })
}

/// See `Guest::loaded_max`.
pub fn loaded_max(i: &Instruction) -> Option<(u8, i64)> {
    let ins = &i.iced;
    let r = wide_register(i, 0)?;
    let bytes = memory_bytes(i);
    let zero_extends = match ins.mnemonic() {
        Mnemonic::Movzx => true,
        Mnemonic::Mov => r.width == u32::BITS as u8 && bytes == u32::BITS / u8::BITS,
        _ => false,
    };
    (zero_extends && ins.op_kind(1) == OpKind::Memory)
        .then(|| (r.number, (1i64 << (bytes * u8::BITS)) - 1))
}
