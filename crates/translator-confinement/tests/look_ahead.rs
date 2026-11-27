//! The look-ahead pieces of `statement/look_ahead.rs` are the ones the translator emits with bounded and full
//! look-ahead.
//!
//! - Each pin of that file is, byte for byte, what the translator emits for the guest code and look-ahead this file
//!   names for it.
//! - The same shapes, written here from the proofs' functions, are what it emits for lines, accesses followed by
//!   instructions of their part, counted loops and tested loops drawn over the registers named directly.

use wcontainers_arm64::way::{Kept, SLOT_BITS};
use wcontainers_arm64::{Arm64, Way};
use wcontainers_translator::{LookAhead, NativeRequest, translate_instructions};

const LOOK_AHEAD: &str = include_str!("../statement/look_ahead.rs");

/// The guest registers the translator names as they are: every one but the withheld.
const DIRECT: [u32; 26] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 20, 21, 22, 25, 26, 27, 29, 30,
];
const GUEST: u64 = 0x40_1000;
const SLOT: u16 = 3;

fn translated(code: &[u32], look_ahead: LookAhead, displaced: bool) -> Vec<u32> {
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
    let out = translate_instructions::<Arm64>(&bytes, GUEST, &request, &way, &())
        .unwrap_or_else(|| panic!("{code:08x?} translates"));
    out.bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|w| u32::from_le_bytes(*w))
        .collect()
}

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

/// `add x0, x0, #1; add x1, x1, #2; eor x2, x0, x1; br x30`.
const LINE: [u32; 4] = [0x9100_0400, 0x9100_0821, 0xca01_0002, 0xd61f_03c0];
/// `add x1, x1, #1; subs x0, x0, #1; b.ne .-8; br x30`.
const COUNTED: [u32; 4] = [0x9100_0421, 0xf100_0400, 0x54ff_ffc1, 0xd61f_03c0];
/// `ldr x2, [x1]; add x2, x2, #1; str x2, [x1, #8]; br x30`.
const LOAD_ADD: [u32; 4] = [0xf940_0022, 0x9100_0442, 0xf900_0422, 0xd61f_03c0];
/// `ldr x2, [x1]; subs x0, x0, #1; b.ne .-8; br x30`.
const TESTED: [u32; 4] = [0xf940_0022, 0xf100_0400, 0x54ff_ffc1, 0xd61f_03c0];

#[test]
fn every_pin_of_the_proofs_is_what_the_translator_emits() {
    let guests: [(&str, &[u32], LookAhead, bool); 8] = [
        ("pin_line_two_adds", &LINE, LookAhead::Bounded(2), false),
        ("pin_line_add_subs", &COUNTED, LookAhead::Bounded(2), false),
        ("pin_load_then_add", &LOAD_ADD, LookAhead::Bounded(2), false),
        ("pin_load_then_subs", &TESTED, LookAhead::Bounded(2), false),
        ("pin_counted_loop", &COUNTED, LookAhead::Full, false),
        (
            "pin_counted_loop_displaced",
            &COUNTED,
            LookAhead::Full,
            true,
        ),
        ("pin_tested_loop", &TESTED, LookAhead::Full, false),
        ("pin_tested_loop_displaced", &TESTED, LookAhead::Full, true),
    ];
    let all = pins(LOOK_AHEAD);
    let names: Vec<&str> = all.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, guests.map(|g| g.0));
    for ((name, words), (_, code, look_ahead, displaced)) in all.iter().zip(guests) {
        assert_eq!(&translated(code, look_ahead, displaced), words, "{name}");
    }
}

/// The entry slot of guest address `guest` (the proofs' `exit_slot`).
fn exit_slot(guest: u64, displaced: bool) -> u64 {
    let table = if displaced {
        1 << 46
    } else {
        (1 << 46) + (u64::from(SLOT) << SLOT_BITS) * 4
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

/// `b .-by`, and `b.ne .+by` (negative `by` back).
fn back(by: u32) -> u32 {
    0x1800_0000 - by / 4
}
fn b_ne(by: i64) -> u32 {
    0x5400_0001 | (((by / 4) as u32) & 0x7_ffff) << 5
}

const COUNT_UP: u32 = 0x9100_0652;

/// look_ahead.rs `line`.
fn line(words: &[u32], slot: u64) -> Vec<u32> {
    [words.to_vec(), exit_words(slot)].concat()
}

/// withheld_access.rs `wide_stand_in` with no words before, x24 the base and x15 the check, and register-only words
/// after: an access then the rest of its part.
fn load_store_then(n: u32, t: u32, store: bool, off: u32, after: &[u32], slot: u64) -> Vec<u32> {
    let al = after.len() as u32;
    let cbnz = 0xb500_000f | (al + 9) << 5;
    let access = (if store { 0xf900_02e0 } else { 0xf940_02e0 }) | (off / 8) << 10 | t;
    let probes = if store {
        vec![0x383f_01ff, 0x9100_1def, 0x383f_01ff, back((al + 17) * 4)]
    } else {
        vec![0x3940_01ff, 0x3940_1dff, back((al + 16) * 4)]
    };
    [
        vec![
            0xd360_dc0f | n << 5,
            cbnz,
            0x8b20_4317 | n << 16,
            0x9240_080f | n << 5,
            cbnz,
            access,
        ],
        after.to_vec(),
        exit_words(slot),
        vec![
            0xd260_030f,
            0x397f_fdff,
            0,
            0x9100_02ef | (off + 7) << 10,
            0x927d_21ef,
            0xb400_004f,
            back((al + 11) * 4),
            0xd280_000f | off << 5,
            0x8b2f_62ef,
        ],
        probes,
    ]
    .concat()
}

/// look_ahead.rs `counted_loop`.
fn counted_loop(words: &[u32], back_word: u32, slot: u64, head: u64) -> Vec<u32> {
    let k = words.len() as u32;
    [
        vec![COUNT_UP],
        words.to_vec(),
        vec![back_word],
        exit_words(slot),
        vec![back((k + 6) * 4)],
        exit_words(head)[1..].to_vec(),
    ]
    .concat()
}

/// look_ahead.rs `tested_loop`.
fn tested_loop(n: u32, t: u32, word: u32, slot: u64, head: u64) -> Vec<u32> {
    let link = exit_words(head)[1..].to_vec();
    [
        vec![0x9240_0817 | n << 5, 0xb400_03d7, COUNT_UP],
        vec![
            0xd360_dc0f | n << 5,
            0xb500_016f,
            0x8b20_4317 | n << 16,
            0x9240_080f | n << 5,
            0xb500_016f,
            0xf940_02e0 | t,
            word,
            b_ne(-40),
        ],
        exit_words(slot),
        vec![
            0xd260_030f,
            0x397f_fdff,
            0,
            0x9100_1eef,
            0x927d_21ef,
            0xb400_004f,
            back(52),
            0xd280_000f,
            0x8b2f_62ef,
            0x3940_01ff,
            0x3940_1dff,
            back(72),
            back(108),
        ],
        link.clone(),
        vec![
            COUNT_UP,
            0xd360_dc0f | n << 5,
            0xb500_010f,
            0xf860_4b00 | n << 16 | t,
            word,
            b_ne(-20),
        ],
        exit_words(slot),
        vec![0xd260_030f, 0x397f_fdff, 0, back(52)],
        link,
    ]
    .concat()
}

/// A register-only instruction (Arm ARM, add/subtract immediate, logical shifted register, move wide) writing `d`
/// from `n` and `m`, by `kind`.
fn integer(kind: u32, d: u32, n: u32, m: u32) -> u32 {
    match kind % 5 {
        0 => 0x9100_0000 | (m & 0xff) << 10 | n << 5 | d,
        1 => 0xd100_0000 | (m & 0xff) << 10 | n << 5 | d,
        2 => 0xca00_0000 | m << 16 | n << 5 | d,
        3 => 0xaa00_0000 | m << 16 | n << 5 | d,
        _ => 0xd280_0000 | (n << 8 | m) << 5 | d,
    }
}

fn register(rng: &mut fastrand::Rng) -> u32 {
    DIRECT[rng.usize(..DIRECT.len())]
}

#[test]
fn every_line_is_the_proof_s_line_with_bounded_and_full_look_ahead() {
    let mut rng = fastrand::Rng::with_seed(0x11_4e);
    let mut checked = 0;
    for k in 1..=8usize {
        for _ in 0..40 {
            let words: Vec<u32> = (0..k)
                .map(|_| {
                    integer(
                        rng.u32(..),
                        register(&mut rng),
                        register(&mut rng),
                        register(&mut rng),
                    )
                })
                .collect();
            for displaced in [false, true] {
                let slot = exit_slot(GUEST + 4 * k as u64, displaced);
                // Bounded to the line inside longer code, and the whole line with full look-ahead.
                let longer = [words.clone(), vec![0xd61f_03c0]].concat();
                assert_eq!(
                    translated(&longer, LookAhead::Bounded(k as u32), displaced),
                    line(&words, slot),
                    "{words:08x?} bounded displaced {displaced}"
                );
                assert_eq!(
                    translated(&words, LookAhead::Full, displaced),
                    line(&words, slot),
                    "{words:08x?} full displaced {displaced}"
                );
                checked += 2;
            }
        }
    }
    assert_eq!(checked, 8 * 40 * 2 * 2);
}

#[test]
fn every_access_followed_by_its_part_is_the_stand_in_piece_with_those_words_after() {
    let mut rng = fastrand::Rng::with_seed(0xacce55);
    let mut checked = 0;
    for j in 0..=4usize {
        for n in DIRECT {
            for store in [false, true] {
                let t = register(&mut rng);
                let off = [0u32, 8, 4088][rng.usize(..3)];
                let access =
                    (if store { 0xf900_0000 } else { 0xf940_0000 }) | (off / 8) << 10 | n << 5 | t;
                let after: Vec<u32> = (0..j)
                    .map(|_| {
                        integer(
                            rng.u32(..),
                            register(&mut rng),
                            register(&mut rng),
                            register(&mut rng),
                        )
                    })
                    .collect();
                let code = [vec![access], after.clone(), vec![0xd61f_03c0]].concat();
                for displaced in [false, true] {
                    let slot = exit_slot(GUEST + 4 * (j as u64 + 1), displaced);
                    assert_eq!(
                        translated(&code, LookAhead::Bounded(j as u32 + 1), displaced),
                        load_store_then(n, t, store, off, &after, slot),
                        "{code:08x?} displaced {displaced}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 5 * 26 * 2 * 2);
}

#[test]
fn every_counted_loop_is_the_proof_s_loop_with_full_look_ahead() {
    let mut rng = fastrand::Rng::with_seed(0x100b);
    let mut checked = 0;
    for j in 0..=5usize {
        for _ in 0..30 {
            // The body writes no x0, which `subs x0, x0, #1` counts down before `b.ne` back to the start.
            let words: Vec<u32> = (0..j)
                .map(|_| {
                    let d = DIRECT[1 + rng.usize(..DIRECT.len() - 1)];
                    integer(rng.u32(..), d, register(&mut rng), register(&mut rng))
                })
                .collect();
            let body = [words, vec![0xf100_0400]].concat();
            let guest_back = b_ne(-4 * (j as i64 + 1));
            let code = [body.clone(), vec![guest_back, 0xd61f_03c0]].concat();
            for displaced in [false, true] {
                let slot = exit_slot(GUEST + 4 * (j as u64 + 2), displaced);
                let head = exit_slot(GUEST, displaced);
                let host_back = b_ne(-4 * (j as i64 + 2));
                assert_eq!(
                    translated(&code, LookAhead::Full, displaced),
                    counted_loop(&body, host_back, slot, head),
                    "{code:08x?} displaced {displaced}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 6 * 30 * 2);
}

#[test]
fn every_tested_loop_is_the_proof_s_loop_with_full_look_ahead() {
    let mut checked = 0;
    // `ldr xT, [xN]; subs x0, x0, #1; b.ne .-8; br x30`: neither N nor T is x0, and T is not N (else N is written in
    // the loop and not tested before it).
    for n in DIRECT.into_iter().skip(1) {
        for t in DIRECT.into_iter().skip(1).filter(|&t| t != n) {
            let code = [
                0xf940_0000 | n << 5 | t,
                0xf100_0400,
                0x54ff_ffc1,
                0xd61f_03c0,
            ];
            for displaced in [false, true] {
                assert_eq!(
                    translated(&code, LookAhead::Full, displaced),
                    tested_loop(
                        n,
                        t,
                        0xf100_0400,
                        exit_slot(GUEST + 12, displaced),
                        exit_slot(GUEST, displaced)
                    ),
                    "{code:08x?} displaced {displaced}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 25 * 24 * 2);
}
