//! The bounds-check-bypass test on translated code (plan 13, W2's bar; brief-native.md, "Speculation and inline
//! memory checks"): a guest gadget behind its own slow bounds check, one per access shape, translated for the
//! in-process way with its window at 0 (slot 0, whose slot table is the process way's) and, as the control, for the
//! process way, whose access is the guest's own and uncut. Both run natively in one stub. Each round flushes a probe
//! array, calls the gadget with in-bounds addresses and then once with an address outside the window (in what
//! would be the next slot's window), whose byte the refused call reads speculatively, then times one line of the
//! probe array. Under the in-process way the secret's line must be cached no more often than an untouched line;
//! under the process way it must be, which shows the test sees a leak. A measurement, run on its own with
//! `--ignored --nocapture`; knobs `WCHP_ROUNDS` rounds per shape and way, `WCHP_SEED`.
//!
//! As in the earlier host program of the same sequences (scratch w1/run6/specarm.c): one line is timed per round,
//! the probe array is one of several copies chosen at random, its lines lie at a different offset in each page,
//! the secret and the in-window byte under the cut address are drawn anew each round, and the outside call comes at
//! random after a run of in-bounds ones, since these cores learn fixed orders.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use rustix::mm::ProtFlags;
use wcontainers_abi::RegisterBlock;
use wcontainers_arm64::encode::{self, form};
use wcontainers_arm64::thread_state::{Register, SCRATCH};
use wcontainers_arm64::way::{PROCESS_ENTRIES, PROCESS_ENTRY_BYTES};
use wcontainers_arm64::{Arm64, Instruction, SystemRegister, Way, decode, forms};
use wcontainers_executor_host_process::code::CodeArea;
use wcontainers_executor_host_process::layout::CODE_AREA;
use wcontainers_executor_host_process::pages::Command;
use wcontainers_executor_host_process::probe::HostProbe;
use wcontainers_executor_host_process::stub::{Stub, TurnEnd};
use wcontainers_executor_host_process::zygote::Zygote;
use wcontainers_guest_cpu::HostCode;
use wcontainers_guest_cpu::position::InsideCall;
use wcontainers_translator::{LookAhead, translate_native_for};

const ROUNDS_VARIABLE: &str = "WCHP_ROUNDS";
/// Rounds per shape and way unless told otherwise.
const DEFAULT_ROUNDS: u64 = 2000;
const SEED_VARIABLE: &str = "WCHP_SEED";
const DEFAULT_SEED: u64 = 0x5eed_b0b5;
/// The gadget's calls in one round.
const CALLS_PER_ROUND: usize = 64;

const PAGE: u64 = 4096;
const LINE: u64 = 64;
/// The guest pages the in-bounds calls read, every byte [`TRAINING`].
const GOOD: u64 = 0x2_0000;
const TRAINING: u8 = 0x33;
/// The bound of the gadget's check, alone on its page.
const LIMIT: u64 = 0x3_0000;
/// The addresses of one round's calls, then the lines its flush clears.
const CALLS: u64 = 0x4_0000;
const FLUSHED: u64 = 0x5_0000;
/// The window's byte the cut access reads in place of the secret: the secret's address less 2^32 times its slot.
const WINDOW_BYTE: u64 = 0x10_1000;
/// The secret, where the next slot's window would be: outside the window, mapped in the stub.
const SECRET: u64 = (1 << 33) + WINDOW_BYTE;
/// The probe array's copies, one chosen each round so that no round reuses the lines of the one before.
const PROBE: u64 = 0x100_0000;
const COPIES: u64 = 32;
const LINES: u64 = 256;
const COPY: u64 = LINES * PAGE;
const STACK: u64 = 0x80_0000;
const STACK_BYTES: u64 = 64 * 1024;
/// Where each shape's gadget lies in the guest's addresses.
const GUEST: u64 = 0x40_0000;
const GUEST_ROOM: u64 = 0x1000;
/// Where each translation is put in the code area, after the harness.
const PIECE_ROOM: usize = 64 * 1024;

/// The gadget's registers: the address, the bound's address, the value read, the probe copy, the offset of the
/// register-offset shape, and the temporaries.
const ADDRESS: Register = 0;
/// Where the harness keeps the call's address for a gadget that loads it.
const CALL: Register = 1;
const OFFSET: Register = 7;
const BOUND: Register = 8;
const BOUND_VALUE: Register = 9;
const VALUE: Register = 10;
const PROBE_BASE: Register = 11;
const SAVED_STACK: Register = 12;
const TEMPORARY: Register = 13;
/// The harness's registers, which the gadget leaves alone.
const CALL_LIST: Register = 19;
const CALLS_LEFT: Register = 20;
const GADGET: Register = 21;
const TIMED_LINE: Register = 22;
const SECRET_REGISTER: Register = 25;
const WINDOW_REGISTER: Register = 26;
const FLUSH_LIST: Register = 27;
const FLUSHES_LEFT: Register = 17;
const LOADED: Register = 14;
const BEFORE: Register = 13;
const AFTER: Register = 16;
const STACK_POINTER: Register = 31;

/// Arm ARM: DSB's CRm for the inner shareable domain, ISB's for the full system, PRFM's PLDL1KEEP, and DC CIVAC as
/// a SYS instruction's op1, CRn, CRm and op2.
const INNER_SHAREABLE: u32 = 0b1011;
const FULL_SYSTEM: u32 = 0b1111;
const PREFETCH_L1_KEEP: u32 = 0;
const CLEAN_INVALIDATE_TO_COHERENCE: [u32; 4] = [3, 7, 14, 1];
/// UMOV's imm5 for byte element 0.
const BYTE_ELEMENT_0: u32 = 0b00001;

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| {
        let v = v.trim();
        v.strip_prefix("0x")
            .map_or_else(|| v.parse(), |hex| u64::from_str_radix(hex, 16))
            .expect("a number")
    })
}

/// A word of Arm's form `name` with `fields` set and every other free field zero.
fn word_of(name: &str, fields: &[(&str, u32)]) -> u32 {
    let form = (forms::all().iter())
        .find(|form| form.name == name)
        .expect("a form of Arm's table");
    fields.iter().fold(form.word(0), |word, &(field, value)| {
        let &(_, low, width) = (form.fields.iter())
            .find(|(name, ..)| name == field)
            .expect("a field of the form");
        assert!(u64::from(value) < 1 << width, "{field} fits");
        word & !(((1 << width) - 1) << low) | value << low
    })
}

fn dsb() -> u32 {
    word_of("DSB_BO_barriers", &[("CRm", INNER_SHAREABLE)])
}

fn isb() -> u32 {
    word_of("ISB_BI_barriers", &[("CRm", FULL_SYSTEM)])
}

fn prefetch(rn: Register) -> u32 {
    word_of(
        "PRFM_P_ldst_pos",
        &[("Rt", PREFETCH_L1_KEEP), ("Rn", rn.into())],
    )
}

fn clean_invalidate(rt: Register) -> u32 {
    let [op1, crn, crm, op2] = CLEAN_INVALIDATE_TO_COHERENCE;
    word_of(
        "SYS_CR_systeminstrs",
        &[
            ("op1", op1),
            ("CRn", crn),
            ("CRm", crm),
            ("op2", op2),
            ("Rt", rt.into()),
        ],
    )
}

/// `mrs rt, cntvct_el0`: the move whose register decodes as the counter, found among every register a move names.
fn read_counter(rt: Register) -> u32 {
    let mrs = (forms::all().iter())
        .find(|form| form.name == "MRS_RS_systemmove")
        .expect("a form of Arm's table");
    (0..1 << 16)
        .map(|system| mrs.word(system << 5) & !0x1f | u32::from(rt))
        .find(|&word| {
            matches!(
                decode(word),
                Instruction::ReadSystem {
                    register: SystemRegister::Counter,
                    ..
                }
            )
        })
        .expect("the counter's move")
}

fn load(form: encode::Form, rt: Register, rn: Register, offset: u64, unit: u64) -> u32 {
    encode::load_store(form, rt, rn, offset, unit).expect("an access fits")
}

fn add(rd: Register, rn: Register, value: i64) -> u32 {
    encode::add_immediate(rd, rn, value).expect("fits")
}

/// `add rd, rn, rm, lsl #shift` of 64-bit registers.
fn add_shifted(rd: Register, rn: Register, rm: Register, shift: u32) -> u32 {
    encode::add_extended(false, rd, rn, rm, false, u64::BITS, shift).expect("registers fit")
}

fn umov_byte_0(rd: Register) -> u32 {
    word_of(
        "UMOV_asimdins_W_w",
        &[("imm5", BYTE_ELEMENT_0), ("Rn", 0), ("Rd", rd.into())],
    )
}

/// One access shape: its name, its guest words reading a byte at the address into the value register, the offset
/// its address is given less (added back by the access), and whether the gadget loads the address from memory the
/// harness flushed, so that the in-process way's own check waits on it too and can be passed under speculation.
#[derive(Clone)]
struct Shape {
    name: &'static str,
    access: Vec<u32>,
    less: u64,
    loaded: bool,
}

/// Every shape with its address in a register, then again with its address loaded.
fn shapes() -> Vec<Shape> {
    let in_register = in_register_shapes();
    let loaded = in_register.iter().map(|shape| Shape {
        loaded: true,
        ..shape.clone()
    });
    in_register.iter().cloned().chain(loaded).collect()
}

fn in_register_shapes() -> Vec<Shape> {
    let byte_at_base = load(form::LDRB, VALUE, ADDRESS, 0, 1);
    let shape = |name, access, less| Shape {
        name,
        access,
        less,
        loaded: false,
    };
    vec![
        shape("at base", vec![byte_at_base], 0),
        shape(
            "immediate offset",
            vec![load(form::LDRB, VALUE, ADDRESS, 8, 1)],
            8,
        ),
        shape(
            "register offset",
            vec![encode::register_offset_at_base(byte_at_base, ADDRESS, OFFSET).expect("fits")],
            8,
        ),
        shape(
            "pair",
            vec![encode::load_pair(form::LDP_64, VALUE, TEMPORARY, ADDRESS)],
            0,
        ),
        shape(
            "acquire",
            vec![word_of(
                "LDARB_LR32_ldstord",
                &[
                    ("Rs", 0x1f),
                    ("Rt2", 0x1f),
                    ("Rn", ADDRESS.into()),
                    ("Rt", VALUE.into()),
                ],
            )],
            0,
        ),
        shape(
            "vector",
            vec![
                word_of("LDR_Q_ldst_pos", &[("Rn", ADDRESS.into()), ("Rt", 0)]),
                umov_byte_0(VALUE),
            ],
            0,
        ),
        shape(
            "structure",
            vec![
                word_of(
                    "LD1_asisdlse_R1_1v",
                    &[("Q", 1), ("size", 0), ("Rn", ADDRESS.into()), ("Rt", 0)],
                ),
                umov_byte_0(VALUE),
            ],
            0,
        ),
        shape(
            "stack pointer",
            vec![
                add(SAVED_STACK, STACK_POINTER, 0),
                add(STACK_POINTER, ADDRESS, 0),
                load(form::LDRB, VALUE, STACK_POINTER, 0, 1),
                add(STACK_POINTER, SAVED_STACK, 0),
            ],
            0,
        ),
    ]
}

/// The gadget: the address loaded if the shape says so, the guest's own check against a bound it loads (flushed
/// before each call, so the check waits on memory), the access, and the probe array's line of the byte read, at a
/// different offset in each page.
fn gadget(shape: &Shape) -> Vec<u32> {
    let access = &shape.access;
    let mut tail = vec![
        encode::and_immediate(VALUE, VALUE, 0xff).expect("a logical immediate"),
        add_shifted(TEMPORARY, VALUE, VALUE, 2),
        add_shifted(TEMPORARY, TEMPORARY, VALUE, 3),
        encode::ubfiz(TEMPORARY, TEMPORARY, LINE.trailing_zeros(), 6),
        encode::ubfiz(VALUE, VALUE, PAGE.trailing_zeros(), 8),
        add_shifted(VALUE, VALUE, TEMPORARY, 0),
    ];
    tail.push(
        encode::register_offset_at_base(
            load(form::LDRB, VALUE, PROBE_BASE, 0, 1),
            PROBE_BASE,
            VALUE,
        )
        .expect("fits"),
    );
    let skip = (access.len() + tail.len() + 1) as i64 * 4;
    let mut words = Vec::new();
    if shape.loaded {
        words.push(load(form::LDR, ADDRESS, CALL, 0, 8));
    }
    words.extend([
        load(form::LDR, BOUND_VALUE, BOUND, 0, 8),
        encode::add_extended(true, BOUND_VALUE, ADDRESS, BOUND_VALUE, false, u64::BITS, 0)
            .expect("registers fit"),
        // Below the bound: the difference is negative.
        encode::test_branch_nonzero(BOUND_VALUE, u64::BITS - 1, 8).expect("fits"),
        encode::branch(false, skip).expect("fits"),
    ]);
    words.extend_from_slice(access);
    words.extend(tail);
    words
}

fn host_words(code: &HostCode) -> Vec<u32> {
    let (words, rest) = code.bytes.as_chunks::<{ size_of::<u32>() }>();
    assert!(rest.is_empty(), "host code is whole words");
    words.iter().map(|w| u32::from_le_bytes(*w)).collect()
}

/// A jump to host address `to` through the scratch register, which translated code owns.
fn jump(to: u64) -> Vec<u32> {
    let mut words = encode::move_address(SCRATCH, to);
    words.push(encode::branch_register(form::BR, SCRATCH));
    words
}

/// The harness at `at`, and where a gadget's end comes back to it. It flushes the listed lines, then calls the
/// gadget once per listed address (prefetching the secret and the window's byte, flushing the bound), then times
/// the line it is given and calls the floor, which ends the turn.
fn harness(at: u64) -> (Vec<u32>, u64) {
    let here = |words: &Vec<u32>| at + words.len() as u64 * 4;
    let mut words = Vec::new();
    let flush = here(&words);
    words.extend([
        load(form::LDR, TEMPORARY, FLUSH_LIST, 0, 8),
        add(FLUSH_LIST, FLUSH_LIST, 8),
        clean_invalidate(TEMPORARY),
        add(FLUSHES_LEFT, FLUSHES_LEFT, -1),
    ]);
    let back = flush as i64 - here(&words) as i64;
    words.push(encode::compare_branch(true, FLUSHES_LEFT, back).expect("fits"));
    words.extend([dsb(), isb()]);
    let call = here(&words);
    words.extend([
        load(form::LDR, ADDRESS, CALL_LIST, 0, 8),
        add(CALL, CALL_LIST, 0),
        add(CALL_LIST, CALL_LIST, 8),
        prefetch(SECRET_REGISTER),
        prefetch(WINDOW_REGISTER),
        clean_invalidate(BOUND),
        clean_invalidate(CALL),
        dsb(),
        isb(),
        encode::branch_register(form::BR, GADGET),
    ]);
    let returned = here(&words);
    words.push(add(CALLS_LEFT, CALLS_LEFT, -1));
    let back = call as i64 - here(&words) as i64;
    words.push(encode::compare_branch(true, CALLS_LEFT, back).expect("fits"));
    words.extend([
        dsb(),
        isb(),
        read_counter(BEFORE),
        isb(),
        load(form::LDRB, LOADED, TIMED_LINE, 0, 1),
        dsb(),
        isb(),
        read_counter(AFTER),
    ]);
    let entry = PROCESS_ENTRIES + u64::from(InsideCall::Answer.code()) * PROCESS_ENTRY_BYTES;
    words.extend(encode::move_address(SCRATCH, entry));
    words.push(encode::branch_register(form::BLR, SCRATCH));
    (words, returned)
}

/// The host address of line `k` of probe copy `copy`.
fn line(copy: u64, k: u64) -> u64 {
    PROBE + copy * COPY + k * PAGE + (13 * k % (PAGE / LINE)) * LINE
}

/// Which line a round times.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Timed {
    Secret,
    Window,
    Other,
    Training,
}

const TIMED: [Timed; 4] = [Timed::Secret, Timed::Window, Timed::Other, Timed::Training];

/// One round: the secret and window bytes, the calls ending with the outside one, the line timed; its time in
/// counter ticks.
fn round(
    stub: &Stub,
    rng: &mut fastrand::Rng,
    gadget: u64,
    shape: &Shape,
    timed: Timed,
    at: u64,
) -> u64 {
    let memory = stub.memory();
    let secret = loop {
        let b = rng.u8(..);
        if b != TRAINING {
            break b;
        }
    };
    let window = loop {
        let b = rng.u8(..);
        if b != TRAINING && b != secret {
            break b;
        }
    };
    rustix::io::pwrite(memory, &[secret], SECRET).expect("the secret");
    rustix::io::pwrite(memory, &[window], WINDOW_BYTE).expect("the window's byte");
    let copy = rng.u64(..COPIES);
    let k = match timed {
        Timed::Secret => u64::from(secret),
        Timed::Window => u64::from(window),
        Timed::Training => u64::from(TRAINING),
        Timed::Other => loop {
            let k = rng.u8(..);
            if ![secret, window, TRAINING].contains(&k) {
                break u64::from(k);
            }
        },
    };
    // One call in four outside, at random, the last always.
    let mut calls: Vec<u64> = (1..CALLS_PER_ROUND)
        .map(|_| {
            if rng.u32(..4) == 0 {
                SECRET
            } else {
                GOOD + (rng.u64(..PAGE) & !15)
            }
        })
        .collect();
    calls.push(SECRET);
    for call in &mut calls {
        *call -= shape.less;
    }
    let list: Vec<u8> = calls.iter().flat_map(|a| a.to_le_bytes()).collect();
    rustix::io::pwrite(memory, &list, CALLS).expect("the calls");
    let flushed: Vec<u8> = (0..LINES)
        .flat_map(|k| line(copy, k).to_le_bytes())
        .collect();
    rustix::io::pwrite(memory, &flushed, FLUSHED).expect("the lines");
    let mut block = RegisterBlock::ZERO;
    let x = &mut block.x;
    x[usize::from(OFFSET)] = shape.less;
    x[usize::from(BOUND)] = LIMIT;
    x[usize::from(PROBE_BASE)] = PROBE + copy * COPY;
    x[usize::from(CALL_LIST)] = CALLS;
    x[usize::from(CALLS_LEFT)] = calls.len() as u64;
    x[usize::from(GADGET)] = gadget;
    x[usize::from(TIMED_LINE)] = line(copy, k);
    x[usize::from(SECRET_REGISTER)] = SECRET;
    x[usize::from(WINDOW_REGISTER)] = WINDOW_BYTE;
    x[usize::from(FLUSH_LIST)] = FLUSHED;
    x[usize::from(FLUSHES_LEFT)] = LINES;
    block.sp = STACK + STACK_BYTES - 64;
    stub.set_registers(0, &block);
    assert_eq!(
        stub.enter(0, &[], at as usize),
        Ok(TurnEnd::Called {
            entry: u64::from(InsideCall::Answer.code())
        }),
        "{}: the round ends at the floor",
        shape.name
    );
    let block = stub.registers(0);
    block.x[usize::from(AFTER)] - block.x[usize::from(BEFORE)]
}

fn median(values: &mut [u64]) -> u64 {
    values.sort_unstable();
    values[values.len() / 2]
}

#[test]
#[ignore = "a measurement, run on its own"]
fn a_refused_access_reads_nothing_outside_the_window_under_speculation() {
    let rounds = setting(ROUNDS_VARIABLE, DEFAULT_ROUNDS);
    let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
    let mut rng = fastrand::Rng::with_seed(seed);
    let probe = HostProbe::new().expect("the host is refused");
    let zygote = Zygote::start(&probe).expect("zygote");
    let stub = zygote.fork_stub().expect("fork");
    let mut area = CodeArea::new().expect("code area");
    let at = CODE_AREA as u64;
    let (words, returned) = harness(at);
    area.write(&probe, CODE_AREA, &words);
    let ways = [
        Way::Process,
        Way::InProcess {
            slot: 0,
            displaced: false,
        },
    ];
    let shapes = shapes();
    let mut pieces = Vec::new();
    for (k, shape) in shapes.iter().enumerate() {
        let guest = gadget(shape);
        let start = GUEST + k as u64 * GUEST_ROOM;
        let end = start + guest.len() as u64 * 4;
        area.write_slots(&probe, Way::Process.slot(end) as usize, &jump(returned));
        let bytes: Vec<u8> = guest.iter().flat_map(|w| w.to_le_bytes()).collect();
        let mut by_way = Vec::new();
        for (w, way) in ways.iter().enumerate() {
            let code =
                translate_native_for::<Arm64>(&bytes, start, LookAhead::Full, true, way, &())
                    .expect("translated");
            let base = CODE_AREA + (1 + k * ways.len() + w) * PIECE_ROOM;
            area.write(&probe, base, &host_words(&code));
            by_way.push(base as u64);
        }
        pieces.push(by_way);
    }
    stub.send_block(area.file()).expect("send the code area");
    stub.send_block(area.file()).expect("send the slot table");
    let map = |address: u64, length: u64| Command::Map {
        address: address as usize,
        length: length as usize,
        permissions: ProtFlags::READ | ProtFlags::WRITE,
        offset: address as usize,
    };
    let commands = [
        Command::MapCode,
        Command::MapSlots,
        map(GOOD, PAGE),
        map(LIMIT, PAGE),
        map(CALLS, PAGE),
        map(FLUSHED, PAGE),
        map(WINDOW_BYTE & !(PAGE - 1), PAGE),
        map(SECRET & !(PAGE - 1), PAGE),
        map(PROBE, COPIES * COPY),
        map(STACK, STACK_BYTES),
    ];
    assert_eq!(
        stub.turn(0, &commands),
        Ok(TurnEnd::Done {
            commands_applied: commands.len() as u64
        })
    );
    let memory = stub.memory();
    rustix::io::pwrite(memory, &[TRAINING; PAGE as usize], GOOD).expect("the in-bounds bytes");
    rustix::io::pwrite(memory, &(WINDOW_BYTE & !(PAGE - 1)).to_le_bytes(), LIMIT)
        .expect("the bound");
    // The secret and the window's byte in the stub's page tables, as a hot byte of the kernel's would be: a load
    // under speculation takes no fault to map a page.
    for address in [SECRET, WINDOW_BYTE] {
        assert!(
            matches!(stub.load(0, address as usize), Ok(TurnEnd::Loaded { .. })),
            "the stub reads {address:#x}"
        );
    }
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    eprintln!(
        "seed {seed:#x}, {rounds} rounds a shape and way, load {}",
        load.trim()
    );
    // Ticks of every round, by shape, way and the line timed.
    let by_line = || -> [Vec<u64>; TIMED.len()] { std::array::from_fn(|_| Vec::new()) };
    let mut ticks = vec![[by_line(), by_line()]; shapes.len()];
    for _ in 0..rounds {
        for (k, shape) in shapes.iter().enumerate() {
            for w in 0..ways.len() {
                let timed = TIMED[rng.usize(..TIMED.len())];
                let t = round(&stub, &mut rng, pieces[k][w], shape, timed, at);
                ticks[k][w][timed as usize].push(t);
            }
        }
    }
    // A line counts as cached below the midpoint of the training line's median (always cached) and an untouched
    // line's.
    let all = |timed: Timed| -> Vec<u64> {
        (ticks.iter())
            .flat_map(|by_way| {
                by_way
                    .iter()
                    .flat_map(|by| by[timed as usize].iter().copied())
            })
            .collect()
    };
    let cached = median(&mut all(Timed::Training));
    let untouched = median(&mut all(Timed::Other));
    let threshold = (cached + untouched) / 2;
    eprintln!(
        "ticks: a cached line's median {cached}, an untouched line's {untouched}, threshold {threshold}"
    );
    let mut failures = Vec::new();
    for (k, shape) in shapes.iter().enumerate() {
        let rate = |w: usize, timed: Timed| {
            let t = &ticks[k][w][timed as usize];
            t.iter().filter(|&&t| t < threshold).count() as f64 / t.len().max(1) as f64
        };
        let name = format!(
            "{}{}",
            shape.name,
            if shape.loaded { ", address loaded" } else { "" }
        );
        for (w, way) in ["process (uncut)", "in process"].iter().enumerate() {
            eprintln!(
                "{name}, {way}: the secret's line cached {:.3}, the window byte's {:.3}, an untouched line {:.3}, \
                 the training line {:.3}",
                rate(w, Timed::Secret),
                rate(w, Timed::Window),
                rate(w, Timed::Other),
                rate(w, Timed::Training)
            );
        }
        // The control must show the leak this test exists to see; the cut must show none.
        if rate(0, Timed::Secret) < rate(0, Timed::Other) + 0.2 {
            failures.push(format!("{name}: the uncut control shows no leak"));
        }
        if rate(1, Timed::Secret) > rate(1, Timed::Other) + 0.02 {
            failures.push(format!("{name}: the in-process way leaks"));
        }
    }
    stub.end(0).expect("end");
    assert!(failures.is_empty(), "{failures:#?}");
}
