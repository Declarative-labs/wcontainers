//! The in-process way's check on hardware (plan section 7's fixed loop): guest loops of one access shape each,
//! translated for the process way and for the in-process way with its window at 0 (slot 0, whose slot table is the
//! process way's, so one stub runs both), run in one stub in alternating rounds. Each round of each way must end
//! with the same guest registers and memory; the in-process way's time over the process way's is the check's cost
//! for that shape. A measurement, run on its own with `--ignored --nocapture`; knobs `WCHP_ITERATIONS` per run and
//! `WCHP_ROUNDS` pairs of runs.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::time::Instant;

use rustix::mm::ProtFlags;
use wcontainers_abi::RegisterBlock;
use wcontainers_arm64::encode::{self, form};
use wcontainers_arm64::thread_state::{COUNT, Register, SCRATCH};
use wcontainers_arm64::way::{Kept, PROCESS_ENTRIES, PROCESS_ENTRY_BYTES};
use wcontainers_arm64::{Arm64, Way};
use wcontainers_executor_host_process::code::CodeArea;
use wcontainers_executor_host_process::layout::CODE_AREA;
use wcontainers_executor_host_process::pages::Command;
use wcontainers_executor_host_process::probe::HostProbe;
use wcontainers_executor_host_process::stub::{Stub, TurnEnd};
use wcontainers_executor_host_process::zygote::Zygote;
use wcontainers_guest_cpu::HostCode;
use wcontainers_guest_cpu::position::InsideCall;
use wcontainers_translator::{LookAhead, translate_native_for};

const ITERATIONS_VARIABLE: &str = "WCHP_ITERATIONS";
/// Loop iterations per run unless told otherwise.
const DEFAULT_ITERATIONS: u64 = 20_000_000;
const ROUNDS_VARIABLE: &str = "WCHP_ROUNDS";
/// Pairs of runs per shape unless told otherwise.
const DEFAULT_ROUNDS: u64 = 15;

/// Where each loop's guest code lies.
const GUEST: u64 = 0x40_0000;
/// The array the loops read and write.
const DATA: u64 = 0x100_0000;
const DATA_BYTES: u64 = 64 * 1024;
/// The loops' index steps by this and wraps inside the array, leaving room for a pair at an offset.
const STEP: u64 = 16;
const INDEX_MASK: u64 = DATA_BYTES - 2 * STEP;
const STACK: u64 = 0x200_0000;
const STACK_BYTES: u64 = 64 * 1024;
/// The guest's stack pointer, 16-aligned with room above it for the stack shape's accesses.
const STACK_POINTER: u64 = STACK + STACK_BYTES - 4 * STEP;
/// Where each loop's translation is put in the code area, one per shape and way.
const PIECE_ROOM: usize = 64 * 1024;
/// The bytes of one instruction.
const INSTRUCTION: u64 = size_of::<u32>() as u64;

/// The loops' registers.
const INDEX: Register = 0;
const LEFT: Register = 1;
const SUM: Register = 2;
const VALUE: Register = 3;
const ARRAY: Register = 4;
const AT: Register = 5;
const OTHER: Register = 6;
/// The stack pointer as a base register.
const SP: Register = 31;

/// The process way, and the in-process way with its window at 0.
const WAYS: [(&str, Way); 2] = [
    ("process", Way::Process),
    (
        "in process",
        Way::InProcess {
            slot: 0,
            displaced: false,
            kept: Kept::USUAL,
        },
    ),
];

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

/// `add rd, rn, rm` of 64-bit registers.
fn add(rd: Register, rn: Register, rm: Register) -> u32 {
    encode::add_extended(false, rd, rn, rm, false, u64::BITS, 0).expect("registers fit")
}

fn load(form: encode::Form, rt: Register, rn: Register, offset: u64, unit: u64) -> u32 {
    encode::load_store(form, rt, rn, offset, unit).expect("an access fits")
}

/// Each shape's loop body: an access of that shape (or none) whose value goes into the sum.
fn shapes() -> Vec<(&'static str, Vec<u32>)> {
    let x = size_of::<u64>() as u64;
    let at_base = load(form::LDR, VALUE, AT, 0, x);
    vec![
        ("no access", vec![add(SUM, SUM, INDEX)]),
        (
            "register offset",
            vec![
                encode::register_offset_at_base(at_base, ARRAY, INDEX)
                    .expect("a register-offset form"),
                add(SUM, SUM, VALUE),
            ],
        ),
        (
            "at base",
            vec![add(AT, ARRAY, INDEX), at_base, add(SUM, SUM, VALUE)],
        ),
        (
            "immediate offset",
            vec![
                add(AT, ARRAY, INDEX),
                load(form::LDR, VALUE, AT, STEP, x),
                add(SUM, SUM, VALUE),
            ],
        ),
        (
            "byte",
            vec![
                add(AT, ARRAY, INDEX),
                load(form::LDRB, VALUE, AT, 3, 1),
                add(SUM, SUM, VALUE),
            ],
        ),
        (
            "pair",
            vec![
                add(AT, ARRAY, INDEX),
                encode::load_pair(form::LDP_64, VALUE, OTHER, AT),
                add(SUM, SUM, VALUE),
                add(SUM, SUM, OTHER),
            ],
        ),
        (
            "load and store",
            vec![
                add(AT, ARRAY, INDEX),
                load(form::LDR, VALUE, AT, 0, x),
                add(SUM, SUM, VALUE),
                load(form::STR, SUM, AT, x, x),
            ],
        ),
        (
            "stack",
            vec![
                load(form::LDR, VALUE, SP, STEP, x),
                add(SUM, SUM, VALUE),
                load(form::STR, SUM, SP, STEP + x, x),
            ],
        ),
    ]
}

/// The loop around `body`: the index steps and wraps, and the loop runs until no iteration is left.
fn guest_loop(body: &[u32]) -> Vec<u32> {
    let mut words = body.to_vec();
    words.push(encode::add_immediate(INDEX, INDEX, STEP as i64).expect("fits"));
    words.push(encode::and_immediate(INDEX, INDEX, INDEX_MASK).expect("a logical immediate"));
    words.push(encode::add_immediate(LEFT, LEFT, -1).expect("fits"));
    let back = -((words.len() as u64 * INSTRUCTION) as i64);
    words.push(encode::compare_branch(true, LEFT, back).expect("fits"));
    words
}

fn host_words(code: &HostCode) -> Vec<u32> {
    let (words, rest) = code.bytes.as_chunks::<{ size_of::<u32>() }>();
    assert!(rest.is_empty(), "host code is whole words");
    words.iter().map(|w| u32::from_le_bytes(*w)).collect()
}

/// The loop translated for `way` as one piece, whose branch back stays inside it.
fn translated(guest: &[u32], way: Way) -> Vec<u32> {
    let bytes: Vec<u8> = guest.iter().flat_map(|w| w.to_le_bytes()).collect();
    let code = translate_native_for::<Arm64>(&bytes, GUEST, LookAhead::Full, true, &way, &())
        .expect("translated");
    assert!(
        code.exits.iter().all(|exit| exit.target != GUEST),
        "the loop's branch back stays in its piece"
    );
    host_words(&code)
}

/// The slot of the guest address after the loop: a call into the floor's entry, which ends the turn with the
/// registers saved.
fn ending() -> Vec<u32> {
    let entry = PROCESS_ENTRIES + u64::from(InsideCall::Answer.code()) * PROCESS_ENTRY_BYTES;
    let mut words = encode::move_address(SCRATCH, entry);
    words.push(encode::branch_register(form::BLR, SCRATCH));
    words
}

/// The array's bytes before every run: each word its own index.
fn array() -> Vec<u8> {
    (0..DATA_BYTES / 8).flat_map(u64::to_le_bytes).collect()
}

/// What a run ends with: the guest's registers that the loops use, and the array and stack bytes.
#[derive(Debug, PartialEq, Eq)]
struct Ended {
    x: [u64; 7],
    memory: Vec<u8>,
}

/// One run of the loop at `at`: its time in nanoseconds and how it ended.
fn run(stub: &Stub, at: usize, iterations: u64) -> (f64, Ended) {
    let memory = stub.memory();
    rustix::io::pwrite(memory, &array(), DATA).expect("the array");
    rustix::io::pwrite(memory, &[0; STACK_BYTES as usize], STACK).expect("the stack");
    let mut block = RegisterBlock::ZERO;
    block.x[usize::from(LEFT)] = iterations;
    block.x[usize::from(ARRAY)] = DATA;
    block.x[usize::from(COUNT)] = 0;
    block.sp = STACK_POINTER;
    stub.set_registers(0, &block);
    let start = Instant::now();
    let end = stub.enter(0, &[], at).expect("enter");
    let nanoseconds = start.elapsed().as_nanos() as f64;
    assert_eq!(
        end,
        TurnEnd::Called {
            entry: u64::from(InsideCall::Answer.code())
        },
        "the loop ends at the slot after it"
    );
    let block = stub.registers(0);
    assert_eq!(block.x[usize::from(LEFT)], 0, "every iteration ran");
    let mut bytes = vec![0; (DATA_BYTES + STACK_BYTES) as usize];
    let (data, stack) = bytes.split_at_mut(DATA_BYTES as usize);
    rustix::io::pread(memory, data, DATA).expect("the array");
    rustix::io::pread(memory, stack, STACK).expect("the stack");
    (
        nanoseconds,
        Ended {
            x: std::array::from_fn(|n| block.x[n]),
            memory: bytes,
        },
    )
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

#[test]
#[ignore = "a measurement, run on its own"]
fn the_in_process_way_s_check_costs_per_access_shape() {
    let iterations = setting(ITERATIONS_VARIABLE, DEFAULT_ITERATIONS);
    let rounds = setting(ROUNDS_VARIABLE, DEFAULT_ROUNDS);
    let probe = HostProbe::new().expect("the host is refused");
    let zygote = Zygote::start(&probe).expect("zygote");
    let stub = zygote.fork_stub().expect("fork");
    let mut area = CodeArea::new().expect("code area");
    let shapes = shapes();
    let mut pieces = Vec::new();
    for (k, (_, body)) in shapes.iter().enumerate() {
        let guest = guest_loop(body);
        // Every loop's slot after it holds the ending; the loops differ in length, so each has its own.
        let after = GUEST + guest.len() as u64 * INSTRUCTION;
        area.write_slots(&probe, Way::Process.slot(after) as usize, &ending());
        let mut by_way = Vec::new();
        for (w, &(_, way)) in WAYS.iter().enumerate() {
            let at = CODE_AREA + (k * WAYS.len() + w) * PIECE_ROOM;
            let words = translated(&guest, way);
            area.write(&probe, at, &words);
            by_way.push((at, words.len()));
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
    assert_eq!(
        stub.turn(
            0,
            &[
                Command::MapCode,
                Command::MapSlots,
                map(DATA, DATA_BYTES),
                map(STACK, STACK_BYTES),
            ]
        ),
        Ok(TurnEnd::Done {
            commands_applied: 4
        })
    );
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    eprintln!(
        "{iterations} iterations a run, {rounds} pairs a shape, load {}",
        load.trim()
    );
    for ((name, body), by_way) in shapes.iter().zip(&pieces) {
        let mut times = [Vec::new(), Vec::new()];
        let mut ratios = Vec::new();
        for round in 0..rounds {
            // Alternating which way runs first.
            let order: [usize; 2] = if round % 2 == 0 { [0, 1] } else { [1, 0] };
            let mut ends: [Option<Ended>; 2] = [None, None];
            let mut pair = [0.0; 2];
            for w in order {
                let (nanoseconds, end) = run(&stub, by_way[w].0, iterations);
                pair[w] = nanoseconds;
                ends[w] = Some(end);
            }
            assert_eq!(ends[0], ends[1], "{name}: the two ways end alike");
            for w in 0..WAYS.len() {
                times[w].push(pair[w] / iterations as f64);
            }
            ratios.push(pair[1] / pair[0]);
        }
        let process = median(&mut times[0]);
        let in_process = median(&mut times[1]);
        let ratio = median(&mut ratios);
        eprintln!(
            "{name}: {} guest instructions an iteration, host words {} and {}; {process:.3} ns and {in_process:.3} ns \
             an iteration (medians); the in-process way {:+.1}% (median of {rounds} pairs' ratios, {:+.1} to {:+.1}%)",
            guest_loop(body).len(),
            by_way[0].1,
            by_way[1].1,
            (ratio - 1.0) * 100.0,
            (ratios[0] - 1.0) * 100.0,
            (ratios[ratios.len() - 1] - 1.0) * 100.0,
        );
    }
    stub.end(0).expect("end");
}
