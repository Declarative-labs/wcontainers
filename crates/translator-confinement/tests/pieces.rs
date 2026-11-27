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
const DISPLACED: &str = include_str!("../statement/displaced.rs");
const OFFSET: &str = include_str!("../statement/offset.rs");

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
        "displaced_jump" => return (0xd61f_0000 | word(&a[0]) << 5, 3, GUEST, true),
        "self_loop_at" => {
            let (slot, guest, displaced) = at(0);
            return (branch(guest, guest), slot, guest, displaced);
        }
        "unaligned_access_at" => {
            let (slot, guest, displaced) = at(4);
            let instruction =
                indexed_instruction(word(&a[0]), word(&a[1]), flag(&a[2]), true, number(&a[3]));
            return (instruction, slot, guest, displaced);
        }
        "stepped_up_at" => {
            let (slot, guest, displaced) = at(1);
            return (stack_step_up(number(&a[0])), slot, guest, displaced);
        }
        "stack_pair_at" => {
            let (slot, guest, displaced) = at(3);
            return (word(&a[0]), slot, guest, displaced);
        }
        "displaced_stack_at" => {
            let base = u64::try_from(number(&a[3])).expect("an address");
            let guest = u64::try_from(number(&a[4])).expect("an address");
            let slot = u16::try_from(base >> SLOT_BITS).expect("a slot");
            // The guest's word is the piece's access with its base the stack pointer again.
            return (word(&a[0]) | 31 << 5, slot, guest, true);
        }
        "register_byte_at" => {
            let (slot, guest, displaced) = at(3);
            return (
                register_byte_instruction(word(&a[0]), word(&a[1]), flag(&a[2])),
                slot,
                guest,
                displaced,
            );
        }
        "register_wide_at" => {
            let (slot, guest, displaced) = at(4);
            let t = word(&a[2]) & 31;
            return (
                register_wide_instruction(word(&a[0]), t, flag(&a[3])),
                slot,
                guest,
                displaced,
            );
        }
        "literal_at" => {
            let (slot, guest, displaced) = at(2);
            let label = u64::try_from(number(&a[0])).expect("an address");
            return (
                literal_instruction(label, guest, word(&a[1])),
                slot,
                guest,
                displaced,
            );
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

/// The other pre-indexed piece (indexed.rs `unaligned_access`): the page test of the access's last byte inline.
fn unaligned_access(n: u32, t: u32, store: bool, imm: i64, slot: u64) -> Vec<u32> {
    let pieces = indexed_access(n, t, store, true, imm, slot);
    let probes = if store {
        vec![0x383f_01ff, 0x9100_1def, 0x383f_01ff, 0x17ff_fff1]
    } else {
        vec![0x3940_01ff, 0x3940_1dff, 0x17ff_fff2]
    };
    [
        vec![
            pieces[0],
            0xb500_018f,
            pieces[2],
            pieces[15],
            0x927d_21ef,
            0xb400_016f,
        ],
        pieces[5..8].to_vec(),
        exit_words(slot),
        vec![0xd260_030f, 0x397f_fdff, 0, pieces[19], 0x8b2f_62ef],
        probes,
    ]
    .concat()
}

/// A pair of quadwords through `sp` (stack.rs `stack_pair`): the page test, the access, the exit; out of line the
/// stack pointer's alignment test and the probes.
fn stack_pair(word: u32, off: i64, store: bool, slot: u64) -> Vec<u32> {
    let page_test = 0x9100_03f7 | ((off + 31).rem_euclid(4096) as u32) << 10;
    let move_offset = if off >= 0 {
        0xd280_0017 | (off as u32) << 5
    } else {
        0x9280_0017 | ((-off - 1) as u32) << 5
    };
    let probes = if store {
        vec![0x383f_02ff, 0x9100_7ef7, 0x383f_02ff, 0x17ff_fff2]
    } else {
        vec![0x3940_02ff, 0x3940_7eff, 0x17ff_fff3]
    };
    [
        vec![page_test, 0x927b_1af7, 0xb400_00d7, word],
        exit_words(slot),
        vec![
            0x9100_03f7,
            0x9240_0ef7,
            0xb400_0057,
            0xd420_00a0,
            move_offset,
            0x8b37_63f7,
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

/// branch.rs `self_loop` (`b .`: the count and `b .-4`, then what the link left of the exit) and `displaced_jump`.
fn self_loop(slot: u64) -> Vec<u32> {
    [
        vec![0x9100_0652, 0x17ff_ffff],
        exit_words(slot)[1..].to_vec(),
    ]
    .concat()
}
fn displaced_jump(n: u32) -> Vec<u32> {
    [
        vec![
            0x9100_0652,
            0xd360_dc17 | n << 5,
            0xb500_00b7,
            0xd37e_7c17 | n << 5,
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

/// displaced.rs `displaced_stack`: the stack pointer's alignment test, its window check and cut into x23, the access
/// through x23 (a written back one between `new_sp` and `mov sp, x15`), the exit, the misaligned floor call's branch,
/// the mark, the floor call, the way back.
fn displaced_stack(word: u32, back: bool, new_sp: u32, slot: u64) -> Vec<u32> {
    let d = if back { 8 } else { 0 };
    [
        vec![
            0x9100_03f7,
            0x9240_0ef7,
            0xb500_0017 | ((40 + d) / 4) << 5,
            0x9100_03f7,
            0xd360_deef,
            0xb500_000f | ((32 + d) / 4) << 5,
            0x8b37_4317,
        ],
        if back {
            vec![new_sp, word, 0x9100_01ff]
        } else {
            vec![word]
        },
        exit_words(slot),
        vec![0x1400_0004, 0xd260_030f, 0x397f_fdff, 0],
        floor_call(6),
        vec![0xf940_2f9e, 0x17ff_fff7],
    ]
    .concat()
}

/// The guest's register-offset accesses (Arm ARM, load/store register, register offset) of the address word `add
/// x23, xN, (w|x)M, extend #s` the piece starts with: the same N, M and extend, S set by a shift of 3.
fn register_byte_instruction(address: u32, t: u32, store: bool) -> u32 {
    (if store { 0x3820_0800 } else { 0x3860_0800 }) | (address & 0x001f_e3e0) | t
}
fn register_wide_instruction(address: u32, t: u32, store: bool) -> u32 {
    let scaled = if (address >> 10) & 7 == 3 { 1 << 12 } else { 0 };
    (if store { 0xf820_0800 } else { 0xf860_0800 }) | (address & 0x001f_e3e0) | scaled | t
}
/// `ldr xT/wT, label` at `guest` (Arm ARM, load register literal) of the piece's load `[x23]`.
fn literal_instruction(label: u64, guest: u64, load: u32) -> u32 {
    let opc = if load >> 30 == 3 {
        0x5800_0000
    } else {
        0x1800_0000
    };
    let imm19 = ((label.wrapping_sub(guest) / 4) & 0x7_ffff) as u32;
    opc | imm19 << 5 | (load & 31)
}

/// offset.rs `register_wide` and `literal`.
fn register_wide(address: u32, n: u32, access: u32, store: bool, slot: u64) -> Vec<u32> {
    let probes = if store {
        vec![0x383f_01ff, 0x9100_1def, 0x383f_01ff, 0x17ff_ffef]
    } else {
        vec![0x3940_01ff, 0x3940_1dff, 0x17ff_fff0]
    };
    [
        vec![
            address,
            0xd360_deef,
            0xb500_012f,
            0x8b37_4317,
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
fn literal(label: u64, load: u32, slot: u64) -> Vec<u32> {
    let half = |k: u32| (((label >> (16 * k)) & 0xffff) as u32) << 5;
    [
        vec![
            0xd280_0000 | half(0) | 23,
            0xf280_0000 | 1 << 21 | half(1) | 23,
            0xf280_0000 | 2 << 21 | half(2) | 23,
            0x8b37_4317,
            load,
        ],
        exit_words(slot),
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
        "displaced_jump" => displaced_jump(word(&a[0])),
        "self_loop_at" => self_loop(exit_slot(slot, guest, displaced)),
        "unaligned_access_at" => {
            unaligned_access(word(&a[0]), word(&a[1]), flag(&a[2]), number(&a[3]), exit)
        }
        "stepped_up_at" => stepped_up(number(&a[0]), exit),
        "stack_pair_at" => stack_pair(word(&a[0]), number(&a[1]), flag(&a[2]), exit),
        "displaced_stack_at" => displaced_stack(word(&a[0]), flag(&a[1]), word(&a[2]), exit),
        "register_byte_at" => [
            vec![word(&a[0])],
            byte_access(23, word(&a[1]), flag(&a[2]), exit),
        ]
        .concat(),
        "register_wide_at" => {
            register_wide(word(&a[0]), word(&a[1]), word(&a[2]), flag(&a[3]), exit)
        }
        "literal_at" => literal(
            u64::try_from(number(&a[0])).expect("an address"),
            word(&a[1]),
            exit,
        ),
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
        pins(DISPLACED),
        pins(OFFSET),
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
            &["unaligned_access_at"; 3],
            &["copied_at"; 2],
            &["stepped_at"; 2],
            &["stepped_up_at"; 2],
            &["stack_pair_at"; 3],
            &["exit_words_at"; 2],
            &[
                "system_call_at",
                "self_loop_at",
                "self_loop_at",
                "displaced_jump",
                "jump"
            ],
            &["displaced_stack_at"; 2],
            &["register_byte_at", "register_wide_at", "register_wide_at"],
            &["literal_at"; 2],
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
            assert_eq!(
                translated(0xd61f_0000 | n << 5, slot, GUEST, true),
                displaced_jump(n),
                "displaced br x{n} slot {slot}"
            );
        }
        for displaced in [false, true] {
            for guest in [GUEST, 0x1000, 0xffff_fff8] {
                assert_eq!(
                    translated(branch(guest, guest), slot, guest, displaced),
                    self_loop(exit_slot(slot, guest, displaced)),
                    "b . at {guest:#x} slot {slot} displaced {displaced}"
                );
            }
        }
    }
}

#[test]
fn the_other_pre_indexed_pieces_are_the_proof_s_for_every_register_named_directly() {
    // A pre-index whose immediate is no multiple of 8 tests the page of the access's own last byte inline.
    let mut checked = 0;
    for imm in [-255, -9, -3, -1, 1, 4, 7, 100, 255] {
        for n in DIRECT {
            for t in DIRECT.into_iter().chain([31]) {
                for store in [false, true] {
                    for displaced in [false, true] {
                        let exit = exit_slot(3, GUEST + 4, displaced);
                        assert_eq!(
                            translated(
                                indexed_instruction(n, t, store, true, imm),
                                3,
                                GUEST,
                                displaced
                            ),
                            unaligned_access(n, t, store, imm, exit),
                            "n {n} t {t} store {store} imm {imm} displaced {displaced}"
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 9 * 26 * 27 * 2 * 2);
}

#[test]
fn every_pair_of_quadwords_through_the_stack_pointer_is_the_proof_s_piece() {
    // `stp/ldp q0, q1, [sp, #off]` and pre-index (Arm ARM, load/store pair, SIMD&FP, opc 10), every immediate: the
    // access's offset from `sp` is the immediate times 16. (Post-index, whose probes start at `mov x23, sp`, a word
    // shorter, is not proven yet.)
    let mut checked = 0;
    for imm7 in -64i64..64 {
        for form in [0xad00_0000u32, 0xad80_0000] {
            for store in [false, true] {
                let word = form
                    | if store { 0 } else { 1 << 22 }
                    | ((imm7 & 0x7f) as u32) << 15
                    | 1 << 10
                    | 31 << 5;
                let off = imm7 * 16;
                for slot in [1, 3, 2047] {
                    assert_eq!(
                        translated(word, slot, GUEST, false),
                        stack_pair(word, off, store, exit_slot(slot, GUEST + 4, false)),
                        "{word:#010x} slot {slot}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 128 * 2 * 2 * 3);
}

#[test]
fn the_displaced_stack_accesses_with_no_probe_are_the_proof_s_piece() {
    // Displaced, an access through `sp` is checked and cut through x23; a written back one makes the new stack
    // pointer in x15 first (`sub/add x15, sp, #k`) and gives it to `sp` after.
    // (A byte at `[sp]` is cut by its own addressing, a word shorter: not proven yet.)
    let words: [(u32, Option<u32>); 14] = [
        (0xf940_07e0, None),              // ldr x0, [sp, #8]
        (0xf97f_ffe0, None),              // ldr x0, [sp, #32760]
        (0xb900_07e5, None),              // str w5, [sp, #4]
        (0x7900_07e0, None),              // strh w0, [sp, #2]
        (0x3dff_ffe0, None),              // ldr q0, [sp, #65520]
        (0x3dbf_ffe0, None),              // str q0, [sp, #65520]
        (0xf850_03e0, None),              // ldur x0, [sp, #-256]
        (0xf81f_03fe, None),              // stur x30, [sp, #-16]
        (0xa940_07e0, None),              // ldp x0, x1, [sp]
        (0xf850_0fe0, Some(0xd104_03ef)), // ldr x0, [sp, #-256]!
        (0xf810_07e0, Some(0xd104_03ef)), // str x0, [sp], #-256
        (0xa9bf_7bfd, Some(0xd100_43ef)), // stp x29, x30, [sp, #-16]!
        (0xa8c1_7bfd, Some(0x9100_43ef)), // ldp x29, x30, [sp], #16
        (0xa9a0_07e0, Some(0xd108_03ef)), // stp x0, x1, [sp, #-512]!
    ];
    for (word, new_sp) in words {
        for slot in [1, 3, 2047] {
            let through_x23 = (word & !(31 << 5)) | 23 << 5;
            assert_eq!(
                translated(word, slot, GUEST, true),
                displaced_stack(
                    through_x23,
                    new_sp.is_some(),
                    new_sp.unwrap_or(0),
                    exit_slot(slot, GUEST + 4, true)
                ),
                "{word:#010x} slot {slot}"
            );
        }
    }
}

#[test]
fn the_register_offset_and_literal_pieces_are_the_proofs_for_every_register_named_directly() {
    let mut checked = 0;
    // `ldrb/strb wT, [xN, (w|x)M, extend]` and `ldr/str xT, [xN, (w|x)M, extend #3]`: uxtw, lsl, sxtw, sxtx.
    for option in [2u32, 3, 6, 7] {
        for n in DIRECT {
            for m in [0, 9, 30] {
                for t in DIRECT.into_iter().chain([31]) {
                    for store in [false, true] {
                        for displaced in [false, true] {
                            let exit = exit_slot(3, GUEST + 4, displaced);
                            let address = 0x8b20_0000 | m << 16 | option << 13 | n << 5 | 23;
                            let byte = register_byte_instruction(address, t, store);
                            assert_eq!(
                                translated(byte, 3, GUEST, displaced),
                                [vec![address], byte_access(23, t, store, exit)].concat(),
                                "{byte:#010x} displaced {displaced}"
                            );
                            let scaled = address | 3 << 10;
                            let wide = register_wide_instruction(scaled, t, store);
                            let access = (wide & !(31 << 16 | 31 << 5)) | 31 << 16 | 23 << 5;
                            assert_eq!(
                                translated(wide, 3, GUEST, displaced),
                                register_wide(scaled, n, access, store, exit),
                                "{wide:#010x} displaced {displaced}"
                            );
                            checked += 2;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 4 * 26 * 3 * 27 * 2 * 2 * 2);
    // `ldr xT, label` and `ldr wT, label`, labels behind and ahead, none crossing a page (a doubleword's that does gets
    // its probes inline, a piece not proven yet).
    for by in [-0x1000i64, -8, 0, 8, 0xf_fff8] {
        for t in DIRECT {
            for (load, opc) in [(0xf940_02e0u32, 0x5800_0000u32), (0xb940_02e0, 0x1800_0000)] {
                for displaced in [false, true] {
                    let label = GUEST.wrapping_add_signed(by);
                    let word = opc | (((by / 4) as u32) & 0x7_ffff) << 5 | t;
                    assert_eq!(
                        translated(word, 3, GUEST, displaced),
                        literal(label, load | t, exit_slot(3, GUEST + 4, displaced)),
                        "{word:#010x} displaced {displaced}"
                    );
                }
            }
        }
    }
}

const WITHHELD: &str = include_str!("../statement/withheld.rs");

/// `ldr/str xT, [x28, #off]`: whether it stores, T, the offset.
fn frame_access(w: u32) -> Option<(bool, u32, u32)> {
    let store = match w & 0xffc0_03e0 {
        0xf900_0380 => true,
        0xf940_0380 => false,
        _ => return None,
    };
    Some((store, w & 31, ((w >> 10) & 0xfff) * 8))
}

/// withheld.rs `stand_in`'s shape and hypotheses: frame accesses inside the frame's 256 bytes loading no x28 and moves
/// to no x28, one word that is `guest` with its registers stood in for and writing no x28, more of them, the exit.
fn is_stand_in(words: &[u32], guest: u32, exit: u64) -> bool {
    let Some((body, tail)) = words.split_at_checked(words.len().wrapping_sub(4)) else {
        return false;
    };
    if tail != exit_words(exit) {
        return false;
    }
    // `mov xT, xN` (`orr xT, xzr, xN`) to any register but x28.
    let moves = |w: u32| w & 0xffe0_ffe0 == 0xaa00_03e0 && w & 31 != 28;
    let middle: Vec<usize> = (0..body.len())
        .filter(|&k| frame_access(body[k]).is_none() && !moves(body[k]))
        .collect();
    let [m] = middle[..] else {
        return false;
    };
    let registers = 31 | 31 << 5 | 31 << 16;
    body.iter()
        .filter_map(|&w| frame_access(w))
        .all(|(store, t, off)| off + 8 <= 256 && (store || t != 28))
        && body[m] & !registers == guest & !registers
        && body[m] & 31 != 28
}

#[test]
fn the_stand_in_pins_are_what_the_translator_emits() {
    // The pins' guest instructions: `add x0, x15, x1` both ways, `add x24, x0, x1`, `mov x23, x0`.
    let guests = [
        (0x8b01_01e0, false),
        (0x8b01_01e0, true),
        (0x8b01_0018, false),
        (0xaa00_03f7, false),
    ];
    let pins: Vec<Vec<u32>> = WITHHELD
        .lines()
        .filter(|line| line.starts_with("proof fn pin_"))
        .map(|line| {
            let words = line.split_once("=~= seq![").expect("the words").1;
            let words = words.split_once(']').expect("their end").0;
            words.split(',').map(word).collect()
        })
        .collect();
    assert_eq!(pins.len(), guests.len());
    for (pin, (guest, displaced)) in pins.iter().zip(guests) {
        assert_eq!(
            &translated(guest, 3, GUEST, displaced),
            pin,
            "{guest:#010x}"
        );
    }
}

#[test]
fn every_integer_instruction_naming_a_withheld_register_is_a_stand_in_piece() {
    // `add xD, xN, xM`, `sub xD, xN, xM, lsl #3`, `eor xD, xN, xM` with a withheld register in each place in turn.
    let mut checked = 0;
    for opcode in [0x8b00_0000u32, 0xcb00_0c00, 0xca00_0000] {
        for withheld in [15, 18, 23, 24, 28] {
            for other in [0, 9, 30] {
                for (d, n, m) in [
                    (withheld, other, 1),
                    (other, withheld, 1),
                    (other, 1, withheld),
                ] {
                    let guest = opcode | m << 16 | n << 5 | d;
                    for displaced in [false, true] {
                        let words = translated(guest, 3, GUEST, displaced);
                        assert!(
                            is_stand_in(&words, guest, exit_slot(3, GUEST + 4, displaced)),
                            "{guest:#010x} displaced {displaced}: {words:08x?}"
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    assert_eq!(checked, 3 * 5 * 3 * 3 * 2);
}
