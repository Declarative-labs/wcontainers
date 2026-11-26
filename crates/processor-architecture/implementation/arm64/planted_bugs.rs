//! PLANTED BUGS: plants each bug of `planted::PLANTED_BUGS` in turn and measures its instruction on this host's chip
//! with the provided `measure_instruction` over every state the ARM64 state generator makes. A bug is caught where no
//! known behavior carrying it agrees with the chip on every state; then prints caught of total by class.
//!
//! `cargo run --release -p wcontainers-processor-architecture-arm64 --features planted --example planted_bugs [--
//! <number>...]`

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn main() {
    use std::collections::BTreeMap;
    use std::time::Instant;

    use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
    use wcontainers_behavior_source_kvm_single_step::KvmSingleStepMachine;
    use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
    use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
    use wcontainers_processor_architecture_arm64::planted::{self, PLANTED_BUGS};

    let named: Vec<u32> = std::env::args()
        .skip(1)
        .filter_map(|argument| argument.parse().ok())
        .collect();
    let bugs: Vec<_> = PLANTED_BUGS
        .iter()
        .chain(&wcontainers_processor_architecture_arm64::advanced_simd::ADVANCED_SIMD_PLANTED_BUGS)
        .chain(&wcontainers_processor_architecture_arm64::load_store::scalable_element_access::ELEMENT_ACCESS_PLANTED_BUGS)
        .chain(&wcontainers_processor_architecture_arm64::sve_remaining::REMAINING_PLANTED_BUGS)
        .filter(|bug| named.is_empty() || named.contains(&bug.number))
        .collect();
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    println!("load {}", load.trim());
    let mut chip = match KvmSingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("no single step machine: {error:?}"),
    };
    let generator = CriticalValueCut::default();
    let started = Instant::now();
    // With no bug planted, every instruction a bug is planted in measures as one behavior or as several no state
    // tells apart, so that a bug's failure is the bug's.
    let mut unplanted = Vec::new();
    for bug in &bugs {
        if !unplanted.contains(&bug.instruction) {
            unplanted.push(bug.instruction);
        }
    }
    planted::plant(planted::NONE);
    for instruction in &unplanted {
        match chip.measure_instruction(*instruction, &generator) {
            Ok(_) | Err(MeasureInstructionError::IndistinguishableBehaviors { .. }) => {}
            Err(_) => panic!("{instruction:?} disagrees with the chip with no bug planted"),
        }
    }
    let mut by_class: BTreeMap<String, (usize, usize, Vec<u32>)> = BTreeMap::new();
    for bug in &bugs {
        planted::plant(bug.number);
        let states = generator.generate_states(bug.instruction).count();
        let caught = match chip.measure_instruction(bug.instruction, &generator) {
            Err(MeasureInstructionError::NoBehaviorAgrees { before, .. }) => {
                println!(
                    "bug {} {:?} ({}, {:?}): CAUGHT of {states} states, first at x0 {:#x} x1 {:#x} x2 {:#x} pc {:#x}",
                    bug.number,
                    bug.instruction,
                    bug.what,
                    bug.planted_by,
                    before.processor.general_purpose_registers[0],
                    before.processor.general_purpose_registers[1],
                    before.processor.general_purpose_registers[2],
                    before.processor.program_counter
                );
                true
            }
            Ok(precise) => {
                println!(
                    "bug {} {:?} ({}, {:?}): MISSED, {states} states agree with {precise:?} carrying it",
                    bug.number, bug.instruction, bug.what, bug.planted_by
                );
                false
            }
            Err(error) => {
                let what = match error {
                    MeasureInstructionError::IndistinguishableBehaviors { behaviors, .. } => {
                        format!("{} behaviors carrying it agree", behaviors.len())
                    }
                    MeasureInstructionError::ExecutionFailed { error, .. } => {
                        format!("the chip refused a state: {error:?}")
                    }
                    _ => "no known behavior".to_owned(),
                };
                println!(
                    "bug {} {:?} ({}, {:?}): MISSED, {states} states, {what}",
                    bug.number, bug.instruction, bug.what, bug.planted_by
                );
                false
            }
        };
        let class = bug
            .instruction
            .to_record()
            .map_or("None".to_owned(), |record| format!("{:?}", record.class));
        let entry = by_class.entry(class).or_default();
        entry.0 += usize::from(caught);
        entry.1 += 1;
        if !caught {
            entry.2.push(bug.number);
        }
    }
    planted::plant(planted::NONE);
    let (mut caught, mut total) = (0, 0);
    for (class, (class_caught, class_total, missed)) in &by_class {
        println!("class {class}: {class_caught} of {class_total} caught, missed {missed:?}");
        caught += class_caught;
        total += class_total;
    }
    let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
    println!(
        "planted: {caught} of {total} caught in {:.1} s; load {}",
        started.elapsed().as_secs_f64(),
        load.trim()
    );
}

#[cfg(not(all(target_arch = "aarch64", target_os = "linux")))]
fn main() {
    panic!("the planted bugs run on ARM64 Linux with KVM");
}
