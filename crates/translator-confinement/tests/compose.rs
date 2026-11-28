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
    Access, Base, Extend, Made, Moved, Offset, Reg, Source, Step, decode,
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
    StackAt {
        j: i64,
        off: i64,
        rt: i64,
        rt2: i64,
        bytes: i64,
        bb: i64,
        cut: i64,
    },
    TableJump {
        j: i64,
        link: bool,
    },
    DisplacedTableJump {
        j: i64,
        n: i64,
        link: bool,
    },
    ShiftThrough {
        j: i64,
        k: i64,
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
            | Desc::ShiftThrough { bb, cut: c, .. } => bb == base && c == cut,
            Desc::TableJump { .. } => base == 24 && cut == 23,
            Desc::Cut { bb, .. }
            | Desc::Own { bb, .. }
            | Desc::Mark { bb, .. }
            | Desc::StackAccess { bb, .. }
            | Desc::StackProbes { bb, .. } => bb == base,
            Desc::Exit { .. } | Desc::DisplacedTableJump { .. } => base == 24,
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
            | Desc::Call { j, .. }
            | Desc::Frame { j, .. }
            | Desc::Stop { j }
            | Desc::StackCut { j, .. }
            | Desc::StackMove { j, .. }
            | Desc::StackStep { j, .. }
            | Desc::StackAccess { j, .. }
            | Desc::StackVia { j, .. }
            | Desc::StackProbes { j, .. }
            | Desc::StackAt { j, .. }
            | Desc::TableJump { j, .. }
            | Desc::DisplacedTableJump { j, .. }
            | Desc::ShiftThrough { j, .. } => j,
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
            Desc::StackStep { j, k, off, bb, cut } => {
                format!("Desc::StackStep {{ j: {j}, k: {k}, off: {off}, bb: {bb}, cut: {cut} }}")
            }
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
            Desc::TableJump { j, link } => format!("Desc::TableJump {{ j: {j}, link: {link} }}"),
            Desc::DisplacedTableJump { j, n, link } => {
                format!("Desc::DisplacedTableJump {{ j: {j}, n: {n}, link: {link} }}")
            }
            Desc::ShiftThrough {
                j,
                k,
                rt,
                rt2,
                bytes,
                bb,
                cut,
            } => format!(
                "Desc::ShiftThrough {{ j: {j}, k: {k}, rt: {rt}, rt2: {rt2}, bytes: {bytes}, bb: {bb}, cut: {cut} }}"
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
        0xd280_0000 + half(0) + 23,
        0xf280_0000 + (1 << 21) + half(1) + 23,
        0xf280_0000 + (2 << 21) + half(2) + 23,
        0xd61f_02e0,
    ]
}

fn b_by(by: i64) -> u32 {
    (0x1400_0000 + if by >= 0 { by / 4 } else { 0x400_0000 + by / 4 }) as u32
}

/// compose.rs `add_cut_into`: `add xc, xbb, wN, uxtw` (c 31: `add sp, ...`, compose.rs `add_sp_cut`).
fn add_cut_into(n: i64, bb: i64, c: i64) -> u32 {
    0x8b20_4000 + ((n as u32) << 16) + ((bb as u32) << 5) + c as u32
}

/// compose.rs `mov_sp_from`, `STACK_TO_X23`.
fn mov_sp_from(c: i64) -> u32 {
    0x9100_001f + ((c as u32) << 5)
}

const STACK_TO_X23: u32 = 0x9100_03f7;

/// branch.rs `UBFIZ_X23`, `BFXIL_X23`, `EOR_X23_TABLE`, copy.rs `BR_X23`.
const UBFIZ_X23: u32 = 0xd37e_aef7;
const BFXIL_X23: u32 = 0xb342_0ef7;
const EOR_X23_TABLE: u32 = 0xd252_02f7;
const BR_X23: u32 = 0xd61f_02e0;

/// branch.rs `ubfiz_x23`: `ubfiz x23, xN, #2, #32` (31: the zero register).
fn ubfiz_x23(n: i64) -> u32 {
    0xd37e_7c17 + ((n as u32) << 5)
}

/// compose.rs `shift_word`: `add xc, xc, #k`, or `sub xc, xc, #-k`.
fn shift_word(c: i64, k: i64) -> u32 {
    let c = c as u32;
    if k >= 0 {
        0x9100_0000 + ((k as u32) << 10) + (c << 5) + c
    } else {
        0xd100_0000 + (((-k) as u32) << 10) + (c << 5) + c
    }
}

/// branch.rs `jump`'s words without the count and its way to the floor: the `cbnz` going `by` bytes.
fn jump_words(n: u32, by: i64) -> [u32; 7] {
    [
        0xd360_dc17 + (n << 5),
        0xb500_0017 + (((by / 4) as u32) << 5),
        0x8b20_4317 + (n << 16),
        0xd37e_aef7,
        0xb342_0ef7,
        0xd252_02f7,
        0xd61f_02e0,
    ]
}

/// branch.rs `displaced_jump`'s words without the count and its way to the floor: the `cbnz` going `by` bytes.
fn displaced_jump_words(n: u32, by: i64) -> [u32; 6] {
    [
        0xd360_dc17 + (n << 5),
        0xb500_0017 + (((by / 4) as u32) << 5),
        0xd37e_7c17 + (n << 5),
        0xb342_0ef7,
        0xd252_02f7,
        0xd61f_02e0,
    ]
}

/// compose.rs `movx_c`: `movz xc, #off`, or `movn xc, #-off-1`; the offset it writes, if it is one.
fn movx_c(c: i64, off: i64) -> u32 {
    if off >= 0 {
        0xd280_0000 + ((off as u32) << 5) + c as u32
    } else {
        0x9280_0000 + (((-off - 1) as u32) << 5) + c as u32
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
    0x8b20_6000 + ((c as u32) << 16) + ((from as u32) << 5) + c as u32
}

/// compose.rs `stack_probes_words`.
fn stack_probes_words(c: i64, bytes: i64, store: bool) -> Vec<u32> {
    let mut w = probes_words(c, 31, 0, bytes, store);
    w.splice(0..2, [0x9100_03e0 + c as u32]);
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
    floor_call_at(FLOOR_PAGE + 16 * code)
}

/// branch.rs `floor_call`.
fn floor_call_at(entry: u64) -> [u32; 5] {
    let half = |k: u32| (((entry >> (16 * k)) & 0xffff) as u32) << 5;
    [
        0xf900_2f9e,
        0xd280_001e + half(0),
        0xf280_001e + (1 << 21) + half(1),
        0xf280_001e + (2 << 21) + half(2),
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
            0x383f_001f + (c << 5),
            0x9100_0000 + k + (c << 5) + c,
            0x383f_001f + (c << 5),
        ]);
    } else {
        w.extend([0x3940_001f + (c << 5), 0x3940_001f + k + (c << 5)]);
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
    describe_with(words, displaced, &decode)
}

/// [`describe`] with what each word does taken from `decode`, so that a test can hand it a meaning no word has today.
fn describe_with(
    words: &[u32],
    displaced: bool,
    decode: &dyn Fn(u32) -> Step,
) -> Result<Vec<Desc>, String> {
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
        // The exit, which hands on the base in x24: the guest address its first three words' halves name, then its
        // four words checked whole.
        let slot = u64::from((word >> 5) & 0xffff)
            + (u64::from((at(j + 1).unwrap_or(0) >> 5) & 0xffff) << 16)
            + (u64::from((at(j + 2).unwrap_or(0) >> 5) & 0xffff) << 32);
        let t = table(displaced);
        if let Some(guest) = slot.checked_sub(t).map(|o| (o / 4) as i64 - 4)
            && (0..(1 << 32) - 4).contains(&guest)
            && holds(j, &exit_words(t + 4 * (guest as u64 + 4)))
        {
            if bb != 24 {
                return Err(format!("word {j}: an exit with the base in x{bb}"));
            }
            next(Desc::Exit { j, guest }, &[]);
            continue;
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
                    0xd260_0000 + ((bb as u32) << 5) + c as u32,
                    0x397f_fc1f + ((c as u32) << 5),
                    0,
                ],
            )
        {
            next(Desc::Mark { j, bb, c }, &[]);
            continue;
        }
        // The floor call: the code its entry's low half names (the floor page's is zero), then its five words checked
        // whole.
        let code = u64::from((at(j + 1).unwrap_or(0) >> 5) & 0xffff) / 16;
        if code < 256 && holds(j, &floor_call(code)) {
            next(
                Desc::Call {
                    j,
                    code: code as i64,
                },
                &[],
            );
            continue;
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
            let store = w2 == 0x383f_001f + ((c as u32) << 5);
            let bytes = i64::from((w3 >> 10) & 0xfff) + 1;
            let length = if store { 5 } else { 4 };
            if c != from
                && needs_cut(from, cut)
                && (from != 31 || !displaced)
                && off + bytes <= 0x1_0000
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
        // (A block that goes on past the piece's last word is refused where it goes on: `at` finds no word there.)
        if rn == bb && word == add_cut_into(n, bb, rd) {
            if rd == 31 {
                if n < 31 {
                    next(Desc::StackCut { j, n, bb, cut }, &[(j + 1, bb, cut)]);
                    continue;
                }
            } else if rd != bb && rd != 28 {
                next(Desc::Cut { j, n, c: rd, bb }, &[(j + 1, bb, rd)]);
                continue;
            }
        }
        // The jump's last words, through the slot table: from the cut in x23, or displaced from xN's low word.
        // With a `blr`, three words writing x30 alone come before the `br x23`.
        let link = (3..6).all(|k| {
            matches!(at(j + k).map(decode), Some(Step::Plain(w)) if matches!(w.as_slice(), [(Reg::X(30), _)]))
        }) && at(j + 6) == Some(BR_X23);
        let tail = |first: u32| -> Option<bool> {
            if !holds(j, &[first, BFXIL_X23, EOR_X23_TABLE]) {
                None
            } else if at(j + 3) == Some(BR_X23) {
                Some(false)
            } else if link {
                Some(true)
            } else {
                None
            }
        };
        if bb == 24
            && cut == 23
            && !displaced
            && let Some(link) = tail(UBFIZ_X23)
        {
            next(Desc::TableJump { j, link }, &[]);
            continue;
        }
        if bb == 24
            && displaced
            && word & 0xffff_fc1f == ubfiz_x23(0)
            && let Some(link) = tail(word)
        {
            let n = i64::from((word >> 5) & 31);
            next(Desc::DisplacedTableJump { j, n, link }, &[]);
            continue;
        }
        // An access through the cut moved by a constant.
        if cut != bb
            && cut != 28
            && let Step::Plain(writes) = decode(word)
            && let [(Reg::X(r), Made::AddConstant(Source::Reg(Reg::X(from)), k))] = writes.as_slice()
            && i64::from(*r) == cut
            && i64::from(*from) == cut
            && word == shift_word(cut, *k)
            && let Some(Step::Access(access)) = at(j + 1).map(decode)
            && access.base == Base::Reg(Source::Reg(Reg::X(cut as u8)))
            && access.offset == Offset::Constant(0, 0)
            && access.moved.is_none()
            && let Some((rt, rt2)) = loaded(&access.loads)
            && reg_ok(rt, bb)
            && reg_ok(rt2, bb)
            // -0x1_0000 <= k (compose.rs): the word is `shift_word`'s, whose immediate is at most 4095.
            && access.bytes >= 1
            && *k + access.bytes as i64 <= 0x1_0000
        {
            next(
                Desc::ShiftThrough {
                    j,
                    k: *k,
                    rt,
                    rt2,
                    bytes: access.bytes as i64,
                    bb,
                    cut,
                },
                &[(j + 2, bb, 31)],
            );
            continue;
        }
        // The stack pointer moved from the cut.
        if cut < 31 && word == mov_sp_from(cut) {
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
            let store = w1 == 0x383f_001f + ((c as u32) << 5);
            let bytes = i64::from((w2 >> 10) & 0xfff) + 1;
            let length = if store { 4 } else { 3 };
            if holds(j, &stack_probes_words(c, bytes, store)) {
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
            && at(j + 1) == Some(add_from_c(23, 31))
            && let Some(Step::Access(access)) = at(j + 2).map(decode)
            && access.base == Base::Reg(Source::Reg(Reg::X(23)))
            && access.offset == Offset::Constant(0, 0)
            && access.moved.is_none()
            && let Some((rt, rt2)) = loaded(&access.loads)
            && reg_ok(rt, bb)
            && reg_ok(rt2, bb)
            && (cut == 31 || (rt != cut && rt2 != cut))
            && access.bytes >= 1
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
        // An access through a copy of the stack pointer.
        if !displaced
            && word == STACK_TO_X23
            && cut != 23
            && let Some(Step::Access(access)) = at(j + 1).map(decode)
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
        if word & 0xffe0_ffe0 == 0xaa00_03e0 {
            let (t, from) = (i64::from(word & 31), i64::from((word >> 16) & 31));
            if from == bb && base_ok(t) {
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
        if let Step::Access(access) = &step
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
            Step::Plain(writes) => {
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
                        && let Some(Step::Access(probe)) = at(j + 1).map(decode)
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
            } => {
                next(Desc::Branch { j, by, bb, cut }, &[(j + by / 4, bb, cut)]);
            }
            Step::Branch {
                by, link: false, ..
            } => {
                next(
                    Desc::Either { j, by, bb, cut },
                    &[(j + 1, bb, cut), (j + by / 4, bb, cut)],
                );
            }
            Step::Leaves => next(Desc::Stop { j }, &[]),
            Step::Access(access) => {
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
                            && bytes >= 1
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
                    ) if i64::from(b) == bb && (1..=16).contains(&bytes) => {
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
                            && bytes >= 1
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
/// `ldp d1, d1, [x20], #-280`, `br x7` and `mov sp, x0`); and for the table jump and the moved cut, `br x28`, `blr x30`,
/// `ret xzr`, displaced `br xzr` and `blr x28`, `ldnp d1, d1, [x13, #-312]` and displaced `ldp d22, d22, [x7, #-328]`.
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
        ("jump_frame", vec![0xd61f_0380], LookAhead::None, false),
        ("call_link", vec![0xd63f_03c0], LookAhead::None, false),
        ("return_zero", vec![0xd65f_03e0], LookAhead::None, false),
        (
            "jump_zero_displaced",
            vec![0xd61f_03e0],
            LookAhead::None,
            true,
        ),
        (
            "call_frame_displaced",
            vec![0xd63f_0380],
            LookAhead::None,
            true,
        ),
        ("shift_through", vec![0x6c6c_85a1], LookAhead::None, false),
        (
            "shift_through_displaced",
            vec![0x6d6b_d8f6],
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

/// The words this file builds are Arm's encodings (GNU as 2.44 for aarch64, by hand; the translator's own forms where
/// it picks another: `add x15, x23, x15` extended, `mov x23, x23` as `add #0`).
#[test]
fn the_word_builders_are_the_arm_encodings() {
    assert_eq!(
        exit_words(0x2_0003_0004),
        [0xd280_0097, 0xf2a0_0077, 0xf2c0_0057, 0xd61f_02e0]
    );
    assert_eq!([b_by(64), b_by(-8)], [0x1400_0010, 0x17ff_fffe]);
    assert_eq!(
        [
            add_cut_into(1, 24, 23),
            add_cut_into(15, 24, 31),
            add_cut_into(31, 29, 22)
        ],
        [0x8b21_4317, 0x8b2f_431f, 0x8b3f_43b6]
    );
    assert_eq!(mov_sp_from(23), 0x9100_02ff);
    assert_eq!([ubfiz_x23(5), ubfiz_x23(31)], [0xd37e_7cb7, 0xd37e_7ff7]);
    assert_eq!(
        [
            shift_word(23, 312),
            shift_word(23, -312),
            shift_word(5, 4095)
        ],
        [0x9104_e2f7, 0xd104_e2f7, 0x913f_fca5]
    );
    assert_eq!(
        jump_words(1, 24),
        [
            0xd360_dc37,
            0xb500_00d7,
            0x8b21_4317,
            0xd37e_aef7,
            0xb342_0ef7,
            0xd252_02f7,
            0xd61f_02e0
        ]
    );
    assert_eq!(
        displaced_jump_words(1, 20),
        [
            0xd360_dc37,
            0xb500_00b7,
            0xd37e_7c37,
            0xb342_0ef7,
            0xd252_02f7,
            0xd61f_02e0
        ]
    );
    assert_eq!(
        [movx_c(23, 8), movx_c(23, -1025), movx_c(15, -312)],
        [0xd280_0117, 0x9280_8017, 0x9280_26ef]
    );
    assert_eq!(
        [
            0xd280_0117,
            0x9280_8017,
            0xd29f_ff17,
            0x5280_0117,
            0xf2a0_2017,
            0x9100_0400
        ]
        .map(movx_off),
        [Some(8), Some(-1025), Some(0xfff8), None, None, None]
    );
    assert_eq!(
        [add_from_c(23, 31), add_from_c(15, 23)],
        [0x8b37_63f7, 0x8b2f_62ef]
    );
    assert_eq!(
        stack_probes_words(22, 8, false),
        [0x9100_03f6, 0x3940_02df, 0x3940_1edf]
    );
    assert_eq!(
        stack_probes_words(22, 8, true),
        [0x9100_03f6, 0x383f_02df, 0x9100_1ed6, 0x383f_02df]
    );
    assert_eq!(
        probes_words(15, 23, -312, 16, false),
        [0x9280_26ef, 0x8b2f_62ef, 0x3940_01ff, 0x3940_3dff]
    );
    assert_eq!(
        floor_call_at(0x2_0003_0004),
        [
            0xf900_2f9e,
            0xd280_009e,
            0xf2a0_007e,
            0xf2c0_005e,
            0xd63f_03c0
        ]
    );
    assert_eq!(
        floor_call(7),
        [
            0xf900_2f9e,
            0xd280_0e1e,
            0xf2a0_001e,
            0xf2c4_003e,
            0xd63f_03c0
        ]
    );
    // The exit to the entry of GUEST + 4 builds 0x4018_0100_4010 in the window, 0x4000_0100_4010 displaced.
    assert_eq!(
        [table(false), table(true)],
        [0x4018_0000_0000, 0x4000_0000_0000]
    );
    assert_eq!(
        [
            through_cut(23, 0, 31, true),
            through_cut(23, 0, 31, false),
            through_cut(23, 23, 31, false),
            through_cut(23, 0, 23, false)
        ],
        [31, 23, 31, 31]
    );
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
    // The cut, then a load through x23: described; a byte at 4095, the largest offset of a byte load: described.
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

/// A word no real word is: [`each_condition_of_compose_is_refused`] gives it a meaning decode.rs never gives.
const MADE_UP: u32 = 0xffff_0001;

/// One condition of compose.rs `shape_ok` or `iface_ok`, a hand-written piece that breaks it (or meets it at its
/// edge), and compose.rs's answer: whether the piece is described.
struct Condition {
    clause: &'static str,
    words: Vec<u32>,
    displaced: bool,
    described: bool,
    /// What [`MADE_UP`] does, for a bound no word decode.rs knows reaches.
    made_up: Option<Step>,
}

/// An access at `base` plus `off` (one place), loading the registers in `loads`.
fn access(base: Reg, off: i64, bytes: u64, loads: &[u8], moved: Option<Moved>) -> Step {
    Step::Access(Access {
        base: Base::Reg(Source::Reg(base)),
        offset: Offset::Constant(off, off),
        bytes,
        write: loads.is_empty(),
        loads: loads.iter().map(|r| (Reg::X(*r), Made::Unknown)).collect(),
        moved,
    })
}

/// Every condition of compose.rs `shape_ok` and `iface_ok`, kind by kind, each with a hand-written piece compose.rs
/// refuses for that condition alone: `describe` must refuse it too, or it would count as described a piece that
/// `described` refuses. An `iface_ok` condition is broken where one word is reached twice, first with what its block
/// needs and then without (the branch's target is described first): the second time must be described anew.
#[test]
fn each_condition_of_compose_is_refused() {
    const ADD: u32 = 0x9100_0400; // add x0, x0, #1
    const CUT: u32 = 0x8b21_4317; // add x23, x24, w1, uxtw
    const CUT29: u32 = 0x8b21_431d; // add x29, x24, w1, uxtw
    const SPOIL: u32 = 0x9100_0417; // add x23, x0, #1
    const SPOIL29: u32 = 0x9100_07bd; // add x29, x29, #1
    const LOAD: u32 = 0xf940_02e0; // ldr x0, [x23]
    const LOAD29: u32 = 0xf940_03a0; // ldr x0, [x29]
    const AWAY: u32 = 0xaa18_03fd; // mov x29, x24
    const BACK: u32 = 0xaa1d_03f8; // mov x24, x29
    const OWN: u32 = 0xf861_4b00; // ldr x0, [x24, w1, uxtw]
    const FRAME: u32 = 0xf940_0780; // ldr x0, [x28, #8]
    const SP_CUT: u32 = 0x8b2f_431f; // add sp, x24, w15, uxtw
    const SP_FROM_CUT: u32 = 0x9100_02ff; // mov sp, x23
    const STEP: u32 = 0xd100_43ff; // sub sp, sp, #16
    const PROBE: u32 = 0xb900_03ff; // str wzr, [sp]
    const STACK: u32 = 0xf940_07e0; // ldr x0, [sp, #8]
    const SAME: u32 = 0x9100_02f7; // mov x23, x23
    let exit = exit_words(table(false) + (GUEST + 4) * 4);
    let exit_displaced = exit_words(table(true) + (GUEST + 4) * 4);
    let cat = |parts: &[&[u32]]| parts.concat();
    let bne = |by: i64| 0x5400_0001 | (((by / 4) as u32) & 0x7_ffff) << 5;
    let mark = |c: u32, bb: u32| vec![0xd260_0000 | bb << 5 | c, 0x397f_fc1f | c << 5, 0];
    let at = |off: i64, word: u32| vec![movx_c(23, off), add_from_c(23, 31), word];
    let mut all: Vec<Condition> = Vec::new();
    let mut case = |clause, words: Vec<u32>, displaced, described, made_up: Option<Step>| {
        all.push(Condition {
            clause,
            words,
            displaced,
            described,
            made_up,
        });
    };
    // In the window, then the exit.
    let mut w = |clause, words: &[u32], described| {
        case(clause, cat(&[words, &exit]), false, described, None);
    };
    w(
        "Register: reg_ok(rd, bb): not the base",
        &[0x9100_0718],
        false,
    );
    w(
        "Register: reg_ok(rd, bb): not the frame",
        &[0x9100_079c],
        false,
    );
    w(
        "Register: reg_ok(rd, bb), the base moved",
        &[AWAY, SPOIL29, BACK],
        false,
    );
    w(
        "Register: reg_ok(rd, bb), the base moved: x24 free",
        &[AWAY, 0x9100_0718, BACK],
        true,
    );
    w("Register: sp ==> a.displaced", &[0x9100_001f], false);
    w(
        "Register: after_cut(cut, rd): lost",
        &[CUT, SPOIL, LOAD],
        false,
    );
    w(
        "Register: after_cut(cut, rd): kept",
        &[CUT, ADD, LOAD],
        true,
    );
    w(
        "Cut: 0 <= n <= 31, the zero register adds nothing",
        &[add_cut_into(31, 24, 23), LOAD],
        true,
    );
    w("Cut: c != bb", &[add_cut_into(1, 24, 24)], false);
    w(
        "Cut: c != frame_register()",
        &[add_cut_into(1, 24, 28)],
        false,
    );
    w(
        "Cut: the word adds to the base",
        &[add_cut_into(1, 0, 23), LOAD],
        false,
    );
    w(
        "Cut: the word adds wN unshifted",
        &[0x8b21_4b17, LOAD],
        false,
    );
    w(
        "Cut: the word adds to the base, moved",
        &[AWAY, add_cut_into(1, 29, 23), LOAD, BACK],
        true,
    );
    w(
        "Cut: the word adds to the base, moved: not x24",
        &[AWAY, CUT, LOAD, BACK],
        false,
    );
    w("Cut: accepts(j + 1, bb, c)", &[CUT29, LOAD], false);
    w("Cut: accepts(j + 1, bb, c): c", &[CUT29, LOAD29], true);
    w("Either: j + by / 4 < len", &[bne(64)], false);
    w("Either: 0 <= j + by / 4", &[bne(-4)], false);
    w(
        "Either: accepts(j + 1, bb, cut)",
        &[CUT, bne(8), LOAD, ADD],
        true,
    );
    w(
        "Either: accepts(j + 1, bb, cut): both ways",
        &[CUT, bne(8), SPOIL, LOAD],
        false,
    );
    w(
        "Either: accepts(j + by / 4, bb, cut)",
        &[CUT, bne(8), ADD, LOAD],
        true,
    );
    w(
        "Either: accepts(j + by / 4, bb, cut): no cut there",
        &[bne(8), CUT, LOAD],
        false,
    );
    w("Either: no link (bl)", &[0x9400_0002, ADD], false);
    w("Branch: j + by / 4 < len", &[b_by(64)], false);
    w("Branch: 0 <= j + by / 4", &[b_by(-4)], false);
    w(
        "Branch: accepts(j + by / 4, bb, cut)",
        &[CUT, b_by(8), SPOIL, LOAD],
        true,
    );
    w(
        "Branch: accepts(j + by / 4, bb, cut): no cut there",
        &[b_by(8), CUT, LOAD],
        false,
    );
    w("Through: 0 <= cut < 31", &[LOAD], false);
    w(
        "Through: cut != bb (the base moved onto the cut)",
        &[CUT29, AWAY, LOAD29, BACK],
        false,
    );
    w(
        "Through: reg_ok(rt, bb): not the base",
        &[CUT, 0xf940_02f8],
        false,
    );
    w(
        "Through: reg_ok(rt, bb): not the frame",
        &[CUT, 0xf940_02fc],
        false,
    );
    w(
        "Through: reg_ok(rt2, bb): not the base",
        &[CUT, 0xa940_62e0],
        false,
    );
    w(
        "Through: reg_ok(rt2, bb): not the frame",
        &[CUT, 0xa940_72e0],
        false,
    );
    w(
        "Through: reg_ok(rt, bb), the base moved",
        &[AWAY, add_cut_into(1, 29, 23), 0xf940_02fd, BACK],
        false,
    );
    w(
        "Through: reg_ok(rt, bb), the base moved: x24 free",
        &[AWAY, add_cut_into(1, 29, 23), 0xf940_02f8, BACK],
        true,
    );
    w("Through: one place (dc zva)", &[CUT, 0xd50b_7437], false);
    w(
        "Through: off + bytes <= 0x1_0000, at the edge",
        &[CUT, 0x3dff_fee0],
        true,
    );
    w(
        "Through: through_cut, written back",
        &[CUT, 0xf840_86e0, LOAD],
        false,
    );
    w(
        "Through: through_cut, rt over the cut",
        &[CUT, 0xf940_02f7, LOAD],
        false,
    );
    w(
        "Through: through_cut, rt2 over the cut",
        &[CUT, 0xa940_5ee0, LOAD],
        false,
    );
    w(
        "Through: through_cut, kept",
        &[CUT, LOAD, 0xf940_06e1],
        true,
    );
    w("Through: a constant offset", &[CUT, 0xf861_6ae0], false);
    w(
        "Through: a constant offset: xzr adds nothing",
        &[CUT, 0xf87f_6ae0],
        true,
    );
    w("Through: the base is the cut", &[CUT29, LOAD], false);
    w("Own: described", &[OWN], true);
    w("Own: the base is bb", &[0xf861_4800], false);
    w(
        "Own: the base is bb, moved",
        &[AWAY, 0xf861_4ba0, BACK],
        true,
    );
    w(
        "Own: the base is bb, moved: not x24",
        &[AWAY, OWN, BACK],
        false,
    );
    w("Own: 0 <= n < 31", &[0xf87f_4b00], false);
    w("Own: wN uxtw (sxtw)", &[0xf861_cb00], false);
    w("Own: wN uxtw (x1)", &[0xf861_6b00], false);
    w("Own: wN uxtw unshifted", &[0xf861_5b00], false);
    w("Own: reg_ok(rt, bb): not the base", &[0xf861_4b18], false);
    w("Own: reg_ok(rt, bb): not the frame", &[0xf861_4b1c], false);
    w("Own: accepts(j + 1, bb, 31)", &[CUT, OWN, LOAD], false);
    w(
        "Probes: described",
        &cat(&[&[CUT], &probes_words(22, 23, 8, 8, false)]),
        true,
    );
    w(
        "Probes: described, a store",
        &cat(&[&[CUT], &probes_words(22, 23, 8, 8, true)]),
        true,
    );
    w(
        "Probes: described, from sp",
        &probes_words(22, 31, 8, 8, false),
        true,
    );
    w(
        "Probes: c != bb",
        &cat(&[&[CUT], &probes_words(24, 23, 8, 8, false)]),
        false,
    );
    w(
        "Probes: c != from",
        &cat(&[&[CUT], &probes_words(23, 23, 8, 8, false)]),
        false,
    );
    w(
        "Probes: c != frame_register()",
        &cat(&[&[CUT], &probes_words(28, 23, 8, 8, false)]),
        false,
    );
    w(
        "Probes: needs_cut(from, cut): not the cut",
        &cat(&[&[CUT], &probes_words(22, 0, 8, 8, false)]),
        false,
    );
    w(
        "Probes: needs_cut(from, cut): no cut",
        &probes_words(22, 23, 8, 8, false),
        false,
    );
    w(
        "Probes: -0x400 <= off",
        &cat(&[&[CUT], &probes_words(22, 23, -0x401, 8, false)]),
        false,
    );
    w(
        "Probes: -0x400 <= off, at the edge",
        &cat(&[&[CUT], &probes_words(22, 23, -0x400, 8, false)]),
        true,
    );
    w(
        "Probes: off + bytes <= 0x1_0000",
        &cat(&[&[CUT], &probes_words(22, 23, 0xfff9, 8, false)]),
        false,
    );
    w(
        "Probes: off + bytes <= 0x1_0000, at the edge",
        &cat(&[&[CUT], &probes_words(22, 23, 0xfff8, 8, false)]),
        true,
    );
    w(
        "Probes: the store's words",
        &{
            let mut p = probes_words(22, 23, 8, 8, true);
            p[4] = 0x383f_001f | 21 << 5;
            cat(&[&[CUT], &p])
        },
        false,
    );
    w(
        "Probes: accepts(j + length, bb, from): from sp, no cut after",
        &cat(&[&[CUT], &probes_words(22, 31, 8, 8, false), &[LOAD]]),
        false,
    );
    w(
        "Probes: accepts(j + length, bb, from): the cut after",
        &cat(&[&[CUT], &probes_words(22, 23, 8, 8, false), &[LOAD]]),
        true,
    );
    w("Exit: iface bb == 24", &[AWAY], false);
    w(
        "Call: iface true, any base",
        &cat(&[&[AWAY], &floor_call(0)]),
        true,
    );
    w("Frame: base_ok(bb2): not the frame", &[0xaa18_03fc], false);
    w(
        "Frame: base_ok(bb2): x23 is not a base, so a register",
        &[0xaa18_03f7],
        true,
    );
    w("Frame: a move from bb only", &[0xaa00_03fd], true);
    w("Frame: the move and back", &[AWAY, BACK], true);
    w("Frame: f.rt != bb", &[0xf940_0798], false);
    w("Frame: f.rt != frame", &[0xf940_079c], false);
    w("Frame: a load", &[FRAME], true);
    w("Frame: a store", &[0xf900_0780], true);
    w("Frame: a store of the base", &[0xf900_0798], true);
    w("Frame: the last word", &[0xf940_7f80], true);
    w("Frame: past the last word", &[0xf940_8380], false);
    w("Frame: 8 bytes", &[0xb940_0b80], false);
    w("Frame: no negative offset", &[0xf85f_8380], false);
    w("Frame: not moved", &[0xf840_8780], false);
    w("Frame: one register", &[0xa940_0780], false);
    w(
        "Frame: after_cut(cut, f.rt)",
        &[CUT, 0xf940_0797, LOAD],
        false,
    );
    w(
        "Frame: after_cut(cut, f.rt): kept",
        &[CUT, FRAME, LOAD],
        true,
    );
    w(
        "Frame: the move keeps the cut",
        &[CUT, AWAY, LOAD, BACK],
        true,
    );
    w("StackCut: described", &[SP_CUT], true);
    w("StackCut: 0 <= n < 31", &[add_cut_into(31, 24, 31)], false);
    w("StackCut: the base", &[0x8b2f_403f], false);
    w(
        "StackCut: the base, moved",
        &[AWAY, add_cut_into(15, 29, 31), BACK],
        true,
    );
    w(
        "StackCut: the base, moved: not x24",
        &[AWAY, SP_CUT, BACK],
        false,
    );
    w(
        "StackCut: accepts(j + 1, bb, cut): kept",
        &[CUT, SP_CUT, LOAD],
        true,
    );
    w(
        "StackCut: accepts(j + 1, bb, cut): sp is no cut",
        &[SP_CUT, LOAD],
        false,
    );
    w("StackMove: described", &[CUT, SP_FROM_CUT, LOAD], true);
    w("StackMove: 0 <= cut < 31", &[SP_FROM_CUT], false);
    w("StackMove: from the cut", &[CUT, 0x9100_001f], false);
    w("StackStep: described", &[STEP, PROBE], true);
    w("StackStep: k != 0", &[0x9100_03ff, PROBE], false);
    w("StackStep: -0xf000 <= k", &[0xd140_3fff, PROBE], true);
    w(
        "StackStep: -0xf000 <= k, past",
        &[0xd140_43ff, PROBE],
        false,
    );
    w("StackStep: k <= 0xf000", &[0x9140_3fff, PROBE], true);
    w("StackStep: k <= 0xf000, past", &[0x9140_43ff, PROBE], false);
    w("StackStep: off <= 0x400", &[STEP, 0xb904_03ff], true);
    w("StackStep: off <= 0x400, past", &[STEP, 0xb904_07ff], false);
    w(
        "StackStep: -0x400 <= off (stur -256)",
        &[STEP, 0xb810_03ff],
        true,
    );
    w(
        "StackStep: the probe is 4 bytes",
        &[STEP, 0xf900_03ff],
        false,
    );
    w(
        "StackStep: the probe loads nothing",
        &[STEP, 0xb940_03e0],
        false,
    );
    w(
        "StackStep: the probe is not moved",
        &[STEP, 0xb800_47ff],
        false,
    );
    w("StackStep: a probe", &[STEP], false);
    w(
        "StackStep: accepts(j + 2, bb, cut)",
        &[CUT, STEP, PROBE, LOAD],
        true,
    );
    w("StackAccess: described", &[STACK], true);
    w(
        "StackAccess: reg_ok(rt, bb): not the base",
        &[0xf940_03f8],
        false,
    );
    w(
        "StackAccess: reg_ok(rt, bb): not the frame",
        &[0xf940_07fc],
        false,
    );
    w(
        "StackAccess: reg_ok(rt2, bb): not the base",
        &[0xa940_63e0],
        false,
    );
    w(
        "StackAccess: reg_ok(rt, bb) of a pair: not the base",
        &[0xa940_03f8],
        false,
    );
    w(
        "StackAccess: -0x400 <= off, at the edge",
        &[0xad60_07e0],
        true,
    );
    w(
        "StackAccess: off + bytes <= 0x1_0000, at the edge",
        &[0x3dff_ffe0],
        true,
    );
    w("StackAccess: moves, pre-index", &[0xade0_07e0], true);
    w("StackAccess: moves, post-index", &[0xacdf_87e0], true);
    w("StackAccess: a constant offset", &[0xf861_6be0], false);
    w(
        "StackAccess: accepts(j + 1, bb, 31)",
        &[CUT, STACK, LOAD],
        false,
    );
    w("StackVia: described", &[STACK_TO_X23, LOAD], true);
    w(
        "StackVia: described, the cut kept",
        &[CUT29, STACK_TO_X23, LOAD, LOAD29],
        true,
    );
    w("StackVia: cut != 23", &[CUT, STACK_TO_X23, LOAD], false);
    w(
        "StackVia: reg_ok(rt, bb): not the base",
        &[STACK_TO_X23, 0xf940_02f8],
        false,
    );
    w(
        "StackVia: reg_ok(rt, bb): not the frame",
        &[STACK_TO_X23, 0xf940_02fc],
        false,
    );
    w(
        "StackVia: reg_ok(rt2, bb): not the frame",
        &[STACK_TO_X23, 0xa940_72e0],
        false,
    );
    w(
        "StackVia: rt != cut",
        &[CUT29, STACK_TO_X23, 0xf940_02fd],
        false,
    );
    w(
        "StackVia: rt2 != cut",
        &[CUT29, STACK_TO_X23, 0xa940_76e0],
        false,
    );
    w(
        "StackVia: at x23 itself",
        &[STACK_TO_X23, 0xf940_06e0],
        false,
    );
    w(
        "StackVia: accepts(j + 2, bb, cut): x23 no cut after",
        &[STACK_TO_X23, LOAD, LOAD],
        false,
    );
    w(
        "StackProbes: described",
        &stack_probes_words(22, 8, false),
        true,
    );
    w(
        "StackProbes: described, a store",
        &stack_probes_words(22, 8, true),
        true,
    );
    w(
        "StackProbes: c != bb",
        &stack_probes_words(24, 8, false),
        false,
    );
    w(
        "StackProbes: c != frame_register()",
        &stack_probes_words(28, 8, false),
        false,
    );
    w(
        "StackProbes: the words",
        &{
            let mut p = stack_probes_words(22, 8, false);
            p[2] = 0x3940_1ebf;
            p
        },
        false,
    );
    w(
        "StackProbes: accepts(j + length, bb, 31)",
        &cat(&[&[CUT], &stack_probes_words(22, 8, false), &[LOAD]]),
        false,
    );
    w(
        "ShiftThrough by 0, mov xc, xc: described",
        &[CUT, SAME, LOAD],
        true,
    );
    w(
        "ShiftThrough by 0, mov xc, xc: 0 <= cut < 31",
        &[SAME, LOAD],
        false,
    );
    w(
        "ShiftThrough by 0, mov xc, xc: of the cut",
        &[CUT29, SAME, LOAD],
        false,
    );
    w(
        "ShiftThrough by 0, mov xc, xc: another register's is a register",
        &[CUT, 0x9100_02d6, LOAD],
        true,
    );
    w("StackAt: described", &at(8, LOAD), true);
    w("StackAt: cut != 23", &cat(&[&[CUT], &at(8, LOAD)]), false);
    w("StackAt: reg_ok(rt, bb)", &at(8, 0xf940_02f8), false);
    w("StackAt: reg_ok(rt2, bb)", &at(8, 0xa940_72e0), false);
    w(
        "StackAt: rt != cut",
        &cat(&[&[CUT29], &at(8, 0xf940_02fd)]),
        false,
    );
    w(
        "StackAt: rt2 != cut",
        &cat(&[&[CUT29], &at(8, 0xa940_76e0)]),
        false,
    );
    w("StackAt: -0x400 <= off", &at(-0x401, LOAD), false);
    w(
        "StackAt: -0x400 <= off, at the edge",
        &at(-0x400, LOAD),
        true,
    );
    w("StackAt: off + bytes <= 0x1_0000", &at(0xfff9, LOAD), false);
    w(
        "StackAt: off + bytes <= 0x1_0000, at the edge",
        &at(0xfff8, LOAD),
        true,
    );
    w("StackAt: at x23 itself", &at(8, 0xf940_06e0), false);
    w("StackAt: not moved", &at(8, 0xf840_86e0), false);
    w(
        "StackAt: the words (add x23, sp, x23)",
        &[movx_c(23, 8), add_from_c(23, 0), LOAD],
        false,
    );
    w(
        "StackAt: accepts(j + 3, bb, cut): kept",
        &cat(&[&[CUT29], &at(8, LOAD), &[LOAD29]]),
        true,
    );
    w(
        "StackAt: accepts(j + 3, bb, cut): x23 no cut after",
        &[at(8, LOAD), vec![LOAD]].concat(),
        false,
    );
    const LINK: [u32; 3] = [0xd282_009e, 0xf2a0_081e, 0xf2c0_001e]; // mov x30, #0x401004
    let tail = |first: u32| vec![first, BFXIL_X23, EOR_X23_TABLE, BR_X23];
    let linked = |first: u32| cat(&[&[first, BFXIL_X23, EOR_X23_TABLE], &LINK, &[BR_X23]]);
    w(
        "StackMove: 0 <= cut < 31, mov sp, sp",
        &[0x9100_03ff],
        false,
    );
    w(
        "StackProbes: 0 <= c < 31",
        &[0x9100_03ff, 0x3940_03ff, 0x3940_1fff],
        false,
    );
    w(
        "ShiftThrough: described, sub",
        &[CUT, shift_word(23, -312), 0xfd40_02e1],
        true,
    );
    w(
        "ShiftThrough: described, add",
        &[CUT, shift_word(23, 496), LOAD],
        true,
    );
    w(
        "ShiftThrough: a 12-bit immediate (#1, lsl #12 is not shift_word's)",
        &[CUT, 0x9140_06f7, LOAD],
        false,
    );
    w(
        "ShiftThrough: 0 <= cut < 31",
        &[shift_word(23, 8), LOAD],
        false,
    );
    w(
        "ShiftThrough: the cut moved, not another",
        &[CUT, shift_word(22, 8), 0xf940_02c0],
        false,
    );
    w(
        "ShiftThrough: the cut moved, from itself",
        &[CUT, 0x9100_22d7, LOAD],
        false,
    );
    w(
        "ShiftThrough: reg_ok(rt, bb)",
        &[CUT, shift_word(23, 8), 0xf940_02f8],
        false,
    );
    w(
        "ShiftThrough: reg_ok(rt2, bb)",
        &[CUT, shift_word(23, 8), 0xa940_72e0],
        false,
    );
    w(
        "ShiftThrough: at offset 0",
        &[CUT, shift_word(23, 8), 0xf940_06e1],
        false,
    );
    w(
        "ShiftThrough: not moved",
        &[CUT, shift_word(23, 8), 0xf840_86e0],
        false,
    );
    w(
        "ShiftThrough: accepts(j + 2, bb, 31)",
        &[CUT, shift_word(23, 8), LOAD, LOAD],
        false,
    );
    w(
        "ShiftThrough: the next word is the access",
        &[CUT, shift_word(23, 8), ADD, LOAD],
        false,
    );
    w(
        "Cut: the zero register, then the table jump",
        &cat(&[&[add_cut_into(31, 24, 23)], &tail(UBFIZ_X23)]),
        true,
    );
    // The piece's last words, with no exit after them: a block that goes on must have a word to go on to.
    let mut last = |clause, words: &[u32]| case(clause, words.to_vec(), false, false, None);
    last("Register: j + 1 < len", &[ADD]);
    last("Cut: j + 1 < len", &[CUT]);
    last("Either: j + 1 < len", &[ADD, bne(-4)]);
    last("Through: j + 1 < len", &[CUT, LOAD]);
    last("Own: j + 1 < len", &[OWN]);
    last(
        "Probes: j + length < len",
        &cat(&[&[CUT], &probes_words(22, 23, 8, 8, false)]),
    );
    last(
        "Probes: j + length < len, a store",
        &cat(&[&[CUT], &probes_words(22, 23, 8, 8, true)]),
    );
    last("Frame: j + 1 < len", &[FRAME]);
    last("Frame: j + 1 < len, a move", &[AWAY]);
    last("StackCut: j + 1 < len", &[SP_CUT]);
    last("StackMove: j + 1 < len", &[CUT, SP_FROM_CUT]);
    last("StackStep: j + 2 < len", &[STEP, PROBE]);
    last("StackAccess: j + 1 < len", &[STACK]);
    last("StackVia: j + 2 < len", &[STACK_TO_X23, LOAD]);
    last(
        "StackProbes: j + length < len",
        &stack_probes_words(22, 8, false),
    );
    last(
        "StackProbes: j + length < len, a store",
        &stack_probes_words(22, 8, true),
    );
    last("ShiftThrough by 0, mov xc, xc: j + 1 < len", &[CUT, SAME]);
    last("StackAt: j + 3 < len", &at(8, LOAD));
    last("ShiftThrough: j + 2 < len", &[CUT, shift_word(23, 8), LOAD]);
    // Whole pieces: the mark, the stop, the exits, the jumps and the floor calls, which go on to no word of theirs.
    let mut whole = |clause, words: Vec<u32>, displaced, described| {
        case(clause, words, displaced, described, None)
    };
    whole("Mark: described", mark(23, 24), false, true);
    whole("Mark: c != bb", mark(24, 24), false, false);
    whole("Mark: c != frame_register()", mark(28, 24), false, false);
    whole(
        "Mark: the load's offset",
        vec![0xd260_0017 | 24 << 5, 0x397f_f81f | 23 << 5, 0],
        false,
        false,
    );
    whole("Mark: from the base", mark(23, 0), false, false);
    whole(
        "Mark: from the base, moved",
        cat(&[&[AWAY], &mark(23, 29)]),
        false,
        true,
    );
    whole(
        "Mark: from the base, moved: not x24",
        cat(&[&[AWAY], &mark(23, 24)]),
        false,
        false,
    );
    whole("Mark: the udf", mark(23, 24)[..2].to_vec(), false, false);
    whole(
        "Mark: 0 <= c < 31",
        vec![0xd260_031f, 0x397f_ffff, 0],
        false,
        false,
    );
    whole("Stop: described", vec![0xd420_00a0], false, true);
    whole("Exit: described", exit.to_vec(), false, true);
    whole(
        "Exit: a slot of the table",
        exit_words(table(false) - 16).to_vec(),
        false,
        false,
    );
    whole(
        "Exit: 0 <= guest",
        exit_words(table(false) + 3 * 4).to_vec(),
        false,
        false,
    );
    whole(
        "Exit: a slot is 4 bytes",
        exit_words(table(false) + (GUEST + 4) * 4 + 2).to_vec(),
        false,
        false,
    );
    whole(
        "Exit: guest + 4 < 2^32",
        exit_words(table(false) + (0xffff_fffc + 4) * 4).to_vec(),
        false,
        false,
    );
    whole(
        "Exit: guest + 4 < 2^32, at the edge",
        exit_words(table(false) + (0xffff_fffb + 4) * 4).to_vec(),
        false,
        true,
    );
    whole(
        "Exit: the table of the way, window",
        exit.to_vec(),
        true,
        false,
    );
    whole(
        "Exit: the table of the way, displaced",
        exit_displaced.to_vec(),
        false,
        false,
    );
    whole("Call: described", floor_call(0).to_vec(), false, true);
    whole(
        "Call: code < 256, at the edge",
        floor_call(255).to_vec(),
        false,
        true,
    );
    whole("Call: code < 256", floor_call(256).to_vec(), false, false);
    whole(
        "Call: floor_entry(code)",
        floor_call_at(FLOOR_PAGE + 8).to_vec(),
        false,
        false,
    );
    // The translator's whole jumps: described as a register word, the branch, the cut (or none displaced) and the
    // table jump.
    whole(
        "Jump: described",
        cat(&[&jump_words(1, 24), &exit]),
        false,
        true,
    );
    whole(
        "Jump: the zero register",
        cat(&[&jump_words(31, 24), &exit]),
        false,
        true,
    );
    whole(
        "Jump: a cbnz to the next word",
        cat(&[&jump_words(1, 0), &exit]),
        false,
        true,
    );
    whole(
        "Jump: the cut of another register",
        {
            let mut j = jump_words(1, 24);
            j[2] = 0x8b22_4317;
            cat(&[&j, &exit])
        },
        false,
        true,
    );
    whole(
        "Jump: in displaced code",
        cat(&[&jump_words(1, 24), &exit_displaced]),
        true,
        false,
    );
    whole(
        "Jump: the cbnz out of the piece",
        cat(&[&jump_words(1, 40), &exit]),
        false,
        false,
    );
    whole(
        "Jump: the cbnz's target without the cut",
        cat(&[&[CUT], &jump_words(1, 24), &[LOAD], &exit]),
        false,
        false,
    );
    whole(
        "Jump: the base moved",
        cat(&[&[AWAY], &jump_words(1, 24), &[BACK], &exit]),
        false,
        false,
    );
    whole(
        "DisplacedJump: described",
        cat(&[&displaced_jump_words(1, 20), &exit_displaced]),
        true,
        true,
    );
    whole(
        "DisplacedJump: the zero register",
        cat(&[&displaced_jump_words(31, 20), &exit_displaced]),
        true,
        true,
    );
    whole(
        "DisplacedJump: a cbnz to the next word",
        cat(&[&displaced_jump_words(1, 0), &exit_displaced]),
        true,
        true,
    );
    whole(
        "DisplacedJump: in the window",
        cat(&[&displaced_jump_words(1, 20), &exit]),
        false,
        false,
    );
    whole(
        "DisplacedJump: the cbnz out of the piece",
        cat(&[&displaced_jump_words(1, 36), &exit_displaced]),
        true,
        false,
    );
    whole(
        "DisplacedJump: the cbnz's target without the cut",
        cat(&[
            &[CUT],
            &displaced_jump_words(1, 20),
            &[LOAD],
            &exit_displaced,
        ]),
        true,
        false,
    );
    whole(
        "DisplacedJump: the base moved",
        cat(&[
            &[AWAY],
            &displaced_jump_words(1, 20),
            &[BACK],
            &exit_displaced,
        ]),
        true,
        false,
    );
    whole(
        "TableJump: described",
        cat(&[&[CUT], &tail(UBFIZ_X23)]),
        false,
        true,
    );
    whole(
        "TableJump: described, link",
        cat(&[&[CUT], &linked(UBFIZ_X23)]),
        false,
        true,
    );
    whole(
        "TableJump: described, after a frame load and its cut",
        cat(&[&[0xf940_0b97, add_cut_into(23, 24, 23)], &tail(UBFIZ_X23)]),
        false,
        true,
    );
    whole(
        "TableJump: !a.displaced",
        cat(&[&[CUT], &tail(UBFIZ_X23)]),
        true,
        false,
    );
    whole("TableJump: iface cut == 23", tail(UBFIZ_X23), false, false);
    whole(
        "TableJump: iface cut == 23, not x29",
        cat(&[&[CUT29], &tail(UBFIZ_X23)]),
        false,
        false,
    );
    whole(
        "TableJump: iface bb == 24",
        cat(&[&[AWAY, add_cut_into(1, 29, 23)], &tail(UBFIZ_X23)]),
        false,
        false,
    );
    whole(
        "TableJump: the words",
        vec![CUT, UBFIZ_X23, EOR_X23_TABLE, BFXIL_X23, BR_X23],
        false,
        false,
    );
    whole(
        "TableJump: br x23",
        vec![CUT, UBFIZ_X23, BFXIL_X23, EOR_X23_TABLE, 0xd61f_0040],
        false,
        false,
    );
    whole(
        "TableJump: j + 3 + link < len",
        vec![CUT, UBFIZ_X23, BFXIL_X23, EOR_X23_TABLE],
        false,
        false,
    );
    whole(
        "TableJump: the link words write x30 alone",
        vec![
            CUT,
            UBFIZ_X23,
            BFXIL_X23,
            EOR_X23_TABLE,
            LINK[0],
            LINK[1],
            ADD,
            BR_X23,
        ],
        false,
        false,
    );
    whole(
        "TableJump: three link words",
        vec![
            CUT,
            UBFIZ_X23,
            BFXIL_X23,
            EOR_X23_TABLE,
            LINK[0],
            LINK[1],
            BR_X23,
        ],
        false,
        false,
    );
    whole(
        "TableJump: br x23 after the link words",
        vec![
            CUT,
            UBFIZ_X23,
            BFXIL_X23,
            EOR_X23_TABLE,
            LINK[0],
            LINK[1],
            LINK[2],
            0xd61f_0040,
        ],
        false,
        false,
    );
    whole(
        "DisplacedTableJump: described",
        tail(ubfiz_x23(1)),
        true,
        true,
    );
    whole(
        "DisplacedTableJump: described, the zero register",
        tail(ubfiz_x23(31)),
        true,
        true,
    );
    whole(
        "DisplacedTableJump: described, link",
        linked(ubfiz_x23(1)),
        true,
        true,
    );
    whole(
        "DisplacedTableJump: a.displaced",
        tail(ubfiz_x23(1)),
        false,
        false,
    );
    whole(
        "DisplacedTableJump: iface bb == 24",
        cat(&[&[AWAY], &tail(ubfiz_x23(1))]),
        true,
        false,
    );
    whole(
        "DisplacedTableJump: the words",
        vec![ubfiz_x23(1), EOR_X23_TABLE, BFXIL_X23, BR_X23],
        true,
        false,
    );
    whole(
        "DisplacedTableJump: the ubfiz",
        tail(0xd37e_7c37 ^ 0x400),
        true,
        false,
    );
    whole(
        "DisplacedTableJump: j + 3 + link < len",
        vec![ubfiz_x23(1), BFXIL_X23, EOR_X23_TABLE],
        true,
        false,
    );
    whole(
        "DisplacedTableJump: br x23",
        vec![ubfiz_x23(1), BFXIL_X23, EOR_X23_TABLE, 0xd61f_0040],
        true,
        false,
    );
    whole(
        "DisplacedTableJump: the link words write x30 alone",
        vec![
            ubfiz_x23(1),
            BFXIL_X23,
            EOR_X23_TABLE,
            LINK[0],
            ADD,
            LINK[2],
            BR_X23,
        ],
        true,
        false,
    );
    whole(
        "ShiftThrough: described, displaced",
        cat(&[&[CUT, shift_word(23, -328), 0xfd40_02f6], &exit_displaced]),
        true,
        true,
    );
    // Displaced: what only the window allows.
    let d = |words: &[u32]| cat(&[words, &exit_displaced]);
    whole(
        "Register: sp ==> a.displaced: displaced",
        d(&[0x9100_001f]),
        true,
        true,
    );
    whole("StackCut: described, displaced", d(&[SP_CUT]), true, true);
    whole(
        "Probes: from == 31 ==> !a.displaced",
        d(&probes_words(22, 31, 8, 8, false)),
        true,
        false,
    );
    whole("StackStep: !a.displaced", d(&[STEP, PROBE]), true, false);
    whole("StackAccess: !a.displaced", d(&[STACK]), true, false);
    whole(
        "StackVia: !a.displaced",
        d(&[STACK_TO_X23, LOAD]),
        true,
        false,
    );
    whole(
        "StackProbes: !a.displaced",
        d(&stack_probes_words(22, 8, false)),
        true,
        false,
    );
    whole("StackAt: !a.displaced", d(&at(8, LOAD)), true, false);
    // iface_ok: the word reached first with what its block needs (the branch's target), then without.
    let mut twice = |clause, before: &[u32], spoil: u32, block: &[u32]| {
        let words = cat(&[before, &[bne(8)], &[spoil], block, &exit]);
        case(clause, words, false, false, None);
    };
    twice("Register: iface b == bb", &[], AWAY, &[ADD]);
    twice(
        "Register: iface needs_cut(c, cut)",
        &[CUT],
        SPOIL,
        &[ADD, LOAD],
    );
    twice("Cut: iface b == bb", &[], AWAY, &[CUT]);
    twice("Either: iface b == bb", &[], AWAY, &[bne(4)]);
    twice(
        "Either: iface needs_cut(c, cut)",
        &[CUT],
        SPOIL,
        &[bne(4), LOAD],
    );
    twice("Branch: iface b == bb", &[], AWAY, &[b_by(4)]);
    twice(
        "Branch: iface needs_cut(c, cut)",
        &[CUT],
        SPOIL,
        &[b_by(4), LOAD],
    );
    twice("Through: iface b == bb", &[CUT], AWAY, &[LOAD]);
    twice("Through: iface c == cut", &[CUT], SPOIL, &[LOAD]);
    twice("Own: iface b == bb", &[], AWAY, &[OWN]);
    twice("Mark: iface b == bb", &[], AWAY, &mark(23, 24));
    twice(
        "Probes: iface b == bb",
        &[CUT],
        AWAY,
        &probes_words(22, 23, 8, 8, false),
    );
    twice(
        "Probes: iface needs_cut(from, cut)",
        &[CUT],
        SPOIL,
        &probes_words(22, 23, 8, 8, false),
    );
    twice("Exit: iface bb == 24, reached twice", &[], AWAY, &[]);
    twice(
        "Jump: iface bb == 24, reached twice",
        &[],
        AWAY,
        &jump_words(1, 24),
    );
    twice("Frame: iface b == bb", &[], AWAY, &[FRAME]);
    twice(
        "Frame: iface needs_cut(c, cut)",
        &[CUT],
        SPOIL,
        &[FRAME, LOAD],
    );
    twice("StackCut: iface b == bb", &[], AWAY, &[SP_CUT]);
    twice(
        "StackCut: iface needs_cut(c, cut)",
        &[CUT],
        SPOIL,
        &[SP_CUT, LOAD],
    );
    twice("StackMove: iface b == bb", &[CUT], AWAY, &[SP_FROM_CUT]);
    twice("StackMove: iface c == cut", &[CUT], SPOIL, &[SP_FROM_CUT]);
    twice("StackStep: iface b == bb", &[], AWAY, &[STEP, PROBE]);
    twice(
        "StackStep: iface needs_cut(c, cut)",
        &[CUT],
        SPOIL,
        &[STEP, PROBE, LOAD],
    );
    twice("StackAccess: iface b == bb", &[], AWAY, &[STACK]);
    twice("StackVia: iface b == bb", &[], AWAY, &[STACK_TO_X23, LOAD]);
    twice(
        "StackVia: iface needs_cut(c, cut)",
        &[CUT29],
        SPOIL29,
        &[STACK_TO_X23, LOAD, LOAD29],
    );
    twice(
        "StackProbes: iface b == bb",
        &[],
        AWAY,
        &stack_probes_words(22, 8, false),
    );
    twice(
        "ShiftThrough by 0, mov xc, xc: iface b == bb",
        &[CUT],
        AWAY,
        &[SAME],
    );
    twice(
        "ShiftThrough by 0, mov xc, xc: iface c == cut",
        &[CUT],
        SPOIL,
        &[SAME, LOAD],
    );
    twice("StackAt: iface b == bb", &[], AWAY, &at(8, LOAD));
    twice(
        "StackAt: iface needs_cut(c, cut)",
        &[CUT29],
        SPOIL29,
        &[at(8, LOAD), vec![LOAD29]].concat(),
    );
    twice(
        "TableJump: iface cut == 23, reached twice",
        &[CUT],
        SPOIL,
        &tail(UBFIZ_X23),
    );
    twice(
        "ShiftThrough: iface c == cut",
        &[CUT],
        SPOIL,
        &[shift_word(23, 8), LOAD],
    );
    twice(
        "TableJump: iface bb == 24, reached twice",
        &[CUT],
        AWAY,
        &tail(UBFIZ_X23),
    );
    twice(
        "ShiftThrough: iface b == bb",
        &[CUT],
        AWAY,
        &[shift_word(23, 8), LOAD],
    );
    let words = cat(&[&[bne(8)], &[AWAY], &tail(ubfiz_x23(1))]);
    case(
        "DisplacedTableJump: iface bb == 24, reached twice",
        words,
        true,
        false,
        None,
    );
    // The displaced jump reached twice, displaced.
    let words = cat(&[
        &[bne(8)],
        &[AWAY],
        &displaced_jump_words(1, 20),
        &exit_displaced,
    ]);
    case(
        "DisplacedJump: iface bb == 24, reached twice",
        words,
        true,
        false,
        None,
    );
    // Bounds no word decode.rs knows reaches: [`MADE_UP`] is given that meaning.
    let mut made_up = |clause, before: &[u32], step: Step, described| {
        case(
            clause,
            cat(&[before, &[MADE_UP], &exit]),
            false,
            described,
            Some(step),
        );
    };
    let x = Reg::X;
    made_up(
        "Register: one register (rd)",
        &[],
        Step::Plain(vec![(x(0), Made::Unknown), (x(1), Made::Unknown)]),
        false,
    );
    made_up(
        "Through: -0x1_0000 <= off",
        &[CUT],
        access(x(23), -0x1_0001, 1, &[], None),
        false,
    );
    made_up(
        "Through: -0x1_0000 <= off, at the edge",
        &[CUT],
        access(x(23), -0x1_0000, 1, &[], None),
        true,
    );
    made_up(
        "Through: 1 <= bytes",
        &[CUT],
        access(x(23), 0, 0, &[], None),
        false,
    );
    made_up(
        "Through: off + bytes <= 0x1_0000",
        &[CUT],
        access(x(23), 0xfff1, 16, &[], None),
        false,
    );
    made_up(
        "Own: 1 <= bytes <= 16",
        &[],
        Step::Access(Access {
            bytes: 17,
            ..own_access()
        }),
        false,
    );
    made_up(
        "Own: 1 <= bytes <= 16, at the edge",
        &[],
        Step::Access(Access {
            bytes: 16,
            ..own_access()
        }),
        true,
    );
    made_up(
        "Own: 1 <= bytes",
        &[],
        Step::Access(Access {
            bytes: 0,
            ..own_access()
        }),
        false,
    );
    made_up(
        "Own: reg_ok(rt2, bb)",
        &[],
        Step::Access(Access {
            loads: vec![(x(0), Made::Unknown), (x(24), Made::Unknown)],
            ..own_access()
        }),
        false,
    );
    made_up(
        "Own: not written back",
        &[],
        Step::Access(Access {
            moved: Some(Moved::By(8)),
            ..own_access()
        }),
        false,
    );
    made_up(
        "Probes (StackStep's probe): -0x400 <= off",
        &[STEP],
        access(Reg::Sp, -0x404, 4, &[], None),
        false,
    );
    made_up(
        "Probes (StackStep's probe): -0x400 <= off, at the edge",
        &[STEP],
        access(Reg::Sp, -0x400, 4, &[], None),
        true,
    );
    made_up(
        "StackStep: the probe at one place",
        &[STEP],
        Step::Access(Access {
            offset: Offset::Constant(0, 4),
            ..match access(Reg::Sp, 0, 4, &[], None) {
                Step::Access(a) => a,
                _ => unreachable!(),
            }
        }),
        false,
    );
    made_up(
        "StackAccess: -0x400 <= off",
        &[],
        access(Reg::Sp, -0x410, 16, &[], None),
        false,
    );
    made_up(
        "StackAccess: 1 <= bytes",
        &[],
        access(Reg::Sp, 0, 0, &[], None),
        false,
    );
    made_up(
        "StackAccess: off + bytes <= 0x1_0000",
        &[],
        access(Reg::Sp, 0xfff1, 16, &[], None),
        false,
    );
    made_up(
        "StackAccess: -0x400 <= moves",
        &[],
        access(Reg::Sp, 0, 16, &[], Some(Moved::By(-0x410))),
        false,
    );
    made_up(
        "StackAccess: moves <= 0x400",
        &[],
        access(Reg::Sp, 0, 16, &[], Some(Moved::By(0x410))),
        false,
    );
    made_up(
        "StackAccess: moves <= 0x400, at the edge",
        &[],
        access(Reg::Sp, 0, 16, &[], Some(Moved::By(0x400))),
        true,
    );
    made_up(
        "StackAccess: moves != 0 ==> off == 0 || off == moves",
        &[],
        access(Reg::Sp, 8, 8, &[], Some(Moved::By(16))),
        false,
    );
    made_up(
        "StackAccess: moved by a register",
        &[],
        access(
            Reg::Sp,
            0,
            16,
            &[],
            Some(Moved::ByRegister(Source::Reg(x(3)))),
        ),
        false,
    );
    made_up(
        "StackAccess: one place",
        &[],
        Step::Access(Access {
            offset: Offset::Constant(0, 8),
            ..match access(Reg::Sp, 0, 8, &[], None) {
                Step::Access(a) => a,
                _ => unreachable!(),
            }
        }),
        false,
    );
    made_up(
        "StackVia: 1 <= bytes <= 0x1_0000",
        &[STACK_TO_X23],
        access(x(23), 0, 0x1_0001, &[], None),
        false,
    );
    made_up(
        "StackVia: 1 <= bytes <= 0x1_0000, at the edge",
        &[STACK_TO_X23],
        access(x(23), 0, 0x1_0000, &[], None),
        true,
    );
    made_up(
        "StackVia: 1 <= bytes",
        &[STACK_TO_X23],
        access(x(23), 0, 0, &[], None),
        false,
    );
    made_up(
        "StackAt: 1 <= bytes",
        &[movx_c(23, 8), add_from_c(23, 31)],
        access(x(23), 0, 0, &[], None),
        false,
    );
    made_up(
        "ShiftThrough: k + bytes <= 0x1_0000",
        &[CUT, shift_word(23, 8)],
        access(x(23), 0, 0xfff9, &[], None),
        false,
    );
    made_up(
        "ShiftThrough: k + bytes <= 0x1_0000, at the edge",
        &[CUT, shift_word(23, 8)],
        access(x(23), 0, 0xfff8, &[], None),
        true,
    );
    made_up(
        "ShiftThrough: 1 <= bytes",
        &[CUT, shift_word(23, 8)],
        access(x(23), 0, 0, &[], None),
        false,
    );
    made_up(
        "ShiftThrough: loads two registers at most",
        &[CUT, shift_word(23, 8)],
        Step::Access(Access {
            loads: vec![
                (x(0), Made::Unknown),
                (x(1), Made::Unknown),
                (x(2), Made::Unknown),
            ],
            ..match access(x(23), 0, 8, &[], None) {
                Step::Access(a) => a,
                _ => unreachable!(),
            }
        }),
        false,
    );
    case(
        "TableJump: a link word writing x30 and sp",
        vec![
            CUT,
            UBFIZ_X23,
            BFXIL_X23,
            EOR_X23_TABLE,
            LINK[0],
            LINK[1],
            MADE_UP,
            BR_X23,
        ],
        false,
        false,
        Some(Step::Plain(vec![
            (x(30), Made::Unknown),
            (Reg::Sp, Made::Unknown),
        ])),
    );
    let mut wrong = Vec::new();
    for c in &all {
        let made_up = c.made_up.clone();
        let decode_word = move |word: u32| match &made_up {
            Some(step) if word == MADE_UP => step.clone(),
            _ => decode(word),
        };
        let got = describe_with(&c.words, c.displaced, &decode_word);
        if got.is_ok() != c.described {
            wrong.push(format!("{}: {got:?}", c.clause));
        }
    }
    assert!(
        wrong.is_empty(),
        "described where compose.rs says otherwise:\n{}",
        wrong.join("\n")
    );
    println!("conditions: {}", all.len());
}

/// `ldr x0, [x24, w1, uxtw]` as decode.rs gives it.
fn own_access() -> Access {
    match decode(0xf861_4b00) {
        Step::Access(a) => a,
        other => panic!("{other:?}"),
    }
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
    // Measured 2026-11-28 (seventh run): every piece is described. Each figure rests on the meaning of each word, which
    // is a hypothesis taken from decode.rs (compose.rs `all_meant`): `step` is still free.
    let measured = counts.map(|c| (c.forms, c.forms_described, c.pieces, c.described));
    assert_eq!(
        measured,
        [(4429, 4429, 35432, 35432), (4429, 4429, 35432, 35432)]
    );
}
