//! The x86-64 litmus shapes, translated and run by two or four host threads on one shared guest memory (XF3).
//!
//! Shapes and rounds are fixed: no fuzzing suite. Each round the threads meet at a system call, wait empty turns
//! that change by round, run their parts and meet again; thread 0 counts the outcome. [`simulate_shape`] runs the
//! same parts on the simulated weaker engine.

use crate::guest::GuestTest;
use crate::guest_memory::SharedGuestMemory;
use crate::runner::{Machine, Meeting, Stopped, Syscalls};
use crate::simulated::{Event, Kind, Script};
use iced_x86::IcedError;
use iced_x86::code_asm::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use wcontainers_abi::{PAGE_SIZE, Permissions};
use wcontainers_guest_cpu::Personality;
use wcontainers_translator::{AccessOrdering, LookAhead};
use wcontainers_x86::X86_64;

type Asm = Result<(), IcedError>;

/// Where each thread's code starts: thread `k`'s page is the `k`th from here.
const CODE: u64 = 0x10000;
/// Where the words the threads share, their results and the counts of outcomes are, held in rbp by every thread.
const DATA: u64 = 0x10_0000;
/// The guest memory, the data's pages and a stack page above them.
const MEMORY_BYTES: u64 = 0x20_0000;
const DATA_BYTES: u64 = 0x1_0000;
/// Bytes between two shared words, so that no two share a cache line on any host.
///
/// https://developer.arm.com/documentation/100616/0401/L1-memory-system/About-the-L1-memory-system
const LINE: i32 = 128;
/// The shared words, by their place from [`DATA`].
const X: i32 = 0;
const Y: i32 = LINE;
const SEQUENCE: i32 = 2 * LINE;
const DATA_0: i32 = 3 * LINE;
const DATA_1: i32 = 4 * LINE;
const FLAG: i32 = 5 * LINE;
const COUNTER: i32 = 6 * LINE;
/// Where thread `k`'s results are: from here, `k` lines on.
const RESULTS: i32 = 0x1000;
/// Where the count of each outcome is, by its number.
const COUNTS: i32 = 0x2000;
/// The most bits an outcome's number has, so that its counts fit before the stack.
const OUTCOME_BITS: u32 = 8;
/// Bytes of a guest word.
const WORD: i32 = wcontainers_abi::WORD as i32;
/// The system call at which the threads meet; Linux has none of this number.
///
/// https://github.com/torvalds/linux/blob/master/arch/x86/entry/syscalls/syscall_64.tbl
pub const MEET: u64 = 0x4D45;
const EXIT: u64 = <X86_64 as GuestTest>::STAND_IN.exit;
/// Each thread's empty turns before its part: its round number times its own odd number, below [`TURNS`].
const MULTIPLIERS: [i32; 4] = [5, 11, 17, 23];
const TURNS: i32 = 64;
/// The store-back's empty turns between its two loads, as the threads plan's test waits.
const STORE_BACK_TURNS: i32 = 200;

/// A thread's part of a round, emitted with rbp holding [`DATA`].
pub type Part = fn(&mut CodeAssembler) -> Asm;

/// One litmus shape.
pub struct Litmus {
    pub name: &'static str,
    /// Each thread's part, thread 0 first.
    pub parts: &'static [Part],
    /// What thread 0 reads after a round: each value's name, place from [`DATA`] and bits, highest first.
    pub observed: &'static [(&'static str, i32, u32)],
    /// The shared words set back to 0 after each round.
    pub reset: &'static [i32],
    /// Whether x86 forbids an outcome, given the observed values in order.
    pub forbidden: fn(&[u64]) -> bool,
    /// Whether no interleaving of the threads gives an outcome x86 allows.
    ///
    /// The ordered form, whose accesses are sequentially consistent, never shows such an outcome either.
    pub also_excluded: fn(&[u64]) -> bool,
}

impl Litmus {
    pub fn threads(&self) -> usize {
        self.parts.len()
    }

    /// Returns how many outcomes the observed values can make.
    pub fn outcomes(&self) -> usize {
        1 << self.observed.iter().map(|o| o.2).sum::<u32>()
    }

    /// Tells whether no interleaving of the threads gives the outcome.
    ///
    /// The ordered form must never show such an outcome, and the control must.
    pub fn excluded_by_interleavings(&self, values: &[u64]) -> bool {
        (self.forbidden)(values) || (self.also_excluded)(values)
    }

    /// Returns the observed values of an outcome's number.
    pub fn values(&self, outcome: usize) -> Vec<u64> {
        let mut rest = outcome as u64;
        let mut values: Vec<u64> = self
            .observed
            .iter()
            .rev()
            .map(|&(_, _, bits)| {
                let value = rest & ((1 << bits) - 1);
                rest >>= bits;
                value
            })
            .collect();
        values.reverse();
        values
    }

    /// Names an outcome by its values.
    pub fn describe(&self, outcome: usize) -> String {
        self.observed
            .iter()
            .zip(self.values(outcome))
            .map(|(&(name, _, _), value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Returns where thread `k`'s `n`th result goes.
const fn result(k: i32, n: i32) -> i32 {
    RESULTS + k * LINE + n * WORD
}

fn store(a: &mut CodeAssembler, at: i32) -> Asm {
    a.mov(qword_ptr(rbp + at), 1)
}

/// Each thread stores to its word and loads the other's; x86 allows both loads to see 0.
pub const STORE_BUFFERING: Litmus = Litmus {
    name: "store buffering",
    parts: &[
        |a| {
            store(a, X)?;
            a.mov(rax, qword_ptr(rbp + Y))?;
            a.mov(qword_ptr(rbp + result(0, 0)), rax)
        },
        |a| {
            store(a, Y)?;
            a.mov(rax, qword_ptr(rbp + X))?;
            a.mov(qword_ptr(rbp + result(1, 0)), rax)
        },
    ],
    observed: &[("r0", result(0, 0), 1), ("r1", result(1, 0), 1)],
    reset: &[X, Y],
    forbidden: |_| false,
    also_excluded: both_saw_zero,
};

/// Tells whether both loads saw 0, which no interleaving gives: the outcome store buffering shows.
pub fn both_saw_zero(values: &[u64]) -> bool {
    values == [0, 0]
}

/// Store buffering with `mfence` between each thread's store and load: x86 forbids both loads seeing 0.
pub const STORE_BUFFERING_FENCED: Litmus = Litmus {
    name: "store buffering with mfence",
    parts: &[
        |a| {
            store(a, X)?;
            a.mfence()?;
            a.mov(rax, qword_ptr(rbp + Y))?;
            a.mov(qword_ptr(rbp + result(0, 0)), rax)
        },
        |a| {
            store(a, Y)?;
            a.mfence()?;
            a.mov(rax, qword_ptr(rbp + X))?;
            a.mov(qword_ptr(rbp + result(1, 0)), rax)
        },
    ],
    observed: &[("r0", result(0, 0), 1), ("r1", result(1, 0), 1)],
    reset: &[X, Y],
    forbidden: both_saw_zero,
    also_excluded: |_| false,
};

/// One thread stores data then a flag, the other loads the flag then the data; x86 forbids flag 1 with data 0.
pub const MESSAGE_PASSING: Litmus = Litmus {
    name: "message passing",
    parts: &[
        |a| {
            store(a, X)?;
            store(a, Y)
        },
        |a| {
            a.mov(rax, qword_ptr(rbp + Y))?;
            a.mov(rbx, qword_ptr(rbp + X))?;
            a.mov(qword_ptr(rbp + result(1, 0)), rax)?;
            a.mov(qword_ptr(rbp + result(1, 1)), rbx)
        },
    ],
    observed: &[("flag", result(1, 0), 1), ("data", result(1, 1), 1)],
    reset: &[X, Y],
    forbidden: |v| v == [1, 0],
    also_excluded: |_| false,
};

/// Each thread loads one word, then stores to the other; x86 forbids both loads seeing 1.
pub const LOAD_BUFFERING: Litmus = Litmus {
    name: "load buffering",
    parts: &[
        |a| {
            a.mov(rax, qword_ptr(rbp + X))?;
            store(a, Y)?;
            a.mov(qword_ptr(rbp + result(0, 0)), rax)
        },
        |a| {
            a.mov(rax, qword_ptr(rbp + Y))?;
            store(a, X)?;
            a.mov(qword_ptr(rbp + result(1, 0)), rax)
        },
    ],
    observed: &[("r0", result(0, 0), 1), ("r1", result(1, 0), 1)],
    reset: &[X, Y],
    forbidden: |v| v == [1, 1],
    also_excluded: |_| false,
};

/// A writer and a reader of a sequence lock; x86 forbids an even, unchanged count with words other than its own.
///
/// The writer makes the count odd, writes two words, makes it even; the reader loads the count, the two words and
/// the count again.
pub const SEQUENCE_LOCK: Litmus = Litmus {
    name: "sequence lock",
    parts: &[
        |a| {
            store(a, SEQUENCE)?;
            store(a, DATA_0)?;
            store(a, DATA_1)?;
            a.mov(qword_ptr(rbp + SEQUENCE), 2)
        },
        |a| {
            a.mov(rax, qword_ptr(rbp + SEQUENCE))?;
            a.mov(rbx, qword_ptr(rbp + DATA_0))?;
            a.mov(rdx, qword_ptr(rbp + DATA_1))?;
            a.mov(rsi, qword_ptr(rbp + SEQUENCE))?;
            a.mov(qword_ptr(rbp + result(1, 0)), rax)?;
            a.mov(qword_ptr(rbp + result(1, 1)), rbx)?;
            a.mov(qword_ptr(rbp + result(1, 2)), rdx)?;
            a.mov(qword_ptr(rbp + result(1, 3)), rsi)
        },
    ],
    observed: &[
        ("count", result(1, 0), 2),
        ("word0", result(1, 1), 1),
        ("word1", result(1, 2), 1),
        ("count again", result(1, 3), 2),
    ],
    reset: &[SEQUENCE, DATA_0, DATA_1],
    forbidden: |v| {
        let (count, words, again) = (v[0], [v[1], v[2]], v[3]);
        count == again && count % 2 == 0 && words != [count / 2; 2]
    },
    also_excluded: |_| false,
};

/// A store of what was loaded, after a flag; x86 forbids the loads seeing 0 and 1 with x ending 1.
///
/// One thread loads x, waits, loads the flag and, if it is set, stores back what it loaded; the other stores x,
/// `mfence`, then the flag.
pub const STORE_BACK: Litmus = Litmus {
    name: "store-back",
    parts: &[
        |a| {
            let mut wait = a.create_label();
            let mut skip = a.create_label();
            a.mov(rax, qword_ptr(rbp + X))?;
            a.mov(r13d, STORE_BACK_TURNS)?;
            a.set_label(&mut wait)?;
            a.dec(r13)?;
            a.jnz(wait)?;
            a.mov(rbx, qword_ptr(rbp + FLAG))?;
            a.test(rbx, rbx)?;
            a.jz(skip)?;
            a.mov(qword_ptr(rbp + X), rax)?;
            a.set_label(&mut skip)?;
            a.mov(qword_ptr(rbp + result(0, 0)), rax)?;
            a.mov(qword_ptr(rbp + result(0, 1)), rbx)
        },
        |a| {
            store(a, X)?;
            a.mfence()?;
            store(a, FLAG)
        },
    ],
    observed: &[
        ("r", result(0, 0), 1),
        ("flag", result(0, 1), 1),
        ("x at the end", X, 1),
    ],
    reset: &[X, FLAG],
    forbidden: |v| v == [0, 1, 1],
    also_excluded: |_| false,
};

/// Two threads each store a word, two others load both in opposite orders; x86 forbids them disagreeing on the order.
pub const INDEPENDENT_READS: Litmus = Litmus {
    name: "independent reads of independent writes",
    parts: &[
        |a| store(a, X),
        |a| store(a, Y),
        |a| {
            a.mov(rax, qword_ptr(rbp + X))?;
            a.mov(rbx, qword_ptr(rbp + Y))?;
            a.mov(qword_ptr(rbp + result(2, 0)), rax)?;
            a.mov(qword_ptr(rbp + result(2, 1)), rbx)
        },
        |a| {
            a.mov(rax, qword_ptr(rbp + Y))?;
            a.mov(rbx, qword_ptr(rbp + X))?;
            a.mov(qword_ptr(rbp + result(3, 0)), rax)?;
            a.mov(qword_ptr(rbp + result(3, 1)), rbx)
        },
    ],
    observed: &[
        ("x2", result(2, 0), 1),
        ("y2", result(2, 1), 1),
        ("y3", result(3, 0), 1),
        ("x3", result(3, 1), 1),
    ],
    reset: &[X, Y],
    forbidden: |v| v == [1, 0, 1, 0],
    also_excluded: |_| false,
};

/// Every shape, in the order the tests run them.
pub const SHAPES: [&Litmus; 7] = [
    &STORE_BUFFERING,
    &STORE_BUFFERING_FENCED,
    &MESSAGE_PASSING,
    &LOAD_BUFFERING,
    &SEQUENCE_LOCK,
    &STORE_BACK,
    &INDEPENDENT_READS,
];

/// Emits the system call that meets the other threads.
fn meet(a: &mut CodeAssembler) -> Asm {
    a.mov(eax, MEET as u32)?;
    a.syscall()
}

/// Emits the end of the thread: `exit(0)`.
fn exit(a: &mut CodeAssembler) -> Asm {
    a.mov(eax, EXIT as u32)?;
    a.xor(edi, edi)?;
    a.syscall()
}

/// Assembles thread `k`'s program for `rounds` rounds of a shape at its address.
fn program(litmus: &Litmus, k: usize, rounds: u64) -> Result<Vec<u8>, IcedError> {
    let mut a = CodeAssembler::new(u64::BITS)?;
    let mut top = a.create_label();
    let mut turn = a.create_label();
    a.mov(rbp, DATA)?;
    a.mov(r15, rounds)?;
    a.xor(r14d, r14d)?;
    a.set_label(&mut top)?;
    meet(&mut a)?;
    a.imul_3(r13, r14, MULTIPLIERS[k])?;
    a.and(r13, TURNS - 1)?;
    a.inc(r13)?;
    a.set_label(&mut turn)?;
    a.dec(r13)?;
    a.jnz(turn)?;
    (litmus.parts[k])(&mut a)?;
    meet(&mut a)?;
    if k == 0 {
        a.xor(eax, eax)?;
        for &(_, at, bits) in litmus.observed {
            a.shl(rax, bits as i32)?;
            a.mov(rcx, qword_ptr(rbp + at))?;
            a.and(rcx, (1 << bits) - 1)?;
            a.or(rax, rcx)?;
        }
        a.inc(qword_ptr(rbp + rax * WORD + COUNTS))?;
        for &at in litmus.reset {
            a.mov(qword_ptr(rbp + at), 0)?;
        }
    }
    a.inc(r14)?;
    a.cmp(r14, r15)?;
    a.jb(top)?;
    exit(&mut a)?;
    a.assemble(code_address(k))
}

/// Assembles a counter thread's program: it meets the others, adds 1 `rounds` times with `lock xadd`, and ends.
fn counter_program(k: usize, rounds: u64) -> Result<Vec<u8>, IcedError> {
    let mut a = CodeAssembler::new(u64::BITS)?;
    let mut top = a.create_label();
    a.mov(rbp, DATA)?;
    a.mov(r15, rounds)?;
    meet(&mut a)?;
    a.set_label(&mut top)?;
    a.mov(eax, 1)?;
    a.lock().xadd(qword_ptr(rbp + COUNTER), rax)?;
    a.dec(r15)?;
    a.jnz(top)?;
    exit(&mut a)?;
    a.assemble(code_address(k))
}

fn code_address(k: usize) -> u64 {
    CODE + k as u64 * PAGE_SIZE
}

/// Runs each program on its own host thread and machine, on one fresh shared memory; returns it once all have ended.
///
/// Each is translated in `form` at full look-ahead.
fn run(programs: &[Vec<u8>], form: AccessOrdering) -> SharedGuestMemory {
    let memory = SharedGuestMemory::new(&crate::runner::engine(), MEMORY_BYTES);
    let meeting = Arc::new(Meeting::new(programs.len() as u64));
    std::thread::scope(|scope| {
        for (k, code) in programs.iter().enumerate() {
            let (memory, meeting) = (&memory, meeting.clone());
            scope.spawn(move || {
                let at = code_address(k);
                let mut machine = Machine::<X86_64>::new_sharing(
                    memory,
                    MEMORY_BYTES,
                    at,
                    PAGE_SIZE as u32,
                    Syscalls::StandIn,
                );
                machine.order_accesses(form);
                machine
                    .set_permissions(at..at + PAGE_SIZE, Permissions::READ | Permissions::EXECUTE);
                machine.set_permissions(
                    DATA..DATA + DATA_BYTES,
                    Permissions::READ | Permissions::WRITE,
                );
                machine.load(
                    code,
                    0..code.len() as u64,
                    (at, None),
                    &[0],
                    LookAhead::Full,
                );
                machine.set_registers(&X86_64::initial_block(at, DATA + DATA_BYTES));
                machine.meet_at(MEET, meeting);
                let (stopped, _) = machine.run();
                assert_eq!(stopped, Stopped::Finished(0), "thread {k} in {form:?}");
            });
        }
    });
    memory
}

/// Runs a shape for `rounds` rounds in `form`; returns the count of each outcome by its number.
pub fn run_shape(litmus: &Litmus, form: AccessOrdering, rounds: u64) -> Vec<u64> {
    let programs: Vec<Vec<u8>> = (0..litmus.threads())
        .map(|k| program(litmus, k, rounds).expect("a litmus program"))
        .collect();
    let memory = run(&programs, form);
    let outcomes = litmus.outcomes();
    assert!(outcomes <= 1 << OUTCOME_BITS);
    let mut bytes = vec![0; outcomes * WORD as usize];
    memory
        .read(DATA + COUNTS as u64, &mut bytes)
        .expect("the counts");
    let counts: Vec<u64> = bytes
        .chunks(WORD as usize)
        .map(|w| u64::from_le_bytes(w.try_into().unwrap()))
        .collect();
    assert_eq!(
        counts.iter().sum::<u64>(),
        rounds,
        "{} in {form:?}",
        litmus.name
    );
    counts
}

/// Runs the lock xadd counter on `threads` threads, each adding 1 `rounds` times in `form`; returns the count.
pub fn run_counter(threads: usize, form: AccessOrdering, rounds: u64) -> u64 {
    let programs: Vec<Vec<u8>> = (0..threads)
        .map(|k| counter_program(k, rounds).expect("a counter program"))
        .collect();
    let memory = run(&programs, form);
    let mut bytes = [0; WORD as usize];
    memory
        .read(DATA + COUNTER as u64, &mut bytes)
        .expect("the counter");
    u64::from_le_bytes(bytes)
}

/// The shared words, as places from [`DATA`]: the simulated engine schedules every access to them.
const SHARED: &[i32] = &[X, Y, SEQUENCE, DATA_0, DATA_1, FLAG, COUNTER];

/// Returns the thread whose results hold the place, which is not a shared word.
const fn result_thread(at: i32) -> usize {
    ((at - RESULTS) / LINE) as usize
}

/// Assembles thread `k`'s part alone at its address: rbp set to [`DATA`], the part, then `exit(0)`.
fn part_program(litmus: &Litmus, k: usize) -> Result<Vec<u8>, IcedError> {
    let mut a = CodeAssembler::new(u64::BITS)?;
    a.mov(rbp, DATA)?;
    (litmus.parts[k])(&mut a)?;
    exit(&mut a)?;
    a.assemble(code_address(k))
}

/// One thread's run of its part on the simulated engine.
#[derive(Clone, Debug)]
pub struct Trace {
    /// What it did to the shared words, with the kind of each access, in its program order.
    pub events: Vec<(Kind, Event)>,
    /// The observed places among its results, with what it left there.
    results: Vec<(i32, u64)>,
}

impl Trace {
    fn result(&self, at: i32) -> u64 {
        self.results
            .iter()
            .find(|&&(place, _)| place == at)
            .map(|&(_, value)| value)
            .expect("an observed result of this thread")
    }
}

/// Runs thread `k`'s part once on the simulated engine in `form` and returns its trace and its reads of shared words.
///
/// Those reads see `reads` in order, and 0 past them.
fn run_simulated(litmus: &Litmus, k: usize, form: AccessOrdering, reads: &[u64]) -> (Trace, usize) {
    let code = part_program(litmus, k).expect("a litmus part");
    let shared = SHARED.iter().map(|&at| DATA + at as u64).collect();
    let script = Arc::new(Mutex::new(Script::new(shared, reads.to_vec())));
    let at = code_address(k);
    let mut machine = Machine::<X86_64>::new(MEMORY_BYTES, at, PAGE_SIZE as u32, Syscalls::StandIn);
    machine.order_accesses(form);
    machine.simulate_accesses(script.clone());
    machine.set_permissions(at..at + PAGE_SIZE, Permissions::READ | Permissions::EXECUTE);
    machine.set_permissions(
        DATA..DATA + DATA_BYTES,
        Permissions::READ | Permissions::WRITE,
    );
    machine.load(
        &code,
        0..code.len() as u64,
        (at, None),
        &[0],
        LookAhead::Full,
    );
    machine.set_registers(&X86_64::initial_block(at, DATA + DATA_BYTES));
    let (stopped, _) = machine.run();
    assert_eq!(
        stopped,
        Stopped::Finished(0),
        "thread {k} of {} in {form:?}",
        litmus.name
    );
    let results = litmus
        .observed
        .iter()
        .filter(|&&(_, place, _)| !SHARED.contains(&place) && result_thread(place) == k)
        .map(|&(_, place, _)| {
            let mut bytes = [0; WORD as usize];
            machine.read(DATA + place as u64, &mut bytes);
            (place, u64::from_le_bytes(bytes))
        })
        .collect();
    let script = script.lock().unwrap();
    let trace = Trace {
        events: script.events.clone(),
        results,
    };
    (trace, script.reads_made())
}

/// Returns every run of thread `k`'s part whose reads of shared words each see a value of `domain`.
///
/// There is one run for each choice, found depth first.
fn thread_traces(
    litmus: &Litmus,
    k: usize,
    form: AccessOrdering,
    domain: &BTreeSet<u64>,
) -> Vec<Trace> {
    let mut found = Vec::new();
    let mut pending = vec![Vec::new()];
    while let Some(reads) = pending.pop() {
        let (trace, made) = run_simulated(litmus, k, form, &reads);
        assert!(
            made >= reads.len(),
            "a run reads no fewer shared words than it was given values for"
        );
        if made == reads.len() {
            found.push(trace);
            continue;
        }
        for &value in domain {
            let mut longer = reads.clone();
            longer.push(value);
            pending.push(longer);
        }
    }
    found
}

/// The value each shared word holds at the end of an execution, by address; a word nobody wrote is absent (0).
type Last = BTreeMap<u64, u64>;

/// Returns what every interleaving of the threads' traces leaves: the executions of sequentially consistent accesses.
///
/// Each interleaving keeps each thread's order, and each read sees the latest write before it.
fn interleaved(traces: &[&Trace]) -> BTreeSet<Last> {
    /// Carries on from each thread's next event, collecting what each complete interleaving leaves.
    fn carry_on(
        traces: &[&Trace],
        next: &mut Vec<usize>,
        memory: &mut Last,
        found: &mut BTreeSet<Last>,
    ) {
        if traces
            .iter()
            .zip(next.iter())
            .all(|(trace, &n)| n == trace.events.len())
        {
            found.insert(memory.clone());
            return;
        }
        for k in 0..traces.len() {
            let Some(&(_, event)) = traces[k].events.get(next[k]) else {
                continue;
            };
            let held = |word: u64, memory: &Last| memory.get(&word).copied().unwrap_or(0);
            let before = memory.clone();
            match event {
                Event::Read { word, value } if held(word, memory) != value => continue,
                Event::Read { .. } | Event::Fence => {}
                Event::Write { word, value } => {
                    memory.insert(word, value);
                }
                Event::Exchange {
                    word, found: seen, ..
                } if held(word, memory) != seen => continue,
                Event::Exchange { word, written, .. } => {
                    if let Some(value) = written {
                        memory.insert(word, value);
                    }
                }
            }
            next[k] += 1;
            carry_on(traces, next, memory, found);
            next[k] -= 1;
            *memory = before;
        }
    }
    let mut found = BTreeSet::new();
    carry_on(
        traces,
        &mut vec![0; traces.len()],
        &mut Last::new(),
        &mut found,
    );
    found
}

/// Returns every permutation of the items.
fn permutations(items: &[usize]) -> Vec<Vec<usize>> {
    if items.is_empty() {
        return vec![Vec::new()];
    }
    let mut all = Vec::new();
    for (i, &first) in items.iter().enumerate() {
        let mut rest = items.to_vec();
        rest.remove(i);
        for mut tail in permutations(&rest) {
            tail.insert(0, first);
            all.push(tail);
        }
    }
    all
}

/// Tells whether the edges between `nodes` nodes make no cycle.
fn acyclic(nodes: usize, edges: &[(usize, usize)]) -> bool {
    let mut incoming = vec![0; nodes];
    for &(_, to) in edges {
        incoming[to] += 1;
    }
    let mut ready: Vec<usize> = (0..nodes).filter(|&n| incoming[n] == 0).collect();
    let mut removed = 0;
    while let Some(node) = ready.pop() {
        removed += 1;
        for &(from, to) in edges {
            if from == node {
                incoming[to] -= 1;
                if incoming[to] == 0 {
                    ready.push(to);
                }
            }
        }
    }
    removed == nodes
}

/// Returns what every execution of the threads' traces leaves in which accesses keep only coherence.
///
/// An execution is one order of each word's writes and a source for each read, with no cycle among one word's
/// accesses in thread order, its write order, what each read saw and what it did not see.
///
/// Different words are not ordered at all, so every reordering ARM64 allows plain accesses is in it, and more.
fn coherent(traces: &[&Trace]) -> BTreeSet<Last> {
    // Each event on a word: its thread, its place in the thread's order, and the event.
    let mut nodes = Vec::new();
    for (k, trace) in traces.iter().enumerate() {
        for (index, &(_, event)) in trace.events.iter().enumerate() {
            match event {
                Event::Read { .. } | Event::Write { .. } => nodes.push((k, index, event)),
                Event::Fence | Event::Exchange { .. } => {
                    panic!("{event:?} in plain code, which the simulation does not order")
                }
            }
        }
    }
    let word_of = |event: Event| match event {
        Event::Read { word, .. } | Event::Write { word, .. } => word,
        Event::Fence | Event::Exchange { .. } => unreachable!("left out above"),
    };
    let words: BTreeSet<u64> = nodes.iter().map(|&(_, _, event)| word_of(event)).collect();
    let writes_of = |word: u64| -> Vec<usize> {
        (0..nodes.len())
            .filter(|&n| matches!(nodes[n].2, Event::Write { word: w, .. } if w == word))
            .collect()
    };
    let reads: Vec<usize> = (0..nodes.len())
        .filter(|&n| matches!(nodes[n].2, Event::Read { .. }))
        .collect();
    // Each read's sources: a write of its word and value, or none for the initial 0.
    let sources: Vec<Vec<Option<usize>>> = reads
        .iter()
        .map(|&r| {
            let Event::Read { word, value } = nodes[r].2 else {
                unreachable!("a read")
            };
            let mut from: Vec<Option<usize>> = writes_of(word)
                .into_iter()
                .filter(|&w| matches!(nodes[w].2, Event::Write { value: v, .. } if v == value))
                .map(Some)
                .collect();
            if value == 0 {
                from.push(None);
            }
            from
        })
        .collect();
    if sources.iter().any(Vec::is_empty) {
        // A read saw a value nobody wrote.
        return BTreeSet::new();
    }
    // The edges of each thread's order between events on one word.
    let mut program_order = Vec::new();
    for a in 0..nodes.len() {
        for b in 0..nodes.len() {
            let ((ka, ia, ea), (kb, ib, eb)) = (nodes[a], nodes[b]);
            if ka == kb && ia < ib && word_of(ea) == word_of(eb) {
                program_order.push((a, b));
            }
        }
    }
    let mut found = BTreeSet::new();
    let orders: Vec<Vec<Vec<usize>>> = words.iter().map(|&w| permutations(&writes_of(w))).collect();
    let mut choice = vec![0; orders.len()];
    loop {
        let write_orders: Vec<&Vec<usize>> =
            orders.iter().zip(&choice).map(|(o, &c)| &o[c]).collect();
        let mut source_choice = vec![0; reads.len()];
        loop {
            let mut edges = program_order.clone();
            for order in &write_orders {
                edges.extend(order.windows(2).map(|pair| (pair[0], pair[1])));
            }
            for (i, &r) in reads.iter().enumerate() {
                let source = sources[i][source_choice[i]];
                let word = word_of(nodes[r].2);
                let order = write_orders[words.iter().position(|&w| w == word).unwrap()];
                if let Some(w) = source {
                    edges.push((w, r));
                }
                // The read did not see the writes after its source.
                let later = match source {
                    Some(w) => order.iter().position(|&o| o == w).unwrap() + 1,
                    None => 0,
                };
                edges.extend(order[later..].iter().map(|&w| (r, w)));
            }
            if acyclic(nodes.len(), &edges) {
                let last = words
                    .iter()
                    .zip(&write_orders)
                    .filter_map(|(&word, order)| {
                        let &w = order.last()?;
                        let Event::Write { value, .. } = nodes[w].2 else {
                            unreachable!("a write")
                        };
                        Some((word, value))
                    })
                    .collect();
                found.insert(last);
            }
            if !advance(&mut source_choice, |i| sources[i].len()) {
                break;
            }
        }
        if !advance(&mut choice, |i| orders[i].len()) {
            break;
        }
    }
    found
}

/// Steps a mixed-radix counter whose digit `i` runs below `size(i)`; false once it has gone round.
fn advance(digits: &mut [usize], size: impl Fn(usize) -> usize) -> bool {
    for (i, digit) in digits.iter_mut().enumerate() {
        *digit += 1;
        if *digit < size(i) {
            return true;
        }
        *digit = 0;
    }
    false
}

/// What the simulated engine gave a shape in one form.
#[derive(Clone, Debug)]
pub struct Simulated {
    /// The values the reads of shared words could see.
    pub domain: BTreeSet<u64>,
    /// How many runs of each thread's part there were, one for each choice of what its reads see.
    pub traces: Vec<usize>,
    /// How many runs' combinations, one of each thread, the model let happen, and in how many ways at the end.
    pub executions: usize,
    /// Every outcome some execution gives.
    pub outcomes: BTreeSet<usize>,
}

/// Runs a shape on the simulated engine exhaustively, checking every combination of runs against the form's model.
///
/// Each part runs once for each choice of what its reads of shared words see, among 0 and every value written.
///
/// The ordered form's accesses must all be atomic, its executions the interleavings; the default form's must all be
/// plain, its executions those that keep coherence alone ([`coherent`]).
pub fn simulate_shape(litmus: &Litmus, form: AccessOrdering) -> Simulated {
    let kind = match form {
        AccessOrdering::Ordered(_) => Kind::Atomic,
        AccessOrdering::Unordered => Kind::Plain,
        AccessOrdering::LoadsOrdered(_) => {
            panic!("the simulation models the ordered and default forms")
        }
    };
    let mut domain = BTreeSet::from([0]);
    let traces: Vec<Vec<Trace>> = loop {
        let traces: Vec<Vec<Trace>> = (0..litmus.threads())
            .map(|k| thread_traces(litmus, k, form, &domain))
            .collect();
        let written: BTreeSet<u64> = traces
            .iter()
            .flatten()
            .flat_map(|trace| trace.events.iter())
            .filter_map(|&(_, event)| match event {
                Event::Write { value, .. } => Some(value),
                Event::Exchange { written, .. } => written,
                Event::Read { .. } | Event::Fence => None,
            })
            .collect();
        if written.is_subset(&domain) {
            break traces;
        }
        domain.extend(written);
    };
    for trace in traces.iter().flatten() {
        for &(seen, event) in &trace.events {
            assert_eq!(seen, kind, "{event:?} of {} in {form:?}", litmus.name);
        }
    }
    let mut outcomes = BTreeSet::new();
    let mut executions = 0;
    let mut choice = vec![0; traces.len()];
    loop {
        let combination: Vec<&Trace> = traces.iter().zip(&choice).map(|(t, &c)| &t[c]).collect();
        let ends = match kind {
            Kind::Atomic => interleaved(&combination),
            Kind::Plain => coherent(&combination),
        };
        executions += ends.len();
        for last in ends {
            let mut outcome = 0;
            for &(_, place, bits) in litmus.observed {
                let value = if SHARED.contains(&place) {
                    last.get(&(DATA + place as u64)).copied().unwrap_or(0)
                } else {
                    combination[result_thread(place)].result(place)
                };
                outcome = outcome << bits | (value & ((1 << bits) - 1));
            }
            outcomes.insert(outcome as usize);
        }
        if !advance(&mut choice, |k| traces[k].len()) {
            break;
        }
    }
    Simulated {
        domain,
        traces: traces.iter().map(Vec::len).collect(),
        executions,
        outcomes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks that the models tell store buffering and load buffering apart, and keep a thread's own order.
    #[test]
    fn the_models_tell_reorderings_apart() {
        let (x, y) = (1, 2);
        let trace = |events: Vec<Event>| Trace {
            events: events.into_iter().map(|e| (Kind::Plain, e)).collect(),
            results: Vec::new(),
        };
        // Store buffering with both reads 0: coherent, not interleaved.
        let first = trace(vec![
            Event::Write { word: x, value: 1 },
            Event::Read { word: y, value: 0 },
        ]);
        let second = trace(vec![
            Event::Write { word: y, value: 1 },
            Event::Read { word: x, value: 0 },
        ]);
        assert!(interleaved(&[&first, &second]).is_empty());
        assert_eq!(coherent(&[&first, &second]).len(), 1);
        // Load buffering with both reads 1: coherent, not interleaved.
        let first = trace(vec![
            Event::Read { word: x, value: 1 },
            Event::Write { word: y, value: 1 },
        ]);
        let second = trace(vec![
            Event::Read { word: y, value: 1 },
            Event::Write { word: x, value: 1 },
        ]);
        assert!(interleaved(&[&first, &second]).is_empty());
        assert_eq!(coherent(&[&first, &second]).len(), 1);
        // A thread's own write, then a read of the initial value of the same word: in neither.
        let alone = trace(vec![
            Event::Write { word: x, value: 1 },
            Event::Read { word: x, value: 0 },
        ]);
        assert!(interleaved(&[&alone]).is_empty());
        assert!(coherent(&[&alone]).is_empty());
    }
}
