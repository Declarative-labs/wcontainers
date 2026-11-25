//! DEFAULT BEHAVIORS: measures every modeled instruction on this host's chip with the provided `measure_instruction`
//! over every state the ARM64 state generator makes, timing it; then runs every state again, timing each part of it
//! (making the state, simulating each known behavior, running it on the chip, comparing) and counting the states each
//! behavior disagrees on; then runs a word of each listed instruction that has no model once.
//!
//! `cargo run --release -p wcontainers-precise-instruction-set-measured --example default_behaviors [-- [--probe]
//! <name>...]` limits the run to the instructions and classes named; `--probe` runs only the measuring loop. The chip
//! loads memory and enters as `SINGLE_STEP_LOADING` and `SINGLE_STEP_ENTRY` say.

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn main() {
    use std::borrow::Cow;
    use std::collections::BTreeMap;
    use std::convert::Infallible;
    use std::time::{Duration, Instant};

    use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
    use wcontainers_behavior_source_kvm_single_step::KvmSingleStepMachine;
    use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
    use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
    use wcontainers_instruction_input_domain::InstructionInputDomain;
    use wcontainers_instruction_input_domain_arm64::{Arm64InputDomain, OperandValue};
    use wcontainers_processor_architecture::{
        ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator,
    };
    use wcontainers_processor_architecture_arm64::{
        Arm64, DataAccess, Exception, Instruction, InstructionClass, InstructionFetch,
        PagePermission, Processor, User, ZERO_PROCESSOR,
    };

    /// Disagreeing states printed whole for each instruction.
    const PRINTED_DISAGREEMENTS: usize = 3;
    /// The page the word of an unmodeled instruction runs from.
    const CODE_PAGE: std::ops::Range<u64> = 0x1000..0x2000;

    type Class = Option<InstructionClass>;

    #[derive(Default, Clone, Copy)]
    struct Split {
        make: Duration,
        simulate: Duration,
        chip: Duration,
        compare: Duration,
    }

    #[derive(Default)]
    struct ClassRow {
        modeled: usize,
        states: Vec<usize>,
        product: u128,
        disagreeing_instructions: usize,
        disagreeing_states: usize,
        probe: Duration,
        split: Split,
        listed: usize,
        absent: usize,
        present: usize,
    }

    fn print_load() {
        let load = std::fs::read_to_string("/proc/loadavg").unwrap_or_default();
        println!("load {}", load.trim());
    }

    fn to_class(instruction: Instruction<Infallible>) -> Class {
        instruction.to_record().map(|record| record.class)
    }

    fn describe(state: &ExecutionState<'_, Arm64<User>>, start: usize) -> String {
        let changes: Vec<String> = state.memory_mutations[start..]
            .iter()
            .map(|change| match change {
                MemoryDelta::ValueAssignment { start, bytes } => {
                    format!("{start:#x} gets {bytes:02x?}")
                }
                MemoryDelta::PermissionAssignment {
                    addresses,
                    permission,
                } => format!("{addresses:#x?} becomes {permission:?}"),
            })
            .collect();
        format!("{:x?} changes {changes:?}", state.processor)
    }

    let mut arguments: Vec<String> = std::env::args().skip(1).collect();
    let probe_only = arguments.iter().any(|argument| argument == "--probe");
    arguments.retain(|argument| argument != "--probe");
    let generator = CriticalValueCut::default();
    let domain = Arm64InputDomain;
    let listed: Vec<Instruction<Infallible>> = Arm64::<User>::list_instructions().collect();
    let is_named = |instruction: &Instruction<Infallible>| {
        let class = to_class(*instruction).map(|class| format!("{class:?}"));
        arguments.is_empty()
            || arguments.contains(&format!("{instruction:?}"))
            || class.is_some_and(|class| arguments.contains(&class))
    };
    let modeled: Vec<Instruction<Infallible>> = listed
        .iter()
        .copied()
        .filter(|instruction| {
            Arm64::<User>::list_known_behaviors(*instruction)
                .next()
                .is_some()
        })
        .filter(is_named)
        .collect();
    let mut chip = match KvmSingleStepMachine::new() {
        Ok(chip) => chip,
        Err(error) => panic!("no single step machine: {error:?}"),
    };
    println!(
        "loading {}, entry {}",
        std::env::var("SINGLE_STEP_LOADING").unwrap_or_else(|_| "full".to_owned()),
        std::env::var("SINGLE_STEP_ENTRY").unwrap_or_else(|_| "plain".to_owned())
    );
    print_load();

    let mut rows: BTreeMap<Class, ClassRow> = BTreeMap::new();
    let mut probe_total = Duration::ZERO;
    let mut probe_states = 0_usize;
    let started = Instant::now();
    for &instruction in &modeled {
        let probe_started = Instant::now();
        let measured = chip.measure_instruction(instruction, &generator);
        let probe = probe_started.elapsed();
        let states = generator.generate_states(instruction).count();
        probe_total += probe;
        probe_states += states;
        let row = rows.entry(to_class(instruction)).or_default();
        row.modeled += 1;
        row.states.push(states);
        row.probe += probe;
        row.product += domain
            .list_operand_types(instruction)
            .map(|operand_type| {
                let mut values: Vec<OperandValue> =
                    domain.generate_critical_values(operand_type).collect();
                for number in domain.generate_distinct_register_numbers(operand_type) {
                    if !values.contains(&number) {
                        values.push(number);
                    }
                }
                values.len() as u128
            })
            .product::<u128>();
        let outcome = match &measured {
            Ok(precise) => format!("measured {precise:?}"),
            Err(MeasureInstructionError::NoKnownBehavior { .. }) => "no known behavior".to_owned(),
            Err(MeasureInstructionError::NoBehaviorAgrees { before, .. }) => {
                format!("NO BEHAVIOR AGREES, first at {}", describe(before, 0))
            }
            Err(MeasureInstructionError::IndistinguishableBehaviors { behaviors, .. }) => {
                format!("INDISTINGUISHABLE {behaviors:?}")
            }
            Err(MeasureInstructionError::ExecutionFailed { before, error, .. }) => {
                format!("CHIP REFUSED {error:?}, first at {}", describe(before, 0))
            }
        };
        println!(
            "probe {instruction:?}: {states} states, {:.0} ns per state, {outcome}",
            probe.as_nanos() as f64 / states.max(1) as f64
        );
        if probe_only {
            if matches!(
                measured,
                Err(MeasureInstructionError::NoBehaviorAgrees { .. }
                    | MeasureInstructionError::ExecutionFailed { .. })
            ) {
                row.disagreeing_instructions += 1;
            }
            continue;
        }
        // Every state again, each part timed, and the states each behavior disagrees on counted.
        let behaviors: Vec<_> = Arm64::<User>::list_known_behaviors(instruction).collect();
        let mut disagreeing = vec![0_usize; behaviors.len()];
        let mut printed: Vec<Vec<String>> = vec![Vec::new(); behaviors.len()];
        let mut split = Split::default();
        let mut states_iter = generator.generate_states(instruction);
        loop {
            let make_started = Instant::now();
            let Some(before) = states_iter.next() else {
                break;
            };
            split.make += make_started.elapsed();
            let simulate_started = Instant::now();
            let simulated: Vec<_> = behaviors
                .iter()
                .map(|precise| {
                    let Ok(after) = precise.simulate(before.clone());
                    after
                })
                .collect();
            split.simulate += simulate_started.elapsed();
            let chip_started = Instant::now();
            let Ok(observed) = chip.execute_single_instruction(before.clone()) else {
                panic!("{instruction:?}: the chip refused a generated state");
            };
            split.chip += chip_started.elapsed();
            let compare_started = Instant::now();
            let agree: Vec<bool> = simulated.iter().map(|after| *after == observed).collect();
            split.compare += compare_started.elapsed();
            for (index, agrees) in agree.into_iter().enumerate() {
                if agrees {
                    continue;
                }
                disagreeing[index] += 1;
                if printed[index].len() < PRINTED_DISAGREEMENTS {
                    let start = before.memory_mutations.len();
                    printed[index].push(format!(
                        "    before    {}\n    simulated {}\n    chip      {}",
                        describe(&before, start),
                        describe(&simulated[index], start),
                        describe(&observed, start)
                    ));
                }
            }
        }
        let kept = (0..behaviors.len())
            .min_by_key(|index| disagreeing[*index])
            .unwrap_or_default();
        let fewest = disagreeing.get(kept).copied().unwrap_or_default();
        row.disagreeing_instructions += usize::from(fewest > 0);
        row.disagreeing_states += fewest;
        row.split.make += split.make;
        row.split.simulate += split.simulate;
        row.split.chip += split.chip;
        row.split.compare += split.compare;
        println!(
            "check {instruction:?}: {states} states, {fewest} disagree with {:?}, chip {:.0} ns per state",
            behaviors.get(kept),
            split.chip.as_nanos() as f64 / states.max(1) as f64
        );
        if let Some(printed) = printed.get(kept) {
            for line in printed {
                println!("{line}");
            }
        }
    }
    let wall = started.elapsed();
    println!(
        "probe: {probe_states} states, {:.3} s, {:.0} ns per state; run {:.3} s wall",
        probe_total.as_secs_f64(),
        probe_total.as_nanos() as f64 / probe_states.max(1) as f64,
        wall.as_secs_f64()
    );
    print_load();

    // Each listed instruction of a class the run covers that has no model, run once at a word LLVM decodes as it.
    for instruction in listed.iter().copied().filter(is_named) {
        let row = rows.entry(to_class(instruction)).or_default();
        row.listed += 1;
        if Arm64::<User>::list_known_behaviors(instruction)
            .next()
            .is_some()
        {
            continue;
        }
        let Some(record) = instruction.to_record() else {
            continue;
        };
        let word = record
            .example
            .unwrap_or(record.fixed_bits.value | record.should_be.value);
        let before = ExecutionState::<Arm64<User>> {
            processor: Processor {
                program_counter: CODE_PAGE.start,
                ..ZERO_PROCESSOR
            },
            memory_mutations: vec![
                MemoryDelta::PermissionAssignment {
                    addresses: CODE_PAGE,
                    permission: PagePermission::Mapped {
                        data_access: DataAccess::ReadOnly,
                        instruction_fetch: InstructionFetch::Allowed,
                    },
                },
                MemoryDelta::ValueAssignment {
                    start: CODE_PAGE.start,
                    bytes: Cow::Owned(word.to_le_bytes().to_vec()),
                },
            ],
        };
        let exception = match chip.execute_single_instruction(before) {
            Ok(after) => after.processor.exception,
            Err(refusal) => panic!("{instruction:?}: the chip refused its word: {refusal:?}"),
        };
        if exception == Some(Exception::UndefinedInstruction) {
            row.absent += 1;
        } else {
            row.present += 1;
        }
        println!(
            "unmodeled {instruction:?} ({}, {:?}) at {word:#010x}: exception {exception:?}",
            record.feature, record.class
        );
    }

    let mut whole = ClassRow::default();
    for (class, row) in &mut rows {
        row.states.sort_unstable();
        let total: usize = row.states.iter().sum();
        let per_state = |time: Duration| time.as_nanos() as f64 / total.max(1) as f64;
        println!(
            "class {class:?}: listed {}, modeled {}, absent {}, present not modeled {}, product {}, cut {total}, median {} largest {}, disagreeing {} instructions {} states, probe {:.0} ns per state {:.3} s, chip {:.0} ns per state {:.3} s, split make {:.0} simulate {:.0} chip {:.0} compare {:.0} ns",
            row.listed,
            row.modeled,
            row.absent,
            row.present,
            row.product,
            row.states
                .get(row.states.len() / 2)
                .copied()
                .unwrap_or_default(),
            row.states.last().copied().unwrap_or_default(),
            row.disagreeing_instructions,
            row.disagreeing_states,
            per_state(row.probe),
            row.probe.as_secs_f64(),
            per_state(row.split.chip),
            row.split.chip.as_secs_f64(),
            per_state(row.split.make),
            per_state(row.split.simulate),
            per_state(row.split.chip),
            per_state(row.split.compare),
        );
        whole.listed += row.listed;
        whole.modeled += row.modeled;
        whole.absent += row.absent;
        whole.present += row.present;
        whole.product += row.product;
        whole.states.extend(row.states.iter().copied());
        whole.disagreeing_instructions += row.disagreeing_instructions;
        whole.disagreeing_states += row.disagreeing_states;
        whole.probe += row.probe;
        whole.split.make += row.split.make;
        whole.split.simulate += row.split.simulate;
        whole.split.chip += row.split.chip;
        whole.split.compare += row.split.compare;
    }
    let total: usize = whole.states.iter().sum();
    let per_state = |time: Duration| time.as_nanos() as f64 / total.max(1) as f64;
    println!(
        "whole: listed {}, modeled {}, absent {}, present not modeled {}, product {}, cut {total}, disagreeing {} instructions {} states, probe {:.0} ns per state {:.3} s, chip {:.0} ns per state {:.3} s, split make {:.0} simulate {:.0} chip {:.0} compare {:.0} ns",
        whole.listed,
        whole.modeled,
        whole.absent,
        whole.present,
        whole.product,
        whole.disagreeing_instructions,
        whole.disagreeing_states,
        per_state(whole.probe),
        whole.probe.as_secs_f64(),
        per_state(whole.split.chip),
        whole.split.chip.as_secs_f64(),
        per_state(whole.split.make),
        per_state(whole.split.simulate),
        per_state(whole.split.chip),
        per_state(whole.split.compare),
    );
    print_load();
}

#[cfg(not(all(target_arch = "aarch64", target_os = "linux")))]
fn main() {
    panic!("this check runs on ARM64 Linux with KVM");
}
