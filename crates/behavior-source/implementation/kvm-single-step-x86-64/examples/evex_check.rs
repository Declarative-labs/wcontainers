//! Runs every generated state of the forms `EVEX_CHECK_FORMS` names (comma-separated) on the chip, compares each
//! with the form's first known behavior, and prints per form the states and the disagreeing ones, then each kind of
//! disagreement (the exceptions and the fields that differ) with its count and its first two states, field by field.
//!
//! `cargo run --release -p wcontainers-behavior-source-kvm-single-step-x86-64 --example evex_check`.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() {
    on_chip::main();
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() {
    println!("the x86-64 chip source needs a Linux x86-64 host with KVM");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod on_chip {
    use std::collections::BTreeMap;
    use std::time::Instant;

    use wcontainers_behavior_source::BehaviorSource;
    use wcontainers_behavior_source_kvm_single_step_x86_64::{EntryPath, MemoryLoading, Options, SingleStepMachine};
    use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
    use wcontainers_distinguishing_state_generator_x86_64::CriticalValueCut;
    use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;
    use wcontainers_instruction_input_domain::InstructionInputDomain;
    use wcontainers_processor_architecture::{ExecutionStateExpectation, MemoryDelta, ProcessorArchitecture, Simulator};
    use wcontainers_processor_architecture_x86_64::{
        Instruction, Processor, ProcessorExpectation, RegisterValueExpectation, X86_64,
    };

    const GENERATOR: CriticalValueCut = CriticalValueCut::new(X86_64InputDomain);

    type Expectation = ExecutionStateExpectation<'static, X86_64>;

    fn read_load() -> String {
        std::fs::read_to_string("/proc/loadavg").map(|load| load.split_whitespace().take(3).collect::<Vec<_>>().join(" ")).unwrap_or_default()
    }

    /// Each field where the expectation and the processor differ, as `name expected/observed`.
    fn compare(expected: &ProcessorExpectation, observed: &Processor) -> Vec<(String, String)> {
        let mut differences = Vec::new();
        for (number, (expected, observed)) in expected.general_purpose_registers.iter().zip(&observed.general_purpose_registers).enumerate() {
            if let RegisterValueExpectation::Deterministic(value) = expected
                && value != observed
            {
                differences.push((format!("r{number}"), format!("{value:x}/{observed:x}")));
            }
        }
        let mut field = |name: &str, expected: String, observed: String| {
            if expected != observed {
                differences.push((name.to_owned(), format!("{expected}/{observed}")));
            }
        };
        field("ip", format!("{:#x}", expected.instruction_pointer), format!("{:#x}", observed.instruction_pointer));
        field("flags", format!("{:?}", expected.flags), format!("{:?}", observed.flags));
        field("fs", format!("{:#x}", expected.fs_base), format!("{:#x}", observed.fs_base));
        field("gs", format!("{:#x}", expected.gs_base), format!("{:#x}", observed.gs_base));
        field("mxcsr", format!("{:#x}", expected.mxcsr), format!("{:#x}", observed.mxcsr));
        field("exception", format!("{:?}", expected.exception), format!("{:?}", observed.exception));
        let expected_vectors = to_processor(expected);
        for number in 0..32 {
            let (want, got) = (expected_vectors.read_zmm(number), observed.read_zmm(number));
            if want != got {
                let render = |value: [u128; 4]| value.iter().rev().map(|part| format!("{part:032x}")).collect::<Vec<_>>().join("_");
                differences.push((format!("zmm{number}"), format!("\n        {}\n        {}", render(want), render(got))));
            }
        }
        for (number, (want, got)) in expected.opmask_registers.iter().zip(&observed.opmask_registers).enumerate() {
            if want != got {
                differences.push((format!("k{number}"), format!("{want:x}/{got:x}")));
            }
        }
        differences
    }

    fn to_processor(expected: &ProcessorExpectation) -> Processor {
        Processor {
            general_purpose_registers: [0; 16],
            instruction_pointer: expected.instruction_pointer,
            flags: expected.flags,
            fs_base: expected.fs_base,
            gs_base: expected.gs_base,
            mxcsr: expected.mxcsr,
            vector_registers: expected.vector_registers,
            upper_vector_halves: expected.upper_vector_halves,
            zmm_upper_halves: expected.zmm_upper_halves,
            high_vector_registers: expected.high_vector_registers,
            opmask_registers: expected.opmask_registers,
            x87: expected.x87,
            exception: expected.exception,
            data_segments: expected.data_segments,
            code_segment: expected.code_segment,
            pkru: expected.pkru,
            xcr0: expected.xcr0,
            xinuse: expected.xinuse,
            xsave_tracking: expected.xsave_tracking,
            tile_configuration: expected.tile_configuration,
            tile_data: expected.tile_data.clone(),
            xfd: expected.xfd,
            xfd_err: expected.xfd_err,
        }
    }

    fn summarize(changes: &[MemoryDelta<'_, X86_64>]) -> String {
        let parts: Vec<String> = changes
            .iter()
            .map(|change| match change {
                MemoryDelta::ValueAssignment { start, bytes } => format!("{start:#x}: {:02x?}", &bytes[..bytes.len().min(64)]),
                MemoryDelta::PermissionAssignment { addresses, permission } => format!("{addresses:#x?}: {permission:?}"),
            })
            .collect();
        parts.join("; ")
    }

    pub fn main() {
        println!("load at start: {}", read_load());
        let names = std::env::var("EVEX_CHECK_FORMS").unwrap_or_default();
        let forms: Vec<Instruction> =
            X86_64::list_instructions().filter(|instruction| names.split(',').any(|name| format!("{instruction:?}") == name)).collect();
        if std::env::var("EVEX_CHECK_CODES").is_ok() {
            for instruction in &forms {
                let sample = wcontainers_processor_architecture_x86_64::SAMPLE_ENCODINGS[*instruction as usize];
                let decoded = iced_x86::Decoder::new(64, sample, iced_x86::DecoderOptions::NONE).decode();
                println!("CODE {instruction:?} {:?} {:02x?}", decoded.code(), sample);
            }
            return;
        }
        let options = Options { loading: MemoryLoading::Full, entry: EntryPath::Parked };
        let mut machine = SingleStepMachine::with_options(options).unwrap_or_else(|error| panic!("no machine: {error:?}"));
        let mut kinds: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
        let (mut all_states, mut all_disagreeing, started) = (0, 0, Instant::now());
        for instruction in forms {
            let behavior = X86_64::list_known_behaviors(instruction).next().expect("a modeled behavior");
            let (mut states, mut disagreeing) = (0, 0);
            let form_started = Instant::now();
            for before in GENERATOR.generate_states(instruction) {
                states += 1;
                let start = before.memory_mutations.len();
                let Ok(simulated): Result<Expectation, _> = behavior.simulate_state(before.clone());
                let executed = machine.execute_single_instruction(before.clone());
                let agrees = executed.as_ref().is_ok_and(|after| after.meets_expectations(&simulated));
                if agrees {
                    continue;
                }
                disagreeing += 1;
                let (key, detail) = match &executed {
                    Err(refusal) => (format!("{instruction:?} refused {refusal:?}"), String::new()),
                    Ok(after) => {
                        let differences = compare(&simulated.processor, &after.processor);
                        let names: Vec<&str> = differences.iter().map(|(name, _)| name.as_str()).collect();
                        let memory = summarize(&simulated.memory_mutations[start..]) != summarize(&after.memory_mutations[start..]);
                        let key = format!(
                            "{:?} sim {:?} chip {:?} fields {}{}",
                            instruction,
                            simulated.processor.exception,
                            after.processor.exception,
                            names.join(","),
                            if memory { " memory" } else { "" }
                        );
                        let code = match before.memory_mutations.get(1) {
                            Some(MemoryDelta::ValueAssignment { bytes, .. }) => format!("{:02x?}", &bytes[..]),
                            _ => String::new(),
                        };
                        let fields: Vec<String> = differences.iter().map(|(name, value)| format!("{name} {value}")).collect();
                        let detail = format!(
                            "    code {code} r {:x?} mxcsr {:#x} flags {:?} k {:x?}\n    data {}\n    {}",
                            &before.processor.general_purpose_registers[..8],
                            before.processor.mxcsr,
                            before.processor.flags,
                            before.processor.opmask_registers,
                            summarize(&before.memory_mutations[2.min(start)..]),
                            fields.join("\n    ")
                        );
                        (key, detail)
                    }
                };
                let entry = kinds.entry(key).or_default();
                entry.0 += 1;
                if entry.1.len() < 2 {
                    entry.1.push(detail);
                }
            }
            let counts: Vec<String> = X86_64InputDomain
                .list_operand_types(instruction)
                .map(|operand_type| format!("{}", X86_64InputDomain.generate_critical_values(operand_type).count()))
                .collect();
            println!(
                "FORM {instruction:?} states {states} disagreeing {disagreeing} {} ms values {}",
                form_started.elapsed().as_millis(),
                counts.join("x")
            );
            all_states += states;
            all_disagreeing += disagreeing;
        }
        println!("ALL states {all_states} disagreeing {all_disagreeing} {} ms load {}", started.elapsed().as_millis(), read_load());
        for (key, (count, details)) in &kinds {
            println!("KIND {count} {key}");
            for detail in details {
                println!("{detail}");
            }
        }
    }
}
