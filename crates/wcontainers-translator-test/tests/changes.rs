//! Page permissions changed at random stops and system calls: full look-ahead ends exactly as look-ahead none
//! given the same changes at the same points (same fault, address and kind, same registers, same memory).
//! Changes often hit the page a register points at and the pages around it, so they land on what a loop's
//! proof or a kept run covers and on the page after the one being accessed.
//!
//! Knobs: `WC_CHANGES_CASES` (random programs, default 64), `WC_CHANGES_SEED` (default 1),
//! `WC_CHANGES_PROGRAM_SEEDS` (runs of each C program, default 3).

// dynasm's expansion converts register numbers to their own type.
#![allow(clippy::useless_conversion)]

use dynasmrt::aarch64::Aarch64Relocation;
use dynasmrt::{DynamicLabel, DynasmApi, DynasmLabelApi, VecAssembler, dynasm};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use wcontainers_abi::{Exit, PAGE_SIZE, Permissions};
use wcontainers_arm64::Arm64;
use wcontainers_translator::{LookAhead, Omit};
use wcontainers_translator_test::changes::{Compared, Plan, compare};
use wcontainers_translator_test::layer2::{assemble, program};
use wcontainers_translator_test::programs::{Blob, LOAD, Library, STACK_BYTES, build, ready};
use wcontainers_translator_test::runner::{Pause, Stopped};

type Ops = VecAssembler<Aarch64Relocation>;

fn knob(name: &str, default: u64) -> u64 {
    std::env::var(name).map_or(default, |v| v.parse().expect("a number"))
}

fn plan() -> impl Strategy<Value = Plan> {
    (
        any::<u64>(),
        prop::sample::select(vec![1u32, 4, 32, 256]),
        prop::sample::select(vec![1.0, 1.0, 1.0, 0.5, 0.1, 0.0]),
    )
        .prop_map(|(seed, every, stops)| Plan { seed, every, stops })
}

/// Counts of what the runs did, printed so a run says how strong it was.
#[derive(Default)]
struct Tally {
    ran: u64,
    changes: u64,
    handled: u64,
    faulted: u64,
    translated: u64,
    left: u64,
}

impl Tally {
    fn add(&mut self, compared: Compared) {
        self.ran += 1;
        self.changes += compared.changes as u64;
        self.handled += compared.handled as u64;
        self.translated += compared.translated;
        self.left += compared.left;
        if matches!(compared.stopped, Stopped::Exit(Exit::MemoryFault { .. })) {
            self.faulted += 1;
        }
    }

    fn report(&self, what: &str) {
        eprintln!(
            "changes: {} {what}; {} changes made, {} faults handled, {} ended by a fault; at full {} pieces \
             translated at run time, {} instructions run alone after a check failed",
            self.ran, self.changes, self.handled, self.faulted, self.translated, self.left
        );
    }
}

#[test]
fn random_programs_with_changes_match_none() {
    let cases = knob("WC_CHANGES_CASES", 64);
    let seed = knob("WC_CHANGES_SEED", 1);
    let mut seed_bytes = [0u8; 32];
    seed_bytes[..8].copy_from_slice(&seed.to_le_bytes());
    let mut runner = TestRunner::new_with_rng(
        Config {
            cases: cases as u32,
            failure_persistence: None,
            ..Config::default()
        },
        TestRng::from_seed(RngAlgorithm::ChaCha, &seed_bytes),
    );
    let tally = std::cell::RefCell::new(Tally::default());
    let result = runner.run(&(program(), plan()), |(program, plan)| {
        let blob = assemble(&program);
        tally.borrow_mut().add(compare::<Arm64>(&blob, plan));
        Ok(())
    });
    tally.borrow().report(&format!(
        "random programs (shrinking included), seed {seed}"
    ));
    if let Err(error) = result {
        panic!("{error}");
    }
}

#[test]
fn programs_with_changes_match_none() {
    let seeds = knob("WC_CHANGES_PROGRAM_SEEDS", 3);
    let mut tally = Tally::default();
    for (name, optimisation) in [
        ("walks", "-O0"),
        ("walks", "-O2"),
        ("integers", "-O2"),
        ("tables", "-O2"),
        ("bench", "-O0"),
        ("bench", "-O2"),
    ] {
        let blob = build::<Arm64>(name, optimisation, Library::None);
        for seed in 0..seeds {
            for every in [16, 1024] {
                let plan = Plan {
                    seed,
                    every,
                    stops: 1.0,
                };
                eprintln!("{name} {optimisation}: {plan:?}");
                tally.add(compare::<Arm64>(&blob, plan));
            }
        }
    }
    tally.report("program runs");
}

/// The page of data after the code page of the programs below.
const DATA: u64 = 0x40_1000;

/// Start with the registers the reference does not clear at zero and x5 at `DATA`.
fn start(ops: &mut Ops) {
    for r in 0..22u8 {
        dynasm!(ops ; .arch aarch64 ; movz X(r), #0);
    }
    dynasm!(ops ; .arch aarch64 ; movz x5, #0x40, lsl #16 ; movk x5, #0x1000);
}

/// A system call that writes nothing.
fn write_nothing(ops: &mut Ops) {
    dynasm!(ops ; .arch aarch64 ; movz x0, #1 ; mov x1, x5 ; movz x2, #0 ; movz x8, #64 ; svc #0);
}

/// Exit with 0 after the program's code, and two pages of data after the code page.
fn data_blob(name: &str, mut ops: Ops) -> Blob {
    dynasm!(ops ; .arch aarch64 ; movz x0, #0 ; movz x8, #94 ; svc #0);
    let mut bytes = ops.finalize().expect("labels");
    bytes.resize(3 * PAGE_SIZE as usize, 0);
    Blob {
        name: name.into(),
        code_bytes: PAGE_SIZE,
        bytes,
        load: LOAD,
        program_headers: None,
        stack_bytes: STACK_BYTES,
    }
}

/// At the first system call, or at the first stop at `stop_at` (an address and x16 there), the page at `page`
/// is made to allow nothing; full must fault exactly as none does. With `stop_at`, none stops at every
/// instruction and full at every stop check up to that point.
fn denied_at_the_first(blob: &Blob, stop_at: Option<(u64, u64)>, page: u64) {
    let run = |look_ahead| {
        let mut machine = ready::<Arm64>(blob, look_ahead, Omit::default());
        let mut first = true;
        machine.on_pause(move |at, paused| {
            let registers = paused.registers();
            let reached = match stop_at {
                None => at == Pause::Syscall,
                Some((pc, x16)) => {
                    let reached = at == Pause::Stop && registers.pc == pc && registers.x[16] == x16;
                    // Full stops at every check until the point, then runs on without stopping.
                    if first && !reached {
                        paused.raise_stop_flag();
                    }
                    reached
                }
            };
            if reached && std::mem::take(&mut first) {
                paused.set_permissions(page..page + PAGE_SIZE, Permissions::empty());
            }
        });
        if stop_at.is_some() {
            if look_ahead == LookAhead::None {
                machine.stop_at_every_check();
            } else {
                machine.raise_stop_flag();
            }
        }
        machine.run()
    };
    let none = run(LookAhead::None);
    assert!(
        matches!(none.0, Stopped::Exit(Exit::MemoryFault { address, .. }) if address / PAGE_SIZE == page / PAGE_SIZE),
        "{}: none {:?}",
        blob.name,
        none.0
    );
    let full = run(LookAhead::Full);
    assert_eq!(full.0, none.0, "{}: why it stopped", blob.name);
    assert_eq!(full.1, none.1, "{}: registers", blob.name);
}

fn denied_at_the_first_syscall(blob: &Blob) {
    denied_at_the_first(blob, None, DATA);
}

/// Two loads through one base with a system call between them: the second is checked again.
#[test]
fn group_across_a_syscall() {
    let mut ops: Ops = VecAssembler::new(0);
    start(&mut ops);
    dynasm!(ops ; .arch aarch64 ; ldr x6, [x5]);
    write_nothing(&mut ops);
    dynasm!(ops ; .arch aarch64 ; ldr x7, [x5, #8]);
    denied_at_the_first_syscall(&data_blob("group-across-syscall", ops));
}

/// A loop that loads through a base and then makes a system call: the next turn's load is checked again.
#[test]
fn loop_holding_a_syscall() {
    let mut ops: Ops = VecAssembler::new(0);
    start(&mut ops);
    let top = ops.new_dynamic_label();
    dynasm!(ops ; .arch aarch64
        ; movz x16, #3
        ; =>top
        ; ldr x6, [x5, x16, lsl #3]
        ; add x9, x9, x6
    );
    write_nothing(&mut ops);
    dynasm!(ops ; .arch aarch64 ; subs x16, x16, #1 ; b.ne =>top);
    denied_at_the_first_syscall(&data_blob("loop-holding-syscall", ops));
}

/// A function that makes a system call like `write_nothing` and returns, at `f`.
fn writing_nothing(ops: &mut Ops, f: DynamicLabel) {
    dynasm!(ops ; .arch aarch64 ; =>f);
    write_nothing(ops);
    dynasm!(ops ; .arch aarch64 ; ret);
}

/// Two loads through one base with a call between them whose callee makes a system call.
#[test]
fn group_across_a_call() {
    let mut ops: Ops = VecAssembler::new(0);
    start(&mut ops);
    let f = ops.new_dynamic_label();
    let end = ops.new_dynamic_label();
    dynasm!(ops ; .arch aarch64 ; ldr x6, [x5] ; bl =>f ; ldr x7, [x5, #8] ; b =>end);
    writing_nothing(&mut ops, f);
    dynasm!(ops ; .arch aarch64 ; =>end);
    denied_at_the_first_syscall(&data_blob("group-across-call", ops));
}

/// A loop over memory holding a call whose callee makes a system call.
#[test]
fn loop_holding_a_call() {
    let mut ops: Ops = VecAssembler::new(0);
    start(&mut ops);
    let f = ops.new_dynamic_label();
    let top = ops.new_dynamic_label();
    let end = ops.new_dynamic_label();
    dynasm!(ops ; .arch aarch64
        ; movz x16, #3
        ; =>top
        ; ldr x6, [x5, x16, lsl #3]
        ; add x9, x9, x6
        ; bl =>f
        ; subs x16, x16, #1
        ; b.ne =>top
        ; b =>end
    );
    writing_nothing(&mut ops, f);
    dynasm!(ops ; .arch aarch64 ; =>end);
    denied_at_the_first_syscall(&data_blob("loop-holding-call", ops));
}

/// The page below the one sp is moved to.
const BELOW: u64 = 0x480e_f000;

/// Move sp to the top of `BELOW`, then make and pop a frame there (x29 keeps the old sp).
fn pop_a_frame_at_below(ops: &mut Ops) {
    dynasm!(ops ; .arch aarch64
        ; movz x6, #0x480f, lsl #16
        ; mov x29, sp
        ; mov sp, x6
        ; sub sp, sp, #64
        ; str x6, [sp]
        ; add sp, sp, #64
    );
}

/// Below sp once a frame is popped is no frame: a change there at a system call is seen at the next access.
#[test]
fn below_sp_after_a_pop_across_a_syscall() {
    let mut ops: Ops = VecAssembler::new(0);
    start(&mut ops);
    pop_a_frame_at_below(&mut ops);
    write_nothing(&mut ops);
    dynasm!(ops ; .arch aarch64 ; ldur x9, [sp, #-8] ; mov sp, x29);
    denied_at_the_first(&data_blob("below-sp-across-syscall", ops), None, BELOW);
}

/// The same with the access in a loop and the change at its first stop.
#[test]
fn below_sp_after_a_pop_in_a_loop() {
    let mut ops: Ops = VecAssembler::new(0);
    start(&mut ops);
    pop_a_frame_at_below(&mut ops);
    let top = ops.new_dynamic_label();
    dynasm!(ops ; .arch aarch64 ; movz x16, #4);
    let head = LOAD + ops.offset().0 as u64;
    dynasm!(ops ; .arch aarch64
        ; =>top
        ; ldur x9, [sp, #-8]
        ; add x10, x10, x9
        ; subs x16, x16, #1
        ; b.ne =>top
        ; mov sp, x29
    );
    // At the head, after the first turn.
    denied_at_the_first(
        &data_blob("below-sp-in-a-loop", ops),
        Some((head, 3)),
        BELOW,
    );
}

/// A counted loop of 64 byte loads walking to the end of `DATA`, the last one 32 bytes onto the page after,
/// which is made to allow nothing at the first stop: the loop's proof fails and its checked copy keeps the run
/// of pages it finds allowed, which must not let that last load through.
#[test]
fn longest_access_onto_a_page_changed_at_a_stop() {
    // ld1 {v0.16b-v3.16b}, [x7]
    let ld1x4 = 0x4c40_20e0u32;
    for (name, word) in [("ld1x4", ld1x4), ("st1x4", ld1x4 & !(1 << 22))] {
        let mut ops: Ops = VecAssembler::new(0);
        start(&mut ops);
        dynasm!(ops ; .arch aarch64 ; add x7, x5, #0xf80 ; movz x16, #4);
        let head = LOAD + ops.offset().0 as u64;
        let top = ops.new_dynamic_label();
        dynasm!(ops ; .arch aarch64
            ; =>top
            ; .u32 word
            ; add x7, x7, #32
            ; subs x16, x16, #1
            ; b.ne =>top
        );
        denied_at_the_first(
            &data_blob(&format!("longest-access-{name}"), ops),
            Some((head, 3)),
            DATA + PAGE_SIZE,
        );
    }
}
