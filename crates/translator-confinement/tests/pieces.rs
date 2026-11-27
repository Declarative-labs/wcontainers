//! The pieces the Verus proofs are about (`statement/copy.rs`, `statement/access.rs`) are the ones the translator
//! emits, else the proofs would be about instructions nobody runs.
//!
//! - Each pin of the proofs (a `pin_*` line: the piece's function, its arguments, and the words Verus computes it to)
//!   is, byte for byte, what the translator emits for that guest instruction, window and address.
//! - The same shapes, written here from the proofs' functions, are what it emits for every guest register it names
//!   directly (all but the withheld x15, x18, x23, x24 and x28), both ways, and match every pin.

use wcontainers_arm64::way::{Kept, SLOT_BITS};
use wcontainers_arm64::{Arm64, Way};
use wcontainers_translator::{LookAhead, NativeRequest, translate_instructions};

const COPY: &str = include_str!("../statement/copy.rs");
const ACCESS: &str = include_str!("../statement/access.rs");
const INDEXED: &str = include_str!("../statement/indexed.rs");

/// The guest registers the translator names as they are: every one but the withheld.
const DIRECT: [u32; 26] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 20, 21, 22, 25, 26, 27, 29, 30,
];
const GUEST: u64 = 0x40_1000;

#[derive(Clone, Debug)]
struct Pin {
    function: String,
    arguments: Vec<String>,
    words: Vec<u32>,
}

/// An integer literal as Verus writes it: hex or decimal, `_` separated, with a type suffix, maybe negative.
fn number(token: &str) -> i64 {
    let token = token
        .trim()
        .trim_end_matches("u32")
        .trim_end_matches("u64")
        .trim_end_matches("int")
        .replace('_', "");
    let (sign, digits) = match token.strip_prefix('-') {
        Some(rest) => (-1, rest.to_owned()),
        None => (1, token),
    };
    sign * match digits.strip_prefix("0x") {
        Some(hex) => i64::from_str_radix(hex, 16).expect("a hex literal"),
        None => digits.parse::<i64>().expect("a decimal literal"),
    }
}

/// A literal that is a register number or a word.
fn word(token: &str) -> u32 {
    u32::try_from(number(token)).expect("a word")
}

fn flag(token: &str) -> bool {
    match token.trim() {
        "true" => true,
        "false" => false,
        other => panic!("not a bool: {other}"),
    }
}

fn pins(text: &str) -> Vec<Pin> {
    text.lines()
        .filter(|line| line.starts_with("proof fn pin_"))
        .map(|line| {
            let call = line.split_once("assert(").expect("an assertion").1;
            let (function, rest) = call.split_once('(').expect("a call");
            let (arguments, rest) = rest.split_once(')').expect("its arguments");
            let words = rest
                .split_once("seq![")
                .and_then(|(_, w)| w.split_once(']'))
                .expect("the words")
                .0;
            Pin {
                function: function.to_owned(),
                arguments: arguments.split(',').map(str::to_owned).collect(),
                words: words.split(',').map(word).collect(),
            }
        })
        .collect()
}

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

/// The guest instruction a pin's piece is of, and where: the word, the window's slot, the guest address, displaced.
fn guest_of(pin: &Pin) -> (u32, u16, u64, bool) {
    let a = &pin.arguments;
    let at = |k: usize| {
        let base = u64::try_from(number(&a[k])).expect("an address");
        assert_eq!(base % (1 << SLOT_BITS), 0, "a slot's base");
        (
            u16::try_from(base >> SLOT_BITS).expect("a slot"),
            u64::try_from(number(&a[k + 1])).expect("an address"),
            flag(&a[k + 2]),
        )
    };
    let word = match pin.function.as_str() {
        "copied_at" => {
            let (slot, guest, displaced) = at(1);
            return (word(&a[0]), slot, guest, displaced);
        }
        "byte_access_at" => byte_instruction(word(&a[0]), word(&a[1]), flag(&a[2])),
        "wide_access_at" => wide_instruction(word(&a[0]), word(&a[1]), flag(&a[2])),
        "indexed_access_at" => {
            let (slot, guest, displaced) = at(5);
            let instruction = indexed_instruction(
                word(&a[0]),
                word(&a[1]),
                flag(&a[2]),
                flag(&a[3]),
                number(&a[4]),
            );
            return (instruction, slot, guest, displaced);
        }
        other => panic!("no translator call for pins of {other}"),
    };
    let (slot, guest, displaced) = at(3);
    (word, slot, guest, displaced)
}

/// `ldrb/strb wT, [xN]` and `ldr/str xT, [xN]` (Arm ARM, load/store register, unsigned immediate, offset 0).
fn byte_instruction(n: u32, t: u32, store: bool) -> u32 {
    (if store { 0x3900_0000 } else { 0x3940_0000 }) | n << 5 | t
}
fn wide_instruction(n: u32, t: u32, store: bool) -> u32 {
    (if store { 0xf900_0000 } else { 0xf940_0000 }) | n << 5 | t
}
/// `ldr/str xT, [xN, #imm]!` and `ldr/str xT, [xN], #imm` (Arm ARM, load/store register, immediate pre- and
/// post-indexed).
fn indexed_instruction(n: u32, t: u32, store: bool, pre: bool, imm: i64) -> u32 {
    let imm9 = (imm & 0x1ff) as u32;
    (if store { 0xf800_0000 } else { 0xf840_0000 })
        | imm9 << 12
        | if pre { 0xc00 } else { 0x400 }
        | n << 5
        | t
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

/// The proofs' pieces, written from their functions (copy.rs `copied`, `exit_words`; access.rs `byte_access`,
/// `wide_access`).
fn exit_words(slot: u64) -> Vec<u32> {
    let half = |k: u32| (((slot >> (16 * k)) & 0xffff) as u32) << 5;
    vec![
        0xd280_0000 | half(0) | 23,
        0xf280_0000 | 1 << 21 | half(1) | 23,
        0xf280_0000 | 2 << 21 | half(2) | 23,
        0xd61f_02e0,
    ]
}
fn copied(word: u32, slot: u64) -> Vec<u32> {
    [vec![word], exit_words(slot)].concat()
}
fn byte_access(n: u32, t: u32, store: bool, slot: u64) -> Vec<u32> {
    let cut = (if store { 0x3820_4b00 } else { 0x3860_4b00 }) | n << 16 | t;
    [
        vec![0xd360_dc0f | n << 5, 0xb500_00cf, cut],
        exit_words(slot),
        vec![0xd260_030f, 0x397f_fdff, 0],
    ]
    .concat()
}
fn wide_access(n: u32, t: u32, store: bool, slot: u64) -> Vec<u32> {
    let access = (if store { 0xf900_02e0 } else { 0xf940_02e0 }) | t;
    let probes = if store {
        vec![0x383f_01ff, 0x9100_1def, 0x383f_01ff, 0x17ff_ffef]
    } else {
        vec![0x3940_01ff, 0x3940_1dff, 0x17ff_fff0]
    };
    [
        vec![
            0xd360_dc0f | n << 5,
            0xb500_012f,
            0x8b20_4317 | n << 16,
            0x9240_080f | n << 5,
            0xb500_012f,
            access,
        ],
        exit_words(slot),
        vec![
            0xd260_030f,
            0x397f_fdff,
            0,
            0x9100_1eef,
            0x927d_21ef,
            0xb400_004f,
            0x17ff_fff5,
            0xd280_000f,
            0x8b2f_62ef,
        ],
        probes,
    ]
    .concat()
}

fn indexed_access(n: u32, t: u32, store: bool, pre: bool, imm: i64, slot: u64) -> Vec<u32> {
    let off = if pre { imm } else { 0 };
    let new_base = if imm >= 0 {
        0x9100_000f | (imm as u32) << 10 | n << 5
    } else {
        0xd100_000f | ((-imm) as u32) << 10 | n << 5
    };
    let access = (indexed_instruction(n, t, store, pre, imm) & !(0x1f << 5)) | 23 << 5;
    let slow_start = 0x9100_02ef | ((off + 7).rem_euclid(4096) as u32) << 10;
    let move_offset = if off >= 0 {
        0xd280_000f | (off as u32) << 5
    } else {
        0x9280_000f | ((-off - 1) as u32) << 5
    };
    let probes = if store {
        vec![0x383f_01ff, 0x9100_1def, 0x383f_01ff, 0x17ff_ffed]
    } else {
        vec![0x3940_01ff, 0x3940_1dff, 0x17ff_ffee]
    };
    [
        vec![
            0xd360_dc0f | n << 5,
            0xb500_016f,
            0x8b20_4317 | n << 16,
            0x9240_080f | n << 5,
            0xb500_016f,
            new_base,
            access,
            0xaa0f_03e0 | n,
        ],
        exit_words(slot),
        vec![
            0xd260_030f,
            0x397f_fdff,
            0,
            slow_start,
            0x927d_21ef,
            0xb400_004f,
            0x17ff_fff3,
            move_offset,
            0x8b2f_62ef,
        ],
        probes,
    ]
    .concat()
}

/// The shape the proofs give a pin, from the functions above.
fn shape_of(pin: &Pin) -> Vec<u32> {
    let a = &pin.arguments;
    let (_, slot, guest, displaced) = guest_of(pin);
    let exit = exit_slot(slot, guest + 4, displaced);
    match pin.function.as_str() {
        "copied_at" => copied(word(&a[0]), exit),
        "byte_access_at" => byte_access(word(&a[0]), word(&a[1]), flag(&a[2]), exit),
        "wide_access_at" => wide_access(word(&a[0]), word(&a[1]), flag(&a[2]), exit),
        "indexed_access_at" => indexed_access(
            word(&a[0]),
            word(&a[1]),
            flag(&a[2]),
            flag(&a[3]),
            number(&a[4]),
            exit,
        ),
        other => panic!("no shape for pins of {other}"),
    }
}

fn all_pins() -> Vec<Pin> {
    [pins(COPY), pins(ACCESS), pins(INDEXED)].concat()
}

#[test]
fn every_pin_of_the_proofs_is_what_the_translator_emits() {
    let pins = all_pins();
    let names: Vec<&str> = pins.iter().map(|p| p.function.as_str()).collect();
    assert_eq!(
        names,
        [
            ["copied_at"; 2].as_slice(),
            &["byte_access_at"; 2],
            &["wide_access_at"; 2],
            &["indexed_access_at"; 6],
        ]
        .concat()
    );
    for pin in &pins {
        let (word, slot, guest, displaced) = guest_of(pin);
        assert_eq!(
            translated(word, slot, guest, displaced),
            pin.words,
            "{pin:?}"
        );
    }
}

#[test]
fn the_shapes_written_here_are_the_pinned_ones() {
    for pin in &all_pins() {
        assert_eq!(shape_of(pin), pin.words, "{pin:?}");
    }
}

#[test]
fn the_access_pieces_are_the_proofs_for_every_register_named_directly() {
    let mut checked = 0;
    for n in DIRECT {
        for t in DIRECT.into_iter().chain([31]) {
            for store in [false, true] {
                for displaced in [false, true] {
                    for slot in [1, 3, 2047] {
                        let exit = exit_slot(slot, GUEST + 4, displaced);
                        assert_eq!(
                            translated(byte_instruction(n, t, store), slot, GUEST, displaced),
                            byte_access(n, t, store, exit),
                            "byte n {n} t {t} store {store} displaced {displaced} slot {slot}"
                        );
                        assert_eq!(
                            translated(wide_instruction(n, t, store), slot, GUEST, displaced),
                            wide_access(n, t, store, exit),
                            "doubleword n {n} t {t} store {store} displaced {displaced} slot {slot}"
                        );
                        checked += 2;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 26 * 27 * 2 * 2 * 3 * 2);
}

#[test]
fn the_indexed_pieces_are_the_proof_s_for_every_register_named_directly() {
    // Post-index with any immediate; pre-index with one that keeps the base's alignment (the other pre-indexed shape,
    // which tests the page of the access itself, is not proven yet).
    let immediates = [
        (true, -256),
        (true, -16),
        (true, -8),
        (true, 0),
        (true, 8),
        (true, 248),
    ]
    .into_iter()
    .chain([-256, -3, 0, 1, 255].map(|imm| (false, imm)));
    let mut checked = 0;
    for (pre, imm) in immediates {
        for n in DIRECT {
            for t in DIRECT.into_iter().chain([31]) {
                for store in [false, true] {
                    for displaced in [false, true] {
                        let exit = exit_slot(3, GUEST + 4, displaced);
                        assert_eq!(
                            translated(
                                indexed_instruction(n, t, store, pre, imm),
                                3,
                                GUEST,
                                displaced
                            ),
                            indexed_access(n, t, store, pre, imm, exit),
                            "n {n} t {t} store {store} pre {pre} imm {imm} displaced {displaced}"
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 11 * 26 * 27 * 2 * 2);
}

#[test]
fn a_register_only_integer_instruction_s_piece_is_the_word_and_the_exit() {
    // `add xD, xN, xM`, `sub xD, xN, #1`, `csel xD, xN, xM, eq`, `madd xD, xN, xM, xN`, `eor wD, wN, wM, lsl #3`.
    let shapes: [fn(u32, u32, u32) -> u32; 5] = [
        |d, n, m| 0x8b00_0000 | m << 16 | n << 5 | d,
        |d, n, _| 0xd100_0400 | n << 5 | d,
        |d, n, m| 0x9a80_0000 | m << 16 | n << 5 | d,
        |d, n, m| 0x9b00_0000 | m << 16 | n << 10 | n << 5 | d,
        |d, n, m| 0x4a00_0c00 | m << 16 | n << 5 | d,
    ];
    for shape in shapes {
        for d in DIRECT {
            for n in DIRECT {
                for m in [0, 9, 30] {
                    for displaced in [false, true] {
                        let word = shape(d, n, m);
                        assert_eq!(
                            translated(word, 3, GUEST, displaced),
                            copied(word, exit_slot(3, GUEST + 4, displaced)),
                            "{word:#010x} displaced {displaced}"
                        );
                    }
                }
            }
        }
    }
}
