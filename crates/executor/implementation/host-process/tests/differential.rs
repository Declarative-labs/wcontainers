//! The virtual-machine form's differential test under form P: every presented ARM64 form's seeded cases (layer 1's)
//! run under the exact executor (one instruction a piece) and the ordinary one (full look-ahead), each compared whole
//! with the WebAssembly output, and the count each made with the counting rule's events. Beside each drawn case of a
//! load or store runs one of the bar's own made from it (across a page that refuses, tagged, with bit 55 set, or
//! misaligned where the form checks alignment); beside them the words layer 1 leaves out (register collisions and
//! the other values of forms swept whole), every word routed for the float status, and each drawn exclusive with
//! its partner.
//!
//! Knobs: `WCHP_DIFFERENTIAL_CASES` cases per form, `WCHP_SEED`, `WCHP_DIFFERENTIAL_FORMS` (form names, separated
//! by commas, run alone), `WCHP_DIFFERENTIAL_JOBS` threads for the WebAssembly side.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

#[allow(dead_code)]
#[path = "../../../../arm64/tests/common/mod.rs"]
mod words;

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use wcontainers_abi::{Access, Exit, MemoryId, Permissions, RegisterBlock, Resume, ThreadId};
use wcontainers_arm64::carry::TAG_LOW;
use wcontainers_arm64::encode::{self, field};
use wcontainers_arm64::forms::{self, Form};
use wcontainers_arm64::{Address, Arm64, Instruction as I, Reg, SystemRegister, Way, decode};
use wcontainers_executor_host_process::executor::HostProcessExecutor;
use wcontainers_executor_host_process::host::HostProcessHost;
use wcontainers_executor_host_process::stub::StopWhen;
use wcontainers_guest_cpu::position::Operand;
use wcontainers_guest_cpu::{Guest, Native, Route, Stopping, counted};
use wcontainers_translator::LookAhead;
use wcontainers_translator_test::guest::arm64::words::{
    Case, Outcome, Stop, cases, covered, excluded, translated,
};
use wcontainers_translator_test::layout::aarch64::CODE_SPAN;
use wcontainers_translator_test::layout::{
    CODE, DATA, DATA_BYTES, DATA_READ_ONLY, DATA_WRITABLE, fill,
};
use wcontainers_translator_test::runner::{Machine, Stopped, Syscalls};

use crate::words::{SWEPT_BITS, words_of};

const CASES_VARIABLE: &str = "WCHP_DIFFERENTIAL_CASES";
const SEED_VARIABLE: &str = "WCHP_SEED";
const FORMS_VARIABLE: &str = "WCHP_DIFFERENTIAL_FORMS";
const JOBS_VARIABLE: &str = "WCHP_DIFFERENTIAL_JOBS";
/// Cases per form unless told otherwise, as the virtual-machine form's test draws.
const DEFAULT_CASES: u64 = 16;
/// The seed unless told otherwise, the virtual-machine form's test's.
const DEFAULT_SEED: u64 = 0x2545_f491_4f6c_dd1d;
/// Threads the WebAssembly side runs on unless told otherwise.
const DEFAULT_JOBS: u64 = 3;
/// What separates form names in the list of forms to run.
const SEPARATOR: char = ',';

/// x0 to x30.
const GENERAL: usize = 31;
/// The case's stack pointer, flags and thread pointer, after the general registers.
const CASE_SP: usize = GENERAL;
const CASE_FLAGS: usize = CASE_SP + 1;
const CASE_THREAD: usize = CASE_FLAGS + 1;
/// The outcome's pc, flags and thread pointer.
const OUT_PC: usize = CASE_SP + 1;
const OUT_FLAGS: usize = OUT_PC + 1;
const OUT_THREAD: usize = OUT_FLAGS + 1;
/// Bytes of an instruction.
const INSTRUCTION: u64 = size_of::<u32>() as u64;
/// `msr fpsr, x0`: its system register's `o0`, `op1`, `CRn`, `CRm` and `op2`.
const FPSR_FIELDS: [u32; 5] = [1, 3, 4, 4, 1];

fn setting(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .map(|value| value.parse().expect("a number"))
        .unwrap_or(default)
}

/// The random generator for a form's cases, from the seed and the form's name.
fn generator(seed: u64, name: &str) -> TestRunner {
    let mut key = [0u8; size_of::<[u64; 4]>()];
    key[..size_of::<u64>()].copy_from_slice(&seed.to_le_bytes());
    let rest = key.len() - size_of::<u64>();
    for (i, byte) in name.bytes().enumerate() {
        key[size_of::<u64>() + i % rest] ^= byte;
    }
    TestRunner::new_with_rng(
        Config::default(),
        TestRng::from_seed(RngAlgorithm::ChaCha, &key),
    )
}

/// Why a word is left out here too: a branch to itself never finishes on the WebAssembly side.
const ENDLESS: &str = "branch to itself";

/// Whether `word` is one of `form`'s that layer 1 leaves out and this test runs.
fn left_out(form: &Form, word: u32) -> bool {
    forms::form(word).is_some_and(|f| std::ptr::eq(f, form))
        && form.presented(word)
        && excluded(form, word).is_some_and(|why| why != ENDLESS)
}

/// The register fields of a form.
fn register_fields(form: &Form) -> Vec<encode::FieldName> {
    form.fields
        .iter()
        .zip(encode::field_names(form))
        .filter(|((name, _, width), _)| {
            name.starts_with(field::REGISTER) && *width == encode::REGISTER_BITS
        })
        .map(|(_, field)| field)
        .collect()
}

/// The drawn case with each word it becomes when one register field takes another's register, where layer 1 leaves
/// that word out.
fn collisions(form: &Form, case: &Case) -> Vec<Case> {
    let fields = register_fields(form);
    let mut words: Vec<u32> = Vec::new();
    for &to in &fields {
        for &from in &fields {
            let Some(value) = form.field(case.word, from.name()).filter(|_| to != from) else {
                continue;
            };
            if let Ok(word) = encode::replace(case.word, to, value)
                && left_out(form, word)
                && !words.contains(&word)
            {
                words.push(word);
            }
        }
    }
    words
        .into_iter()
        .map(|word| Case {
            word,
            ..case.clone()
        })
        .collect()
}

/// The bits of a form other than its register fields that its words may set.
fn swept_bits(form: &Form) -> u32 {
    let registers = form
        .fields
        .iter()
        .filter(|(name, ..)| name.starts_with(field::REGISTER))
        .fold(0u32, |m, &(_, low, width)| {
            m | (((1u32 << width) - 1) << low)
        });
    (!form.mask & !form.should_mask & !registers).count_ones()
}

/// For a form swept whole, every word of it that `keep` takes, each on the registers of one of `drawn` in turn.
fn swept(form: &Form, drawn: &[Case], seed: u64, keep: impl Fn(u32) -> bool) -> Vec<Case> {
    if !(1..=SWEPT_BITS).contains(&swept_bits(form)) {
        return Vec::new();
    }
    let mut rng = fastrand::Rng::with_seed(seed);
    let words: Vec<u32> = words_of(form, 1, &mut rng)
        .into_iter()
        .filter(|&w| keep(w))
        .collect();
    words
        .into_iter()
        .zip(drawn.iter().cycle())
        .map(|(word, case)| Case {
            word,
            ..case.clone()
        })
        .collect()
}

/// A drawn exclusive case's word with its partner, in the order they run (a load exclusive then the store exclusive
/// of its size, acquire bit and base, or a store exclusive after such a load), the partner's other registers ones the
/// word does not name; with them, the store's status register.
fn paired(form: &Form, case: &Case) -> Option<([u32; 2], usize)> {
    let (load, pair) = match decode(case.word) {
        I::LoadExclusive { pair, .. } => (true, pair),
        I::StoreExclusive { pair, .. } => (false, pair),
        _ => return None,
    };
    let named: Vec<u32> = register_fields(form)
        .iter()
        .filter_map(|f| form.field(case.word, f.name()))
        .collect();
    let mut free = (0..u32::from(encode::ZERO)).filter(|r| !named.contains(r));
    let mut next = || free.next().expect("a register the word does not name");
    let zero = u32::from(encode::ZERO);
    let status = if load { next() } else { zero };
    let first = next();
    let second = if pair { next() } else { zero };
    let partner = [
        (field::LOAD, u32::from(!load)),
        (field::RS, status),
        (field::RT, first),
        (field::RT2, second),
    ]
    .into_iter()
    .try_fold(case.word, |word, (name, value)| {
        encode::replace(word, name, value)
    })
    .ok()
    .filter(|&word| forms::presented(word))?;
    let store = if load { partner } else { case.word };
    let status = form.field(store, field::RS.name())? as usize;
    Some((
        if load {
            [case.word, partner]
        } else {
            [partner, case.word]
        },
        status,
    ))
}

/// A counter read's destination: its value is the clock's when read, on either side.
fn clock_destination(word: u32) -> Option<usize> {
    match decode(word) {
        I::ReadSystem {
            rt: Reg::X(n),
            register: SystemRegister::Counter,
        } => Some(usize::from(n)),
        _ => None,
    }
}

/// The boundaries of the data area where a page refuses: before it, before its read-only page, after it.
const REFUSING: [u64; 3] = [DATA, DATA + DATA_WRITABLE, DATA + DATA_BYTES];

/// The kinds of the bar's own cases made from a drawn load or store.
const EDGE_KINDS: u64 = 4;

/// The bar's own case made from a drawn case of a load or store, chosen by the case's seed: its access placed across
/// one of the data area's boundaries where a page refuses, at an offset within it; the same with a tag in the base's
/// top byte; the base with bit 55 set; or, for a form that checks alignment, the base moved off it. `None` for a form
/// with no base register to move.
fn edge(case: &Case) -> Option<Case> {
    let seed = case.seed;
    let (i, bytes, _, _) = base_of(case)?;
    let within = (seed >> u8::BITS) % bytes.max(2);
    let boundary = REFUSING[(seed >> u16::BITS) as usize % REFUSING.len()];
    let mut edge = across(case, boundary, within)?;
    let (_, _, slot, crossing) = base_of(&edge)?;
    let original = case.registers[slot];
    let tag = (seed >> u32::BITS) << TAG_LOW;
    edge.registers[slot] = match seed % EDGE_KINDS {
        0 => crossing,
        1 => crossing | tag & !(1 << (TAG_LOW - 1)),
        2 => original | tag | 1 << (TAG_LOW - 1),
        _ if matches!(
            Arm64::route(&Way::Process, &i, &()),
            Route::Aligned | Route::Exclusive
        ) =>
        {
            original + within.max(1)
        }
        _ => crossing,
    };
    Some(edge)
}

/// A load or store case's instruction, its access's bytes, the slot of its base register in the case's registers and
/// the base's value; `None` for an instruction with no base register to move.
fn base_of(case: &Case) -> Option<(I, u64, usize, u64)> {
    let i = decode(case.word);
    let m = i.memory_access()?;
    let (base, index) = match m.address {
        Address::Offset { base, .. }
        | Address::PreIndex { base, .. }
        | Address::PostIndex { base, .. } => (base, None),
        Address::Indexed { base, index, .. } => (base, Some(index)),
        Address::Literal { .. } => return None,
    };
    if index == Some(base) {
        return None;
    }
    let slot = match base {
        Reg::X(n) => usize::from(n),
        Reg::Sp => CASE_SP,
        Reg::Zero => return None,
    };
    Some((i, m.bytes.into(), slot, case.registers[slot]))
}

/// The case with its access's base moved so that the access starts `within` bytes before `boundary`.
fn across(case: &Case, boundary: u64, within: u64) -> Option<Case> {
    let (i, _, slot, _) = base_of(case)?;
    let mut x = [0u64; GENERAL];
    x.copy_from_slice(&case.registers[..GENERAL]);
    let mut sp = case.registers[CASE_SP];
    if slot == CASE_SP {
        sp = 0;
    } else {
        x[slot] = 0;
    }
    let Stopping::Access(operand) = Arm64::stopping(&i, CODE) else {
        return None;
    };
    // The access's first byte with a base of zero.
    let delta = access_first(&operand, &x, sp);
    let mut moved = case.clone();
    moved.registers[slot] = boundary.wrapping_sub(within).wrapping_sub(delta);
    Some(moved)
}

/// The first byte of an access at [`CODE`] with the registers `x` and `sp`.
fn access_first(operand: &Operand, x: &[u64; GENERAL], sp: u64) -> u64 {
    wcontainers_executor_arm64_stop::access_first(operand, CODE, x, sp)
}

/// How a run under form P ended: its stop in layer 1's terms, its registers, its data area and the count it made.
#[derive(Clone, Debug)]
struct Ran {
    stop: Stop,
    after: RegisterBlock,
    data: Vec<u8>,
    progress: u64,
}

impl Ran {
    /// Layer 1's outcome of the run of one case, against the data area it started with.
    fn outcome(&self, start: &[u8]) -> Outcome {
        let a = &self.after;
        let mut registers = [0u64; OUT_THREAD + 1];
        registers[..GENERAL].copy_from_slice(&a.x);
        registers[CASE_SP] = a.sp;
        registers[OUT_PC] = a.pc;
        registers[OUT_FLAGS] = a.pstate;
        registers[OUT_THREAD] = a.tpidr_el0;
        Outcome {
            stop: self.stop,
            registers,
            vectors: vectors(a),
            float: [a.fpsr, a.fpcr],
            changed: self
                .data
                .iter()
                .zip(start)
                .enumerate()
                .filter(|(_, (now, before))| now != before)
                .map(|(k, (now, _))| (k as u32, *now))
                .collect(),
        }
    }
}

fn vectors(registers: &RegisterBlock) -> [u128; 32] {
    registers
        .v
        .map(|[low, high]| u128::from(high) << u64::BITS | u128::from(low))
}

/// The register block a case starts from, at `pc`.
fn block_of(case: &Case, pc: u64) -> RegisterBlock {
    let mut registers = RegisterBlock::ZERO;
    registers.x.copy_from_slice(&case.registers[..GENERAL]);
    registers.sp = case.registers[CASE_SP];
    registers.pc = pc;
    registers.pstate = case.registers[CASE_FLAGS];
    registers.tpidr_el0 = case.registers[CASE_THREAD];
    registers.v = case.vectors.map(|v| [v as u64, (v >> u64::BITS) as u64]);
    [registers.fpsr, registers.fpcr] = case.float;
    registers
}

/// The data area a case starts with.
fn start_data(case: &Case) -> Vec<u8> {
    let mut data = vec![0u8; DATA_BYTES as usize];
    fill(case.seed, &mut data);
    data
}

/// An executor of form P with one memory laid out as layer 1's: the data area, and the code area holding the filler
/// (zero, `udf #0`) apart from the words under test.
struct Side {
    cpu: HostProcessExecutor,
    memory: MemoryId,
}

impl Side {
    fn new(cpu: HostProcessExecutor) -> Side {
        let mut cpu = cpu;
        let memory = cpu.memory_new().expect("a memory");
        let regions = [
            (DATA, DATA_WRITABLE, Permissions::READ | Permissions::WRITE),
            (DATA + DATA_WRITABLE, DATA_READ_ONLY, Permissions::READ),
            (
                CODE - CODE_SPAN,
                2 * CODE_SPAN,
                Permissions::READ | Permissions::EXECUTE,
            ),
        ];
        for (at, length, allowed) in regions {
            cpu.set_permissions(memory, at, length, allowed)
                .expect("a region of layer 1's");
        }
        Side { cpu, memory }
    }

    /// Puts `words` at [`CODE`], dropping what was translated from there.
    fn place(&mut self, words: &[u32]) {
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        self.cpu
            .write(self.memory, CODE, &bytes)
            .expect("the code area");
        self.cpu
            .code_changed(self.memory, CODE, bytes.len() as u64, &[]);
    }

    /// Runs `thread` to its first exit, its turn unlimited (a case never branches to itself), with the count made
    /// until it.
    fn first_exit(&mut self, thread: ThreadId) -> (Exit, u64) {
        let before = self.cpu.guest_count(thread);
        let unlimited = StopWhen {
            slice: None,
            wanted: None,
        };
        let exit = self
            .cpu
            .run_translated(thread, Resume::Continue, None, unlimited, &mut || 0);
        (exit, self.cpu.guest_count(thread) - before)
    }

    /// Runs `words` at [`CODE`] from `case`'s state on a new thread to the first exit.
    fn run(&mut self, words: &[u32], case: &Case) -> Ran {
        let thread = self.cpu.thread_new(self.memory).expect("a thread");
        if case.tracking {
            // Its float flags are kept exact, as after a program wrote FPSR: the thread writes it first.
            let write = encode::write_system_register(FPSR_FIELDS, 0).expect("msr fpsr, x0");
            debug_assert!(matches!(
                decode(write),
                I::WriteSystem {
                    register: SystemRegister::Fpsr,
                    ..
                }
            ));
            self.place(&[write]);
            let mut registers = RegisterBlock::ZERO;
            registers.pc = CODE;
            registers.x[0] = u64::from(case.float[0]);
            self.cpu.set_registers(thread, &registers);
            let (exit, _) = self.first_exit(thread);
            let mut after = RegisterBlock::ZERO;
            self.cpu.registers(thread, &mut after);
            assert!(
                matches!(exit, Exit::IllegalInstruction | Exit::Stop)
                    && after.pc == CODE + INSTRUCTION,
                "the float status's write ends at the filler after it: {exit:?} at {:#x}",
                after.pc
            );
        }
        self.cpu
            .write(self.memory, DATA, &start_data(case))
            .expect("the data area");
        self.place(words);
        self.cpu.set_registers(thread, &block_of(case, CODE));
        let (exit, progress) = self.first_exit(thread);
        let mut after = RegisterBlock::ZERO;
        self.cpu.registers(thread, &mut after);
        let mut data = vec![0u8; DATA_BYTES as usize];
        self.cpu
            .read(self.memory, DATA, &mut data)
            .expect("the data area");
        self.place(&vec![0; words.len()]);
        self.cpu.thread_drop(thread);
        let end = CODE + words.len() as u64 * INSTRUCTION;
        let stop = match exit {
            // The filler, or an instruction past the words, is where control landed.
            Exit::IllegalInstruction if (CODE..end).contains(&after.pc) => Stop::Illegal,
            Exit::IllegalInstruction | Exit::Stop => Stop::Landed(after.pc),
            Exit::MemoryFault {
                address,
                access: Access::Execute,
            } => Stop::Landed(address),
            Exit::MemoryFault { address, .. } => Stop::MemoryFault(address),
            Exit::AlignmentFault { address } => Stop::Alignment(address),
            Exit::Syscall(_) => Stop::Syscall,
            Exit::Breakpoint { .. } => Stop::Breakpoint,
            // No stop of layer 1's: it differs from every outcome.
            _ => Stop::Other(u32::MAX, 0),
        };
        Ran {
            stop,
            after,
            data,
            progress,
        }
    }
}

/// What differs between `got` and `expected`, with the count against the counting rule's.
fn differs(got: &Outcome, expected: &Outcome, count: u64, rule: u64) -> Option<String> {
    let mut found = Vec::new();
    if got.stop != expected.stop {
        found.push(format!("stop {:?}, expected {:?}", got.stop, expected.stop));
    }
    for (k, (g, e)) in got.registers.iter().zip(&expected.registers).enumerate() {
        if g != e {
            found.push(format!("register {k} {g:#x}, expected {e:#x}"));
        }
    }
    for (k, (g, e)) in got.vectors.iter().zip(&expected.vectors).enumerate() {
        if g != e {
            found.push(format!("v{k} {g:#x}, expected {e:#x}"));
        }
    }
    if got.float != expected.float {
        found.push(format!(
            "float {:x?}, expected {:x?}",
            got.float, expected.float
        ));
    }
    if got.changed != expected.changed {
        found.push(format!(
            "changed {:x?}, expected {:x?}",
            got.changed, expected.changed
        ));
    }
    if count != rule {
        found.push(format!("count {count}, the counting rule's {rule}"));
    }
    (!found.is_empty()).then(|| found.join("; "))
}

/// The counting rule's events of a one-instruction case that stopped as `stop` with `after`: those of its instruction
/// if it ran.
fn rule(case: &Case, stop: Stop, after: &RegisterBlock) -> u64 {
    let executed = match stop {
        Stop::Landed(_) => true,
        Stop::Alignment(_) | Stop::MemoryFault(_) => after.pc != CODE,
        _ => false,
    };
    u64::from(executed && counted(CODE, &Arm64::flow(&decode(case.word))))
}

/// The WebAssembly output's run of `words` at [`CODE`] from the state of `case`, at `look_ahead`: its stop, registers
/// and data area.
fn webassembly(
    words: &[u32],
    case: &Case,
    look_ahead: LookAhead,
) -> (Stop, RegisterBlock, Vec<u8>) {
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let slots = u32::try_from(words.len()).expect("a short function");
    let mut machine = Machine::<Arm64>::new(CODE + CODE_SPAN, CODE, slots, Syscalls::Stop);
    machine.set_permissions(
        DATA..DATA + DATA_WRITABLE,
        Permissions::READ | Permissions::WRITE,
    );
    machine.set_permissions(DATA + DATA_WRITABLE..DATA + DATA_BYTES, Permissions::READ);
    machine.set_permissions(
        CODE - CODE_SPAN..CODE + CODE_SPAN,
        Permissions::READ | Permissions::EXECUTE,
    );
    let mut data = start_data(case);
    machine.write(DATA, &data);
    let starts: Vec<u64> = (0..bytes.len() as u64)
        .step_by(INSTRUCTION as usize)
        .collect();
    machine.load(
        &bytes,
        0..bytes.len() as u64,
        (CODE, None),
        &starts,
        look_ahead,
    );
    machine.set_registers(&block_of(case, CODE));
    if case.tracking {
        machine.track_float_flags();
    }
    let (stopped, after) = machine.run();
    machine.read(DATA, &mut data);
    let stop = match stopped {
        Stopped::NoCode(address)
        | Stopped::Exit(Exit::MemoryFault {
            address,
            access: Access::Execute,
        }) => Stop::Landed(address),
        Stopped::Exit(Exit::MemoryFault { address, .. }) => Stop::MemoryFault(address),
        Stopped::Exit(Exit::AlignmentFault { address }) => Stop::Alignment(address),
        other => panic!("not a stop these pairs make: {other:?}"),
    };
    (stop, after, data)
}

/// `run` of each of `items`, on `jobs` threads.
fn on_threads<T: Sync, R: Send>(items: &[T], jobs: usize, run: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let chunk = items.len().div_ceil(jobs).max(1);
    let run = &run;
    std::thread::scope(|scope| {
        let handles: Vec<_> = items
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(run).collect::<Vec<_>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("the WebAssembly side"))
            .collect()
    })
}

/// What the thread of form P is asked to run: words from a case's state.
type Job = (Vec<u32>, Case);

/// The runs of each job under the exact executor and the ordinary one, made on a thread of their own so that the
/// executors' pinning of their runner leaves the WebAssembly side's threads free.
struct FormP {
    jobs: mpsc::Sender<Vec<Job>>,
    runs: mpsc::Receiver<Vec<[Ran; 2]>>,
}

impl FormP {
    fn start() -> FormP {
        let (jobs, asked) = mpsc::channel::<Vec<Job>>();
        let (ran, runs) = mpsc::channel();
        std::thread::spawn(move || {
            let host = Arc::new(HostProcessHost::new().expect("the host is refused"));
            let mut sides = [
                Side::new(HostProcessExecutor::exact(host.clone())),
                Side::new(HostProcessExecutor::new(host)),
            ];
            for jobs in asked {
                let runs = jobs
                    .iter()
                    .map(|(words, case)| sides.each_mut().map(|side| side.run(words, case)))
                    .collect();
                if ran.send(runs).is_err() {
                    break;
                }
            }
        });
        FormP { jobs, runs }
    }

    fn ask(&self, jobs: Vec<Job>) {
        self.jobs.send(jobs).expect("form P's thread");
    }

    fn take(&self) -> Vec<[Ran; 2]> {
        self.runs.recv().expect("form P's thread")
    }
}

/// The names of the two executors of form P, in the order [`FormP`] runs them.
const SIDES: [&str; 2] = ["exact", "ordinary"];

#[test]
fn every_form_s_cases_end_as_the_web_assembly_output_s() {
    let count = setting(CASES_VARIABLE, DEFAULT_CASES);
    let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
    let jobs = setting(JOBS_VARIABLE, DEFAULT_JOBS).max(1) as usize;
    let only: Option<Vec<String>> = std::env::var(FORMS_VARIABLE)
        .ok()
        .map(|list| list.split(SEPARATOR).map(String::from).collect());
    let started = Instant::now();
    let p = FormP::start();
    let chosen: Vec<_> = forms::all()
        .iter()
        .filter(|f| covered(f))
        .filter(|f| only.as_ref().is_none_or(|names| names.contains(&f.name)))
        .collect();
    let mut failing = Vec::new();
    let mut total = 0u64;
    let mut edge_cases = 0u64;
    let mut left_out_cases = 0u64;
    let mut float_status_cases = 0u64;
    let mut pair_cases = 0u64;
    let mut stored = 0u64;
    for form in &chosen {
        let mut generator = generator(seed, &form.name);
        let strategy = cases((*form).clone());
        let mut drawn: Vec<Case> = (0..count)
            .map(|_| strategy.new_tree(&mut generator).expect("a case").current())
            .collect();
        let edges: Vec<Case> = drawn.iter().filter_map(edge).collect();
        edge_cases += edges.len() as u64;
        let mut collided: Vec<Case> = drawn.iter().flat_map(|c| collisions(form, c)).collect();
        collided.extend(swept(form, &drawn, seed, |w| left_out(form, w)));
        left_out_cases += collided.len() as u64;
        let float_status = swept(form, &drawn, seed, |w| {
            !left_out(form, w) && Arm64::route(&Way::Process, &decode(w), &()) == Route::FloatStatus
        });
        float_status_cases += float_status.len() as u64;
        drawn.extend(edges);
        drawn.extend(collided);
        drawn.extend(float_status);
        let pairs: Vec<(Case, ([u32; 2], usize))> = drawn
            .iter()
            .take(count as usize)
            .filter_map(|c| Some((c.clone(), paired(form, c)?)))
            .collect();
        pair_cases += pairs.len() as u64;
        let mut asked: Vec<Job> = drawn.iter().map(|c| (vec![c.word], c.clone())).collect();
        asked.extend(
            pairs
                .iter()
                .map(|(c, (words, _))| (words.to_vec(), c.clone())),
        );
        p.ask(asked);
        let wasm = on_threads(&drawn, jobs, translated);
        let wasm_pairs = on_threads(&pairs, jobs, |(case, (words, _))| {
            webassembly(words, case, LookAhead::Full)
        });
        let native = p.take();
        let (singles, doubles) = native.split_at(drawn.len());
        let mut first = None;
        let mut failed = 0;
        for ((case, expected), runs) in drawn.iter().zip(&wasm).zip(singles) {
            for (side, ran) in SIDES.iter().zip(runs) {
                total += 1;
                let mut got = ran.outcome(&start_data(case));
                if let Some(rt) = clock_destination(case.word) {
                    got.registers[rt] = expected.registers[rt];
                }
                let rule = rule(case, got.stop, &ran.after);
                if let Some(difference) = differs(&got, expected, ran.progress, rule) {
                    failed += 1;
                    first.get_or_insert_with(|| {
                        format!(
                            "{side}, word {:#010x}: {difference}\n  case {case:?}",
                            case.word
                        )
                    });
                }
            }
        }
        for (((case, (words, status)), (stop, after, data)), runs) in
            pairs.iter().zip(&wasm_pairs).zip(doubles)
        {
            for (side, ran) in SIDES.iter().zip(runs) {
                total += 1;
                let end = CODE + words.len() as u64 * INSTRUCTION;
                if ran.stop == Stop::Landed(end) && ran.after.x.get(*status) == Some(&0) {
                    stored += 1;
                }
                let same = *stop == ran.stop
                    && after.x == ran.after.x
                    && after.sp == ran.after.sp
                    && after.pc == ran.after.pc
                    && after.v == ran.after.v
                    && [after.fpsr, after.fpcr] == [ran.after.fpsr, ran.after.fpcr]
                    && after.pstate == ran.after.pstate
                    && after.tpidr_el0 == ran.after.tpidr_el0
                    && *data == ran.data;
                if !same {
                    failed += 1;
                    first.get_or_insert_with(|| {
                        format!(
                            "{side}, words {words:#010x?} run as a pair: the WebAssembly output ends {stop:?} {after:x?}, form P {:?} {:x?}\n  case {case:?}",
                            ran.stop, ran.after
                        )
                    });
                }
            }
        }
        if let Some(first) = first {
            println!(
                "FAIL {}: {failed} of {}: {first}",
                form.name,
                2 * (drawn.len() + pairs.len())
            );
            failing.push(form.name.clone());
        }
    }
    println!(
        "form P against WebAssembly: {} forms, {count} drawn cases each, {edge_cases} of the bar's own, {left_out_cases} words layer 1 leaves out, {float_status_cases} float status words and {pair_cases} exclusive pairs ({stored} storing), each under the exact and the ordinary executor ({total} runs in all), seed {seed:#x}, {} failing, in {:?}",
        chosen.len(),
        failing.len(),
        started.elapsed()
    );
    if !failing.is_empty() {
        println!("failing forms: {}", failing.join(" "));
    }
    assert!(failing.is_empty(), "forms differ");
}
