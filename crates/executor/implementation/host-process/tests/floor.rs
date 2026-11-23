//! The floor on a real ARM64 host, with pieces written here standing in for translated code: an enter loads every
//! register of a thread's block, and a call, a stop or a fault in the code saves every register it held; each thread
//! reads only its own registers, or zeros, and a stop landing anywhere in the enter or the save loses nothing.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use wcontainers_abi::{RegisterBlock, pstate};
use wcontainers_arm64::encode::{self, form};
use wcontainers_arm64::thread_state::{PAGE, SCRATCH};
use wcontainers_arm64::{FPCR_BITS, FPSR_BITS};
use wcontainers_executor_host_process::code::CodeArea;
use wcontainers_executor_host_process::layout::{
    CODE_AREA, FLOOR_ENTRIES, FLOOR_ENTRY_COUNT, FLOOR_ENTRY_SIZE, GUEST_START, REGISTER_PAGES,
    THREAD_PAGE_SIZE,
};
use wcontainers_executor_host_process::pages::Command;
use wcontainers_executor_host_process::probe::HostProbe;
use wcontainers_executor_host_process::stub::{Stub, TurnEnd};
use wcontainers_executor_host_process::zygote::Zygote;

/// One embedder process has one zygote place, so tests running at once start theirs one at a time.
static STARTING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn zygote(probe: &HostProbe) -> Zygote {
    let _one_at_a_time = STARTING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Zygote::start(probe).expect("the zygote did not start")
}

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

const SIGSEGV: u32 = linux_raw_sys::general::SIGSEGV;
const SIGILL: u32 = linux_raw_sys::general::SIGILL;
const SIGTRAP: u32 = linux_raw_sys::general::SIGTRAP;
const MAPPED: u32 = linux_raw_sys::general::SEGV_MAPERR;
const REFUSED: u32 = linux_raw_sys::general::SEGV_ACCERR;
const INSTRUCTION: usize = size_of::<u32>();

/// The pieces, each at its own place in the code area.
const CALL: usize = CODE_AREA;
const SPIN: usize = CODE_AREA + 64;
const LOAD: usize = CODE_AREA + 128;
const STORE: usize = CODE_AREA + 192;
const UNDEFINED: usize = CODE_AREA + 256;
const BREAKPOINT: usize = CODE_AREA + 320;
/// The piece at the far end of the code area, which still reaches every entry.
const FAR_CALL: usize =
    CODE_AREA + wcontainers_executor_host_process::layout::CODE_AREA_SIZE - INSTRUCTION;

/// Pieces that change every register the guest has, then spin, fault or call.
const SCRAMBLE_SPIN: usize = CODE_AREA + PAGE_BYTES;
const SCRAMBLE_FAULT: usize = CODE_AREA + 2 * PAGE_BYTES;
const SCRAMBLE_CALL: usize = CODE_AREA + 3 * PAGE_BYTES;
/// A call piece that changes every register many times over, so that stops land in it often.
const LONG_CALL: usize = CODE_AREA + 4 * PAGE_BYTES;
const LONG_REPEATS: usize = 64;

const PAGE_BYTES: usize = wcontainers_abi::PAGE_SIZE as usize;

/// The threads with a data page of their own, the first guest pages, from which their pieces load vectors.
const DATA_THREADS: usize = 8;

/// An address nothing maps, past the data pages.
const UNMAPPED: u64 = (GUEST_START + (1 << 32)) as u64;

/// The registers `msr` names here, by o0 (op0 less 2), op1, CRn, CRm and op2 (Arm ARM, each register's encoding).
const NZCV: [u32; 5] = [1, 3, 4, 2, 0];
const FPCR: [u32; 5] = [1, 3, 4, 4, 0];
const FPSR: [u32; 5] = [1, 3, 4, 4, 1];
const TPIDR_EL0: [u32; 5] = [1, 3, 13, 0, 2];

/// The entry the call pieces name.
const ENTRY: usize = 37;

fn entry(entry: usize) -> usize {
    assert!(entry < FLOOR_ENTRY_COUNT);
    FLOOR_ENTRIES + entry * FLOOR_ENTRY_SIZE
}

fn call_to(at: usize, entry_number: usize) -> u32 {
    encode::branch(true, entry(entry_number) as i64 - at as i64).expect("an entry within reach")
}

/// A stub with a code area holding the pieces.
struct Floor {
    zygote: Zygote,
    stub: Stub,
    _code: CodeArea,
    /// The stops and faults that ended translated code, as this test saw them.
    from_code: std::cell::Cell<u64>,
}

fn floor() -> Floor {
    let probe = HostProbe::new().expect("the host is refused");
    let zygote = zygote(&probe);
    let stub = zygote.fork_stub().expect("fork");
    let mut code = CodeArea::new().expect("code area");
    code.write(&probe, CALL, &[call_to(CALL, ENTRY)]);
    code.write(&probe, SPIN, &[encode::branch(false, 0).expect("a branch")]);
    let load = encode::load_store(form::LDR, 0, 1, 0, size_of::<u64>() as u64).expect("a load");
    code.write(&probe, LOAD, &[load]);
    let store = encode::load_store(form::STR, 0, 1, 0, size_of::<u64>() as u64).expect("a store");
    code.write(&probe, STORE, &[store]);
    code.write(&probe, UNDEFINED, &[encode::exception(form::UDF, 0)]);
    code.write(&probe, BREAKPOINT, &[encode::exception(form::BRK, 0)]);
    code.write(
        &probe,
        FAR_CALL,
        &[call_to(FAR_CALL, FLOOR_ENTRY_COUNT - 1)],
    );
    for (at, ending) in [
        (SCRAMBLE_SPIN, Ending::Spin),
        (SCRAMBLE_FAULT, Ending::Fault),
        (SCRAMBLE_CALL, Ending::Call),
    ] {
        code.write(&probe, at, &scramble_piece(at, ending, 1));
    }
    code.write(
        &probe,
        LONG_CALL,
        &scramble_piece(LONG_CALL, Ending::Call, LONG_REPEATS),
    );
    stub.send_block(code.file()).expect("send the code area");
    let floor = Floor {
        zygote,
        stub,
        _code: code,
        from_code: std::cell::Cell::new(0),
    };
    assert_eq!(
        floor.turn(0, &[Command::MapCode]),
        TurnEnd::Done {
            commands_applied: 1
        }
    );
    let data = Command::Map {
        address: GUEST_START,
        length: DATA_THREADS * PAGE_BYTES,
        permissions: rustix::mm::ProtFlags::READ | rustix::mm::ProtFlags::WRITE,
        offset: GUEST_START,
    };
    assert_eq!(
        floor.turn(0, &[data]),
        TurnEnd::Done {
            commands_applied: 1
        }
    );
    floor
}

impl Floor {
    fn turn(&self, thread: usize, commands: &[Command]) -> TurnEnd {
        let end = self.stub.turn(thread, commands).expect("turn");
        self.alive(end)
    }

    fn enter(&self, thread: usize, at: usize) -> TurnEnd {
        let end = self.stub.enter(thread, &[], at).expect("enter");
        self.alive(end)
    }

    fn alive(&self, end: TurnEnd) -> TurnEnd {
        if matches!(end, TurnEnd::Interrupted { .. } | TurnEnd::Fault { .. }) {
            self.from_code.set(self.from_code.get() + 1);
        }
        if end == TurnEnd::StubEnded {
            panic!(
                "the stub ended: {:?}",
                self.zygote.reap(self.stub.process())
            );
        }
        end
    }

    /// Makes a thread through thread 0's turn and gives its index.
    fn new_thread(&mut self) -> usize {
        let command = self.stub.new_thread().expect("a free index");
        let Command::NewThread { thread, .. } = command else {
            unreachable!("a new thread's command")
        };
        assert_eq!(
            self.turn(0, &[command]),
            TurnEnd::Done {
                commands_applied: 1
            }
        );
        thread
    }

    /// Fills a thread's data page with fresh values, which its scramble pieces then load as vectors.
    fn fill_data(&self, thread: usize, rng: &mut fastrand::Rng) -> [[u64; 2]; 32] {
        use std::os::unix::fs::FileExt;
        let vectors: [[u64; 2]; 32] = core::array::from_fn(|_| [rng.u64(..), rng.u64(..)]);
        let bytes: Vec<u8> = vectors
            .as_flattened()
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        let memory = std::fs::File::from(self.stub.memory().try_clone_to_owned().expect("dup"));
        memory
            .write_all_at(&bytes, data_page(thread) as u64)
            .expect("write the data page");
        vectors
    }

    /// Enters a piece whose last instruction spins, at `at`, and stops it there, sending the stop again each
    /// millisecond until the turn ends; enters again where the stop came before the code ran, and goes on from a stop
    /// before the spin with the registers saved there, as a resumed thread does.
    /// The turn's end, and where the thread was last entered.
    fn stop_spin(&self, thread: usize, mut at: usize, spin: usize) -> (TurnEnd, usize) {
        let mut turn = self.stub.begin_enter(thread, &[], at).expect("enter");
        for _ in 0..10_000 {
            self.stub.stop(thread).expect("stop");
            match self
                .stub
                .wait_within(thread, turn, Duration::from_millis(1))
                .expect("wait")
                .map(|end| self.alive(end))
            {
                None => {}
                Some(TurnEnd::Stopped {
                    commands_applied: 0,
                }) => {
                    turn = self.stub.begin_enter(thread, &[], at).expect("enter");
                }
                Some(TurnEnd::Interrupted { at: stop }) if stop as usize != spin => {
                    assert!((at..spin).contains(&(stop as usize)), "a stop at {stop:#x}");
                    at = stop as usize;
                    turn = self.stub.begin_enter(thread, &[], at).expect("enter");
                }
                Some(end) => return (end, at),
            }
        }
        panic!("thread {thread} ignored 10,000 stops");
    }
}

fn page(thread: usize) -> u64 {
    (REGISTER_PAGES + thread * THREAD_PAGE_SIZE) as u64
}

fn data_page(thread: usize) -> usize {
    assert!(thread < DATA_THREADS);
    GUEST_START + thread * PAGE_BYTES
}

/// One instruction of a scramble piece.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// `ldr qN, [x1, #16N]`.
    LoadVector(u8),
    /// `add xN, xN, #1`.
    Increment(u8),
    /// `add sp, sp, #16`.
    MoveStack,
    /// `msr nzcv, x0`.
    Flags,
    /// `msr fpsr, x2`.
    Status,
    /// `msr fpcr, x3`.
    Control,
    /// `msr tpidr_el0, x4`.
    ThreadPointer,
}

const VECTOR_BYTES: u64 = 16;
const STACK_STEP: u64 = 16;
const STACK_POINTER: u8 = 31;

/// The register the pieces load vectors through.
const DATA_BASE: u8 = 1;

/// A scramble piece's steps: `repeats` times every vector loaded and every general register but the data base and
/// the withheld ones incremented and sp moved, then the flags, status, control and thread pointer set once.
fn steps(repeats: usize) -> Vec<Step> {
    let round = (0..32).map(Step::LoadVector).chain(
        (0..=30)
            .filter(|&n| n != SCRATCH && n != PAGE && n != DATA_BASE)
            .map(Step::Increment)
            .chain([Step::MoveStack]),
    );
    let rounds: Vec<Step> = round.collect();
    rounds
        .iter()
        .copied()
        .cycle()
        .take(rounds.len() * repeats)
        .chain([
            Step::Flags,
            Step::Status,
            Step::Control,
            Step::ThreadPointer,
        ])
        .collect()
}

fn step_word(step: Step) -> u32 {
    let fits = "an instruction that fits";
    match step {
        Step::LoadVector(n) => encode::load_store(
            form::LDR_Q,
            n,
            DATA_BASE,
            u64::from(n) * VECTOR_BYTES,
            VECTOR_BYTES,
        )
        .expect(fits),
        Step::Increment(n) => encode::add_immediate(n, n, 1).expect(fits),
        Step::MoveStack => {
            encode::add_immediate(STACK_POINTER, STACK_POINTER, STACK_STEP as i64).expect(fits)
        }
        Step::Flags => encode::write_system_register(NZCV, 0).expect(fits),
        Step::Status => encode::write_system_register(FPSR, 2).expect(fits),
        Step::Control => encode::write_system_register(FPCR, 3).expect(fits),
        Step::ThreadPointer => encode::write_system_register(TPIDR_EL0, 4).expect(fits),
    }
}

/// What a step does to a thread's registers.
fn apply(step: Step, state: &mut RegisterBlock, vectors: &[[u64; 2]; 32]) {
    match step {
        Step::LoadVector(n) => state.v[usize::from(n)] = vectors[usize::from(n)],
        Step::Increment(n) => state.x[usize::from(n)] = state.x[usize::from(n)].wrapping_add(1),
        Step::MoveStack => state.sp = state.sp.wrapping_add(STACK_STEP),
        Step::Flags => state.pstate = state.x[0] & pstate::NZCV,
        Step::Status => state.fpsr = state.x[2] as u32,
        Step::Control => state.fpcr = state.x[3] as u32,
        Step::ThreadPointer => state.tpidr_el0 = state.x[4],
    }
}

#[derive(Clone, Copy)]
enum Ending {
    /// `b .`
    Spin,
    /// `ldr x0, [x5]`, x5 then [`UNMAPPED`].
    Fault,
    /// `bl` to entry [`ENTRY`].
    Call,
}

fn scramble_piece(at: usize, ending: Ending, repeats: usize) -> Vec<u32> {
    let mut words: Vec<u32> = steps(repeats).into_iter().map(step_word).collect();
    let end = at + words.len() * INSTRUCTION;
    words.push(match ending {
        Ending::Spin => encode::branch(false, 0).expect("a branch"),
        Ending::Fault => {
            encode::load_store(form::LDR, 0, 5, 0, size_of::<u64>() as u64).expect("a load")
        }
        Ending::Call => call_to(end, ENTRY),
    });
    words
}

/// The address of a scramble piece's last instruction.
fn ending(at: usize, repeats: usize) -> usize {
    at + steps(repeats).len() * INSTRUCTION
}

/// Sets what a scramble piece needs in a thread's registers, keeping the rest: x1 its data page, and x0, x2, x3 and
/// x5 `repeats` less than valid flags, status, control and [`UNMAPPED`], since the piece adds that to each first.
fn prepare(block: &mut RegisterBlock, thread: usize, rng: &mut fastrand::Rng, repeats: usize) {
    let added = repeats as u64;
    block.x[usize::from(DATA_BASE)] = data_page(thread) as u64;
    block.x[0] = (rng.u64(..) & pstate::NZCV).wrapping_sub(added);
    block.x[2] = u64::from(rng.u32(..) & FPSR_BITS).wrapping_sub(added);
    block.x[3] = u64::from(rng.u32(..) & FPCR_BITS).wrapping_sub(added);
    block.x[5] = UNMAPPED - added;
}

/// The registers of a thread that entered with `loaded` after the first `count` steps of a scramble piece.
fn after_steps(
    loaded: &RegisterBlock,
    count: usize,
    vectors: &[[u64; 2]; 32],
    repeats: usize,
) -> RegisterBlock {
    let mut state = *loaded;
    for step in steps(repeats).into_iter().take(count) {
        apply(step, &mut state, vectors);
    }
    state
}

/// Distinct values in every register the guest has.
fn random_block(rng: &mut fastrand::Rng) -> RegisterBlock {
    let mut block = RegisterBlock::ZERO;
    block.x.iter_mut().for_each(|x| *x = rng.u64(..));
    block.sp = rng.u64(..);
    block.pc = rng.u64(..);
    block.pstate = rng.u64(..) & pstate::NZCV;
    block
        .v
        .iter_mut()
        .for_each(|v| *v = [rng.u64(..), rng.u64(..)]);
    block.fpsr = rng.u32(..) & FPSR_BITS;
    block.fpcr = rng.u32(..) & FPCR_BITS;
    block.tpidr_el0 = rng.u64(..);
    block
}

/// The block a call from the instruction at `at` to entry `entry` saves for a thread that held `state`.
fn after_call(state: &RegisterBlock, thread: usize, at: usize, entry: usize) -> RegisterBlock {
    let mut expected = *state;
    expected.x[usize::from(SCRATCH)] = entry as u64;
    expected.x[usize::from(PAGE)] = page(thread);
    expected.x[30] = (at + INSTRUCTION) as u64;
    expected.pc = (at + INSTRUCTION) as u64;
    expected
}

/// The block a stop or fault at the instruction at `at` saves for a thread that held `state` there, entered at
/// `entered`.
fn stopped_at(state: &RegisterBlock, thread: usize, at: usize, entered: usize) -> RegisterBlock {
    let mut expected = *state;
    expected.x[usize::from(SCRATCH)] = entered as u64;
    expected.x[usize::from(PAGE)] = page(thread);
    expected.pc = at as u64;
    expected
}

fn assert_block(stub: &Stub, thread: usize, expected: &RegisterBlock, what: &str) {
    let saved = stub.registers(thread);
    if saved != *expected {
        let differing: Vec<String> = (0..31)
            .filter(|&n| saved.x[n] != expected.x[n])
            .map(|n| format!("x{n} {:#x} not {:#x}", saved.x[n], expected.x[n]))
            .chain(
                (0..32)
                    .filter(|&n| saved.v[n] != expected.v[n])
                    .map(|n| format!("v{n} {:x?} not {:x?}", saved.v[n], expected.v[n])),
            )
            .collect();
        panic!("{what}: thread {thread} saved {differing:?} in {saved:x?}, not {expected:x?}");
    }
}

#[test]
fn a_call_saves_every_register_the_code_held() {
    let floor = floor();
    let mut rng = fastrand::Rng::with_seed(knob("WCHP_SEED", 1));
    for round in 0..knob("WCHP_TURNS", 1000) {
        let what = format!("round {round}");
        // At once from what the enter loaded.
        let loaded = random_block(&mut rng);
        floor.stub.set_registers(0, &loaded);
        assert_eq!(
            floor.enter(0, CALL),
            TurnEnd::Called {
                entry: ENTRY as u64
            }
        );
        assert_block(&floor.stub, 0, &after_call(&loaded, 0, CALL, ENTRY), &what);
        // After the code changed every register.
        let vectors = floor.fill_data(0, &mut rng);
        let mut loaded = random_block(&mut rng);
        prepare(&mut loaded, 0, &mut rng, 1);
        floor.stub.set_registers(0, &loaded);
        assert_eq!(
            floor.enter(0, SCRAMBLE_CALL),
            TurnEnd::Called {
                entry: ENTRY as u64
            }
        );
        let state = after_steps(&loaded, steps(1).len(), &vectors, 1);
        assert_block(
            &floor.stub,
            0,
            &after_call(&state, 0, ending(SCRAMBLE_CALL, 1), ENTRY),
            &what,
        );
    }
    // The last entry, from the far end of the code area.
    let loaded = random_block(&mut rng);
    floor.stub.set_registers(0, &loaded);
    let last = FLOOR_ENTRY_COUNT - 1;
    assert_eq!(
        floor.enter(0, FAR_CALL),
        TurnEnd::Called { entry: last as u64 }
    );
    assert_block(
        &floor.stub,
        0,
        &after_call(&loaded, 0, FAR_CALL, last),
        "far call",
    );
}

#[test]
fn an_enter_outside_the_code_area_is_refused() {
    let floor = floor();
    for at in [CODE_AREA - INSTRUCTION, entry(0), GUEST_START] {
        assert_eq!(floor.enter(0, at), TurnEnd::BadTurn);
    }
    assert_eq!(
        floor.enter(0, CALL),
        TurnEnd::Called {
            entry: ENTRY as u64
        }
    );
}

#[test]
fn a_stop_or_a_fault_in_translated_code_saves_every_register() {
    let floor = floor();
    let mut rng = fastrand::Rng::with_seed(knob("WCHP_SEED", 1));
    for round in 0..knob("WCHP_TURNS", 1000) / 10 {
        let what = format!("round {round}");
        let vectors = floor.fill_data(0, &mut rng);
        let mut loaded = random_block(&mut rng);
        prepare(&mut loaded, 0, &mut rng, 1);
        let state = after_steps(&loaded, steps(1).len(), &vectors, 1);

        floor.stub.set_registers(0, &loaded);
        let spin = ending(SCRAMBLE_SPIN, 1);
        let (end, entered) = floor.stop_spin(0, SCRAMBLE_SPIN, spin);
        assert_eq!(end, TurnEnd::Interrupted { at: spin as u64 });
        assert_block(&floor.stub, 0, &stopped_at(&state, 0, spin, entered), &what);

        floor.stub.set_registers(0, &loaded);
        let fault = ending(SCRAMBLE_FAULT, 1);
        assert_eq!(
            floor.enter(0, SCRAMBLE_FAULT),
            TurnEnd::Fault {
                address: UNMAPPED,
                signal: SIGSEGV,
                code: MAPPED
            }
        );
        assert_block(
            &floor.stub,
            0,
            &stopped_at(&state, 0, fault, SCRAMBLE_FAULT),
            &what,
        );

        let mut loaded = random_block(&mut rng);
        loaded.x[1] = UNMAPPED;
        floor.stub.set_registers(0, &loaded);
        assert_eq!(
            floor.stop_spin(0, SPIN, SPIN),
            (TurnEnd::Interrupted { at: SPIN as u64 }, SPIN)
        );
        assert_block(&floor.stub, 0, &stopped_at(&loaded, 0, SPIN, SPIN), &what);
        for (piece, address, signal, code) in [
            (LOAD, UNMAPPED, SIGSEGV, MAPPED),
            (STORE, UNMAPPED, SIGSEGV, MAPPED),
            (
                UNDEFINED,
                UNDEFINED as u64,
                SIGILL,
                linux_raw_sys::general::ILL_ILLOPC,
            ),
            (
                BREAKPOINT,
                BREAKPOINT as u64,
                SIGTRAP,
                linux_raw_sys::general::TRAP_BRKPT,
            ),
        ] {
            floor.stub.set_registers(0, &loaded);
            assert_eq!(
                floor.enter(0, piece),
                TurnEnd::Fault {
                    address,
                    signal,
                    code
                },
                "piece at {piece:#x}"
            );
            assert_block(&floor.stub, 0, &stopped_at(&loaded, 0, piece, piece), &what);
        }
    }
    // A host either puts the FP/SIMD registers in every frame or in none (gVisor), where each exit returns for them.
    let exits = floor.from_code.get();
    let by_return = floor.stub.vectors_by_return();
    println!("{by_return} of {exits} exits saved their FP/SIMD registers by a return");
    assert!(
        by_return == 0 || by_return == exits,
        "{by_return} of {exits}"
    );
}

#[test]
fn translated_code_can_neither_read_nor_write_the_code_area() {
    let floor = floor();
    let mut loaded = RegisterBlock::ZERO;
    loaded.x[1] = CALL as u64;
    for piece in [LOAD, STORE] {
        floor.stub.set_registers(0, &loaded);
        assert_eq!(
            floor.enter(0, piece),
            TurnEnd::Fault {
                address: CALL as u64,
                signal: SIGSEGV,
                code: REFUSED
            }
        );
    }
    // The code is unchanged.
    assert_eq!(
        floor.enter(0, CALL),
        TurnEnd::Called {
            entry: ENTRY as u64
        }
    );
}

#[test]
fn threads_interleaved_read_back_only_their_own_registers_or_zeros() {
    let mut floor = floor();
    let mut rng = fastrand::Rng::with_seed(knob("WCHP_SEED", 1));
    // Thread 0 holds its values in the hardware when it makes the others, which start from zero blocks.
    let mut vectors = vec![floor.fill_data(0, &mut rng)];
    let mut blocks = vec![random_block(&mut rng)];
    prepare(&mut blocks[0], 0, &mut rng, 1);
    floor.stub.set_registers(0, &blocks[0]);
    assert_eq!(
        floor.enter(0, SCRAMBLE_CALL),
        TurnEnd::Called {
            entry: ENTRY as u64
        }
    );
    for _ in 1..DATA_THREADS {
        let thread = floor.new_thread();
        assert_eq!(thread, blocks.len());
        assert_eq!(
            floor.enter(thread, CALL),
            TurnEnd::Called {
                entry: ENTRY as u64
            }
        );
        let zero = RegisterBlock::ZERO;
        assert_block(
            &floor.stub,
            thread,
            &after_call(&zero, thread, CALL, ENTRY),
            "a new thread",
        );
        vectors.push(floor.fill_data(thread, &mut rng));
        blocks.push(random_block(&mut rng));
    }
    let rounds = knob("WCHP_ROUNDS", 4000);
    for round in 0..rounds {
        let thread = rng.usize(..DATA_THREADS);
        let what = format!("round {round}");
        // Each thread goes on from the registers it last saved, as a resumed thread does.
        let mut loaded = blocks[thread];
        prepare(&mut loaded, thread, &mut rng, 1);
        floor.stub.set_registers(thread, &loaded);
        let state = after_steps(&loaded, steps(1).len(), &vectors[thread], 1);
        let expected = if rng.bool() {
            assert_eq!(
                floor.enter(thread, SCRAMBLE_CALL),
                TurnEnd::Called {
                    entry: ENTRY as u64
                }
            );
            after_call(&state, thread, ending(SCRAMBLE_CALL, 1), ENTRY)
        } else {
            let spin = ending(SCRAMBLE_SPIN, 1);
            let (end, entered) = floor.stop_spin(thread, SCRAMBLE_SPIN, spin);
            assert_eq!(end, TurnEnd::Interrupted { at: spin as u64 });
            stopped_at(&state, thread, spin, entered)
        };
        assert_block(&floor.stub, thread, &expected, &what);
        blocks[thread] = expected;
        if rng.u8(..4) == 0 {
            vectors[thread] = floor.fill_data(thread, &mut rng);
        }
    }
    println!("{rounds} rounds over {DATA_THREADS} threads");
}

#[test]
fn stops_landing_anywhere_in_the_code_the_enter_or_the_save_lose_nothing() {
    let floor = floor();
    let target = floor.stub.stop_target(0);
    let done = AtomicBool::new(false);
    let sent = AtomicU64::new(0);
    let seed = knob("WCHP_SEED", 1);
    let rounds = knob("WCHP_ROUNDS", 20_000);
    let (mut called, mut stopped, mut interrupted) = (0, 0, 0);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut rng = fastrand::Rng::with_seed(seed ^ 0x5709);
            while !done.load(Ordering::Relaxed) {
                if target.stop().is_ok() {
                    sent.fetch_add(1, Ordering::Relaxed);
                }
                for _ in 0..rng.u32(..knob("WCHP_STOP_PAUSE", 50) as u32) {
                    core::hint::spin_loop();
                }
            }
        });
        // The sender stops with the rounds, also when one fails.
        let _stop_sending = Finished(&done);
        let mut rng = fastrand::Rng::with_seed(seed);
        let vectors = floor.fill_data(0, &mut rng);
        for round in 0..rounds {
            let mut loaded = random_block(&mut rng);
            prepare(&mut loaded, 0, &mut rng, LONG_REPEATS);
            let what = format!("round {round}");
            // Where the thread goes on, and what it holds there, as the floor resolves a stop.
            let (mut at, mut state) = (LONG_CALL, loaded);
            loop {
                floor.stub.set_registers(0, &state);
                match floor.enter(0, at) {
                    TurnEnd::Called { entry } => {
                        assert_eq!(entry, ENTRY as u64, "{what}");
                        let full =
                            after_steps(&loaded, steps(LONG_REPEATS).len(), &vectors, LONG_REPEATS);
                        let expected = after_call(&full, 0, ending(LONG_CALL, LONG_REPEATS), ENTRY);
                        assert_block(&floor.stub, 0, &expected, &what);
                        called += 1;
                        break;
                    }
                    // Before the code ran: the block is as written.
                    TurnEnd::Stopped { commands_applied } => {
                        assert_eq!(commands_applied, 0, "{what}");
                        assert_block(&floor.stub, 0, &state, &what);
                        stopped += 1;
                    }
                    TurnEnd::Interrupted { at: stop } => {
                        let stop = stop as usize;
                        assert!(
                            (at..=ending(LONG_CALL, LONG_REPEATS)).contains(&stop),
                            "{what}: stopped at {stop:#x}, entered at {at:#x}"
                        );
                        let steps_done = (stop - LONG_CALL) / INSTRUCTION;
                        let held = after_steps(&loaded, steps_done, &vectors, LONG_REPEATS);
                        assert_block(&floor.stub, 0, &stopped_at(&held, 0, stop, at), &what);
                        (at, state) = (stop, floor.stub.registers(0));
                        interrupted += 1;
                    }
                    other => panic!("{what}: the turn ended {other:?}"),
                }
            }
        }
    });
    let landed = floor.stub.stops_landed(0);
    println!(
        "{rounds} rounds: {called} called, {stopped} stopped before the code, {interrupted} in the code; {} stops sent, landed {landed:?}",
        sent.load(Ordering::Relaxed)
    );
    assert!(
        landed.in_call > 0 && stopped > 0 && interrupted > 0,
        "no stop landed in the save, the enter or the code"
    );
}

/// Sets its flag when dropped.
struct Finished<'a>(&'a AtomicBool);

impl Drop for Finished<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// A call's round trip from translated code against a turn with no code, alternating batches on one CPU, the stub's
/// thread and the test's pinned together: the median over batches of the mean per turn of each. Knobs: WCHP_CPU,
/// WCHP_PER_BATCH, WCHP_BATCHES. Run with `--ignored --nocapture`.
#[test]
#[ignore = "a measurement, run on its own"]
fn call_round_trip_on_one_cpu() {
    let floor = floor();
    let cpu = knob("WCHP_CPU", 1) as usize;
    let per_batch = knob("WCHP_PER_BATCH", 10_000);
    let batches = knob("WCHP_BATCHES", 21);
    let mut set = rustix::thread::CpuSet::new();
    set.set(cpu);
    rustix::thread::sched_setaffinity(None, &set).expect("pin");
    rustix::thread::sched_setaffinity(Some(floor.stub.process()), &set).expect("pin");
    let mut rng = fastrand::Rng::with_seed(1);
    floor.stub.set_registers(0, &random_block(&mut rng));
    let batch = |call: bool| {
        let start = std::time::Instant::now();
        for _ in 0..per_batch {
            if call {
                floor.stub.enter(0, &[], CALL).expect("call");
            } else {
                floor.stub.turn(0, &[]).expect("turn");
            }
        }
        start.elapsed().as_secs_f64() * 1e9 / per_batch as f64
    };
    batch(true);
    batch(false);
    let (mut calls, mut turns): (Vec<f64>, Vec<f64>) =
        (0..batches).map(|_| (batch(true), batch(false))).unzip();
    for means in [&mut calls, &mut turns] {
        means.sort_by(f64::total_cmp);
    }
    println!(
        "on cpu {cpu}: call median {:.0} ns (lowest {:.0}, highest {:.0}), turn median {:.0} ns (lowest {:.0}, highest {:.0}), {batches} batches of {per_batch} each",
        calls[calls.len() / 2],
        calls[0],
        calls[calls.len() - 1],
        turns[turns.len() / 2],
        turns[0],
        turns[turns.len() - 1],
    );
}
