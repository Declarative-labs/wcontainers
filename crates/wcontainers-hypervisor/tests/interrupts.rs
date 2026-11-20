//! N2's forced interrupts and the count across look-aheads: guest functions run as the native output's pieces,
//! one piece after another, inside Hypervisor.framework, at every look-ahead setting. At look-ahead none every
//! instruction boundary is a stop, which gives the run's boundaries; each is checked against definition E's
//! count, and the end against the WebAssembly output under wasmtime at every look-ahead. Then, at every
//! look-ahead, the function runs whole and an interrupt is forced at every host instruction it is about to run
//! (found by software step and at every return from the inside): the guest state the position map resolves
//! there (registers, pc, count and the rest) must be one of the boundaries, in order, every boundary at an
//! instruction the run carries in host bytes must be reached by some interrupt, and the run going on from there
//! must end as the unbroken one. So the count at
//! every stop is the same at every look-ahead. Each case runs from seeded starting states (flags, thread
//! pointer, vectors, data area). It runs itself as a copy signed with the hypervisor entitlement.

#[cfg(target_os = "macos")]
mod common;

#[cfg(target_os = "macos")]
#[path = "../../wcontainers-inside/src/bin/native-cases/protocol.rs"]
mod protocol;

#[cfg(target_os = "macos")]
mod runner;

#[cfg(target_os = "macos")]
fn main() {
    macos::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("the interrupt test needs Hypervisor.framework; nothing to run on this host");
}

#[cfg(target_os = "macos")]
mod macos {
    use std::time::Instant;

    use wcontainers_abi::{PAGE_SIZE, RegisterBlock};
    use wcontainers_arm64::carry::{COUNT, PAGE, SCRATCH};
    use wcontainers_arm64::encode::{self, Register, field};
    use wcontainers_arm64::{Arm64, FPCR_BITS, FPSR_BITS, decode};
    use wcontainers_guest_cpu::{Guest, Native, Route, counted};
    use wcontainers_translator::LookAhead;
    use wcontainers_translator_test::layer1::Case;
    use wcontainers_translator_test::layout::{CODE, DATA, DATA_WRITABLE};

    use crate::common::{be_signed, load_average, setting};
    use crate::runner::function::{Run, same_end, webassembly};
    use crate::runner::{GuestState, Runner, nzcv};

    /// Where the functions return to: an address with no code.
    const RETURNS: u64 = CODE + PAGE_SIZE;
    /// The stack the functions start with, in the writable data.
    const STACK: u64 = DATA + DATA_WRITABLE / 2;
    /// The general registers: x0 to x30, then sp, flags and the thread pointer in a case.
    const GENERAL: usize = RegisterBlock::ZERO.x.len();
    const LINK: usize = GENERAL - 1;
    const SEED_VARIABLE: &str = "WCONTAINERS_INTERRUPTS_SEED";
    const STARTS_VARIABLE: &str = "WCONTAINERS_INTERRUPTS_STARTS";
    /// The seed unless told otherwise.
    const DEFAULT_SEED: u64 = 0x1405_7b7e_f767_814f;
    /// Seeded starting states per case unless told otherwise.
    const DEFAULT_STARTS: u64 = 1;
    /// A bounded look-ahead shorter than most of the functions' straight-line runs, so that runs are cut.
    const SHORT: u32 = 3;
    /// Every look-ahead the functions run at; look-ahead none first, whose stops are every boundary.
    const LOOK_AHEADS: [LookAhead; 3] =
        [LookAhead::None, LookAhead::Bounded(SHORT), LookAhead::Full];

    /// Sums `x1` words from `x0`, keeping the sum in x27 and the pointer in x28 (both withheld: stand-ins).
    const SUM: [u32; 8] = [
        0xd280001b, // mov x27, #0
        0xaa0003fc, // mov x28, x0
        0xf8408782, // ldr x2, [x28], #8
        0x8b02037b, // add x27, x27, x2
        0xf1000421, // subs x1, x1, #1
        0x54ffffa1, // b.ne back to the load
        0xaa1b03e0, // mov x0, x27
        0xd65f03c0, // ret
    ];

    /// A frame, a direct call, a call through a register and a leaf with a conditional branch.
    const CALLS: [u32; 12] = [
        0xa9bf7bfd, // stp x29, x30, [sp, #-16]!
        0x910003fd, // mov x29, sp
        0xd28000a0, // mov x0, #5
        0x94000005, // bl leaf
        0x10000083, // adr x3, leaf
        0xd63f0060, // blr x3
        0xa8c17bfd, // ldp x29, x30, [sp], #16
        0xd65f03c0, // ret
        0x91000c00, // leaf: add x0, x0, #3
        0xb4000040, // cbz x0, the second ret
        0xd65f03c0, // ret
        0xd65f03c0, // ret
    ];

    /// A call through a register in a loop, so that it and the leaf's return go to their last targets.
    const LOOPED_CALLS: [u32; 12] = [
        0xa9bf7bfd, // stp x29, x30, [sp, #-16]!
        0x910003fd, // mov x29, sp
        0xd28000a0, // mov x0, #5
        0x100000e3, // adr x3, leaf
        0xd2800064, // mov x4, #3
        0xd63f0060, // blr x3
        0xf1000484, // subs x4, x4, #1
        0x54ffffc1, // b.ne back to the blr
        0xa8c17bfd, // ldp x29, x30, [sp], #16
        0xd65f03c0, // ret
        0x91000c00, // leaf: add x0, x0, #3
        0xd65f03c0, // ret
    ];

    /// An exclusive pair in a loop, a compare and swap, loads and stores across a page, a float operation that
    /// raises a flag, and the float status read and written.
    const ATOMICS: [u32; 16] = [
        0xc85f7c02, // ldxr x2, [x0]
        0x91000442, // add x2, x2, #1
        0xc8037c02, // stxr w3, x2, [x0]
        0x35ffffa3, // cbnz w3, back to the ldxr
        0xd2800004, // mov x4, #0
        0xd28000e5, // mov x5, #7
        0xc8a47c05, // cas x4, x5, [x0]
        0xf9400026, // ldr x6, [x1]
        0xf9000026, // str x6, [x1]
        0xa93f8826, // stp x6, x2, [x1, #-8]
        0x1e6e1000, // fmov d0, #1.0
        0x1e611001, // fmov d1, #3.0
        0x1e611802, // fdiv d2, d0, d1
        0xd53b4427, // mrs x7, fpsr
        0xd51b443f, // msr fpsr, xzr
        0xd65f03c0, // ret
    ];

    /// A dispatch loop through a jump table of bytes that one of its operations rewrites: the stream x9 is stored at
    /// x0, the table x10 at x0 + 16; operation 0 adds one, 1 doubles, 2 swaps the table's first two entries, 3
    /// returns, and an entry of 255 goes where there is no code.
    const JUMP_TABLE: [u32; 19] = [
        0xf9000009, // str x9, [x0]
        0x91004006, // add x6, x0, #16
        0xf90000ca, // str x10, [x6]
        0xd2800005, // mov x5, #0
        0x38401403, // ldrb w3, [x0], #1
        0x386368c3, // ldrb w3, [x6, x3]
        0x10000061, // adr x1, the base
        0x8b030821, // add x1, x1, x3, lsl #2
        0xd61f0020, // br x1
        0x910004a5, // the base: add x5, x5, #1
        0x17fffffa, // b back to the first ldrb
        0x8b0500a5, // add x5, x5, x5
        0x17fffff8, // b back to the first ldrb
        0x794000c8, // ldrh w8, [x6]
        0x5ac00508, // rev16 w8, w8
        0x790000c8, // strh w8, [x6]
        0x17fffff4, // b back to the first ldrb
        0xaa0503e0, // mov x0, x5
        0xd65f03c0, // ret
    ];

    /// As [`JUMP_TABLE`] with signed bytes: 0 adds one, -1 doubles then adds one, -8 adds ten, 2 returns, and -128
    /// goes back before the code.
    const SIGNED_JUMP_TABLE: [u32; 17] = [
        0xf9000009, // str x9, [x0]
        0x91004006, // add x6, x0, #16
        0xf90000ca, // str x10, [x6]
        0xd2800005, // mov x5, #0
        0x14000003, // b to the first ldrb
        0x910028a5, // add x5, x5, #10
        0x14000001, // b to the first ldrb
        0x38401403, // ldrb w3, [x0], #1
        0x386368c3, // ldrb w3, [x6, x3]
        0x10000081, // adr x1, the base
        0x8b238821, // add x1, x1, w3, sxtb #2
        0xd61f0020, // br x1
        0x8b0500a5, // add x5, x5, x5
        0x910004a5, // the base: add x5, x5, #1
        0x17fffff9, // b back to the first ldrb
        0xaa0503e0, // mov x0, x5
        0xd65f03c0, // ret
    ];

    /// A byte load that writes back its base, which is then the register added to the jump's base: the target is
    /// a pointer's worth past it, not one of a byte's.
    const WRITTEN_BACK_INDEX: [u32; 6] = [
        0x10000081, // adr x1, the base
        0x38401469, // ldrb w9, [x3], #1
        0x8b030821, // add x1, x1, x3, lsl #2
        0xd61f0020, // br x1
        0xd28006e0, // the base: mov x0, #55
        0xd65f03c0, // ret
    ];

    /// `clrex` between a load and a store exclusive: the store fails.
    const CLEARED: [u32; 4] = [
        0xc85f7c02, // ldxr x2, [x0]
        0xd5033f5f, // clrex
        0xc8037c02, // stxr w3, x2, [x0]
        0xd65f03c0, // ret
    ];

    /// A load exclusive and a store exclusive whose addresses the cases choose: the same writable one, the same
    /// read-only one (the store faults), different ones, and a misaligned store.
    const EXCLUSIVE: [u32; 3] = [
        0xc85f7c02, // ldxr x2, [x0]
        0xc8037c22, // stxr w3, x2, [x1]
        0xd65f03c0, // ret
    ];

    /// Fix-ups on stand-ins: a constant built for a withheld register, a store exclusive whose status, data and
    /// base are all withheld, and a load pair into one register.
    fn withheld() -> &'static [u32] {
        let named = |word: u32, fields: &[(&str, Register)]| {
            fields.iter().fold(word, |w, &(f, r)| {
                encode::replace(w, f, r.into()).expect("a register fits")
            })
        };
        vec![
            named(0x1000007b, &[(field::RD, SCRATCH)]), // adr t, #12
            named(0xaa1b03e1, &[(field::RM, SCRATCH)]), // mov x1, t
            named(0xc85f7f9b, &[(field::RT, SCRATCH), (field::RN, PAGE)]), // ldxr t, [page]
            named(
                0xc81a7f9b,
                &[(field::RS, COUNT), (field::RT, SCRATCH), (field::RN, PAGE)],
            ), // stxr wcount, t, [page]
            named(0xa9400781, &[(field::RN, PAGE)]),    // ldp x1, x1, [page]
            0xd65f03c0,                                 // ret
        ]
        .leak()
    }

    /// System registers copied, answered and held as constants, a cache operation the inside answers, hints
    /// that are whole pieces, and float status writes followed by them.
    const SYSTEM: [u32; 21] = [
        0xd53bd042, // mrs x2, tpidr_el0
        0xd51bd043, // msr tpidr_el0, x3
        0xd503201f, // nop
        0xd53b4204, // mrs x4, nzcv
        0xd51b4206, // msr nzcv, x6
        0xd53b00e5, // mrs x5, dczid_el0
        0xd50b7530, // ic ivau, x16
        0x1e6e1000, // fmov d0, #1.0
        0x1e611001, // fmov d1, #3.0
        0x1e611802, // fdiv d2, d0, d1
        0xd51b442d, // msr fpsr, x13
        0xd503201f, // nop
        0xd503203f, // yield
        0xd53b4427, // mrs x7, fpsr
        0xd503201f, // nop
        0x91000631, // add x17, x17, #1
        0xd503201f, // nop
        0xd51b4408, // msr fpcr, x8
        0xd503201f, // nop
        0x9a9081ee, // csel x14, x15, x16, hi
        0xd65f03c0, // ret
    ];

    /// Every float status form after a float operation that raised flags.
    const FLOAT: [u32; 13] = [
        0x1e6e1000, // fmov d0, #1.0
        0x1e611001, // fmov d1, #3.0
        0x1e611802, // fdiv d2, d0, d1
        0xd53b4427, // mrs x7, fpsr
        0x1e611803, // fdiv d3, d0, d1
        0xd51b4428, // msr fpsr, x8
        0x1e611804, // fdiv d4, d0, d1
        0xd53b4409, // mrs x9, fpcr
        0x1e611805, // fdiv d5, d0, d1
        0xd51b440a, // msr fpcr, x10
        0x1e611806, // fdiv d6, d0, d1
        0xd53b442b, // mrs x11, fpsr
        0xd65f03c0, // ret
    ];

    /// A case's starting state with the registers given, the flags, thread pointer, vectors and data area drawn
    /// from `seed`.
    fn start(seed: u64, registers: &[(usize, u64)]) -> Case {
        let mut rng = fastrand::Rng::with_seed(seed);
        let mut all = [0u64; GENERAL + 3];
        for &(r, v) in registers {
            all[r] = v;
        }
        all[LINK] = RETURNS;
        all[GENERAL] = STACK;
        all[GENERAL + 1] = rng.u64(..) & nzcv();
        all[GENERAL + 2] = rng.u64(..);
        Case {
            word: 0,
            registers: all,
            vectors: std::array::from_fn(|_| rng.u128(..)),
            float: [0, 0],
            tracking: false,
            seed,
        }
    }

    /// The float status cases: tracking on and off, FPCR default and not, each writing the other mode into
    /// `fpcr_written` and every status bit into `fpsr_written`.
    fn float_cases(
        seed: u64,
        fpsr_written: usize,
        fpcr_written: usize,
        others: &[(usize, u64)],
    ) -> Vec<Case> {
        let mut cases = Vec::new();
        for fpcr in [0, FPCR_BITS] {
            for tracking in [false, true] {
                let written = FPCR_BITS & !fpcr;
                let mut registers = vec![
                    (fpsr_written, FPSR_BITS.into()),
                    (fpcr_written, written.into()),
                ];
                registers.extend_from_slice(others);
                let mut case = start(seed, &registers);
                case.float = [0, fpcr];
                case.tracking = tracking;
                cases.push(case);
            }
        }
        cases
    }

    /// What one function's checks found.
    #[derive(Default)]
    struct Tally {
        forced: u64,
        failed: u64,
    }

    /// The function's word at guest address `pc`, if it has one there.
    fn word_at(words: &[u32], pc: u64) -> Option<&u32> {
        let index = pc.checked_sub(CODE)? / Arm64::MAX_LENGTH;
        words.get(usize::try_from(index).ok()?)
    }

    /// Runs one function's checks from one case.
    fn check(runner: &mut Runner, name: &str, words: &[u32], case: &Case, tally: &mut Tally) {
        let mut fail = |what: String| {
            println!("FAIL {name} (starting state {:#x}): {what}", case.seed);
            tally.failed += 1;
        };
        // Look-ahead none stops at every boundary: they are the run's boundaries.
        let function = runner.load_function(words, LookAhead::None);
        let reference = runner.run_function(&function, case, RETURNS, None);
        runner.unload();
        // A stop at an instruction (a fault) is at the boundary before it.
        let mut boundaries = reference.trace.clone();
        boundaries.dedup();
        let boundaries = &boundaries;
        let mut count = 0;
        for (k, state) in boundaries.iter().enumerate() {
            if state.count != count {
                fail(format!(
                    "count {} at boundary {k}, E gives {count}",
                    state.count
                ));
            }
            if let Some(&word) = word_at(words, state.pc) {
                count += u64::from(counted(state.pc, &Arm64::flow(&decode(word))));
            }
        }
        let last = boundaries.last().expect("a boundary");
        for look_ahead in LOOK_AHEADS {
            let (stop, after, data) = webassembly(words, case, look_ahead);
            if !same_end(stop, &after, reference.stop, last) || data != reference.data {
                fail(format!(
                    "the WebAssembly output at {look_ahead:?} ends {stop:?} {after:x?}, natively {:?} {last:x?}",
                    reference.stop
                ));
            }
        }
        let mut lines = Vec::new();
        for look_ahead in LOOK_AHEADS {
            let function = runner.load_function(words, look_ahead);
            let whole = runner.run_function(&function, case, RETURNS, None);
            let stepped = runner.run_function(&function, case, RETURNS, Some(u64::MAX));
            if whole.stop != reference.stop || whole.data != reference.data {
                fail(format!(
                    "at {look_ahead:?} the run ends {:?}, at none {:?}",
                    whole.stop, reference.stop
                ));
            }
            if whole.trace != stepped.trace || whole.data != stepped.data {
                fail(format!("at {look_ahead:?} stepping changed the run"));
            }
            let mut from = 0;
            for state in &whole.trace {
                match boundaries[from..].iter().position(|b| b == state) {
                    Some(k) => from += k,
                    None => fail(format!(
                        "at {look_ahead:?} a piece's stop is no boundary: {state:x?}"
                    )),
                }
            }
            let mut reached = vec![false; boundaries.len()];
            let mut from = 0;
            for budget in 0..stepped.landings {
                let run: Run = runner.run_function(&function, case, RETURNS, Some(budget));
                let (state, _) = run.interrupted.as_ref().expect("an interrupt").clone();
                tally.forced += 1;
                if run.stop != whole.stop
                    || run.trace.last() != whole.trace.last()
                    || run.data != whole.data
                {
                    fail(format!(
                        "at {look_ahead:?}, going on after an interrupt at landing point {budget} ends {:?} {:x?}, unbroken {:?} {:x?}",
                        run.stop,
                        run.trace.last(),
                        whole.stop,
                        whole.trace.last()
                    ));
                }
                match boundaries[from..].iter().position(|b| *b == state) {
                    Some(k) => {
                        from += k;
                        reached[from] = true;
                    }
                    None => fail(format!(
                        "at {look_ahead:?}, interrupt at landing point {budget}: {state:x?}\n  boundary {from}: {:x?}",
                        boundaries[from]
                    )),
                }
            }
            // In a run of more than one instruction the Nothing route carries no host bytes: the host
            // instruction before it is the one before its successor, which the position map names, with
            // the same count and registers. Only a stop asked for at that pc (replay's check) lands there.
            let carried = |state: &GuestState| {
                look_ahead == LookAhead::None
                    || word_at(words, state.pc)
                        .is_none_or(|&word| Arm64::route(&decode(word)) != Route::Nothing)
            };
            if let Some(k) = (0..boundaries.len()).find(|&k| !reached[k] && carried(&boundaries[k]))
            {
                fail(format!(
                    "at {look_ahead:?} no interrupt reached boundary {k}: {:x?}",
                    boundaries[k]
                ));
            }
            runner.unload();
            lines.push(format!(
                "{look_ahead:?} {} pieces run, {} landing points",
                whole.trace.len() - 1,
                stepped.landings
            ));
        }
        println!(
            "{name}: {} boundaries, stop {:?}, count {} at the end; {}",
            boundaries.len(),
            reference.stop,
            last.count,
            lines.join(", ")
        );
    }

    /// [`JUMP_TABLE`]'s operations, a byte each from the lowest: one that returns, and one that reaches entry 4.
    const JUMPS: u64 = 0x0300_0201_0002_0100;
    const JUMPS_NOWHERE: u64 = 0x0303_0303_0304_0100;
    /// Its table: 0, 2, 4, 8 and 255.
    const TABLE: u64 = 0xff_0804_0200;
    /// [`SIGNED_JUMP_TABLE`]'s operations, one that returns and one that reaches entry 4, and its table: 0, -1, -8,
    /// 2 and -128.
    const SIGNED_JUMPS: u64 = 0x0301_0002_0100;
    const SIGNED_JUMPS_BACK: u64 = u64::from_le_bytes([0, 4, 0, 0, 0, 0, 0, 0]);
    const SIGNED_TABLE: u64 = 0x80_02f8_ff00;

    /// The functions and their cases, starting states drawn from `seed`.
    fn functions(seed: u64) -> Vec<(&'static str, &'static [u32], Vec<Case>)> {
        let start = |registers: &[(usize, u64)]| start(seed, registers);
        let words = 8;
        let writable = DATA + PAGE_SIZE / 2;
        let read_only = DATA + DATA_WRITABLE + PAGE_SIZE / 2;
        let misaligned = writable + size_of::<u32>() as u64;
        let mut rng = fastrand::Rng::with_seed(seed);
        let system = [
            (3, rng.u64(..)),
            (6, rng.u64(..) & nzcv()),
            (15, rng.u64(..)),
            (16, DATA),
        ];
        vec![
            ("sum", &SUM, vec![start(&[(0, DATA), (1, words)])]),
            ("calls", &CALLS, vec![start(&[])]),
            ("looped calls", &LOOPED_CALLS, vec![start(&[])]),
            (
                "atomics",
                &ATOMICS,
                vec![start(&[
                    (0, writable),
                    (1, DATA + PAGE_SIZE - size_of::<u32>() as u64),
                ])],
            ),
            ("cleared", &CLEARED, vec![start(&[(0, writable)])]),
            (
                "exclusive",
                &EXCLUSIVE,
                vec![
                    start(&[(0, writable), (1, writable)]),
                    start(&[(0, read_only), (1, read_only)]),
                    start(&[(0, writable), (1, read_only)]),
                    start(&[(0, writable), (1, misaligned)]),
                ],
            ),
            (
                "jump table",
                &JUMP_TABLE,
                vec![
                    start(&[(0, writable), (9, JUMPS), (10, TABLE)]),
                    start(&[(0, writable), (9, JUMPS_NOWHERE), (10, TABLE)]),
                ],
            ),
            (
                "signed jump table",
                &SIGNED_JUMP_TABLE,
                vec![
                    start(&[(0, writable), (9, SIGNED_JUMPS), (10, SIGNED_TABLE)]),
                    start(&[(0, writable), (9, SIGNED_JUMPS_BACK), (10, SIGNED_TABLE)]),
                ],
            ),
            (
                "written-back index",
                &WRITTEN_BACK_INDEX,
                vec![start(&[(3, writable)])],
            ),
            ("float", &FLOAT, float_cases(seed, 8, 10, &[])),
            ("system", &SYSTEM, float_cases(seed, 13, 8, &system)),
            (
                "withheld",
                withheld(),
                vec![
                    start(&[(PAGE.into(), writable)]),
                    start(&[(PAGE.into(), read_only)]),
                ],
            ),
        ]
    }

    pub fn main() {
        be_signed();
        let started = Instant::now();
        let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
        let starts = setting(STARTS_VARIABLE, DEFAULT_STARTS);
        let mut runner = Runner::start();
        let mut seeds = fastrand::Rng::with_seed(seed);
        let mut tally = Tally::default();
        let mut cases = 0;
        let mut names = 0;
        for draw in 0..starts {
            // The first starting state is the seed's own, so a failing one replays with it alone.
            let draw_seed = if draw == 0 { seed } else { seeds.u64(..) };
            let functions = functions(draw_seed);
            names = functions.len();
            for (name, words, function_cases) in &functions {
                for case in function_cases {
                    check(&mut runner, name, words, case, &mut tally);
                    cases += 1;
                }
            }
        }
        runner.finish();
        println!(
            "forced interrupts: {} over {names} functions ({cases} cases from {starts} starting states, seed {seed:#x}) at {} look-aheads, {} failed, in {:?}, load {}",
            tally.forced,
            LOOK_AHEADS.len(),
            tally.failed,
            started.elapsed(),
            load_average()
        );
        assert_eq!(tally.failed, 0, "interrupts resolved wrong");
    }
}
