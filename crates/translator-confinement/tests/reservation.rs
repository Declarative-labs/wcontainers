//! What form I's instructions reach below a window, and whether the in-process pool reserves it (a security defect
//! found on 2026-11-27: accesses reached up to 60 KiB below a window's base, where nothing was reserved). The
//! native track runs this: `cargo test -p wcontainers-translator-confinement --test reservation`.
//!
//! The first two tests measure the reach with the checker over the translator's pieces of words drawn from every form.
//! The third (ARM64 Linux) reads where the pool put a new memory (`/proc/self/maps`), makes the checker's [`Place`] of
//! that layout, and checks every piece against it: it fails on a pool that reserves nothing below a window, and passes
//! on one that reserves the 64 KiB.

use wcontainers_arm64::way::{Kept, SLOT_BITS};
use wcontainers_arm64::{Arm64, Way, forms};
use wcontainers_translator::{LookAhead, NativeRequest, translate_instructions};
use wcontainers_translator_confinement::{Breach, Place, State, check};

const SLOT: u16 = 3;
/// The pool's reservation below each window (`in-process/memory.rs`, `BELOW`).
const POOL_BELOW: u64 = 1 << 16;
/// The most a stack step moves the stack pointer below the window before its probe (carry.rs `STACK_STEP`).
const STACK_STEP: u64 = 60 << 10;

/// Words drawn from every form, each also with its base register (bits 5 to 9) the stack pointer and with its
/// destination (bits 0 to 4) register 31; and every `add` and `sub` of `sp` by an immediate, shifted or not (rule c
/// keeps a step of at most 60 KiB in reach with a probe alone; a larger one must be checked, and random immediates
/// seldom draw either), with the largest negative offsets from `sp`.
fn words() -> Vec<u32> {
    let mut rng = fastrand::Rng::with_seed(0x5e5e_7e5e);
    let mut words = Vec::new();
    for form in forms::all() {
        for _ in 0..4 {
            let word = form.word(rng.u32(..));
            words.extend([word, form.word(word | 0x1f << 5), form.word(word | 0x1f)]);
        }
    }
    for imm in 0..4096 {
        // `sub sp, sp, #imm{, lsl #12}` and `add sp, sp, #imm{, lsl #12}`.
        for shift in [0, 1 << 22] {
            words.extend([
                0xd100_03ff | shift | imm << 10,
                0x9100_03ff | shift | imm << 10,
            ]);
        }
    }
    words.extend([
        0xada0_07e0, // stp q0, q1, [sp, #-1024]!
        0xad60_07e0, // ldp q0, q1, [sp, #-1024]
        0x3cd0_03e0, // ldur q0, [sp, #-256]
        0xf850_0fe0, // ldr x0, [sp, #-256]!
    ]);
    words
}

/// Form I's pieces of [`words`], in the window's way and the displaced way.
fn pieces(slot: u16) -> Vec<(Vec<u8>, bool)> {
    let request = NativeRequest {
        look_ahead: LookAhead::None,
        exact_replay: true,
        stop: false,
        step: false,
    };
    let mut made = Vec::new();
    for word in words() {
        for displaced in [false, true] {
            let way = Way::InProcess {
                slot,
                displaced,
                kept: Kept::USUAL,
            };
            if let Some(code) =
                translate_instructions::<Arm64>(&word.to_le_bytes(), 0x40_1000, &request, &way, &())
            {
                made.push((code.bytes().to_vec(), displaced));
            }
        }
    }
    made
}

fn place(base: u64, below: u64, displaced: bool) -> Place {
    Place {
        base,
        frame: 0x7fff_0000_1000,
        code: (1 << 45) + (1 << 34),
        below,
        displaced,
    }
}

/// The breaches of every piece entered at its first instruction under the entry rule.
fn breaches(pieces: &[(Vec<u8>, bool)], base: u64, below: u64) -> Vec<Breach> {
    pieces
        .iter()
        .flat_map(|(bytes, displaced)| {
            let place = place(base, below, *displaced);
            check(bytes, &place, &[(0, State::entry(&place))]).0
        })
        .map(|found| found.breach)
        .collect()
}

/// The fewest bytes below the window that, reserved, leave no piece breaching.
fn reach_below(pieces: &[(Vec<u8>, bool)]) -> u64 {
    let base = u64::from(SLOT) << SLOT_BITS;
    let (mut lo, mut hi) = (0, 1 << 20);
    assert_eq!(breaches(pieces, base, hi), [], "a megabyte below is enough");
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if breaches(pieces, base, mid).is_empty() {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo
}

#[test]
fn a_push_at_the_window_s_base_writes_below_it() {
    // `stp x29, x30, [sp, #-16]!` with the guest's stack pointer anywhere in the window, 0 included: the instruction is
    // copied as it is (rule a), so it writes the 16 bytes below the base.
    let request = NativeRequest {
        look_ahead: LookAhead::None,
        exact_replay: true,
        stop: false,
        step: false,
    };
    let way = Way::InProcess {
        slot: SLOT,
        displaced: false,
        kept: Kept::USUAL,
    };
    let code = translate_instructions::<Arm64>(
        &0xa9bf_7bfdu32.to_le_bytes(),
        0x40_1000,
        &request,
        &way,
        &(),
    )
    .expect("stp translates");
    let base = u64::from(SLOT) << SLOT_BITS;
    let piece = [(code.bytes().to_vec(), false)];
    let kinds = |below: u64| {
        let mut found = breaches(&piece, base, below);
        found.dedup();
        found
    };
    assert_eq!(kinds(0), [Breach::AccessOutside]);
    assert_eq!(kinds(15), [Breach::AccessOutside]);
    assert_eq!(kinds(16), []);
}

#[test]
fn the_pieces_reach_a_stack_step_below_a_window_which_the_pool_s_reservation_covers() {
    let pieces = pieces(SLOT);
    assert!(pieces.len() > 60_000, "{} pieces", pieces.len());
    let reach = reach_below(&pieces);
    // A full stack step from the window's base leaves the stack pointer 60 KiB below it, where its probe (`ldr wzr,
    // [sp]`) reads; nothing reaches lower.
    assert_eq!(reach, STACK_STEP);
    assert!(reach <= POOL_BELOW, "{reach}");
    assert!(!breaches(&pieces, u64::from(SLOT) << SLOT_BITS, 0).is_empty());
    println!("reach below the window: {reach} bytes");
}

/// The ranges `/proc/self/maps` lists, with whether each is inaccessible (`---`).
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn mappings() -> Vec<(u64, u64, bool)> {
    std::fs::read_to_string("/proc/self/maps")
        .expect("this process's mappings")
        .lines()
        .map(|line| {
            let mut fields = line.split_whitespace();
            let range = fields.next().expect("a range");
            let perms = fields.next().expect("permissions");
            let (start, end) = range.split_once('-').expect("start-end");
            (
                u64::from_str_radix(start, 16).expect("hex"),
                u64::from_str_radix(end, 16).expect("hex"),
                perms.starts_with("---"),
            )
        })
        .collect()
}

/// Where the held run around `[from, to)` starts going down from `from`, every mapping met inaccessible.
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn held_down_from(maps: &[(u64, u64, bool)], from: u64) -> u64 {
    let mut at = from;
    while let Some(&(start, _, _)) = maps
        .iter()
        .find(|&&(start, end, none)| none && start < at && at <= end)
    {
        at = start;
    }
    at
}

/// Whether every byte of `[from, to)` is in a mapping, inaccessible unless `any` allows otherwise.
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn held(maps: &[(u64, u64, bool)], from: u64, to: u64, any: bool) -> bool {
    let mut at = from;
    while at < to {
        match maps
            .iter()
            .find(|&&(start, end, none)| start <= at && at < end && (any || none))
        {
            Some(&(_, end, _)) => at = end,
            None => return false,
        }
    }
    true
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[test]
fn the_pool_reserves_what_every_piece_reaches_around_a_window() {
    use wcontainers_executor_in_process::memory::InProcessPool;
    use wcontainers_translator_confinement::SLOT_BYTES;
    let pool = InProcessPool::new().expect("a host with the guest's pages");
    let (memory, _table) = pool.create_memory().expect("a free slot");
    let base = memory.window_base();
    let maps = mappings();
    // The layout: the run of inaccessible mappings reaching down from the window's base, then the slot from there.
    let reserved = held_down_from(&maps, base);
    let below = base - reserved;
    let window = 1 << (SLOT_BITS - 1);
    assert!(
        held(&maps, base, base + window, true),
        "the window at {base:#x} is mapped"
    );
    assert!(
        held(&maps, base + window, reserved + SLOT_BYTES, false),
        "the slot above the window at {base:#x}, up to {:#x}, is held inaccessible",
        reserved + SLOT_BYTES
    );
    // Every piece of that slot, checked against that layout.
    let slot = u16::try_from(base >> SLOT_BITS).expect("a slot");
    let pieces = pieces(slot);
    let found = breaches(&pieces, base, below);
    assert_eq!(
        found,
        [],
        "with {below} bytes reserved below the window at {base:#x}, {} pieces' accesses reach where nothing is held",
        found.len()
    );
}
