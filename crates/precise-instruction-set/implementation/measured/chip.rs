//! This host's chip through the single step machine: measured as a program would at start-up, its batch form and
//! the executor suite each against the chip's own set on every state the critical value generator makes, and the
//! time each takes.
//!
//! It runs from `main`, not as `#[test]`s: Hypervisor.framework allows one machine per process, bound to the
//! thread that made it, and the program must first run itself as a copy signed with the hypervisor entitlement.

#[cfg(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux")))]
mod signing;

#[cfg(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux")))]
fn main() {
    on_chip::main();
}

#[cfg(not(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux"))))]
fn main() {
    println!("the single step machine needs an ARM64 host with Hypervisor.framework or KVM");
}

#[cfg(all(target_arch = "aarch64", any(target_os = "macos", target_os = "linux")))]
mod on_chip {
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    use std::time::{Duration, Instant};

    use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
    use wcontainers_distinguishing_state_generator_input_domain::CriticalValueProduct;
    use wcontainers_executor_conformance::agrees_with_precise_instruction_set;
    use wcontainers_executor_hardware_virtualization::single_step::SingleStepMachine;
    use wcontainers_instruction_input_domain_arm64::Arm64InputDomain;
    use wcontainers_precise_instruction_set::PreciseInstructionSet;
    #[cfg(target_os = "linux")]
    use wcontainers_precise_instruction_set_apple_m5::AppleM5 as OtherChip;
    #[cfg(target_os = "macos")]
    use wcontainers_precise_instruction_set_apple_m5::AppleM5 as ThisChip;
    use wcontainers_precise_instruction_set_apple_m5::AppleM5;
    use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
    #[cfg(target_os = "macos")]
    use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2 as OtherChip;
    #[cfg(target_os = "linux")]
    use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2 as ThisChip;
    use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2;
    use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};
    use wcontainers_processor_architecture_arm64::Arm64;
    use wcontainers_processor_architecture_conformance::leave_same_memory;

    use crate::signing::be_signed;

    const GENERATOR: CriticalValueProduct<Arm64InputDomain> =
        CriticalValueProduct::new(Arm64InputDomain);

    fn to_microseconds_per_state(time: Duration, states: usize) -> f64 {
        time.as_secs_f64() * 1e6 / states as f64
    }

    fn make_machine() -> SingleStepMachine {
        match SingleStepMachine::new() {
            Ok(machine) => machine,
            Err(error) => panic!("this host's hypervisor makes no single step machine: {error:?}"),
        }
    }

    pub fn main() {
        be_signed();
        measure_at_start_up();
        let states = time_batches();
        run_suite(states);
    }

    /// Measures the chip against both known sets, five times on a fresh machine each, and checks it is this chip.
    fn measure_at_start_up() {
        let known: [&dyn PreciseInstructionSet<Arm64>; 2] = [&AppleM5, &NeoverseV2];
        for _ in 0..5 {
            let mut chip = make_machine();
            let started = Instant::now();
            let measured = MeasuredInstructionSet::measure(&mut chip, &known, &GENERATOR);
            let elapsed = started.elapsed();
            let measured = match measured {
                Ok(measured) => measured,
                Err(failure) => panic!("the chip's behaviors are no known set's: {failure:?}"),
            };
            println!(
                "measured at start-up in {:.3} ms, {:.3} ms of it in the guest",
                elapsed.as_secs_f64() * 1e3,
                chip.time_in_guest().as_secs_f64() * 1e3
            );
            for instruction in Arm64::list_instructions() {
                assert!(
                    measured.to_precise(instruction) == ThisChip.to_precise(instruction),
                    "{instruction:?} measures as another chip's"
                );
            }
        }
    }

    /// Runs every state as a batch twice, cold then warm, and checks each after against this chip's set.
    fn time_batches() -> usize {
        let mut executor = make_machine();
        let mut states = 0;
        let mut disagreeing = Vec::new();
        for pass in ["cold", "warm"] {
            states = 0;
            disagreeing.clear();
            let mut batch_time = Duration::ZERO;
            let guest_time_before = executor.time_in_guest();
            for instruction in Arm64::list_instructions() {
                let befores: Vec<ExecutionState<'static, Arm64>> =
                    DistinguishingStateGenerator::<Arm64>::generate_states(&GENERATOR, instruction)
                        .collect();
                let guest_time_before_instruction = executor.time_in_guest();
                let started = Instant::now();
                let afters = executor.execute_each_single_instruction(befores.iter().cloned());
                let elapsed = started.elapsed();
                batch_time += elapsed;
                states += befores.len();
                println!(
                    "  {instruction:?}: {} states, {:.3} us per state, {:.3} us of it in the guest",
                    befores.len(),
                    to_microseconds_per_state(elapsed, befores.len()),
                    to_microseconds_per_state(
                        executor.time_in_guest() - guest_time_before_instruction,
                        befores.len()
                    ),
                );
                let precise = ThisChip.to_precise(instruction);
                let disagree = befores
                    .into_iter()
                    .zip(&afters)
                    .filter(|(before, executed)| {
                        let Ok(simulated) = precise.simulate(before.clone());
                        let start = before.memory_mutations.len();
                        simulated.processor != executed.processor
                            || !leave_same_memory(
                                &before.memory_mutations,
                                &simulated.memory_mutations[start..],
                                &executed.memory_mutations[start..],
                            )
                    })
                    .count();
                if disagree > 0 {
                    disagreeing.push(format!("{instruction:?}: {disagree}"));
                }
            }
            println!(
                "batch, {pass}: {states} states, {:.3} us per state, {:.3} us of it in the guest",
                to_microseconds_per_state(batch_time, states),
                to_microseconds_per_state(executor.time_in_guest() - guest_time_before, states),
            );
        }
        assert!(
            disagreeing.is_empty(),
            "the batch disagrees with this chip's set on states of {disagreeing:?}"
        );
        states
    }

    /// Runs the executor suite one state at a time against this chip's set, and against the other chip's, which
    /// it must reject.
    fn run_suite(states: usize) {
        let started = Instant::now();
        let this_chip = catch_unwind(AssertUnwindSafe(|| {
            agrees_with_precise_instruction_set(make_machine(), ThisChip, GENERATOR)
        }));
        println!(
            "one at a time: {:.3} us per state",
            to_microseconds_per_state(started.elapsed(), states),
        );
        if let Err(panic) = this_chip {
            resume_unwind(panic);
        }
        let other_chip = catch_unwind(AssertUnwindSafe(|| {
            agrees_with_precise_instruction_set(make_machine(), OtherChip, GENERATOR)
        }));
        assert!(
            other_chip.is_err(),
            "the suite found this chip agreeing with the other chip's set"
        );
        println!("the other chip's set disagrees, as it must");
    }
}
