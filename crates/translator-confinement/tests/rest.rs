//! The pieces of `statement/rest.rs` and `statement/withheld_access.rs` are the ones the translator emits.
//!
//! - Each pin of those files (a `pin_*` line: the piece's function and arguments, and the words Verus computes it
//!   to) is, byte for byte, what the translator emits for the guest instruction this file names for it.
//! - The same shapes, written here from the proofs' functions, are what it emits for every register, offset and slot
//!   swept below, and every load and store naming a withheld register is a stand-in piece whose parameters meet the
//!   proof's hypotheses.

use wcontainers_arm64::way::{Kept, SLOT_BITS};
use wcontainers_arm64::{Arm64, Way};
use wcontainers_translator::{LookAhead, NativeRequest, translate_instructions};

const REST: &str = include_str!("../statement/rest.rs");
const WITHHELD_ACCESS: &str = include_str!("../statement/withheld_access.rs");

/// The guest registers the translator names as they are: every one but the withheld.
const DIRECT: [u32; 26] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 20, 21, 22, 25, 26, 27, 29, 30,
];
const WITHHELD: [u32; 5] = [15, 18, 23, 24, 28];
const GUEST: u64 = 0x40_1000;

fn translated(word: u32, slot: u16, guest: u64, displaced: bool) -> Vec<u32> {
    let request = NativeRequest {
        look_ahead: LookAhead::None,
        exact_replay: true,
        stop: false,
        step: false,
    };
    let way = Way::InProcess {
        slot,
        displaced,
        kept: Kept::USUAL,
    };
    let code = translate_instructions::<Arm64>(&word.to_le_bytes(), guest, &request, &way, &())
        .unwrap_or_else(|| panic!("{word:#010x} translates"));
    code.bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w))
        .collect()
}

/// The pins of a file: each `pin_*` line's name and words.
fn pins(text: &str) -> Vec<(String, Vec<u32>)> {
    text.lines()
        .filter(|line| line.starts_with("proof fn pin_"))
        .map(|line| {
            let name = line["proof fn ".len()..].split_once('(').expect("a name").0;
            let words = line.split_once("=~= seq![").expect("the words").1;
            let words = words.split_once(']').expect("their end").0;
            let words = words
                .split(',')
                .map(|w| {
                    let hex = w.trim().trim_end_matches("u32").replace('_', "");
                    u32::from_str_radix(hex.trim_start_matches("0x"), 16).expect("a word")
                })
                .collect();
            (name.to_owned(), words)
        })
        .collect()
}

/// The guest instruction of each pin, at slot 3 and `GUEST`, and whether the way is displaced.
const GUESTS: [(&str, u32, bool); 15] = [
    // stp q0, q1, [sp], #-32 and ldp q0, q1, [sp], #32
    ("pin_stack_pair_post_store", 0xacbf_07e0, false),
    ("pin_stack_pair_post_load", 0xacc1_07e0, false),
    // ldrb w0, [sp] and strb w0, [sp]
    ("pin_displaced_byte_load", 0x3940_03e0, true),
    ("pin_displaced_byte_store", 0x3900_03e0, true),
    // ldr x0, [x1, x2], str x0, [x1, x2], ldr w0, [x1, x2]
    ("pin_register_unscaled_load", 0xf862_6820, false),
    ("pin_register_unscaled_store", 0xf822_6820, false),
    ("pin_register_unscaled_word_displaced", 0xb862_6820, true),
    // ldr x0, .+0xffc
    ("pin_literal_crossing", 0x5800_7fe0, false),
    // ldr x15, [x1] both ways, ldr x0, [x15], str x24, [x1, #8] both ways, str x0, [x28], ldrb w23, [x0]
    ("pin_wide_stand_in_load_x15", 0xf940_002f, false),
    ("pin_wide_stand_in_load_x15_displaced", 0xf940_002f, true),
    ("pin_wide_stand_in_load_through_x15", 0xf940_01e0, false),
    ("pin_wide_stand_in_store_x24", 0xf900_0438, false),
    ("pin_wide_stand_in_store_x24_displaced", 0xf900_0438, true),
    ("pin_wide_stand_in_store_through_x28", 0xf900_0380, false),
    ("pin_byte_stand_in_load_w23", 0x3940_0017, false),
];

#[test]
fn every_pin_of_the_proofs_is_what_the_translator_emits() {
    let all = [pins(REST), pins(WITHHELD_ACCESS)].concat();
    let names: Vec<&str> = all.iter().map(|(n, _)| n.as_str()).collect();
    let expected: Vec<&str> = GUESTS.iter().map(|g| g.0).collect();
    assert_eq!(names, expected);
    for ((name, words), (_, guest, displaced)) in all.iter().zip(GUESTS) {
        assert_eq!(&translated(guest, 3, GUEST, displaced), words, "{name}");
    }
}

/// The entry slot of guest address `guest` (the proofs' `exit_slot`).
fn exit_slot(slot: u16, guest: u64, displaced: bool) -> u64 {
    let table = if displaced {
        1 << 46
    } else {
        (1 << 46) + (u64::from(slot) << SLOT_BITS) * 4
    };
    table + guest * 4
}

fn exit_words(slot: u64) -> Vec<u32> {
    let half = |k: u32| (((slot >> (16 * k)) & 0xffff) as u32) << 5;
    vec![
        0xd280_0000 | half(0) | 23,
        0xf280_0000 | 1 << 21 | half(1) | 23,
        0xf280_0000 | 2 << 21 | half(2) | 23,
        0xd61f_02e0,
    ]
}

/// `b .-by`.
fn back(by: u32) -> u32 {
    0x1800_0000 - by / 4
}

/// rest.rs `stack_pair_post`.
fn stack_pair_post(word: u32, store: bool, slot: u64) -> Vec<u32> {
    let probes = if store {
        vec![0x383f_02ff, 0x9100_7ef7, 0x383f_02ff, back(52)]
    } else {
        vec![0x3940_02ff, 0x3940_7eff, back(48)]
    };
    [
        vec![0x9100_7ff7, 0x927b_1af7, 0xb400_00d7, word],
        exit_words(slot),
        vec![
            0x9100_03f7,
            0x9240_0ef7,
            0xb400_0057,
            0xd420_00a0,
            0x9100_03f7,
        ],
        probes,
    ]
    .concat()
}

/// A floor call to call `code`'s entry (branch.rs `floor_call`).
fn floor_call(code: u64) -> Vec<u32> {
    let entry = (1u64 << 45) + (1 << 32) + 16 * code;
    let half = |k: u32| (((entry >> (16 * k)) & 0xffff) as u32) << 5;
    vec![
        0xf900_2f9e,
        0xd280_001e | half(0),
        0xf280_001e | 1 << 21 | half(1),
        0xf280_001e | 2 << 21 | half(2),
        0xd63f_03c0,
    ]
}

/// rest.rs `displaced_byte`.
fn displaced_byte(t: u32, store: bool, slot: u64) -> Vec<u32> {
    let cut = (if store { 0x3820_4b00 } else { 0x3860_4b00 }) | 23 << 16 | t;
    [
        vec![
            0x9100_03f7,
            0x9240_0ef7,
            0xb500_0017 | 9 << 5,
            0x9100_03f7,
            0xd360_deef,
            0xb500_000f | 7 << 5,
            cut,
        ],
        exit_words(slot),
        vec![0x1400_0004, 0xd260_030f, 0x397f_fdff, 0],
        floor_call(6),
        vec![0xf940_2f9e, back(36)],
    ]
    .concat()
}

/// rest.rs `register_unscaled`: `bytes` is 4 or 8.
fn register_unscaled(address: u32, access: u32, bytes: u32, store: bool, slot: u64) -> Vec<u32> {
    let k = bytes - 1;
    let and_page = if bytes == 8 { 0x927d_21ef } else { 0x927e_25ef };
    let probes = if store {
        vec![0x383f_01ff, 0x9100_01ef | k << 10, 0x383f_01ff, back(52)]
    } else {
        vec![0x3940_01ff, 0x3940_01ff | k << 10, back(48)]
    };
    [
        vec![
            address,
            0xd360_deef,
            0xb500_014f,
            0x8b37_4317,
            0x9100_02ef | k << 10,
            and_page,
            0xb400_012f,
            access,
        ],
        exit_words(slot),
        vec![0xd260_030f, 0x397f_fdff, 0, 0xd280_000f, 0x8b2f_62ef],
        probes,
    ]
    .concat()
}

/// rest.rs `literal_crossing`.
fn literal_crossing(label: u64, load: u32, bytes: u32, slot: u64) -> Vec<u32> {
    let half = |k: u32| (((label >> (16 * k)) & 0xffff) as u32) << 5;
    [
        vec![
            0xd280_0000 | half(0) | 23,
            0xf280_0000 | 1 << 21 | half(1) | 23,
            0xf280_0000 | 2 << 21 | half(2) | 23,
            0x8b37_4317,
            0x3940_02ff,
            0x3940_02ff | (bytes - 1) << 10,
            load,
        ],
        exit_words(slot),
    ]
    .concat()
}

#[test]
fn every_post_indexed_pair_through_the_stack_pointer_is_the_proof_s_piece() {
    let mut checked = 0;
    for imm7 in 0..128u32 {
        for store in [false, true] {
            // stp/ldp qT, qT2, [sp], #imm (opc 10, post-index): T 0, T2 1.
            let word =
                (if store { 0xac80_0000 } else { 0xacc0_0000 }) | imm7 << 15 | 1 << 10 | 31 << 5;
            for slot in [1, 3, 2047] {
                assert_eq!(
                    translated(word, slot, GUEST, false),
                    stack_pair_post(word, store, exit_slot(slot, GUEST + 4, false)),
                    "{word:#010x} slot {slot}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 128 * 2 * 3);
}

#[test]
fn every_displaced_byte_at_the_stack_pointer_is_the_proof_s_piece() {
    let mut checked = 0;
    for t in DIRECT.into_iter().chain([31]) {
        for store in [false, true] {
            let word = (if store { 0x3900_03e0 } else { 0x3940_03e0 }) | t;
            for slot in [1, 3, 2047] {
                assert_eq!(
                    translated(word, slot, GUEST, true),
                    displaced_byte(t, store, exit_slot(slot, GUEST + 4, true)),
                    "{word:#010x} slot {slot}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 27 * 2 * 3);
}

#[test]
fn every_unscaled_register_offset_access_is_the_proof_s_piece() {
    let mut checked = 0;
    // uxtw, lsl (uxtx), sxtw, sxtx, with S clear.
    for option in [2u32, 3, 6, 7] {
        for n in DIRECT {
            for m in [0, 9, 30] {
                for t in DIRECT.into_iter().chain([31]) {
                    for (bytes, size) in [(4u32, 0xb800_0000u32), (8, 0xf800_0000)] {
                        for store in [false, true] {
                            if !store && t == 31 {
                                continue;
                            }
                            let opc = if store { 0x0020_0800 } else { 0x0060_0800 };
                            let word = size | opc | m << 16 | option << 13 | n << 5 | t;
                            let address = 0x8b20_0017 | m << 16 | option << 13 | n << 5;
                            let access = (word & !(0x1f << 16 | 0x1f << 5)) | 31 << 16 | 23 << 5;
                            for displaced in [false, true] {
                                assert_eq!(
                                    translated(word, 3, GUEST, displaced),
                                    register_unscaled(
                                        address,
                                        access,
                                        bytes,
                                        store,
                                        exit_slot(3, GUEST + 4, displaced)
                                    ),
                                    "{word:#010x} displaced {displaced}"
                                );
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 4 * 26 * 3 * (27 * 2 + 26 * 2) * 2);
}

#[test]
fn every_doubleword_literal_that_crosses_a_page_is_the_proof_s_piece() {
    let mut checked = 0;
    for page in [0x40_1000u64, 0x40_2000, 0xffe_f000] {
        // From the page's first word to its last, the literal its last word's 8 bytes.
        for guest in [page, page + 0x800, page + 0xff8] {
            let label = page + 0xffc;
            for t in DIRECT {
                let word = 0x5800_0000 | (((label - guest) / 4) as u32) << 5 | t;
                for displaced in [false, true] {
                    assert_eq!(
                        translated(word, 3, guest, displaced),
                        literal_crossing(
                            label,
                            0xf940_02e0 | t,
                            8,
                            exit_slot(3, guest + 4, displaced)
                        ),
                        "{word:#010x} at {guest:#x} displaced {displaced}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 3 * 3 * 26 * 2);
}

/// A word around a stand-in's access (withheld.rs `FrameAccess`): `ldr/str xT, [x28, #off]` or `mov xT, xN`. Its
/// register written (31 for a store), its offset or source, whether it moves.
fn around(w: u32) -> Option<(u32, u32, bool)> {
    match w & 0xffc0_03e0 {
        0xf900_0380 => return Some((31, ((w >> 10) & 0xfff) * 8, false)),
        0xf940_0380 => return Some((w & 31, ((w >> 10) & 0xfff) * 8, false)),
        _ => {}
    }
    (w & 0xffe0_ffe0 == 0xaa00_03e0).then_some((w & 31, (w >> 16) & 31, true))
}

/// withheld.rs `frame_access_means`'s and withheld_access.rs `base_moved`'s hypotheses on the words before and after.
fn around_meets(words: &[u32]) -> bool {
    words.iter().all(|&w| match around(w) {
        Some((rt, off, false)) => rt != 28 && off + 8 <= 256,
        Some((rt, n, true)) => rt != 28 && rt < 31 && n < 31,
        None => false,
    })
}
fn base_moved(before: &[u32], bb: u32) -> bool {
    let rt = |w: u32| around(w).expect("a word around").0;
    if bb == 24 {
        return before.iter().all(|&w| rt(w) != 24);
    }
    let Some(j) = before
        .iter()
        .position(|&w| around(w) == Some((bb, 24, true)))
    else {
        return false;
    };
    before[..j].iter().all(|&w| rt(w) != 24 && rt(w) != bb)
        && before[j + 1..].iter().all(|&w| rt(w) != bb)
}
fn stand_in_registers(n: u32, t: u32, store: bool, bb: u32, c: u32) -> bool {
    n < 31
        && n != 23
        && n != c
        && bb < 31
        && ![23, 28, c].contains(&bb)
        && c < 31
        && ![23, 24, 28].contains(&c)
        && t < 32
        && (store || t != 28)
}

/// withheld_access.rs `byte_stand_in` and `wide_stand_in`.
/// The access is (N, T, whether it stores, the offset), the registers (bb, c).
type Access = (u32, u32, bool, u32);

fn byte_stand_in(
    before: &[u32],
    (n, t, store, _): Access,
    (bb, c): (u32, u32),
    after: &[u32],
    slot: u64,
) -> Vec<u32> {
    let al = after.len() as u32;
    let cut = (if store { 0x3820_4800 } else { 0x3860_4800 }) | n << 16 | bb << 5 | t;
    [
        before.to_vec(),
        vec![
            0xd360_dc00 | n << 5 | c,
            0xb500_0000 | (al + 6) << 5 | c,
            cut,
        ],
        after.to_vec(),
        exit_words(slot),
        vec![0xd260_0000 | bb << 5 | c, 0x397f_fc1f | c << 5, 0],
    ]
    .concat()
}
fn wide_stand_in(
    before: &[u32],
    (n, t, store, off): Access,
    (bb, c): (u32, u32),
    after: &[u32],
    slot: u64,
) -> Vec<u32> {
    let al = after.len() as u32;
    let cbnz = 0xb500_0000 | (al + 9) << 5 | c;
    let access = (if store { 0xf900_02e0 } else { 0xf940_02e0 }) | (off / 8) << 10 | t;
    let probes = if store {
        vec![
            0x383f_001f | c << 5,
            0x9100_1c00 | c << 5 | c,
            0x383f_001f | c << 5,
            back((al + 17) * 4),
        ]
    } else {
        vec![
            0x3940_001f | c << 5,
            0x3940_1c1f | c << 5,
            back((al + 16) * 4),
        ]
    };
    [
        before.to_vec(),
        vec![
            0xd360_dc00 | n << 5 | c,
            cbnz,
            0x8b20_4017 | n << 16 | bb << 5,
            0x9240_0800 | n << 5 | c,
            cbnz,
            access,
        ],
        after.to_vec(),
        exit_words(slot),
        vec![
            0xd260_0000 | bb << 5 | c,
            0x397f_fc1f | c << 5,
            0,
            0x9100_02e0 | (off + 7) << 10 | c,
            0x927d_2000 | c << 5 | c,
            0xb400_0040 | c,
            back((al + 11) * 4),
            0xd280_0000 | off << 5 | c,
            0x8b20_62e0 | c << 16 | c,
        ],
        probes,
    ]
    .concat()
}

/// The stand-in piece a load or store's words are, read from them, with its parameters checked against the proof's
/// hypotheses: the words before, the core (whose check gives N and c, whose cut gives bb, whose access gives T and the
/// offset), the words after, then the whole piece rebuilt from the proof's shape must equal the words.
fn is_stand_in(words: &[u32], wide: bool, store: bool, exit: u64) -> Result<(), String> {
    let b = words
        .iter()
        .position(|&w| around(w).is_none())
        .ok_or("no core")?;
    let (before, rest) = words.split_at(b);
    let check = rest[0];
    if check & 0xffff_fc00 != 0xd360_dc00 {
        return Err(format!("no check at {b}: {check:#010x}"));
    }
    let (n, c) = ((check >> 5) & 31, check & 31);
    let (bb, t, off, core) = if wide {
        let (cut, access) = (rest[2], rest[5]);
        (
            (cut >> 5) & 31,
            access & 31,
            ((access >> 10) & 0xfff) * 8,
            6,
        )
    } else {
        let cut = rest[2];
        ((cut >> 5) & 31, cut & 31, 0, 3)
    };
    let al = rest[core..]
        .iter()
        .position(|&w| around(w).is_none())
        .ok_or("no exit")?;
    let after = &rest[core..core + al];
    if !(around_meets(before) && around_meets(after)) {
        return Err("a word around breaks the hypotheses".into());
    }
    if !base_moved(before, bb) {
        return Err(format!("the base does not reach x{bb}"));
    }
    if !stand_in_registers(n, t, store, bb, c) || (wide && off + 7 >= 4096) {
        return Err(format!("registers n {n} t {t} bb {bb} c {c} off {off}"));
    }
    let shape = if wide {
        wide_stand_in(before, (n, t, store, off), (bb, c), after, exit)
    } else {
        byte_stand_in(before, (n, t, store, 0), (bb, c), after, exit)
    };
    if shape == words {
        Ok(())
    } else {
        Err(format!("not the shape: {shape:08x?}"))
    }
}

#[test]
fn every_load_and_store_naming_a_withheld_register_is_a_stand_in_piece() {
    let mut checked = 0;
    let others = [0, 9, 30];
    for withheld in WITHHELD {
        for other in others {
            for (n, t) in [(withheld, other), (other, withheld), (withheld, withheld)] {
                for store in [false, true] {
                    // ldrb/strb wT, [xN] and ldr/str xT, [xN, #off].
                    let mut cases = vec![(
                        false,
                        (if store { 0x3900_0000 } else { 0x3940_0000 }) | n << 5 | t,
                    )];
                    for off in [0u32, 8, 256, 4088] {
                        let word = (if store { 0xf900_0000 } else { 0xf940_0000 })
                            | (off / 8) << 10
                            | n << 5
                            | t;
                        cases.push((true, word));
                    }
                    for (wide, word) in cases {
                        for displaced in [false, true] {
                            let words = translated(word, 3, GUEST, displaced);
                            if let Err(why) =
                                is_stand_in(&words, wide, store, exit_slot(3, GUEST + 4, displaced))
                            {
                                panic!("{word:#010x} displaced {displaced}: {why}: {words:08x?}");
                            }
                            checked += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 5 * 3 * 3 * 2 * 5 * 2);
}

#[test]
fn a_plain_access_at_an_offset_is_the_stand_in_piece_with_nothing_around() {
    // The stand-in proof with no words around is the piece of an access naming no withheld register.
    let mut checked = 0;
    for n in DIRECT {
        for t in DIRECT.into_iter().chain([31]) {
            for store in [false, true] {
                if !store && t == 31 {
                    continue;
                }
                for off in [8u32, 4088] {
                    let word = (if store { 0xf900_0000 } else { 0xf940_0000 })
                        | (off / 8) << 10
                        | n << 5
                        | t;
                    for displaced in [false, true] {
                        assert_eq!(
                            translated(word, 3, GUEST, displaced),
                            wide_stand_in(
                                &[],
                                (n, t, store, off),
                                (24, 15),
                                &[],
                                exit_slot(3, GUEST + 4, displaced)
                            ),
                            "{word:#010x} displaced {displaced}"
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 26 * (27 + 26) * 2 * 2);
}

#[test]
fn a_literal_past_the_window_is_the_mark_and_never_a_load() {
    // A literal from 4 GiB up (within the 1 MiB a literal reaches) faults as the guest's load would: the piece starts
    // with the mark (carry.rs `outside_words`), never with a load cut into the window. What follows the mark (the way
    // to the next instruction) is never reached. A test that the window's top is 4 GiB and not 8.
    let mark = [0xd260_030f, 0x397f_fdff, 0];
    let mut checked = 0;
    for guest in [0xffff_f000u64, 0xfff0_0004, 0xffff_fffc] {
        for label in [0x1_0000_0000u64, 0x1_0000_0008, 0x1_0000_0ff8] {
            let Some(distance) = label.checked_sub(guest).filter(|&d| d < 1 << 20) else {
                continue;
            };
            for (opc, t) in [(0x5800_0000u32, 0u32), (0x1800_0000, 7), (0x9c00_0000, 1)] {
                let word = opc | ((distance / 4) as u32) << 5 | t;
                for displaced in [false, true] {
                    assert_eq!(
                        translated(word, 3, guest, displaced)[..3],
                        mark,
                        "{word:#010x} at {guest:#x} displaced {displaced}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 7 * 3 * 2);
}
