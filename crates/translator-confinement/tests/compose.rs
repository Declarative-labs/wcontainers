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
use wcontainers_translator_confinement::{Base, Extend, Offset, Reg, Source, Step, decode};

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

/// compose.rs `Desc`, its `FrameAccess` inline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Desc {
    Register {
        j: i64,
        rd: i64,
        bb: i64,
        cut: bool,
    },
    Cut {
        j: i64,
        n: i64,
        bb: i64,
    },
    Either {
        j: i64,
        by: i64,
        bb: i64,
        cut: bool,
    },
    Branch {
        j: i64,
        by: i64,
        bb: i64,
        cut: bool,
    },
    Through {
        j: i64,
        rt: i64,
        back: bool,
        off: i64,
        bytes: i64,
        bb: i64,
    },
    Own {
        j: i64,
        n: i64,
        rt: i64,
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
    },
}

impl Desc {
    /// compose.rs `iface_ok`.
    fn takes(&self, base: i64, cut: bool) -> bool {
        match *self {
            Desc::Register { bb, cut: c, .. }
            | Desc::Either { bb, cut: c, .. }
            | Desc::Branch { bb, cut: c, .. } => bb == base && (!c || cut),
            Desc::Cut { bb, .. }
            | Desc::Own { bb, .. }
            | Desc::Mark { bb, .. }
            | Desc::Frame { bb, .. } => bb == base,
            Desc::Through { bb, .. } | Desc::Probes { bb, .. } => bb == base && cut,
            Desc::Exit { .. } | Desc::Call { .. } => true,
            Desc::Jump { .. } => base == 24,
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
            | Desc::Frame { j, .. } => j,
        }
    }

    /// The Verus expression.
    fn verus(&self) -> String {
        match *self {
            Desc::Register { j, rd, bb, cut } => {
                format!("Desc::Register {{ j: {j}, rd: {rd}, bb: {bb}, cut: {cut} }}")
            }
            Desc::Cut { j, n, bb } => format!("Desc::Cut {{ j: {j}, n: {n}, bb: {bb} }}"),
            Desc::Either { j, by, bb, cut } => {
                format!("Desc::Either {{ j: {j}, by: {by}, bb: {bb}, cut: {cut} }}")
            }
            Desc::Branch { j, by, bb, cut } => {
                format!("Desc::Branch {{ j: {j}, by: {by}, bb: {bb}, cut: {cut} }}")
            }
            Desc::Through {
                j,
                rt,
                back,
                off,
                bytes,
                bb,
            } => {
                format!(
                    "Desc::Through {{ j: {j}, rt: {rt}, back: {back}, off: {off}, bytes: {bytes}, bb: {bb} }}"
                )
            }
            Desc::Own {
                j,
                n,
                rt,
                bytes,
                bb,
            } => format!("Desc::Own {{ j: {j}, n: {n}, rt: {rt}, bytes: {bytes}, bb: {bb} }}"),
            Desc::Mark { j, bb, c } => format!("Desc::Mark {{ j: {j}, bb: {bb}, c: {c} }}"),
            Desc::Probes {
                j,
                c,
                off,
                bytes,
                store,
                bb,
            } => format!(
                "Desc::Probes {{ j: {j}, c: {c}, off: {off}, bytes: {bytes}, store: {store}, bb: {bb} }}"
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
            } => format!(
                "Desc::Frame {{ j: {j}, f: FrameAccess {{ word: {word:#010x}u32, rt: {rt}, off: {off}, moves: {moves} }}, bb: {bb}, bb2: {bb2} }}"
            ),
        }
    }
}

/// compose.rs `base_ok`.
fn base_ok(bb: i64) -> bool {
    (0..31).contains(&bb) && bb != 23 && bb != 28
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

fn add_cut_by(n: i64, bb: i64) -> u32 {
    0x8b20_4017 | (n as u32) << 16 | (bb as u32) << 5
}

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
fn probes_words(c: i64, off: i64, bytes: i64, store: bool) -> Vec<u32> {
    let c = c as u32;
    let k = ((bytes - 1) as u32) << 10;
    let mut w = vec![
        0xd280_0000 | (off as u32) << 5 | c,
        0x8b20_62e0 | c << 16 | c,
    ];
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

/// The one general register `word` loads (31 for none), if it loads at most one and not the stack pointer.
fn one_loaded(loads: &[(Reg, wcontainers_translator_confinement::Made)]) -> Option<i64> {
    match loads {
        [] => Some(31),
        [(Reg::X(r), _)] => Some(i64::from(*r)),
        _ => None,
    }
}

/// Finds the description of `words` (compose.rs `described`): the blocks entered from the first word with the base in
/// x24, each place a block goes on to described in turn. `Err` names the first word no block kind describes.
fn describe(words: &[u32], displaced: bool) -> Result<Vec<Desc>, String> {
    let len = words.len() as i64;
    let mut descs: Vec<Desc> = Vec::new();
    let mut todo: Vec<(i64, i64, bool)> = vec![(0, 24, false)];
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
        let mut next = |d: Desc, goes: &[(i64, i64, bool)]| {
            descs.push(d);
            todo.extend_from_slice(goes);
        };
        // The exit.
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
                &[(out, 24, false)],
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
        // The probes.
        let c = i64::from(word & 31);
        if cut
            && word & 0xffe0_0000 == 0xd280_0000
            && (0..31).contains(&c)
            && c != bb
            && c != 23
            && c != 28
            && let (Some(w2), Some(w3)) = (at(j + 2), at(j + 3))
        {
            let off = i64::from((word >> 5) & 0xffff);
            let store = w2 == 0x383f_001f | (c as u32) << 5;
            let bytes = i64::from((w3 >> 10) & 0xfff) + 1;
            let length = if store { 5 } else { 4 };
            if off + bytes <= 0x1_0000
                && j + length < len
                && holds(j, &probes_words(c, off, bytes, store))
            {
                next(
                    Desc::Probes {
                        j,
                        c,
                        off,
                        bytes,
                        store,
                        bb,
                    },
                    &[(j + length, bb, true)],
                );
                continue;
            }
        }
        // The cut.
        let n = i64::from((word >> 16) & 31);
        if j + 1 < len && n < 31 && word == add_cut_by(n, bb) {
            next(Desc::Cut { j, n, bb }, &[(j + 1, bb, true)]);
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
                };
                next(d, &[(j + 1, t, false)]);
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
            && let Some(rt) = one_loaded(&access.loads)
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
                },
                &[(j + 1, bb, false)],
            );
            continue;
        }
        match step {
            Step::Plain(writes) if j + 1 < len => {
                // The stack pointer is not general register: compose.rs's `keeps_but` lets it change.
                let general: Vec<u8> = writes
                    .iter()
                    .filter_map(|w| match w.0 {
                        Reg::X(r) => Some(r),
                        Reg::Sp => None,
                    })
                    .collect();
                let rd = match general.as_slice() {
                    [] => 31,
                    [r] => i64::from(*r),
                    _ => return Err(format!("word {j} {word:#010x}: writes {writes:?}")),
                };
                if rd == bb || rd == 28 || (cut && rd == 23) {
                    return Err(format!("word {j} {word:#010x}: writes x{rd}"));
                }
                next(Desc::Register { j, rd, bb, cut }, &[(j + 1, bb, cut)]);
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
            Step::Access(access) if j + 1 < len => {
                let Some(rt) = one_loaded(&access.loads) else {
                    return Err(format!("word {j} {word:#010x}: loads {:?}", access.loads));
                };
                if rt == bb || rt == 28 {
                    return Err(format!("word {j} {word:#010x}: loads x{rt}"));
                }
                let bytes = access.bytes as i64;
                match (access.base, access.offset, access.moved) {
                    (Base::Reg(Source::Reg(Reg::X(23))), Offset::Constant(off, high), moved)
                        if cut && off == high && off >= 0 && off + bytes <= 0x1_0000 =>
                    {
                        let back = moved.is_some();
                        next(
                            Desc::Through {
                                j,
                                rt,
                                back,
                                off,
                                bytes,
                                bb,
                            },
                            &[(j + 1, bb, false)],
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
                                bytes,
                                bb,
                            },
                            &[(j + 1, bb, false)],
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
/// a line ending in `br x30`; stand-ins for x24 in a line; offsets of 4096 and more.
fn pinned() -> Vec<(&'static str, Vec<u32>, LookAhead, bool)> {
    let tested = |a: u32| vec![a, 0xf100_0400, 0x54ff_ffc1, 0xd61f_03c0];
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
    // `mov sp, x0`: described, since no block needs the stack pointer (compose.rs `part`).
    assert!(describe(&with_exit(&[0x9100_001f]), false).is_ok());
    // The cut, then a load through x23: described; with an offset of 64 KiB: not.
    assert!(describe(&with_exit(&[add_cut_by(1, 24), 0xf940_02e0]), false).is_ok());
    assert!(
        describe(
            &with_exit(&[add_cut_by(1, 24), 0x3940_02e0 | 0xfff << 10]),
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
    // Measured 2026-11-27 (fifth run): the rest are the stack's accesses, pairs, and probes outside these blocks.
    let measured = counts.map(|c| (c.forms, c.forms_described, c.pieces, c.described));
    assert_eq!(
        measured,
        [(4429, 4205, 35432, 34632), (4429, 4274, 35432, 34685)]
    );
}
