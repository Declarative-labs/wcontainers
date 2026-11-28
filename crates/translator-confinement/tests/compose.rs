//! The descriptions of `statement/compose.rs`: what the translator emits is described by its blocks.
//!
//! - [`describe`] finds, word by word from a piece's first, the blocks `compose.rs`'s `described` asks for, with the
//!   same shapes (its `shape_ok`); where it finds none the piece is not covered.
//! - Each pin of that file is, byte for byte, what the translator emits for the guest code this file names, with the
//!   description `describe` finds (the pin's text is made here and compared whole). `compute` then checks there that the
//!   description is one `described` accepts, and `described_is_confined` proves every such piece confined.
//! - Over every form of the instruction list, the pieces of look-ahead none are counted described or not, by form.

use std::collections::{BTreeMap, BTreeSet};

use wcontainers_arm64::way::{Kept, SLOT_BITS};
use wcontainers_arm64::{Arm64, Way, forms};
use wcontainers_translator::{LookAhead, NativeRequest, translate_instructions};
use wcontainers_translator_confinement::{
    Base, Extend, Made, Moved, Offset, Reg, Source, Step, decode,
};

const COMPOSE: &str = include_str!("../statement/compose.rs");
const GUEST: u64 = 0x40_1000;
const SLOT: u16 = 3;
const FLOOR_PAGE: u64 = (1 << 45) + (1 << 32);

fn translated(code: &[u32], look_ahead: LookAhead, displaced: bool) -> Option<Vec<u32>> {
    let request = NativeRequest {
        look_ahead,
        exact_replay: true,
        stop: false,
        step: false,
    };
    let way = Way::InProcess {
        slot: SLOT,
        displaced,
        kept: Kept::USUAL,
    };
    let bytes: Vec<u8> = code.iter().flat_map(|w| w.to_le_bytes()).collect();
    let out = translate_instructions::<Arm64>(&bytes, GUEST, &request, &way, &())?;
    Some(
        out.bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| u32::from_le_bytes(*w))
            .collect(),
    )
}

/// compose.rs `Desc`, its `FrameAccess` inline; 31 for no register.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Desc {
    Register {
        j: i64,
        rd: i64,
        sp: bool,
        bb: i64,
        cut: i64,
    },
    Cut {
        j: i64,
        n: i64,
        c: i64,
        bb: i64,
    },
    Either {
        j: i64,
        by: i64,
        bb: i64,
        cut: i64,
    },
    Branch {
        j: i64,
        by: i64,
        bb: i64,
        cut: i64,
    },
    Through {
        j: i64,
        rt: i64,
        rt2: i64,
        back: bool,
        off: i64,
        bytes: i64,
        bb: i64,
        cut: i64,
    },
    Own {
        j: i64,
        n: i64,
        rt: i64,
        rt2: i64,
        bytes: i64,
        bb: i64,
    },
    Mark {
        j: i64,
        bb: i64,
        c: i64,
    },
    Probes {
        j: i64,
        c: i64,
        from: i64,
        off: i64,
        bytes: i64,
        store: bool,
        bb: i64,
    },
    Exit {
        j: i64,
        guest: i64,
    },
    Jump {
        j: i64,
        n: i64,
        out: i64,
    },
    Call {
        j: i64,
        code: i64,
    },
    Frame {
        j: i64,
        word: u32,
        rt: i64,
        off: i64,
        moves: bool,
        bb: i64,
        bb2: i64,
        cut: i64,
    },
    Stop {
        j: i64,
    },
    StackCut {
        j: i64,
        n: i64,
        bb: i64,
        cut: i64,
    },
    StackMove {
        j: i64,
        bb: i64,
        cut: i64,
    },
    StackStep {
        j: i64,
        k: i64,
        off: i64,
        bb: i64,
        cut: i64,
    },
    StackAccess {
        j: i64,
        rt: i64,
        rt2: i64,
        off: i64,
        moves: i64,
        bytes: i64,
        bb: i64,
    },
    StackVia {
        j: i64,
        rt: i64,
        rt2: i64,
        bytes: i64,
        bb: i64,
        cut: i64,
    },
    StackProbes {
        j: i64,
        c: i64,
        bytes: i64,
        store: bool,
        bb: i64,
    },
    Same {
        j: i64,
        bb: i64,
        cut: i64,
    },
    DisplacedJump {
        j: i64,
        n: i64,
        out: i64,
    },
    StackAt {
        j: i64,
        off: i64,
        rt: i64,
        rt2: i64,
        bytes: i64,
        bb: i64,
        cut: i64,
    },
}

/// compose.rs `needs_cut`.
fn needs_cut(need: i64, cut: i64) -> bool {
    need == 31 || need == cut
}

/// compose.rs `after_cut`.
fn after_cut(cut: i64, r: i64) -> i64 {
    if r == cut { 31 } else { cut }
}

impl Desc {
    /// compose.rs `iface_ok`.
    fn takes(&self, base: i64, cut: i64) -> bool {
        match *self {
            Desc::Register { bb, cut: c, .. }
            | Desc::Either { bb, cut: c, .. }
            | Desc::Branch { bb, cut: c, .. }
            | Desc::Frame { bb, cut: c, .. }
            | Desc::StackCut { bb, cut: c, .. }
            | Desc::StackStep { bb, cut: c, .. }
            | Desc::StackVia { bb, cut: c, .. }
            | Desc::StackAt { bb, cut: c, .. }
            | Desc::Probes { bb, from: c, .. } => bb == base && needs_cut(c, cut),
            Desc::Through { bb, cut: c, .. }
            | Desc::StackMove { bb, cut: c, .. }
            | Desc::Same { bb, cut: c, .. } => bb == base && c == cut,
            Desc::Cut { bb, .. }
            | Desc::Own { bb, .. }
            | Desc::Mark { bb, .. }
            | Desc::StackAccess { bb, .. }
            | Desc::StackProbes { bb, .. } => bb == base,
            Desc::Exit { .. } | Desc::Jump { .. } | Desc::DisplacedJump { .. } => base == 24,
            Desc::Call { .. } | Desc::Stop { .. } => true,
        }
    }

    fn entry(&self) -> i64 {
        match *self {
            Desc::Register { j, .. }
            | Desc::Cut { j, .. }
            | Desc::Either { j, .. }
            | Desc::Branch { j, .. }
            | Desc::Through { j, .. }
            | Desc::Own { j, .. }
            | Desc::Mark { j, .. }
            | Desc::Probes { j, .. }
            | Desc::Exit { j, .. }
            | Desc::Jump { j, .. }
            | Desc::Call { j, .. }
            | Desc::Frame { j, .. }
            | Desc::Stop { j }
            | Desc::StackCut { j, .. }
            | Desc::StackMove { j, .. }
            | Desc::StackStep { j, .. }
            | Desc::StackAccess { j, .. }
            | Desc::StackVia { j, .. }
            | Desc::StackProbes { j, .. }
            | Desc::Same { j, .. }
            | Desc::DisplacedJump { j, .. }
            | Desc::StackAt { j, .. } => j,
        }
    }

    /// The Verus expression.
    fn verus(&self) -> String {
        match *self {
            Desc::Register { j, rd, sp, bb, cut } => {
                format!("Desc::Register {{ j: {j}, rd: {rd}, sp: {sp}, bb: {bb}, cut: {cut} }}")
            }
            Desc::Cut { j, n, c, bb } => {
                format!("Desc::Cut {{ j: {j}, n: {n}, c: {c}, bb: {bb} }}")
            }
            Desc::Either { j, by, bb, cut } => {
                format!("Desc::Either {{ j: {j}, by: {by}, bb: {bb}, cut: {cut} }}")
            }
            Desc::Branch { j, by, bb, cut } => {
                format!("Desc::Branch {{ j: {j}, by: {by}, bb: {bb}, cut: {cut} }}")
            }
            Desc::Through {
                j,
                rt,
                rt2,
                back,
                off,
                bytes,
                bb,
                cut,
            } => format!(
                "Desc::Through {{ j: {j}, rt: {rt}, rt2: {rt2}, back: {back}, off: {off}, bytes: {bytes}, bb: {bb}, cut: {cut} }}"
            ),
            Desc::Own {
                j,
                n,
                rt,
                rt2,
                bytes,
                bb,
            } => format!(
                "Desc::Own {{ j: {j}, n: {n}, rt: {rt}, rt2: {rt2}, bytes: {bytes}, bb: {bb} }}"
            ),
            Desc::Mark { j, bb, c } => format!("Desc::Mark {{ j: {j}, bb: {bb}, c: {c} }}"),
            Desc::Probes {
                j,
                c,
                from,
                off,
                bytes,
                store,
                bb,
            } => format!(
                "Desc::Probes {{ j: {j}, c: {c}, from: {from}, off: {off}, bytes: {bytes}, store: {store}, bb: {bb} }}"
            ),
            Desc::Exit { j, guest } => format!("Desc::Exit {{ j: {j}, guest: {guest:#x} }}"),
            Desc::Jump { j, n, out } => format!("Desc::Jump {{ j: {j}, n: {n}, out: {out} }}"),
            Desc::Call { j, code } => format!("Desc::Call {{ j: {j}, code: {code} }}"),
            Desc::Frame {
                j,
                word,
                rt,
                off,
                moves,
                bb,
                bb2,
                cut,
            } => format!(
                "Desc::Frame {{ j: {j}, f: FrameAccess {{ word: {word:#010x}u32, rt: {rt}, off: {off}, moves: {moves} }}, bb: {bb}, bb2: {bb2}, cut: {cut} }}"
            ),
            Desc::Stop { j } => format!("Desc::Stop {{ j: {j} }}"),
            Desc::StackCut { j, n, bb, cut } => {
                format!("Desc::StackCut {{ j: {j}, n: {n}, bb: {bb}, cut: {cut} }}")
            }
            Desc::StackMove { j, bb, cut } => {
                format!("Desc::StackMove {{ j: {j}, bb: {bb}, cut: {cut} }}")
            }
            Desc::StackStep {
                j,
                k,
                off,
                bb,
                cut,
            } => format!("Desc::StackStep {{ j: {j}, k: {k}, off: {off}, bb: {bb}, cut: {cut} }}"),
            Desc::StackAccess {
                j,
                rt,
                rt2,
                off,
                moves,
                bytes,
                bb,
            } => format!(
                "Desc::StackAccess {{ j: {j}, rt: {rt}, rt2: {rt2}, off: {off}, moves: {moves}, bytes: {bytes}, bb: {bb} }}"
            ),
            Desc::StackVia {
                j,
                rt,
                rt2,
                bytes,
                bb,
                cut,
            } => format!(
                "Desc::StackVia {{ j: {j}, rt: {rt}, rt2: {rt2}, bytes: {bytes}, bb: {bb}, cut: {cut} }}"
            ),
            Desc::StackProbes {
                j,
                c,
                bytes,
                store,
                bb,
            } => format!(
                "Desc::StackProbes {{ j: {j}, c: {c}, bytes: {bytes}, store: {store}, bb: {bb} }}"
            ),
            Desc::Same { j, bb, cut } => format!("Desc::Same {{ j: {j}, bb: {bb}, cut: {cut} }}"),
            Desc::DisplacedJump { j, n, out } => {
                format!("Desc::DisplacedJump {{ j: {j}, n: {n}, out: {out} }}")
            }
            Desc::StackAt {
                j,
                off,
                rt,
                rt2,
                bytes,
                bb,
                cut,
            } => format!(
                "Desc::StackAt {{ j: {j}, off: {off}, rt: {rt}, rt2: {rt2}, bytes: {bytes}, bb: {bb}, cut: {cut} }}"
            ),
        }
    }
}

/// compose.rs `base_ok`.
fn base_ok(bb: i64) -> bool {
    (0..31).contains(&bb) && bb != 23 && bb != 28
}

/// compose.rs `reg_ok`.
fn reg_ok(r: i64, bb: i64) -> bool {
    (0..32).contains(&r) && r != bb && r != 28
}

fn table(displaced: bool) -> u64 {
    if displaced {
        1 << 46
    } else {
        (1 << 46) + (u64::from(SLOT) << SLOT_BITS) * 4
    }
}

fn exit_words(slot: u64) -> [u32; 4] {
    let half = |k: u32| (((slot >> (16 * k)) & 0xffff) as u32) << 5;
    [
        0xd280_0000 | half(0) | 23,
        0xf280_0000 | 1 << 21 | half(1) | 23,
        0xf280_0000 | 2 << 21 | half(2) | 23,
        0xd61f_02e0,
    ]
}

fn b_by(by: i64) -> u32 {
    (0x1400_0000 + if by >= 0 { by / 4 } else { 0x400_0000 + by / 4 }) as u32
}

/// compose.rs `add_cut_into`: `add xc, xbb, wN, uxtw` (c 31: `add sp, ...`, compose.rs `add_sp_cut`).
fn add_cut_into(n: i64, bb: i64, c: i64) -> u32 {
    0x8b20_4000 | (n as u32) << 16 | (bb as u32) << 5 | c as u32
}

/// compose.rs `mov_sp_from`, `STACK_TO_X23`.
fn mov_sp_from(c: i64) -> u32 {
    0x9100_001f | (c as u32) << 5
}

const STACK_TO_X23: u32 = 0x9100_03f7;

/// compose.rs `jump_words`: the `cbnz` going `by` bytes.
fn jump_words(n: u32, by: i64) -> [u32; 7] {
    [
        0xd360_dc17 | n << 5,
        0xb500_0017 | ((by / 4) as u32) << 5,
        0x8b20_4317 | n << 16,
        0xd37e_aef7,
        0xb342_0ef7,
        0xd252_02f7,
        0xd61f_02e0,
    ]
}

/// compose.rs `displaced_jump_words`: the `cbnz` going `by` bytes.
fn displaced_jump_words(n: u32, by: i64) -> [u32; 6] {
    [
        0xd360_dc17 | n << 5,
        0xb500_0017 | ((by / 4) as u32) << 5,
        0xd37e_7c17 | n << 5,
        0xb342_0ef7,
        0xd252_02f7,
        0xd61f_02e0,
    ]
}

/// compose.rs `movx_c`: `movz xc, #off`, or `movn xc, #-off-1`; the offset it writes, if it is one.
fn movx_c(c: i64, off: i64) -> u32 {
    if off >= 0 {
        0xd280_0000 | (off as u32) << 5 | c as u32
    } else {
        0x9280_0000 | ((-off - 1) as u32) << 5 | c as u32
    }
}

fn movx_off(word: u32) -> Option<i64> {
    let k = i64::from((word >> 5) & 0xffff);
    match word & 0xffe0_0000 {
        0xd280_0000 => Some(k),
        0x9280_0000 => Some(-k - 1),
        _ => None,
    }
}

/// compose.rs `add_from_c`: `add xc, xfrom, xc` (31: the stack pointer).
fn add_from_c(c: i64, from: i64) -> u32 {
    0x8b20_6000 | (c as u32) << 16 | (from as u32) << 5 | c as u32
}

/// compose.rs `stack_probes_words`.
fn stack_probes_words(c: i64, bytes: i64, store: bool) -> Vec<u32> {
    let mut w = probes_words(c, 31, 0, bytes, store);
    w.splice(0..2, [0x9100_03e0 | c as u32]);
    w
}

/// compose.rs `through_cut`.
fn through_cut(cut: i64, rt: i64, rt2: i64, back: bool) -> i64 {
    if back {
        31
    } else {
        after_cut(after_cut(cut, rt), rt2)
    }
}

/// branch.rs `floor_call` of `floor_entry(code)`.
fn floor_call(code: u64) -> [u32; 5] {
    let entry = FLOOR_PAGE + 16 * code;
    let half = |k: u32| (((entry >> (16 * k)) & 0xffff) as u32) << 5;
    [
        0xf900_2f9e,
        0xd280_001e | half(0),
        0xf280_001e | 1 << 21 | half(1),
        0xf280_001e | 2 << 21 | half(2),
        0xd63f_03c0,
    ]
}

/// compose.rs `probes_words`.
fn probes_words(c: i64, from: i64, off: i64, bytes: i64, store: bool) -> Vec<u32> {
    let k = ((bytes - 1) as u32) << 10;
    let mut w = vec![movx_c(c, off), add_from_c(c, from)];
    let c = c as u32;
    if store {
        w.extend([
            0x383f_001f | c << 5,
            0x9100_0000 | k | c << 5 | c,
            0x383f_001f | c << 5,
        ]);
    } else {
        w.extend([0x3940_001f | c << 5, 0x3940_001f | k | c << 5]);
    }
    w
}

/// The general registers `writes` writes (x0 to x30), and whether it writes the stack pointer: of a register-only
/// word, which may (a stack step, a stack cut), or of an access's loads, which decode.rs never gives it.
fn general(writes: &[(Reg, Made)]) -> (Vec<i64>, bool) {
    let mut regs = Vec::new();
    let mut sp = false;
    for (reg, _) in writes {
        match reg {
            Reg::X(r) => regs.push(i64::from(*r)),
            Reg::Sp => sp = true,
        }
    }
    (regs, sp)
}

/// The one or two general registers an access loads (31 for none).
fn loaded(loads: &[(Reg, Made)]) -> Option<(i64, i64)> {
    match general(loads) {
        (regs, false) => match regs.as_slice() {
            [] => Some((31, 31)),
            [r] => Some((*r, 31)),
            [r, s] => Some((*r, *s)),
            _ => None,
        },
        (_, true) => None,
    }
}

/// Finds the description of `words` (compose.rs `described`): the blocks entered from the first word with the base in
/// x24 and no cut, each place a block goes on to described in turn. `Err` names the first word no block kind describes.
fn describe(words: &[u32], displaced: bool) -> Result<Vec<Desc>, String> {
    let len = words.len() as i64;
    let mut descs: Vec<Desc> = Vec::new();
    let mut todo: Vec<(i64, i64, i64)> = vec![(0, 24, 31)];
    let mut seen = BTreeSet::new();
    let at = |i: i64| -> Option<u32> { (0..len).contains(&i).then(|| words[i as usize]) };
    let holds =
        |j: i64, w: &[u32]| -> bool { (0..w.len()).all(|k| at(j + k as i64) == Some(w[k])) };
    while let Some((j, bb, cut)) = todo.pop() {
        if !seen.insert((j, bb, cut)) || descs.iter().any(|d| d.entry() == j && d.takes(bb, cut)) {
            continue;
        }
        let Some(word) = at(j) else {
            return Err(format!("word {j} outside the piece"));
        };
        let mut next = |d: Desc, goes: &[(i64, i64, i64)]| {
            descs.push(d);
            todo.extend_from_slice(goes);
        };
        // The exit, which hands on the base in x24.
        let slot = u64::from((words[j as usize] >> 5) & 0xffff)
            | u64::from((at(j + 1).unwrap_or(0) >> 5) & 0xffff) << 16
            | u64::from((at(j + 2).unwrap_or(0) >> 5) & 0xffff) << 32;
        let t = table(displaced);
        if holds(j, &exit_words(slot))
            && slot >= t
            && (slot - t).is_multiple_of(4)
            && (slot - t) / 4 >= 4
        {
            let guest = ((slot - t) / 4 - 4) as i64;
            if guest + 4 < 1 << 32 {
                if bb != 24 {
                    return Err(format!("word {j}: an exit with the base in x{bb}"));
                }
                next(Desc::Exit { j, guest }, &[]);
                continue;
            }
        }
        // The mark.
        let c = i64::from(word & 31);
        if base_ok(bb)
            && c != bb
            && c != 28
            && c < 31
            && holds(
                j,
                &[
                    0xd260_0000 | (bb as u32) << 5 | c as u32,
                    0x397f_fc1f | (c as u32) << 5,
                    0,
                ],
            )
        {
            next(Desc::Mark { j, bb, c }, &[]);
            continue;
        }
        // The jump.
        let n = (word >> 5) & 31;
        if bb == 24
            && !displaced
            && n < 31
            && let Some(Step::Branch { by, .. }) = at(j + 1).map(decode)
            && j + 1 + by / 4 > j + 1
            && j + 1 + by / 4 < len
            && holds(j, &jump_words(n, by))
        {
            let out = j + 1 + by / 4;
            next(
                Desc::Jump {
                    j,
                    n: i64::from(n),
                    out,
                },
                &[(out, 24, 31)],
            );
            continue;
        }
        // The displaced jump.
        if bb == 24
            && displaced
            && n < 31
            && let Some(Step::Branch { by, .. }) = at(j + 1).map(decode)
            && j + 1 + by / 4 > j + 1
            && j + 1 + by / 4 < len
            && holds(j, &displaced_jump_words(n, by))
        {
            let out = j + 1 + by / 4;
            next(
                Desc::DisplacedJump {
                    j,
                    n: i64::from(n),
                    out,
                },
                &[(out, 24, 31)],
            );
            continue;
        }
        // The floor call.
        let entry = u64::from((at(j + 1).unwrap_or(0) >> 5) & 0xffff)
            | u64::from((at(j + 2).unwrap_or(0) >> 5) & 0xffff) << 16
            | u64::from((at(j + 3).unwrap_or(0) >> 5) & 0xffff) << 32;
        if entry >= FLOOR_PAGE
            && (entry - FLOOR_PAGE).is_multiple_of(16)
            && (entry - FLOOR_PAGE) / 16 < 256
        {
            let code = (entry - FLOOR_PAGE) / 16;
            if holds(j, &floor_call(code)) {
                next(
                    Desc::Call {
                        j,
                        code: code as i64,
                    },
                    &[],
                );
                continue;
            }
        }
        if !base_ok(bb) {
            return Err(format!("word {j}: base in x{bb}"));
        }
        // The probes, through the cut or the stack pointer.
        let c = i64::from(word & 31);
        if let Some(off) = movx_off(word)
            && (0..31).contains(&c)
            && c != bb
            && c != 28
            && off >= -0x400
            && let (Some(w1), Some(w2), Some(w3)) = (at(j + 1), at(j + 2), at(j + 3))
        {
            let from = i64::from((w1 >> 5) & 31);
            let store = w2 == 0x383f_001f | (c as u32) << 5;
            let bytes = i64::from((w3 >> 10) & 0xfff) + 1;
            let length = if store { 5 } else { 4 };
            if c != from
                && needs_cut(from, cut)
                && (from != 31 || !displaced)
                && off + bytes <= 0x1_0000
                && j + length < len
                && holds(j, &probes_words(c, from, off, bytes, store))
            {
                next(
                    Desc::Probes {
                        j,
                        c,
                        from,
                        off,
                        bytes,
                        store,
                        bb,
                    },
                    &[(j + length, bb, from)],
                );
                continue;
            }
        }
        // A cut, of a register or of the stack pointer.
        let (n, rn, rd) = (
            i64::from((word >> 16) & 31),
            i64::from((word >> 5) & 31),
            i64::from(word & 31),
        );
        if j + 1 < len && n < 31 && rn == bb && word == add_cut_into(n, bb, rd) {
            if rd == 31 {
                next(Desc::StackCut { j, n, bb, cut }, &[(j + 1, bb, cut)]);
                continue;
            }
            if rd != bb && rd != 28 {
                next(Desc::Cut { j, n, c: rd, bb }, &[(j + 1, bb, rd)]);
                continue;
            }
        }
        // The stack pointer moved from the cut.
        if j + 1 < len && cut < 31 && word == mov_sp_from(cut) {
            next(Desc::StackMove { j, bb, cut }, &[(j + 1, bb, cut)]);
            continue;
        }
        // The probes of the first and last byte at the stack pointer.
        if !displaced
            && word & 0xffff_ffe0 == 0x9100_03e0
            && c < 31
            && c != bb
            && c != 28
            && let (Some(w1), Some(w2)) = (at(j + 1), at(j + 2))
        {
            let store = w1 == 0x383f_001f | (c as u32) << 5;
            let bytes = i64::from((w2 >> 10) & 0xfff) + 1;
            let length = if store { 4 } else { 3 };
            if j + length < len && holds(j, &stack_probes_words(c, bytes, store)) {
                next(
                    Desc::StackProbes {
                        j,
                        c,
                        bytes,
                        store,
                        bb,
                    },
                    &[(j + length, bb, 31)],
                );
                continue;
            }
        }
        // An access at the stack pointer plus an offset made in x23.
        if !displaced
            && cut != 23
            && let Some(off) = movx_off(word)
            && c == 23
            && (-0x400..0x1_0000).contains(&off)
            && j + 3 < len
            && at(j + 1) == Some(add_from_c(23, 31))
            && let Step::Access(access) = decode(words[j as usize + 2])
            && access.base == Base::Reg(Source::Reg(Reg::X(23)))
            && access.offset == Offset::Constant(0, 0)
            && access.moved.is_none()
            && let Some((rt, rt2)) = loaded(&access.loads)
            && reg_ok(rt, bb)
            && reg_ok(rt2, bb)
            && (cut == 31 || (rt != cut && rt2 != cut))
            && off + access.bytes as i64 <= 0x1_0000
        {
            next(
                Desc::StackAt {
                    j,
                    off,
                    rt,
                    rt2,
                    bytes: access.bytes as i64,
                    bb,
                    cut,
                },
                &[(j + 3, bb, cut)],
            );
            continue;
        }
        // The cut moved onto itself.
        if j + 1 < len && cut < 31 && word == 0x9100_0000 | (cut as u32) << 5 | cut as u32 {
            next(Desc::Same { j, bb, cut }, &[(j + 1, bb, cut)]);
            continue;
        }
        // An access through a copy of the stack pointer.
        if j + 2 < len
            && !displaced
            && word == STACK_TO_X23
            && cut != 23
            && let Step::Access(access) = decode(words[j as usize + 1])
            && access.base == Base::Reg(Source::Reg(Reg::X(23)))
            && access.offset == Offset::Constant(0, 0)
            && let Some((rt, rt2)) = loaded(&access.loads)
            && reg_ok(rt, bb)
            && reg_ok(rt2, bb)
            && (cut == 31 || (rt != cut && rt2 != cut))
            && (1..=0x1_0000).contains(&(access.bytes as i64))
        {
            next(
                Desc::StackVia {
                    j,
                    rt,
                    rt2,
                    bytes: access.bytes as i64,
                    bb,
                    cut,
                },
                &[(j + 2, bb, cut)],
            );
            continue;
        }
        let step = decode(word);
        // A move of the base, or a frame access.
        if j + 1 < len && word & 0xffe0_ffe0 == 0xaa00_03e0 {
            let (t, from) = (i64::from(word & 31), i64::from((word >> 16) & 31));
            if from == bb && t < 31 && base_ok(t) {
                let d = Desc::Frame {
                    j,
                    word,
                    rt: t,
                    off: from,
                    moves: true,
                    bb,
                    bb2: t,
                    cut,
                };
                next(d, &[(j + 1, t, after_cut(cut, t))]);
                continue;
            }
        }
        if j + 1 < len
            && let Step::Access(access) = &step
            && access.base == Base::Reg(Source::Reg(Reg::X(28)))
            && access.bytes == 8
            && access.moved.is_none()
            && let Offset::Constant(off, high) = access.offset
            && off == high
            && (0..=248).contains(&off)
            && let Some((rt, 31)) = loaded(&access.loads)
            && rt != 28
            && rt != bb
        {
            let rt = if access.write { 31 } else { rt };
            next(
                Desc::Frame {
                    j,
                    word,
                    rt,
                    off,
                    moves: false,
                    bb,
                    bb2: bb,
                    cut,
                },
                &[(j + 1, bb, after_cut(cut, rt))],
            );
            continue;
        }
        match step {
            Step::Plain(writes) if j + 1 < len => {
                let (regs, sp) = general(&writes);
                let rd = match regs.as_slice() {
                    [] => 31,
                    [r] => *r,
                    _ => return Err(format!("word {j} {word:#010x}: writes {writes:?}")),
                };
                if sp && !displaced {
                    // In the window the stack pointer moves only by a stack step and its probe, or by a cut.
                    if let [(Reg::Sp, Made::AddConstant(Source::Reg(Reg::Sp), k))] =
                        writes.as_slice()
                        && *k != 0
                        && (-0xf000..=0xf000).contains(k)
                        && j + 2 < len
                        && let Step::Access(probe) = decode(words[j as usize + 1])
                        && probe.base == Base::Reg(Source::Reg(Reg::Sp))
                        && let Offset::Constant(off, high) = probe.offset
                        && off == high
                        && (-0x400..=0x400).contains(&off)
                        && probe.bytes == 4
                        && probe.loads.is_empty()
                        && probe.moved.is_none()
                    {
                        next(
                            Desc::StackStep {
                                j,
                                k: *k,
                                off,
                                bb,
                                cut,
                            },
                            &[(j + 2, bb, cut)],
                        );
                        continue;
                    }
                    return Err(format!("word {j} {word:#010x}: writes sp {writes:?}"));
                }
                if !reg_ok(rd, bb) {
                    return Err(format!("word {j} {word:#010x}: writes x{rd}"));
                }
                next(
                    Desc::Register { j, rd, sp, bb, cut },
                    &[(j + 1, bb, after_cut(cut, rd))],
                );
            }
            Step::Branch {
                by,
                conditional: false,
                link: false,
            } if (0..len).contains(&(j + by / 4)) => {
                next(Desc::Branch { j, by, bb, cut }, &[(j + by / 4, bb, cut)]);
            }
            Step::Branch {
                by, link: false, ..
            } if j + 1 < len && (0..len).contains(&(j + by / 4)) => {
                next(
                    Desc::Either { j, by, bb, cut },
                    &[(j + 1, bb, cut), (j + by / 4, bb, cut)],
                );
            }
            Step::Leaves => next(Desc::Stop { j }, &[]),
            Step::Access(access) if j + 1 < len => {
                let Some((rt, rt2)) = loaded(&access.loads) else {
                    return Err(format!("word {j} {word:#010x}: loads {:?}", access.loads));
                };
                if !reg_ok(rt, bb) || !reg_ok(rt2, bb) {
                    return Err(format!("word {j} {word:#010x}: loads x{rt}, x{rt2}"));
                }
                let bytes = access.bytes as i64;
                // An index of the zero register adds nothing.
                let offset = match access.offset {
                    Offset::Index {
                        rm: Source::Zero, ..
                    } => Offset::Constant(0, 0),
                    other => other,
                };
                match (access.base, offset, access.moved) {
                    (Base::Reg(Source::Reg(Reg::X(b))), Offset::Constant(off, high), moved)
                        if i64::from(b) == cut
                            && cut != bb
                            && cut != 28
                            && off == high
                            && off >= -0x1_0000
                            && off + bytes <= 0x1_0000 =>
                    {
                        let back = moved.is_some();
                        next(
                            Desc::Through {
                                j,
                                rt,
                                rt2,
                                back,
                                off,
                                bytes,
                                bb,
                                cut,
                            },
                            &[(j + 1, bb, through_cut(cut, rt, rt2, back))],
                        );
                    }
                    (
                        Base::Reg(Source::Reg(Reg::X(b))),
                        Offset::Index {
                            rm: Source::Reg(Reg::X(n)),
                            extend: Extend::Unsigned(32),
                            shift: 0,
                        },
                        None,
                    ) if i64::from(b) == bb && n < 31 && bytes <= 16 => {
                        next(
                            Desc::Own {
                                j,
                                n: i64::from(n),
                                rt,
                                rt2,
                                bytes,
                                bb,
                            },
                            &[(j + 1, bb, 31)],
                        );
                    }
                    (Base::Reg(Source::Reg(Reg::Sp)), Offset::Constant(off, high), moved)
                        if !displaced
                            && off == high
                            && off >= -0x400
                            && off + bytes <= 0x1_0000 =>
                    {
                        let moves = match moved {
                            None => 0,
                            Some(Moved::By(v))
                                if (-0x400..=0x400).contains(&v) && (off == 0 || off == v) =>
                            {
                                v
                            }
                            other => {
                                return Err(format!(
                                    "word {j} {word:#010x}: the stack moved {other:?}"
                                ));
                            }
                        };
                        next(
                            Desc::StackAccess {
                                j,
                                rt,
                                rt2,
                                off,
                                moves,
                                bytes,
                                bb,
                            },
                            &[(j + 1, bb, 31)],
                        );
                    }
                    other => {
                        return Err(format!("word {j} {word:#010x}: access {other:?} cut {cut}"));
                    }
                }
            }
            other => return Err(format!("word {j} {word:#010x}: {other:?}")),
        }
    }
    // compose.rs asks the first block to be entered at the first word.
    descs.sort_by_key(|d| (d.entry() != 0, d.entry()));
    Ok(descs)
}

/// A pin line of compose.rs.
fn pin(name: &str, words: &[u32], descs: &[Desc], displaced: bool) -> String {
    let words: Vec<String> = words.iter().map(|w| format!("{w:#010x}u32")).collect();
    let descs: Vec<String> = descs.iter().map(Desc::verus).collect();
    format!(
        "proof fn pin_{name}() {{ assert(described(seq![{}], pin_at({displaced}), seq![{}])) by (compute); }}",
        words.join(", "),
        descs.join(", ")
    )
}

/// The guest code of each pin: `str x2, [x1]` in a tested loop; `ldr x2, [x1], #8` and `str x2, [x1], #8` in a loop;
/// a line ending in `br x30`; stand-ins for x24 in a line; offsets of 4096 and more; and one word for each block kind
/// the stack, the pairs and the displaced jump add (`sub sp, sp, #16`, `add sp, sp, #16`, `ldp x29, x30, [sp], #16`,
/// `stp x29, x30, [sp, #-16]!`, `mov sp, x0`, `ld4 {..}, [sp]`, `st4 {..}, [sp], x3`, `ldnp s12, s12, [sp, #-208]`,
/// `st1 {..}, [sp], #24`, `casp x8, x9, ..., [x25]`, `stxp w15, w9, w13, [x5]`, `stnp w6, w2, [x7, #-100]`, displaced
/// `ldp d1, d1, [x20], #-280`, `br x7` and `mov sp, x0`).
fn pinned() -> Vec<(&'static str, Vec<u32>, LookAhead, bool)> {
    let tested = |a: u32| vec![a, 0xf100_0400, 0x54ff_ffc1, 0xd61f_03c0];
    let line = |a: u32| vec![a, 0xd61f_03c0];
    vec![
        (
            "store_tested_loop",
            tested(0xf900_0022),
            LookAhead::Full,
            false,
        ),
        (
            "post_load_loop",
            tested(0xf840_8422),
            LookAhead::Full,
            false,
        ),
        (
            "post_store_loop",
            tested(0xf800_8422),
            LookAhead::Full,
            false,
        ),
        (
            "post_load_line",
            tested(0xf840_8422),
            LookAhead::Bounded(2),
            false,
        ),
        (
            "line_to_jump",
            vec![0x9100_0400, 0xd61f_03c0],
            LookAhead::Bounded(2),
            false,
        ),
        (
            "stand_in_add_line",
            vec![0x9100_0718, 0x9100_0400, 0xd61f_03c0],
            LookAhead::Bounded(2),
            false,
        ),
        (
            "stand_in_load_line",
            vec![0xf940_0302, 0x9100_0442, 0xd61f_03c0],
            LookAhead::Bounded(2),
            false,
        ),
        (
            "load_4096",
            vec![0xf948_0022, 0xd61f_03c0],
            LookAhead::None,
            false,
        ),
        (
            "store_4096",
            vec![0xf908_0022, 0xd61f_03c0],
            LookAhead::None,
            false,
        ),
        (
            "load_32760",
            vec![0xf97f_fc22, 0xd61f_03c0],
            LookAhead::None,
            false,
        ),
        (
            "load_word_4096",
            vec![0xb950_0022, 0xd61f_03c0],
            LookAhead::None,
            false,
        ),
        (
            "load_byte_4095",
            vec![0x397f_fc22, 0xd61f_03c0],
            LookAhead::None,
            false,
        ),
        (
            "load_4096_to_jump",
            vec![0xf948_0022, 0xd61f_03c0],
            LookAhead::Bounded(2),
            false,
        ),
        (
            "store_tested_loop_displaced",
            tested(0xf900_0022),
            LookAhead::Full,
            true,
        ),
        ("stack_step", line(0xd100_43ff), LookAhead::None, false),
        ("stack_step_up", line(0x9100_43ff), LookAhead::None, false),
        ("stack_pair_post", line(0xa8c1_7bfd), LookAhead::None, false),
        ("stack_pair_pre", line(0xa9bf_7bfd), LookAhead::None, false),
        ("stack_cut", line(0x9100_001f), LookAhead::None, false),
        ("stack_structure", line(0x4c40_03ec), LookAhead::None, false),
        (
            "stack_post_register",
            line(0x4c83_0bf3),
            LookAhead::None,
            false,
        ),
        ("stack_at", line(0x2c66_33ec), LookAhead::None, false),
        (
            "stack_short_probes",
            line(0x0c9f_67ff),
            LookAhead::None,
            false,
        ),
        (
            "pair_through_cut",
            line(0x4828_7f20),
            LookAhead::None,
            false,
        ),
        ("cut_into_x29", line(0x882f_34a9), LookAhead::None, false),
        ("negative_probes", line(0x2833_88e6), LookAhead::None, false),
        ("same_displaced", line(0x6cee_8681), LookAhead::None, true),
        ("jump_displaced", line(0xd61f_00e0), LookAhead::None, true),
        (
            "stack_cut_displaced",
            line(0x9100_001f),
            LookAhead::None,
            true,
        ),
    ]
}

#[test]
fn every_pin_of_the_composition_is_what_the_translator_emits_described() {
    let lines: Vec<&str> = COMPOSE
        .lines()
        .filter(|l| l.starts_with("proof fn pin_"))
        .collect();
    let made: Vec<String> = pinned()
        .iter()
        .map(|(name, code, look_ahead, displaced)| {
            let words = translated(code, *look_ahead, *displaced)
                .unwrap_or_else(|| panic!("{name} translates"));
            let descs = describe(&words, *displaced).unwrap_or_else(|e| panic!("{name}: {e}"));
            pin(name, &words, &descs, *displaced)
        })
        .collect();
    if lines != made {
        for line in &made {
            println!("{line}");
        }
    }
    assert_eq!(
        lines, made,
        "each pin is the translator's words for its guest code, with their description"
    );
}

/// What an access loads, as the blocks name it: none, one or a pair of general registers; a load of the stack pointer
/// (which decode.rs never gives, but `Reg` can hold) or of more than two registers is refused, by hand-written input.
#[test]
fn loaded_takes_up_to_a_pair_of_general_registers() {
    let x = |r: u8| (Reg::X(r), Made::Unknown);
    assert_eq!(loaded(&[]), Some((31, 31)));
    assert_eq!(loaded(&[x(3)]), Some((3, 31)));
    assert_eq!(loaded(&[x(3), x(30)]), Some((3, 30)));
    assert_eq!(loaded(&[x(0), x(1), x(2)]), None);
    assert_eq!(loaded(&[(Reg::Sp, Made::Unknown)]), None);
    assert_eq!(loaded(&[x(0), (Reg::Sp, Made::Unknown)]), None);
}

/// Hand-written pieces and whether they are described: each block kind's refusal.
#[test]
fn describe_refuses_what_compose_refuses() {
    let exit = exit_words(table(false) + (GUEST + 4) * 4);
    let with_exit = |w: &[u32]| [w, &exit[..]].concat();
    // An add then the exit: described.
    assert!(describe(&with_exit(&[0x9100_0400]), false).is_ok());
    // An add writing x24 (the base), or x28 (the frame): not.
    assert!(describe(&with_exit(&[0x9100_0718]), false).is_err());
    assert!(describe(&with_exit(&[0x9100_079c]), false).is_err());
    // A system call, a load through x1, a load through x23 before any cut: not.
    assert!(describe(&with_exit(&[0xd400_0001]), false).is_err());
    assert!(describe(&with_exit(&[0xf940_0020]), false).is_err());
    assert!(describe(&with_exit(&[0xf940_02e0]), false).is_err());
    // `mov sp, x0` copied as it is: not in the window, where the stack pointer moves only by a step and its probe or
    // a cut (compose.rs `part`); displaced, where it is the guest's own: described.
    assert!(describe(&with_exit(&[0x9100_001f]), false).is_err());
    let exit_displaced = exit_words(table(true) + (GUEST + 4) * 4);
    assert!(describe(&[&[0x9100_001f][..], &exit_displaced[..]].concat(), true).is_ok());
    // A stack step and its probe, which may look either way: described; with no probe, or one 64 KiB away: not.
    assert!(describe(&with_exit(&[0xd100_43ff, 0xb940_03ff]), false).is_ok());
    assert!(describe(&with_exit(&[0x9100_43ff, 0xb85f_c3ff]), false).is_ok());
    assert!(describe(&with_exit(&[0xd100_43ff, 0xb85f_c3ff]), false).is_ok());
    assert!(describe(&with_exit(&[0xd100_43ff]), false).is_err());
    assert!(describe(&with_exit(&[0xd100_43ff, 0xb97f_ffff]), false).is_err());
    // The stack pointer cut into the window: described; added to an unchecked register: not.
    assert!(describe(&with_exit(&[add_cut_into(15, 24, 31)]), false).is_ok());
    assert!(describe(&with_exit(&[0x8b2f_403f]), false).is_err());
    // Accesses through the stack pointer: `ldr x0, [sp, #8]`, `stp x29, x30, [sp, #-16]!`, `ldp x29, x30, [sp], #16`
    // described; one indexed by a register, or loading the frame register: not.
    assert!(describe(&with_exit(&[0xf940_07e0]), false).is_ok());
    assert!(describe(&with_exit(&[0xa9bf_7bfd]), false).is_ok());
    assert!(describe(&with_exit(&[0xa8c1_7bfd]), false).is_ok());
    assert!(describe(&with_exit(&[0xf861_6be0]), false).is_err());
    assert!(describe(&with_exit(&[0xf940_07fc]), false).is_err());
    // A pair through the cut: described; one loading the base: not. A negative offset through the cut: described.
    assert!(describe(&with_exit(&[add_cut_into(1, 24, 23), 0xa940_06e0]), false).is_ok());
    assert!(describe(&with_exit(&[add_cut_into(1, 24, 23), 0xa940_62e0]), false).is_err());
    assert!(describe(&with_exit(&[add_cut_into(1, 24, 23), 0xf85f_02e0]), false).is_ok());
    // A cut into x29 and an access through it: described; through x29 with no cut: not.
    assert!(describe(&with_exit(&[add_cut_into(1, 24, 29), 0xf940_03a0]), false).is_ok());
    assert!(describe(&with_exit(&[0xf940_03a0]), false).is_err());
    // `brk #5`: a stop, described.
    assert!(describe(&[0xd420_00a0], false).is_ok());
    // An exit while x24 is the guest's (the base moved to x29): not.
    assert!(describe(&with_exit(&[0xaa18_03fd]), false).is_err());
    // The cut, then a load through x23: described; with an offset of 64 KiB: not.
    assert!(describe(&with_exit(&[add_cut_into(1, 24, 23), 0xf940_02e0]), false).is_ok());
    assert!(
        describe(
            &with_exit(&[add_cut_into(1, 24, 23), 0x3940_02e0 | 0xfff << 10]),
            false
        )
        .is_ok()
    );
    // A branch out of the piece: not.
    assert!(describe(&with_exit(&[b_by(64)]), false).is_err());
    // An exit to a slot outside the slot table: not.
    assert!(describe(&exit_words(table(false) - 16), false).is_err());
    // The frame: a store of x29 and a load at 248 are described; past the frame's words not.
    assert!(describe(&with_exit(&[0xf900_4f9d, 0xf940_7f80]), false).is_ok());
    assert!(describe(&with_exit(&[0xf940_8380]), false).is_err());
}

#[derive(Default)]
struct Count {
    pieces: u64,
    described: u64,
    forms: u64,
    forms_described: u64,
    /// Why pieces are not described, by the kind of the word that stops them, and an example.
    why: BTreeMap<String, (u64, String)>,
}

#[test]
fn the_composition_covers_this_much_of_the_instruction_list() {
    let all = forms::all();
    let mut rng = fastrand::Rng::with_seed(0x2545_f491_4f6c_dd1d);
    let mut counts = [Count::default(), Count::default()];
    for form in all {
        for (displaced, count) in [false, true].into_iter().zip(counts.iter_mut()) {
            let mut every = true;
            let mut any = false;
            for _ in 0..8 {
                let word = form.word(rng.u32(..));
                let Some(words) = translated(&[word], LookAhead::None, displaced) else {
                    continue;
                };
                any = true;
                count.pieces += 1;
                match describe(&words, displaced) {
                    Ok(_) => count.described += 1,
                    Err(e) => {
                        every = false;
                        let kind: String = e
                            .split(": ")
                            .nth(1)
                            .unwrap_or(&e)
                            .chars()
                            .take(24)
                            .collect();
                        let entry = count.why.entry(kind).or_insert((0, String::new()));
                        entry.0 += 1;
                        if entry.1.is_empty() {
                            entry.1 = format!("{} {word:#010x}: {e}", form.name);
                        }
                    }
                }
            }
            if any {
                count.forms += 1;
                count.forms_described += u64::from(every);
            }
        }
    }
    for (displaced, count) in [false, true].into_iter().zip(&counts) {
        println!(
            "displaced {displaced}: forms {} of which every piece described {}; pieces {} described {}",
            count.forms, count.forms_described, count.pieces, count.described
        );
        let mut why: Vec<_> = count.why.iter().collect();
        why.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
        for (kind, (n, example)) in why.iter().take(25) {
            println!("    {n} {kind}: e.g. {example}");
        }
    }
    // Measured 2026-11-28 (sixth run): the rest are jumps through a register loaded from the frame and accesses
    // through x23 that hold no window address in these blocks. Each figure rests on the meaning of each word, which
    // is a hypothesis taken from decode.rs (compose.rs `all_meant`): `step` is still free.
    let measured = counts.map(|c| (c.forms, c.forms_described, c.pieces, c.described));
    assert_eq!(
        measured,
        [(4429, 4422, 35432, 35424), (4429, 4425, 35432, 35426)]
    );
}
