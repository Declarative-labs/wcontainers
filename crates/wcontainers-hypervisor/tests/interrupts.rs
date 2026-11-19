//! N2's forced interrupts: three guest functions run as the native output's pieces, one piece after another,
//! inside Hypervisor.framework. Each runs once whole and is compared with the WebAssembly output under
//! wasmtime; its count at every instruction boundary is compared with definition E; then an interrupt is
//! forced after every number of host instructions it executes (software step), and the guest state the
//! position map resolves there (registers, pc, count) must be the state at the boundary before or after the
//! instruction it fell in. It runs itself as a copy signed with the hypervisor entitlement.

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

    use wcontainers_abi::{PAGE_SIZE, Permissions, RegisterBlock};
    use wcontainers_arm64::{Arm64, decode};
    use wcontainers_guest_cpu::{Guest, counted};
    use wcontainers_translator::LookAhead;
    use wcontainers_translator_test::layer1::{Case, Stop};
    use wcontainers_translator_test::layout::{
        CODE, CODE_SPAN, DATA, DATA_BYTES, DATA_WRITABLE, fill,
    };
    use wcontainers_translator_test::runner::{Machine, Stopped, Syscalls};

    use crate::common::{be_signed, load_average};
    use crate::runner::function::Run;
    use crate::runner::{GuestState, Runner};

    /// Where the functions return to: an address with no code.
    const RETURNS: u64 = CODE + PAGE_SIZE;
    /// The stack the functions start with, in the writable data.
    const STACK: u64 = DATA + DATA_WRITABLE / 2;
    /// The general registers: x0 to x30, then sp, flags and the thread pointer in a case.
    const GENERAL: usize = RegisterBlock::ZERO.x.len();
    const LINK: usize = GENERAL - 1;
    const SEED: u64 = 0x1405_7b7e_f767_814f;

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

    /// A case's starting state with the registers given.
    fn start(registers: &[(usize, u64)]) -> Case {
        let mut all = [0u64; GENERAL + 3];
        for &(r, v) in registers {
            all[r] = v;
        }
        all[LINK] = RETURNS;
        all[GENERAL] = STACK;
        Case {
            word: 0,
            registers: all,
            vectors: [0; RegisterBlock::ZERO.v.len()],
            float: [0, 0],
            tracking: false,
            seed: SEED,
        }
    }

    /// The WebAssembly output's run of the function: its stop, registers and data area.
    fn webassembly(words: &[u32], case: &Case) -> (Stopped, RegisterBlock, Vec<u8>) {
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let slots = u32::try_from(words.len()).expect("a short function");
        let mut machine = Machine::new(CODE + CODE_SPAN, CODE, slots, Syscalls::Stop);
        machine.set_permissions(
            DATA..DATA + DATA_BYTES,
            Permissions::READ | Permissions::WRITE,
        );
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
        machine.load(
            &bytes,
            0..bytes.len() as u64,
            CODE,
            &starts,
            LookAhead::None,
        );
        let mut registers = RegisterBlock::ZERO;
        registers.x.copy_from_slice(&case.registers[..GENERAL]);
        registers.sp = case.registers[GENERAL];
        registers.pc = CODE;
        machine.set_registers(&registers);
        let (stopped, after) = machine.run();
        machine.read(DATA, &mut data);
        (stopped, after, data)
    }

    /// What differs between two guest states.
    fn differs(a: &GuestState, b: &GuestState) -> String {
        format!("{a:x?}\n  against {b:x?}")
    }

    /// Runs one function's checks; gives the interrupts forced and how many failed.
    fn check(runner: &mut Runner, name: &str, words: &[u32], case: &Case) -> (u64, u64) {
        let function = runner.load_function(words);
        let whole = runner.run_function(&function, case, RETURNS, None);
        let stepped = runner.run_function(&function, case, RETURNS, Some(u64::MAX));
        let mut failed = 0;
        let mut fail = |what: String| {
            println!("FAIL {name}: {what}");
            failed += 1;
        };
        if whole.stop != Stop::Landed(RETURNS) {
            fail(format!("stopped {:?}", whole.stop));
        }
        if whole.trace != stepped.trace || whole.data != stepped.data {
            fail("stepping changed the run".into());
        }
        // The count at each boundary is definition E's over the instructions before it.
        let mut count = 0;
        for (k, state) in whole.trace.iter().enumerate() {
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
        let (stopped, after, data) = webassembly(words, case);
        let last = whole.trace.last().expect("a boundary");
        if !matches!(stopped, Stopped::NoCode(RETURNS))
            || after.x != last.x
            || after.sp != last.sp
            || after.pc != last.pc
            || after
                .v
                .map(|[low, high]| u128::from(high) << u64::BITS | u128::from(low))
                != last.v
            || [after.fpsr, after.fpcr] != last.float
            || data != whole.data
        {
            fail(format!(
                "the WebAssembly output ends {stopped:?} {after:x?}, natively {last:x?}"
            ));
        }
        let mut forced = 0;
        for budget in 0..=stepped.steps {
            let run: Run = runner.run_function(&function, case, RETURNS, Some(budget));
            let (state, passed) = run.interrupted.expect("an interrupt");
            forced += 1;
            let before = &whole.trace[passed];
            let after = whole.trace.get(passed + 1);
            if state != *before && Some(&state) != after {
                fail(format!(
                    "interrupt after {budget} host instructions: {}",
                    differs(&state, before)
                ));
            }
        }
        runner.unload();
        println!(
            "{name}: {} guest instructions, {} host instructions, {forced} interrupts, count {} at the end",
            whole.trace.len() - 1,
            stepped.steps,
            last.count
        );
        (forced, failed)
    }

    pub fn main() {
        be_signed();
        let started = Instant::now();
        let mut runner = Runner::start();
        let words = 8;
        let sum = start(&[(0, DATA), (1, words)]);
        let calls = start(&[]);
        let atomics = start(&[
            (0, DATA + PAGE_SIZE / 2),
            (1, DATA + PAGE_SIZE - size_of::<u32>() as u64),
        ]);
        let mut forced = 0;
        let mut failed = 0;
        for (name, words, case) in [
            ("sum", &SUM[..], &sum),
            ("calls", &CALLS[..], &calls),
            ("atomics", &ATOMICS[..], &atomics),
        ] {
            let (f, n) = check(&mut runner, name, words, case);
            forced += f;
            failed += n;
        }
        runner.finish();
        println!(
            "forced interrupts: {forced} over three functions, {failed} failed, in {:?}, load {}",
            started.elapsed(),
            load_average()
        );
        assert_eq!(failed, 0, "interrupts resolved wrong");
    }
}
