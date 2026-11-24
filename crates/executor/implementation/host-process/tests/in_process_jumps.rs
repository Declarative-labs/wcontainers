//! A jump through a register in the in-process way's translated code (docs/native-without-virtual-machine.md
//! section 3.2), on hardware: `br`, `blr` and `ret` through every register the way does not withhold, translated for
//! the in-process way in slot 1 (whose slots lie in the process way's slot table) and run in a stub, to seeded
//! targets chosen to defeat the window: inside it (aligned, misaligned, tagged), just past it, one bit past it, far
//! outside it, the guest address or the host address of a target whose slot is written, and the host addresses a guest must never reach: the
//! middle of translated code, the floor's entries, the slot table, the register pages and the zygote. Every run must
//! end at the entry slot of the target's window address (the ending written there, or the fault of an unwritten or
//! misaligned slot) or, for a target outside the window, at the floor's entry for an unreachable target, which the
//! stop crate resolves to the guest landing at the target; never anywhere else. The guest's registers must be its own
//! there, x30 the return address after a `blr`. A stub has no floor at the in-process way's entries, so each call
//! into the floor is pointed at the process way's entry of the same call, its only change to the translated code.
//! Knobs `WCHP_CASES` targets per piece, `WCHP_SEED`.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use wcontainers_abi::RegisterBlock;
use wcontainers_arm64::encode::{self, form};
use wcontainers_arm64::thread_state::{RegisterPage, SCRATCH, TAG_LOW, untag_address};
use wcontainers_arm64::way::{
    IN_PROCESS_ENTRIES, PROCESS_ENTRIES, PROCESS_ENTRY_BYTES, WINDOW_BITS,
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
    let zygote = Zygote::start(&probe).expect("zygote");
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
