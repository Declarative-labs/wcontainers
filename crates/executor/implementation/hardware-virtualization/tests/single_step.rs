//! The single step executor on this host's chip: its batch form and the executor suite, each against the chip's
//! precise instruction set on every state the critical value generator makes, and the time each takes.
//!
//! The suite runs from `main`, not as a `#[test]`: Hypervisor.framework allows one machine per process, bound to
//! the thread that made it, and the test must first run itself as a copy signed with the hypervisor entitlement.

#[cfg(all(host_hypervisor, target_arch = "aarch64"))]
mod common;

#[cfg(all(host_hypervisor, target_arch = "aarch64"))]
fn main() {
    measured::main();
}

#[cfg(not(all(host_hypervisor, target_arch = "aarch64")))]
fn main() {
    println!(
        "the single step executor needs an ARM64 hypervisor this crate drives; nothing to run on this host"
    );
}

#[cfg(all(host_hypervisor, target_arch = "aarch64"))]
mod measured {
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
    #[cfg(target_os = "macos")]
    use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2 as OtherChip;
    #[cfg(target_os = "linux")]
    use wcontainers_precise_instruction_set_neoverse_v2::NeoverseV2 as ThisChip;
    use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};
    use wcontainers_processor_architecture_arm64::Arm64;
    use wcontainers_processor_architecture_recording::leave_same_memory;

    use crate::common::{be_signed, load_average};

    const GENERATOR: CriticalValueProduct<Arm64InputDomain> =
        CriticalValueProduct::new(Arm64InputDomain);

    fn to_microseconds_per_state(time: Duration, states: usize) -> f64 {
        time.as_secs_f64() * 1e6 / states as f64
    }

    pub fn main() {
        be_signed();
        println!("load before: {}", load_average());

        let mut executor = SingleStepMachine::new().expect("making the single step executor");
        let mut states = 0;
        let mut batch_time = Duration::ZERO;
        let mut disagreeing = Vec::new();
        for instruction in Arm64::list_instructions() {
            let befores: Vec<ExecutionState<'static, Arm64>> =
                DistinguishingStateGenerator::<Arm64>::generate_states(&GENERATOR, instruction)
                    .collect();
            let started = Instant::now();
            let afters = executor.execute_each_single_instruction(befores.iter().cloned());
            batch_time += started.elapsed();
            states += befores.len();
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
            "batch: {states} states, {:.3} us per state, {:.3} us of it in the guest; load after: {}",
            to_microseconds_per_state(batch_time, states),
            to_microseconds_per_state(executor.time_in_guest(), states),
            load_average(),
        );
        drop(executor);

        let executor = SingleStepMachine::new().expect("making the single step executor");
        let started = Instant::now();
        agrees_with_precise_instruction_set(executor, ThisChip, GENERATOR);
        println!(
            "one at a time: {:.3} us per state; load after: {}",
            to_microseconds_per_state(started.elapsed(), states),
            load_average(),
        );
        assert!(
            disagreeing.is_empty(),
            "the batch disagrees with this chip's set on states of {disagreeing:?}"
        );

        // The suite must catch a chip that is not the one the set describes: the two chips differ in RDVL.
        let executor = SingleStepMachine::new().expect("making the single step executor");
        let other = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            agrees_with_precise_instruction_set(executor, OtherChip, GENERATOR)
        }));
        assert!(
            other.is_err(),
            "the suite found this chip agreeing with the other chip's set"
        );
        println!("the other chip's set disagrees, as it must");
    }
}
