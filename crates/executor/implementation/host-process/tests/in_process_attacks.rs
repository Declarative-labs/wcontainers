//! Attacks on the in-process way's inline protection (docs/native-without-virtual-machine.md section 3.2), on hardware.
//!
//! A jump through a register: `br`, `blr` and `ret` through every register the way does not withhold, translated for
//! the in-process way in slot 1 (whose slots lie in the process way's slot table) and run in a stub, to seeded
//! targets chosen to defeat the window: inside it (aligned, misaligned, tagged), just past it, one bit past it, far
//! outside it, the guest address or the host address of a target whose slot is written, and the host addresses a
//! guest must never reach: the middle of translated code, the floor's entries, the slot table, the register pages and
//! the zygote. Every run must end at the entry slot of the target's window address (the ending written there, or
//! the fault of an unwritten or misaligned slot) or, for a target outside the window, at the floor's entry for an
//! unreachable target, which the stop crate resolves to the guest landing at the target, untagged as Linux does;
//! never anywhere else. The guest's registers must be its own there, x30 the return address after a `blr`. A stub
//! has no floor at the in-process way's entries, so each call into the floor is pointed at the process way's entry
//! of the same call, its only change to the translated code.
//!
//! An access: twelve shapes (immediate, unscaled, register offsets plain, shifted and extended both ways, byte, pair,
//! store, pre- and post-index), translated for the same way, each case its own drawn word, at addresses chosen to
//! defeat the window: inside a mapping, just past or before one, across the window's top, a base wrapped past 2^64
//! by its index, an alias above the window, one bit above it, tagged, with bit 55 set, the host address of a window
//! byte. Linux's outcome is the model: an access completes when every byte is in a mapped page that allows it, its
//! address untagged as Linux does. Every page mapped outside the window where a wrong cut, no cut or a wrong base
//! would land (the guest's own addresses, the next slot's window) is a canary; the never mapped bytes beside the
//! window and its top band stay unmapped, as the way requires; the window may change only where a store completed.
//! A completed access must move the bytes Linux moves; a fault must resolve, through the stop crate, to the guest's
//! fault at a byte of the access with the state before it.
//!
//! Knobs `WCHP_CASES` targets per piece and accesses per shape, `WCHP_SEED`.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use rustix::mm::ProtFlags;
use wcontainers_abi::RegisterBlock;
use wcontainers_arm64::encode::{self, form};
use wcontainers_arm64::thread_state::{RegisterPage, SCRATCH, TAG_LOW, untag_address};
use wcontainers_arm64::way::{
    IN_PROCESS_ENTRIES, Kept, PROCESS_ENTRIES, PROCESS_ENTRY_BYTES, WINDOW_BITS,
};
use wcontainers_arm64::{Arm64, Way};
use wcontainers_executor_arm64_stop::in_process::{self, Resolution};
use wcontainers_executor_arm64_stop::{
    self as stop, AFTER_PAGE, FloatStatus, Piece, Resolved, Stop, Taken,
};
use wcontainers_executor_host_process::code::CodeArea;
use wcontainers_executor_host_process::layout::{CODE_AREA, REGISTER_PAGES, ZYGOTE};
use wcontainers_executor_host_process::pages::Command;
use wcontainers_executor_host_process::probe::HostProbe;
use wcontainers_executor_host_process::stub::{Stub, TurnEnd};
use wcontainers_executor_host_process::zygote::Zygote;
use wcontainers_guest_cpu::HostCode;
use wcontainers_guest_cpu::position::InsideCall;
use wcontainers_translator::{LookAhead, translate_native_for};

const CASES_VARIABLE: &str = "WCHP_CASES";
/// Targets per piece unless told otherwise.
const DEFAULT_CASES: u64 = 200;
const SEED_VARIABLE: &str = "WCHP_SEED";
const DEFAULT_SEED: u64 = 0x6a75_6d70_7321;

/// The way: a slot other than 0, so that the window's base is in every slot's address.
const WAY: Way = Way::InProcess {
    slot: 1,
    displaced: false,
    kept: Kept::USUAL,
};
/// Where the jump lies in the guest's addresses.
const GUEST: u64 = 0x40_0000;
/// Where each piece is put in the code area.
const PIECE_ROOM: usize = 4096;
/// The bytes of one instruction.
const INSTRUCTION: u64 = size_of::<u32>() as u64;
/// The window's bytes.
const WINDOW: u64 = 1 << WINDOW_BITS;
/// Targets whose entry slots hold the ending: the window's first and last instructions, and some between.
const LANDINGS: [u64; 5] = [0, 0x1_0000, 0x40_0040, 0x7fff_f000, WINDOW - INSTRUCTION];
/// The registers the in-process way withholds from the guest, which a jump through takes from the register page.
const WITHHELD: [u8; 5] = [15, 18, 23, 24, 28];
/// The window's base register of the way, and its value here.
const BASE: usize = 24;
/// The link register.
const LINK: usize = 30;
/// The jumps.
const JUMPS: [(&str, encode::Form); 3] = [("br", form::BR), ("blr", form::BLR), ("ret", form::RET)];
/// Linux's signals for an unwritten slot (its zero word is `udf #0`) and a misaligned one (Arm's PC alignment fault).
const ILLEGAL: u32 = libc::SIGILL as u32;
const MISALIGNED: u32 = libc::SIGBUS as u32;

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

/// One embedder process has one zygote place, so tests running at once start theirs one at a time.
static STARTING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn zygote(probe: &HostProbe) -> Zygote {
    let _one_at_a_time = STARTING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Zygote::start(probe).expect("zygote")
}

/// The guest address a target's window address is: its low 32 bits, whatever its tag.
fn in_window(target: u64) -> u64 {
    target % WINDOW
}

/// Whether the window check lets a target through to its slot: its bits from the window's top to the tag clear.
fn checked_in(target: u64) -> bool {
    (target & ((1 << TAG_LOW) - 1)) >> WINDOW_BITS == 0
}

/// The entry slot a jump to `target` reaches when it is in the window: its slot with the target's own low two bits,
/// so that a misaligned target faults there; a tag goes as Linux drops it at a branch.
fn slot_of(target: u64) -> u64 {
    let g = in_window(target);
    WAY.slot(g) | (g % INSTRUCTION)
}

/// The words at a landing's slot: a call into the process way's floor entry that ends the turn.
fn ending() -> Vec<u32> {
    let entry = PROCESS_ENTRIES + u64::from(InsideCall::Answer.code()) * PROCESS_ENTRY_BYTES;
    let mut words = encode::move_address(SCRATCH, entry);
    words.push(encode::branch_register(form::BLR, SCRATCH));
    words
}

/// The most floor entries.
const ENTRIES: u64 = 64;

/// Points every call into the in-process floor at the process way's entry of the same call, the floor a stub has
/// (a stub ends at a fault outside its code area and slot table, where I's entries would be); gives how many.
/// Nothing else of the code changes: a call is `move_address(x30, entry)` then `blr x30` in both ways.
fn to_process_entries(words: &mut [u32]) -> usize {
    let call = encode::branch_register(form::BLR, LINK as u8);
    let mut moved = 0;
    let calls: Vec<usize> = (0..words.len()).filter(|&k| words[k] == call).collect();
    for end in calls {
        for code in 0..ENTRIES {
            let from =
                encode::move_address(LINK as u8, IN_PROCESS_ENTRIES + code * PROCESS_ENTRY_BYTES);
            let to = encode::move_address(LINK as u8, PROCESS_ENTRIES + code * PROCESS_ENTRY_BYTES);
            assert_eq!(
                from.len(),
                to.len(),
                "both ways' entries take as many moves"
            );
            if let Some(start) = end.checked_sub(from.len())
                && words[start..end] == from[..]
            {
                words[start..end].copy_from_slice(&to);
                moved += 1;
            }
        }
    }
    moved
}

/// One translated jump, loaded.
struct Loaded {
    name: &'static str,
    register: u8,
    jump: encode::Form,
    at: u64,
    code: HostCode,
}

/// A target drawn from one of the classes that try to leave the window.
fn target(rng: &mut fastrand::Rng, pieces: &[Loaded]) -> (&'static str, u64) {
    let tag = |rng: &mut fastrand::Rng| u64::from(rng.u8(1..)) << TAG_LOW;
    match rng.u8(0..14) {
        0 => ("landing", LANDINGS[rng.usize(..LANDINGS.len())]),
        1 => ("in the window", rng.u64(..WINDOW) & !(INSTRUCTION - 1)),
        2 => ("misaligned", rng.u64(..WINDOW) | rng.u64(1..INSTRUCTION)),
        3 => (
            "tagged landing",
            tag(rng) | LANDINGS[rng.usize(..LANDINGS.len())],
        ),
        4 => ("just past the window", WINDOW + rng.u64(..64) * INSTRUCTION),
        5 => (
            "a landing's alias past the window",
            (rng.u64(1..1 << (TAG_LOW - WINDOW_BITS)) << WINDOW_BITS)
                | LANDINGS[rng.usize(..LANDINGS.len())],
        ),
        6 => {
            let piece = &pieces[rng.usize(..pieces.len())];
            let words = piece.code.bytes.len() as u64 / INSTRUCTION;
            (
                "inside translated code",
                piece.at + rng.u64(..words) * INSTRUCTION,
            )
        }
        7 => (
            "a floor entry",
            PROCESS_ENTRIES + rng.u64(..64) * PROCESS_ENTRY_BYTES,
        ),
        8 => (
            "the slot table",
            WAY.slot(LANDINGS[rng.usize(..LANDINGS.len())]),
        ),
        9 => (
            "our own pages",
            [REGISTER_PAGES as u64, ZYGOTE as u64, CODE_AREA as u64][rng.usize(..3)],
        ),
        10 => ("tagged outside", tag(rng) | rng.u64(WINDOW..1 << TAG_LOW)),
        11 => (
            "one bit past the window",
            (1 << rng.u32(WINDOW_BITS..TAG_LOW)) | LANDINGS[rng.usize(..LANDINGS.len())],
        ),
        12 => (
            "a landing's host address",
            WAY.window_base() + LANDINGS[rng.usize(..LANDINGS.len())],
        ),
        _ => ("anywhere", rng.u64(..)),
    }
}

/// Runs `piece` with its register at `target` and checks where it ended; gives the case's failure, if any.
fn run(
    stub: &Stub,
    piece: &Loaded,
    class: &str,
    target: u64,
    rng: &mut fastrand::Rng,
) -> Option<String> {
    let mut block = RegisterBlock::ZERO;
    for (r, x) in block.x.iter_mut().enumerate() {
        if !WITHHELD.contains(&(r as u8)) {
            *x = rng.u64(..);
        }
    }
    block.x[usize::from(piece.register)] = target;
    block.x[BASE] = WAY.window_base();
    block.sp = 0x80_0000;
    let before = block.x;
    stub.set_registers(0, &block);
    stub.set_translated_page(0, &RegisterPage::default());
    let case = format!(
        "{} x{} to {target:#x} ({class})",
        piece.name, piece.register
    );
    let piece_view = Piece {
        base: piece.at,
        length: piece.code.bytes.len(),
        spans: &piece.code.spans,
    };
    let mut float = FloatStatus::default();
    let mut resolve = |h: &mut stop::Interrupted, call: InsideCall| {
        // The call's link is in x30, and the guest's x30 in the register page.
        h.pc = h.x[LINK].wrapping_sub(INSTRUCTION);
        in_process::resolve(
            WAY,
            &piece_view,
            Taken::Call(call.code()),
            h,
            &stub.translated_page(0),
            &[0; AFTER_PAGE],
            &mut float,
            &mut |_, _| panic!("a jump reads no guest memory"),
        )
    };
    // A return's or a call's first jump asks the floor to make its target the last one; the floor here makes none
    // and goes on.
    let mut at = piece.at;
    let (end, mut h) = loop {
        let end = stub.enter(0, &[], at as usize).expect("enter");
        let (mut h, _) = stub.scalars(0);
        if end
            != (TurnEnd::Called {
                entry: InsideCall::LastTarget.code().into(),
            })
        {
            break (end, h);
        }
        match resolve(&mut h, InsideCall::LastTarget) {
            Some(Resolution::Resolved(Resolved::LastTarget {
                target: asked,
                then,
                page,
                ..
            })) if asked == target && at == piece.at => {
                stub.set_scalars(0, &then, 0);
                stub.set_translated_page(0, &page);
                at = then.pc;
            }
            other => return Some(format!("{case}: its last target resolved to {other:?}")),
        }
    };
    // The guest's registers are its own wherever it ends: the target's register as set, x30 the return after `blr`.
    let mut expected = before;
    if piece.jump == form::BLR {
        expected[LINK] = GUEST + INSTRUCTION;
    }
    let differing: Vec<usize> = (0..LINK + 1)
        .filter(|&r| !WITHHELD.contains(&(r as u8)) && h.x[r] != expected[r])
        .collect();
    if checked_in(target) {
        let slot = slot_of(target);
        let landed = LANDINGS.contains(&in_window(target));
        let good = match end {
            TurnEnd::Called { entry } => {
                landed
                    && target.is_multiple_of(INSTRUCTION)
                    && entry == u64::from(InsideCall::Answer.code())
            }
            TurnEnd::Fault {
                address, signal, ..
            } => {
                address == slot
                    && h.pc == slot
                    && if target.is_multiple_of(INSTRUCTION) {
                        !landed && signal == ILLEGAL
                    } else {
                        signal == MISALIGNED
                    }
            }
            _ => false,
        };
        if !good {
            return Some(format!(
                "{case}: ended {end:?} at {:#x}, wanted its slot {slot:#x}",
                h.pc
            ));
        }
        // The ending's own call takes x30; anywhere else x30 is the guest's.
        let differing: Vec<usize> = differing
            .into_iter()
            .filter(|&r| !(r == LINK && matches!(end, TurnEnd::Called { .. })))
            .collect();
        return (!differing.is_empty())
            .then(|| format!("{case}: registers {differing:?} not the guest's at its slot"));
    }
    let unreachable = TurnEnd::Called {
        entry: InsideCall::Unreachable.code().into(),
    };
    if end != unreachable {
        return Some(format!(
            "{case}: ended {end:?} at {:#x}, wanted {unreachable:?}",
            h.pc
        ));
    }
    let resolution = resolve(&mut h, InsideCall::Unreachable);
    let Some(Resolution::Resolved(Resolved::Stop(stop, state))) = resolution else {
        return Some(format!("{case}: resolved to {resolution:?}"));
    };
    // Linux runs a branch to a tagged address at the untagged one, and faults there with the untagged pc (measured:
    // scratch w2/run6/tagged-branch-arm.out).
    let landing = untag_address(target);
    let wanted = if target.is_multiple_of(INSTRUCTION) {
        Stop::Landed(landing)
    } else {
        Stop::Alignment(landing)
    };
    if stop != wanted || state.pc != landing {
        return Some(format!(
            "{case}: resolved to {stop:?} at {:#x}, wanted {wanted:?}",
            state.pc
        ));
    }
    let differing: Vec<usize> = (0..LINK + 1)
        .filter(|&r| !WITHHELD.contains(&(r as u8)) && state.x[r] != expected[r])
        .collect();
    (!differing.is_empty())
        .then(|| format!("{case}: resolved registers {differing:?} not the guest's"))
}

#[test]
fn a_jump_through_a_register_reaches_only_its_target_s_slot_or_the_floor() {
    let cases = setting(CASES_VARIABLE, DEFAULT_CASES);
    let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
    let mut rng = fastrand::Rng::with_seed(seed);
    let probe = HostProbe::new().expect("the host is refused");
    let zygote = zygote(&probe);
    let stub = zygote.fork_stub().expect("fork");
    let mut area = CodeArea::new().expect("code area");
    for landing in LANDINGS {
        area.write_slots(&probe, WAY.slot(landing) as usize, &ending());
    }
    let mut pieces = Vec::new();
    for (name, jump) in JUMPS {
        for register in (0..=LINK as u8).filter(|r| !WITHHELD.contains(r)) {
            let word = encode::branch_register(jump, register);
            let code = translate_native_for::<Arm64>(
                &word.to_le_bytes(),
                GUEST,
                LookAhead::Full,
                true,
                &WAY,
                &(),
            )
            .expect("translated");
            let (words, rest) = code.bytes.as_chunks::<{ size_of::<u32>() }>();
            assert!(rest.is_empty(), "host code is whole words");
            let mut words: Vec<u32> = words.iter().map(|w| u32::from_le_bytes(*w)).collect();
            assert!(
                to_process_entries(&mut words) >= 1,
                "a jump through a register calls the floor when its target is outside the window"
            );
            assert!(
                words.len() * size_of::<u32>() <= PIECE_ROOM,
                "a piece fits its room"
            );
            let at = (CODE_AREA + pieces.len() * PIECE_ROOM) as u64;
            area.write(&probe, at as usize, &words);
            pieces.push(Loaded {
                name,
                register,
                jump,
                at,
                code,
            });
        }
    }
    stub.send_block(area.file()).expect("send the code area");
    stub.send_block(area.file()).expect("send the slot table");
    assert_eq!(
        stub.turn(0, &[Command::MapCode, Command::MapSlots]),
        Ok(TurnEnd::Done {
            commands_applied: 2
        })
    );
    let mut failures = Vec::new();
    let mut by_class = std::collections::BTreeMap::<&str, u64>::new();
    for piece in &pieces {
        for _ in 0..cases {
            let (class, target) = target(&mut rng, &pieces);
            *by_class.entry(class).or_default() += 1;
            if let Some(failure) = run(&stub, piece, class, target, &mut rng) {
                failures.push(failure);
            }
        }
    }
    stub.end(0).expect("end");
    eprintln!(
        "seed {seed:#x}: {} pieces, {} cases, by class {by_class:?}, {} failing",
        pieces.len(),
        pieces.len() as u64 * cases,
        failures.len()
    );
    for failure in failures.iter().take(20) {
        eprintln!("{failure}");
    }
    assert!(failures.is_empty(), "{} cases failed", failures.len());
}

/// The guest's mapped regions in the window: start, pages, writable. The last two pages are the highest a guest maps:
/// the window's top band ([`GUARD`]) is never mapped.
const REGIONS: [(u64, u64, bool); 3] = [
    (0x10_0000, 2, true),
    (0x10_2000, 1, false),
    (WINDOW - GUARD - 2 * PAGE, 2, true),
];
/// The window's top band that no guest maps (plan 3.1): as wide as the bottom one, below the lowest address a guest
/// maps, so that an access whose base is outside the window but whose immediate brings it in (at most 64 KiB from
/// 2^64 or 1 KiB from 2^32) is in a band and faults where Linux faults.
const GUARD: u64 = wcontainers_abi::LOWEST_ADDRESS;
const PAGE: u64 = 4096;
/// The canaries' byte, in every page mapped outside the window where an access cut wrongly or not at all would land.
const CANARY: u8 = 0xc3;
/// The registers of an access: its data (two for a pair), its base and its index.
const DATA: u8 = 3;
const DATA2: u8 = 4;
const ADDRESS: u8 = 1;
const INDEX: u8 = 2;

/// How an access shape forms its address from its base, its index register and its immediate.
#[derive(Clone, Copy)]
enum Offset {
    /// An unsigned immediate in units of the access, up to the bound.
    Scaled(u32),
    /// A signed immediate in units of `unit` bytes, of this many bits.
    Signed { bits: u32, unit: u64 },
    /// The index register, shifted left by this much.
    Index(u32),
    /// The index register's low half, sign-extended or not.
    Word { signed: bool },
    /// No offset in the address; the base moves by a signed 9-bit immediate after the access.
    After,
}

struct Shape {
    name: &'static str,
    /// Arm's form, and its fields other than the registers and the immediate.
    form: &'static str,
    fields: &'static [(&'static str, u32)],
    offset: Offset,
    bytes: u64,
    write: bool,
    pair: bool,
    /// Whether the base is written back with the address.
    writeback: bool,
}

const fn shape(
    name: &'static str,
    form: &'static str,
    offset: Offset,
    bytes: u64,
    write: bool,
) -> Shape {
    Shape {
        name,
        form,
        fields: &[],
        offset,
        bytes,
        write,
        pair: false,
        writeback: false,
    }
}

/// Arm ARM: a register offset's option field for LSL, UXTW and SXTW.
const LSL: u32 = 0b011;
const UXTW: u32 = 0b010;
const SXTW: u32 = 0b110;

fn shapes() -> Vec<Shape> {
    let x = size_of::<u64>() as u64;
    vec![
        shape(
            "ldr, immediate",
            "LDR_64_ldst_pos",
            Offset::Scaled(4095),
            x,
            false,
        ),
        shape(
            "ldur",
            "LDUR_64_ldst_unscaled",
            Offset::Signed { bits: 9, unit: 1 },
            x,
            false,
        ),
        Shape {
            fields: &[("option", LSL), ("S", 0)],
            ..shape(
                "ldr, register",
                "LDR_64_ldst_regoff",
                Offset::Index(0),
                x,
                false,
            )
        },
        Shape {
            fields: &[("option", LSL), ("S", 1)],
            ..shape(
                "ldr, register shifted",
                "LDR_64_ldst_regoff",
                Offset::Index(3),
                x,
                false,
            )
        },
        Shape {
            fields: &[("option", SXTW), ("S", 0)],
            ..shape(
                "ldr, sxtw",
                "LDR_64_ldst_regoff",
                Offset::Word { signed: true },
                x,
                false,
            )
        },
        Shape {
            fields: &[("option", UXTW), ("S", 0)],
            ..shape(
                "ldr, uxtw",
                "LDR_64_ldst_regoff",
                Offset::Word { signed: false },
                x,
                false,
            )
        },
        Shape {
            fields: &[("option", LSL), ("S", 0)],
            ..shape(
                "ldrb, register",
                "LDRB_32BL_ldst_regoff",
                Offset::Index(0),
                1,
                false,
            )
        },
        Shape {
            pair: true,
            ..shape(
                "ldp",
                "LDP_64_ldstpair_off",
                Offset::Signed { bits: 7, unit: x },
                2 * x,
                false,
            )
        },
        Shape {
            fields: &[("option", LSL), ("S", 0)],
            ..shape(
                "str, register",
                "STR_64_ldst_regoff",
                Offset::Index(0),
                x,
                true,
            )
        },
        Shape {
            pair: true,
            ..shape(
                "stp",
                "STP_64_ldstpair_off",
                Offset::Signed { bits: 7, unit: x },
                2 * x,
                true,
            )
        },
        Shape {
            writeback: true,
            ..shape(
                "ldr, pre-index",
                "LDR_64_ldst_immpre",
                Offset::Signed { bits: 9, unit: 1 },
                x,
                false,
            )
        },
        Shape {
            writeback: true,
            ..shape(
                "str, post-index",
                "STR_64_ldst_immpost",
                Offset::After,
                x,
                true,
            )
        },
    ]
}

/// A word of Arm's form `name` with `fields` set and every other free field zero.
fn word_of(name: &str, fields: &[(&str, u32)]) -> u32 {
    let form = (wcontainers_arm64::forms::all().iter())
        .find(|form| form.name == name)
        .unwrap_or_else(|| panic!("{name}: a form of Arm's table"));
    fields.iter().fold(form.word(0), |word, &(field, value)| {
        let &(_, low, width) = (form.fields.iter())
            .find(|(name, ..)| name == field)
            .unwrap_or_else(|| panic!("{name}: a field {field}"));
        word & !(((1 << width) - 1) << low) | (value & ((1 << width) - 1)) << low
    })
}

/// One drawn access: its word, its registers before it, and the address it reaches, as Linux computes it.
struct Access {
    word: u32,
    base: u64,
    index: u64,
    /// The address before the top byte is set aside.
    address: u64,
}

/// An access of `shape` whose address is `address`: its immediate and index drawn, its base what makes the sum.
fn access(shape: &Shape, address: u64, wide_index: bool, rng: &mut fastrand::Rng) -> Access {
    let mut fields = shape.fields.to_vec();
    fields.extend([("Rt", u32::from(DATA)), ("Rn", u32::from(ADDRESS))]);
    if shape.pair {
        fields.push(("Rt2", u32::from(DATA2)));
    }
    let index = if wide_index {
        rng.u64(..)
    } else {
        rng.u64(..1 << 20)
    };
    let offset = match shape.offset {
        Offset::Scaled(most) => {
            let k = rng.u32(..=most);
            fields.push(("imm12", k));
            u64::from(k) * shape.bytes
        }
        Offset::Signed { bits, unit } => {
            let k = rng.i32(-(1 << (bits - 1))..1 << (bits - 1));
            fields.push((if bits == 7 { "imm7" } else { "imm9" }, k as u32));
            (i64::from(k) as u64).wrapping_mul(unit)
        }
        Offset::Index(shift) => index << shift,
        Offset::Word { signed } => {
            if signed {
                index as u32 as i32 as i64 as u64
            } else {
                u64::from(index as u32)
            }
        }
        Offset::After => {
            fields.push(("imm9", rng.u32(..1 << 9)));
            0
        }
    };
    if matches!(shape.offset, Offset::Index(_) | Offset::Word { .. }) {
        fields.push(("Rm", u32::from(INDEX)));
    }
    Access {
        word: word_of(shape.form, &fields),
        base: address.wrapping_sub(offset),
        index,
        address,
    }
}

/// An address drawn from one of the classes that try to leave the window, and whether the index is drawn wide.
fn address(rng: &mut fastrand::Rng, bytes: u64) -> (&'static str, u64, bool) {
    let (start, pages, _) = REGIONS[rng.usize(..REGIONS.len())];
    let end = start + pages * PAGE;
    let inside = start + rng.u64(..pages * PAGE - bytes);
    let tag = u64::from(rng.u8(1..)) << TAG_LOW;
    match rng.u8(0..11) {
        0 => ("inside", inside, false),
        1 => ("just past a mapping", end - rng.u64(..bytes), false),
        2 => ("just before a mapping", start - rng.u64(1..=bytes), false),
        3 => (
            "past the window's top",
            WINDOW - rng.u64(..bytes) + rng.u64(..2) * PAGE,
            false,
        ),
        4 => ("wrapping to inside", inside, true),
        5 => (
            "an alias past the window",
            (rng.u64(1..1 << (TAG_LOW - WINDOW_BITS)) << WINDOW_BITS) | inside,
            false,
        ),
        6 => ("tagged", tag | inside, rng.bool()),
        7 => ("bit 55 set", (1 << (TAG_LOW - 1)) | inside, false),
        8 => ("its host address", WAY.window_base() + inside, false),
        9 => (
            "one bit past the window",
            (1 << rng.u32(WINDOW_BITS..TAG_LOW)) | inside,
            false,
        ),
        _ => ("anywhere", rng.u64(..), true),
    }
}

/// What Linux does with an access of `bytes` at guest address `address` to the regions: whether it completes.
fn completes(address: u64, bytes: u64, write: bool) -> bool {
    let a = untag_address(address);
    (a..a.saturating_add(bytes)).all(|byte| {
        REGIONS.iter().any(|&(start, pages, writable)| {
            (start..start + pages * PAGE).contains(&byte) && (writable || !write)
        })
    })
}

/// The host pages outside the window that a wrong cut, no cut or a wrong base would reach. The never mapped bytes
/// beside each window stay unmapped, as the way requires: an immediate added after the check reaches into them.
fn canaries() -> Vec<u64> {
    let next = Way::InProcess {
        slot: 2,
        displaced: false,
        kept: Kept::USUAL,
    }
    .window_base();
    let mut pages = Vec::new();
    for &(start, count, _) in &REGIONS {
        for k in 0..count {
            let page = start + k * PAGE;
            pages.extend([page, next + page]);
        }
    }
    pages
}

fn turn_map(address: u64, permissions: ProtFlags) -> Command {
    Command::Map {
        address: address as usize,
        length: PAGE as usize,
        permissions,
        offset: address as usize,
    }
}

/// The window's pages as each case starts: seeded bytes.
fn fill_window(stub: &Stub, seed: u64) -> Vec<Vec<u8>> {
    let mut rng = fastrand::Rng::with_seed(seed);
    REGIONS
        .iter()
        .map(|&(start, pages, _)| {
            let bytes: Vec<u8> = (0..pages * PAGE).map(|_| rng.u8(..)).collect();
            rustix::io::pwrite(stub.memory(), &bytes, WAY.window_base() + start)
                .expect("the window");
            bytes
        })
        .collect()
}

fn read_host(stub: &Stub, at: u64, into: &mut [u8]) {
    rustix::io::pread(stub.memory(), into, at).expect("a read of the stub's memory");
}

/// The window's bytes at guest address `at`, from the region holding it.
fn guest_bytes(stub: &Stub, at: u64, into: &mut [u8]) {
    read_host(stub, WAY.window_base() + at, into);
}

/// The syndrome's exception classes and fields that tell a data abort's kind (Arm ARM, ESR_EL1).
const CLASS_SHIFT: u32 = 26;
const DATA_ABORT: u64 = 0x24;
const WRITE_NOT_READ: u64 = 1 << 6;
const CACHE_MAINTENANCE: u64 = 1 << 8;
const STATUS: u64 = 0x3f;
const ALIGNMENT_STATUS: u64 = 0x21;

/// Runs one drawn access and checks it against Linux's outcome and against every canary; gives its failure.
#[expect(clippy::too_many_arguments, reason = "the case and where it runs")]
fn run_access(
    stub: &Stub,
    shape: &Shape,
    at: u64,
    code: &HostCode,
    class: &str,
    a: &Access,
    window: &[Vec<u8>],
    tally: &mut std::collections::BTreeMap<&'static str, u64>,
    rng: &mut fastrand::Rng,
) -> Option<String> {
    let case = format!(
        "{} at {:#x} ({class}, base {:#x}, index {:#x})",
        shape.name, a.address, a.base, a.index
    );
    let mut block = RegisterBlock::ZERO;
    block.x[usize::from(ADDRESS)] = a.base;
    block.x[usize::from(INDEX)] = a.index;
    block.x[usize::from(DATA)] = rng.u64(..);
    block.x[usize::from(DATA2)] = rng.u64(..);
    block.x[BASE] = WAY.window_base();
    block.sp = WAY.window_base() + REGIONS[0].0 + PAGE;
    let before = block.x;
    stub.set_registers(0, &block);
    stub.set_translated_page(0, &RegisterPage::default());
    let end = stub.enter(0, &[], at as usize).expect("enter");
    let (h, _) = stub.scalars(0);
    // Nothing outside the window changed, and the window only where the access completed.
    for page in canaries() {
        let mut bytes = vec![0; PAGE as usize];
        read_host(stub, page, &mut bytes);
        if bytes.iter().any(|&b| b != CANARY) {
            return Some(format!(
                "{case}: the canary page {page:#x} changed ({end:?})"
            ));
        }
    }
    let first = untag_address(a.address);
    let expected = completes(a.address, shape.bytes, shape.write);
    for (&(start, pages, _), was) in REGIONS.iter().zip(window) {
        let mut now = vec![0; (pages * PAGE) as usize];
        guest_bytes(stub, start, &mut now);
        let written = |at: u64| shape.write && (first..first + shape.bytes).contains(&at);
        if let Some(k) = (0..now.len()).find(|&k| now[k] != was[k] && !written(start + k as u64)) {
            return Some(format!(
                "{case}: the window's byte {:#x} changed ({end:?})",
                start + k as u64
            ));
        }
    }
    let answer = TurnEnd::Called {
        entry: InsideCall::Answer.code().into(),
    };
    if end == answer {
        *tally.entry("completed").or_default() += 1;
        if !expected {
            return Some(format!("{case}: completed where Linux faults"));
        }
        let mut bytes = vec![0; shape.bytes as usize];
        guest_bytes(stub, first, &mut bytes);
        let words: Vec<u64> = bytes
            .chunks(8)
            .map(|c| c.iter().rev().fold(0, |w, &b| w << 8 | u64::from(b)))
            .collect();
        let data = if shape.pair {
            vec![before[usize::from(DATA)], before[usize::from(DATA2)]]
        } else {
            vec![before[usize::from(DATA)] & (u64::MAX >> (64 - 8 * shape.bytes.min(8)))]
        };
        let held = if shape.pair {
            vec![h.x[usize::from(DATA)], h.x[usize::from(DATA2)]]
        } else {
            vec![h.x[usize::from(DATA)]]
        };
        let (wanted, got) = if shape.write {
            (data, words)
        } else {
            (words, held)
        };
        if wanted != got {
            return Some(format!("{case}: moved {got:x?}, wanted {wanted:x?}"));
        }
        let base = h.x[usize::from(ADDRESS)];
        let wanted_base = match (shape.writeback, shape.offset) {
            (false, _) => a.base,
            (true, Offset::After) => {
                let imm = (a.word >> 12) & 0x1ff;
                a.base
                    .wrapping_add((((imm << 23) as i32) >> 23) as i64 as u64)
            }
            (true, _) => a.address,
        };
        return (base != wanted_base)
            .then(|| format!("{case}: base {base:#x}, wanted {wanted_base:#x}"));
    }
    let TurnEnd::Fault { address, .. } = end else {
        return Some(format!("{case}: ended {end:?}"));
    };
    if expected {
        return Some(format!(
            "{case}: faulted at {address:#x} where Linux completes"
        ));
    }
    let syndrome = stub.fault_syndrome(0);
    if syndrome >> CLASS_SHIFT != DATA_ABORT {
        return Some(format!("{case}: faulted with syndrome {syndrome:#x}"));
    }
    let piece_view = Piece {
        base: at,
        length: code.bytes.len(),
        spans: &code.spans,
    };
    let taken = Taken::Access {
        alignment: syndrome & STATUS == ALIGNMENT_STATUS,
        fault: address,
        write: syndrome & (WRITE_NOT_READ | CACHE_MAINTENANCE) == WRITE_NOT_READ,
    };
    let resolution = in_process::resolve(
        WAY,
        &piece_view,
        taken,
        &h,
        &stub.translated_page(0),
        &[0; AFTER_PAGE],
        &mut FloatStatus::default(),
        &mut |at, into| guest_bytes(stub, at, into),
    );
    let (fault, state) = match resolution {
        Some(Resolution::Outside { address, state }) => {
            *tally.entry("outside").or_default() += 1;
            (address, state)
        }
        Some(Resolution::Resolved(Resolved::Stop(Stop::MemoryFault { address, .. }, state))) => {
            *tally.entry("faulted in the window").or_default() += 1;
            (address, state)
        }
        other => return Some(format!("{case}: resolved to {other:?}")),
    };
    // The guest faults at a byte of its access, with its registers as they were before it.
    let within = fault.wrapping_sub(first) < shape.bytes;
    let differing: Vec<usize> = (0..LINK + 1)
        .filter(|&r| !WITHHELD.contains(&(r as u8)) && state.x[r] != before[r])
        .collect();
    if !within || state.pc != GUEST || !differing.is_empty() {
        return Some(format!(
            "{case}: the guest faulted at {fault:#x}, pc {:#x}, registers {differing:?} changed",
            state.pc
        ));
    }
    None
}

#[test]
fn an_access_reaches_only_the_window_s_mapped_pages_as_linux_does() {
    let cases = setting(CASES_VARIABLE, DEFAULT_CASES);
    let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
    let mut rng = fastrand::Rng::with_seed(seed);
    let probe = HostProbe::new().expect("the host is refused");
    let zygote = zygote(&probe);
    let stub = zygote.fork_stub().expect("fork");
    let mut area = CodeArea::new().expect("code area");
    area.write_slots(&probe, WAY.slot(GUEST + INSTRUCTION) as usize, &ending());
    let shapes = shapes();
    stub.send_block(area.file()).expect("send the code area");
    stub.send_block(area.file()).expect("send the slot table");
    let base = WAY.window_base();
    let mut commands = vec![Command::MapCode, Command::MapSlots];
    for &(start, pages, writable) in &REGIONS {
        let permissions = if writable {
            ProtFlags::READ | ProtFlags::WRITE
        } else {
            ProtFlags::READ
        };
        commands.extend((0..pages).map(|k| turn_map(base + start + k * PAGE, permissions)));
    }
    for page in canaries() {
        commands.push(turn_map(page, ProtFlags::READ | ProtFlags::WRITE));
        rustix::io::pwrite(stub.memory(), &[CANARY; PAGE as usize], page).expect("a canary");
    }
    let count = commands.len() as u64;
    assert_eq!(
        stub.turn(0, &commands),
        Ok(TurnEnd::Done {
            commands_applied: count
        })
    );
    let mut failures = Vec::new();
    let mut tally = std::collections::BTreeMap::<&'static str, u64>::new();
    let mut by_class = std::collections::BTreeMap::<&'static str, u64>::new();
    for shape in &shapes {
        for case in 0..cases {
            let window = fill_window(&stub, seed ^ case);
            let (class, address, wide) = address(&mut rng, shape.bytes);
            *by_class.entry(class).or_default() += 1;
            let a = access(shape, address, wide, &mut rng);
            let code = translate_native_for::<Arm64>(
                &a.word.to_le_bytes(),
                GUEST,
                LookAhead::Full,
                true,
                &WAY,
                &(),
            )
            .expect("translated");
            let (words, rest) = code.bytes.as_chunks::<{ size_of::<u32>() }>();
            assert!(rest.is_empty(), "host code is whole words");
            let mut words: Vec<u32> = words.iter().map(|w| u32::from_le_bytes(*w)).collect();
            to_process_entries(&mut words);
            area.write(&probe, CODE_AREA, &words);
            if let Some(failure) = run_access(
                &stub,
                shape,
                CODE_AREA as u64,
                &code,
                class,
                &a,
                &window,
                &mut tally,
                &mut rng,
            ) {
                failures.push(failure);
            }
        }
    }
    stub.end(0).expect("end");
    eprintln!(
        "seed {seed:#x}: {} shapes, {} cases, by class {by_class:?}, outcomes {tally:?}, {} failing",
        shapes.len(),
        shapes.len() as u64 * cases,
        failures.len()
    );
    for failure in failures.iter().take(20) {
        eprintln!("{failure}");
    }
    assert!(failures.is_empty(), "{} cases failed", failures.len());
}
