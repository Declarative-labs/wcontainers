//! N2's forced interrupts and the count across look-aheads: guest functions run as the native output's pieces,
//! one piece after another, inside Hypervisor.framework, at every look-ahead setting. At look-ahead none every
//! instruction boundary is a stop, which gives the run's boundaries; each is checked against definition E's
//! count, and the end against the WebAssembly output under wasmtime at every look-ahead. Then, at every
//! look-ahead, the function runs whole and an interrupt is forced after every number of host instructions it
//! executes (software step): the guest state the position map resolves there (registers, pc, count and the
//! rest) must be one of the boundaries, in order, and every boundary must be reached by some interrupt. So the
//! count at every stop is the same at every look-ahead. It runs itself as a copy signed with the hypervisor
//! entitlement.

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

    use wcontainers_abi::{Access, Exit, PAGE_SIZE, Permissions, RegisterBlock};
    use wcontainers_arm64::{Arm64, FPCR_BITS, FPSR_BITS, decode};
    use wcontainers_guest_cpu::{Guest, counted};
    use wcontainers_translator::LookAhead;
    use wcontainers_translator_test::layer1::{Case, Stop};
    use wcontainers_translator_test::layout::{
        CODE, CODE_SPAN, DATA, DATA_BYTES, DATA_WRITABLE, fill,
    };
    use wcontainers_translator_test::runner::{Machine, Stopped, Syscalls};

    use crate::common::{be_signed, load_average};
    use crate::runner::function::Run;
    use crate::runner::{GuestState, Runner, nzcv};

    /// Where the functions return to: an address with no code.
    const RETURNS: u64 = CODE + PAGE_SIZE;
    /// The stack the functions start with, in the writable data.
    const STACK: u64 = DATA + DATA_WRITABLE / 2;
    /// The general registers: x0 to x30, then sp, flags and the thread pointer in a case.
    const GENERAL: usize = RegisterBlock::ZERO.x.len();
    const LINK: usize = GENERAL - 1;
    const SEED: u64 = 0x1405_7b7e_f767_814f;
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
    const WITHHELD: [u32; 6] = [
        0x1000007b, // adr x27, #12
        0xaa1b03e1, // mov x1, x27
        0xc85f7f9b, // ldxr x27, [x28]
        0xc81a7f9b, // stxr w26, x27, [x28]
        0xa9400781, // ldp x1, x1, [x28]
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

    /// A case's starting state with the registers given, the flags, thread pointer and vectors drawn from the
    /// seed.
    fn start(registers: &[(usize, u64)]) -> Case {
        let mut rng = fastrand::Rng::with_seed(SEED);
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
            seed: SEED,
        }
    }

    /// The float function's cases: tracking on and off, FPCR default and not, each writing the other mode.
    fn float_cases() -> Vec<Case> {
        let mut cases = Vec::new();
        for fpcr in [0, FPCR_BITS] {
            for tracking in [false, true] {
                let written = FPCR_BITS & !fpcr;
                let mut case = start(&[(8, FPSR_BITS.into()), (10, written.into())]);
                case.float = [0, fpcr];
                case.tracking = tracking;
                cases.push(case);
            }
        }
        cases
    }

    /// The WebAssembly output's run of the function at `look_ahead`: its stop, registers and data area.
    fn webassembly(
        words: &[u32],
        case: &Case,
        look_ahead: LookAhead,
    ) -> (Stop, RegisterBlock, Vec<u8>) {
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let slots = u32::try_from(words.len()).expect("a short function");
        let mut machine = Machine::new(CODE + CODE_SPAN, CODE, slots, Syscalls::Stop);
        machine.set_permissions(
            DATA..DATA + DATA_WRITABLE,
            Permissions::READ | Permissions::WRITE,
        );
        machine.set_permissions(DATA + DATA_WRITABLE..DATA + DATA_BYTES, Permissions::READ);
        machine.set_permissions(
            CODE - CODE_SPAN..CODE + CODE_SPAN,
            Permissions::READ | Permissions::EXECUTE,
        );
        let mut data = vec![0u8; DATA_BYTES as usize];
        fill(case.seed, &mut data);
        machine.write(DATA, &data);
        let starts: Vec<u64> = (0..bytes.len() as u64)
            .step_by(Arm64::MAX_LENGTH as usize)
            .collect();
        machine.load(&bytes, 0..bytes.len() as u64, CODE, &starts, look_ahead);
        let mut registers = RegisterBlock::ZERO;
        registers.x.copy_from_slice(&case.registers[..GENERAL]);
        registers.sp = case.registers[GENERAL];
        registers.pc = CODE;
        [registers.fpsr, registers.fpcr] = case.float;
        registers.pstate = case.registers[GENERAL + 1];
        registers.tpidr_el0 = case.registers[GENERAL + 2];
        registers.v = case.vectors.map(|v| [v as u64, (v >> u64::BITS) as u64]);
        machine.set_registers(&registers);
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
            other => panic!("not a stop these functions make: {other:?}"),
        };
        (stop, after, data)
    }

    /// Whether the WebAssembly output's end is the native one's.
    fn same_end(stop: Stop, after: &RegisterBlock, native_stop: Stop, last: &GuestState) -> bool {
        stop == native_stop
            && after.x == last.x
            && after.sp == last.sp
            && after.pc == last.pc
            && after
                .v
                .map(|[low, high]| u128::from(high) << u64::BITS | u128::from(low))
                == last.v
            && [after.fpsr, after.fpcr] == last.float
            && after.pstate == last.flags
            && after.tpidr_el0 == last.thread
    }

    /// What one function's checks found.
    #[derive(Default)]
    struct Tally {
        forced: u64,
        failed: u64,
    }

    /// Runs one function's checks from one case.
    fn check(runner: &mut Runner, name: &str, words: &[u32], case: &Case, tally: &mut Tally) {
        let mut fail = |what: String| {
            println!("FAIL {name}: {what}");
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
            let index = ((state.pc - CODE) / Arm64::MAX_LENGTH) as usize;
            if let Some(&word) = words.get(index) {
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
            for budget in 0..=stepped.steps {
                let run: Run = runner.run_function(&function, case, RETURNS, Some(budget));
                let (state, _) = run.interrupted.as_ref().expect("an interrupt").clone();
                tally.forced += 1;
                if run.stop != whole.stop
                    || run.trace.last() != whole.trace.last()
                    || run.data != whole.data
                {
                    fail(format!(
                        "at {look_ahead:?}, going on after an interrupt after {budget} host instructions ends {:?} {:x?}, unbroken {:?} {:x?}",
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
                        "at {look_ahead:?}, interrupt after {budget} host instructions: {state:x?}\n  boundary {from}: {:x?}",
                        boundaries[from]
                    )),
                }
            }
            if let Some(k) = reached.iter().position(|r| !r) {
                fail(format!(
                    "at {look_ahead:?} no interrupt reached boundary {k}: {:x?}",
                    boundaries[k]
                ));
            }
            runner.unload();
            lines.push(format!(
                "{look_ahead:?} {} pieces run, {} host instructions",
                whole.trace.len() - 1,
                stepped.steps
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

    pub fn main() {
        be_signed();
        let started = Instant::now();
        let mut runner = Runner::start();
        let words = 8;
        let writable = DATA + PAGE_SIZE / 2;
        let read_only = DATA + DATA_WRITABLE + PAGE_SIZE / 2;
        let misaligned = writable + size_of::<u32>() as u64;
        let functions: [(&str, &[u32], Vec<Case>); 7] = [
            ("sum", &SUM, vec![start(&[(0, DATA), (1, words)])]),
            ("calls", &CALLS, vec![start(&[])]),
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
            ("float", &FLOAT, float_cases()),
            (
                "withheld",
                &WITHHELD,
                vec![start(&[(28, writable)]), start(&[(28, read_only)])],
            ),
        ];
        let mut tally = Tally::default();
        for (name, words, cases) in &functions {
            for case in cases {
                check(&mut runner, name, words, case, &mut tally);
            }
        }
        runner.finish();
        println!(
            "forced interrupts: {} over {} functions at {} look-aheads, {} failed, in {:?}, load {}",
            tally.forced,
            functions.len(),
            LOOK_AHEADS.len(),
            tally.failed,
            started.elapsed(),
            load_average()
        );
        assert_eq!(tally.failed, 0, "interrupts resolved wrong");
    }
}
