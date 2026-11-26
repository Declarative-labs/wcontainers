//! x86-64 litmus programs run translated by two or four host threads on one shared guest memory (XF3): each named
//! shape, its outcomes counted, and the outcomes x86 forbids picked out. Not a fuzzing suite: the shapes and their
//! rounds are fixed.
//!
//! Each round the threads meet at a system call the stand-in answers by waiting for all of them, each waits a
//! number of empty turns that changes from round to round so that the threads' parts overlap in many ways, and runs
//! its part; they meet again, and thread 0 counts the outcome and sets the shared words back to 0.

use crate::guest::GuestTest;
use crate::guest_memory::SharedGuestMemory;
use crate::runner::{Machine, Meeting, Stopped, Syscalls};
use iced_x86::IcedError;
use iced_x86::code_asm::*;
use std::sync::Arc;
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
    /// What thread 0 reads after a round, each value's place from [`DATA`] and the bits it takes, highest first in
    /// the outcome's number.
    pub observed: &'static [(&'static str, i32, u32)],
    /// The shared words set back to 0 after each round.
    pub reset: &'static [i32],
    /// Whether x86 forbids an outcome, given the observed values in order.
    pub forbidden: fn(&[u64]) -> bool,
}

impl Litmus {
    pub fn threads(&self) -> usize {
        self.parts.len()
    }

    /// The observed values of an outcome's number.
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

/// Where thread `k`'s `n`th result goes.
const fn result(k: i32, n: i32) -> i32 {
    RESULTS + k * LINE + n * WORD
}

fn store(a: &mut CodeAssembler, at: i32) -> Asm {
    a.mov(qword_ptr(rbp + at), 1)
}

/// Store buffering: each thread stores to its word and loads the other's. x86 allows both loads to see 0.
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
};

/// Whether both loads saw 0, which no interleaving gives: the outcome store buffering shows.
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
};

/// Message passing: data then flag; the reader loads the flag then the data. x86 forbids flag 1 with data 0.
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
};

/// Load buffering: each thread loads one word, then stores to the other. x86 forbids both loads seeing 1.
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
};

/// The sequence lock: the writer makes the count odd, writes two words, makes it even; the reader loads the count,
/// the two words and the count again. x86 forbids an even, unchanged count with words other than the count's.
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
};

/// The store-back: one thread loads x, waits, loads the flag and, if it is set, stores back what it loaded; the
/// other stores x, `mfence`, then the flag. x86 forbids the loads seeing 0 and 1 with x ending 1.
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
};

/// Independent reads of independent writes: two threads each store a word, two others load both in opposite
/// orders. x86 forbids the readers seeing the two stores in different orders.
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

/// Thread `k`'s program for `rounds` rounds of a shape, assembled at its address.
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

/// The lock xadd counter's program for each thread: they meet, add 1 `rounds` times with `lock xadd`, and end.
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

/// Runs each thread's program on its own host thread and machine, all on one fresh shared memory, translated in
/// `form` at full look-ahead; returns the memory once every thread has ended.
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
    let outcomes = 1 << litmus.observed.iter().map(|o| o.2).sum::<u32>();
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
