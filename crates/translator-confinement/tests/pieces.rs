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
const STACK: &str = include_str!("../statement/stack.rs");
const BRANCH: &str = include_str!("../statement/branch.rs");

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
        "stepped_at" => {
            let (slot, guest, displaced) = at(1);
            return (stack_step(number(&a[0])), slot, guest, displaced);
        }
        "exit_words_at" => {
            let base = u64::try_from(number(&a[1])).expect("an address");
            let slot = u16::try_from(base >> SLOT_BITS).expect("a slot");
            let target = u64::try_from(number(&a[0])).expect("an address");
            return (branch(target, GUEST), slot, GUEST, flag(&a[2]));
        }
        "system_call_at" => {
            let (slot, guest, displaced) = at(0);
            return (0xd400_0001, slot, guest, displaced);
        }
        "jump" => return (0xd61f_0000 | word(&a[0]) << 5, 3, GUEST, false),
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

/// `sub sp, sp, #k`, shifted by 12 for a k of 4096 or more (Arm ARM, add/subtract immediate).
fn stack_step(k: i64) -> u32 {
    if k < 4096 {
        0xd100_03ff | (k as u32) << 10
    } else {
        0xd140_03ff | ((k / 4096) as u32) << 10
    }
}
/// The step's piece (stack.rs `stepped`): the step, the probe `ldr wzr, [sp]`, the exit.
fn stepped(k: i64, slot: u64) -> Vec<u32> {
    [vec![stack_step(k), 0xb940_03ff], exit_words(slot)].concat()
}

/// `b target` from `guest` (Arm ARM, unconditional branch immediate).
fn branch(target: u64, guest: u64) -> u32 {
    0x1400_0000 | ((target.wrapping_sub(guest) / 4) as u32 & 0x3ff_ffff)
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
/// branch.rs `system_call` and `jump`.
fn system_call(slot: u64) -> Vec<u32> {
    [
        vec![0x1400_0005],
        exit_words(slot),
        floor_call(2),
        vec![0xf940_2f9e, 0x17ff_fff6],
    ]
    .concat()
}
fn jump(n: u32) -> Vec<u32> {
    [
        vec![
            0x9100_0652,
            0xd360_dc17 | n << 5,
            0xb500_00d7,
            0x8b20_4317 | n << 16,
            0xd37e_aef7,
            0xb342_0ef7,
            0xd252_02f7,
            0xd61f_02e0,
            0x1400_0001,
        ],
        floor_call(7),
        vec![0xf940_2f9e, 0x17ff_fffa],
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
        "stepped_at" => stepped(number(&a[0]), exit),
        "exit_words_at" => exit_words(exit_slot(slot, number(&a[0]) as u64, displaced)),
        "system_call_at" => system_call(exit),
        "jump" => jump(word(&a[0])),
        other => panic!("no shape for pins of {other}"),
    }
}

fn all_pins() -> Vec<Pin> {
    [
        pins(COPY),
        pins(ACCESS),
        pins(INDEXED),
        pins(STACK),
        pins(BRANCH),
    ]
    .concat()
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
            &["copied_at"; 2],
            &["stepped_at"; 2],
            &["exit_words_at"; 2],
            &["system_call_at", "jump"],
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

#[test]
fn a_stack_access_s_piece_is_the_word_and_the_exit() {
    // Through `sp` with an immediate (Arm ARM, load/store register unsigned offset, unscaled, pre- and post-index;
    // load/store pair offset, pre- and post-index), the largest offsets either way among them, up to 16 bytes.
    let words = [
        0xf940_07e0, // ldr x0, [sp, #8]
        0xf97f_ffe0, // ldr x0, [sp, #32760]
        0xb900_07e5, // str w5, [sp, #4]
        0x3940_03e0, // ldrb w0, [sp]
        0x7900_07e0, // strh w0, [sp, #2]
        0x3dff_ffe0, // ldr q0, [sp, #65520]
        0x3dbf_ffe0, // str q0, [sp, #65520]
        0xf850_03e0, // ldur x0, [sp, #-256]
        0xf81f_03fe, // stur x30, [sp, #-16]
        0xf850_0fe0, // ldr x0, [sp, #-256]!
        0xf810_07e0, // str x0, [sp], #-256
        0xa9bf_7bfd, // stp x29, x30, [sp, #-16]!
        0xa8c1_7bfd, // ldp x29, x30, [sp], #16
        0xa940_07e0, // ldp x0, x1, [sp]
        0xa920_07e0, // stp x0, x1, [sp, #-512]
    ];
    // A pair of quadwords (32 bytes) gets page probes first, a piece not proven yet.
    for word in words {
        for slot in [1, 3, 2047] {
            assert_eq!(
                translated(word, slot, GUEST, false),
                copied(word, exit_slot(slot, GUEST + 4, false)),
                "{word:#010x} slot {slot}"
            );
        }
    }
}

/// `add sp, sp, #k`, shifted by 12 for a k of 4096 or more.
fn stack_step_up(k: i64) -> u32 {
    stack_step(k) ^ 0x4000_0000
}
/// An upward step's piece: the step, the probe `ldur wzr, [sp, #-4]` (the freed frame's last word), the exit.
fn stepped_up(k: i64, slot: u64) -> Vec<u32> {
    [vec![stack_step_up(k), 0xb85f_c3ff], exit_words(slot)].concat()
}
/// The checked piece of a step `step` (an `add` or `sub` of `sp` by an immediate) past what a probe alone keeps in
/// reach: `sub x23, sp, x24` (the guest's stack pointer), the step into x15 from x23, the test of x15's top half
/// (`ubfx x23, x15, #32, #32`, `cbnz x23` to the tail), `add sp, x24, w15, uxtw` (the stack pointer cut to the
/// window), the exit, and the tail that faults on the slot's top byte.
fn checked_step(step: u32, slot: u64) -> Vec<u32> {
    let into_x15_from_x23 = (step & !0x3ff) | 23 << 5 | 15;
    [
        vec![
            0xcb38_63f7,
            into_x15_from_x23,
            0xd360_fdf7,
            0xb500_00d7,
            0x8b2f_431f,
        ],
        exit_words(slot),
        vec![0xd260_030f, 0x397f_fdff, 0],
    ]
    .concat()
}

#[test]
fn every_stack_step_a_probe_alone_keeps_in_reach_is_the_step_the_probe_and_the_exit() {
    let steps = (1..4096).chain((1..=15).map(|j| j << 12));
    for k in steps {
        let exit = exit_slot(3, GUEST + 4, false);
        assert_eq!(
            translated(stack_step(k), 3, GUEST, false),
            stepped(k, exit),
            "sub sp, sp, #{k}"
        );
        assert_eq!(
            translated(stack_step_up(k), 3, GUEST, false),
            stepped_up(k, exit),
            "add sp, sp, #{k}"
        );
    }
}

#[test]
fn a_stack_step_past_0xf000_is_checked_and_cut_not_the_proof_s_stepped_piece() {
    // stack.rs proves `stepped` confined only for `0 < k <= 0xf000` (60 KiB, what the pool's reservation below a
    // window covers with the probe's page): every larger step must get the checked piece, which sets the stack pointer
    // only to the window's base plus a 32-bit cut.
    let mut checked = 0;
    for j in 16..4096 {
        let k = j << 12;
        for slot in [1, 3, 2047] {
            let exit = exit_slot(slot, GUEST + 4, false);
            for word in [stack_step(k), stack_step_up(k)] {
                assert_eq!(
                    translated(word, slot, GUEST, false),
                    checked_step(word, exit),
                    "{word:#010x} (a step of {k:#x}) slot {slot}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 4080 * 3 * 2);
}

#[test]
fn branches_system_calls_and_jumps_are_the_proofs_pieces() {
    for slot in [1, 3, 2047] {
        for displaced in [false, true] {
            // A branch back counts first (the progress count, x18). A branch to itself is linked inside the piece
            // (`b .-4` back to the count), a piece not proven yet.
            for by in [-0x40_0000i64, -4, 4, 8, 0x100_0000] {
                let target = GUEST.wrapping_add_signed(by);
                let count = if by < 0 { vec![0x9100_0652] } else { vec![] };
                assert_eq!(
                    translated(branch(target, GUEST), slot, GUEST, displaced),
                    [count, exit_words(exit_slot(slot, target, displaced))].concat(),
                    "b {by:+} slot {slot} displaced {displaced}"
                );
            }
            assert_eq!(
                translated(0xd400_0001, slot, GUEST, displaced),
                system_call(exit_slot(slot, GUEST + 4, displaced)),
                "svc slot {slot} displaced {displaced}"
            );
        }
        for n in DIRECT {
            assert_eq!(
                translated(0xd61f_0000 | n << 5, slot, GUEST, false),
                jump(n),
                "br x{n} slot {slot}"
            );
        }
    }
}
