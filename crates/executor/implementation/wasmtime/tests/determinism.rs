//! Which instructions give two different states after when one state is executed twice, and what differs.

use std::thread;

use wcontainers_behavior_source::BehaviorSource;
use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
use wcontainers_distinguishing_state_generator_arm64::CriticalValueCut;
use wcontainers_executor_conformance::AGREEMENT_EXECUTORS;
use wcontainers_executor_wasmtime::{WasmtimeExecutor, WasmtimeTranslatorConfig};
use wcontainers_processor_architecture::ProcessorArchitecture;
use wcontainers_processor_architecture_arm64::{Arm64, User};

#[test]
#[ignore = "a probe over every listed instruction, run on its own: cargo test --release --test determinism -- --ignored --nocapture"]
fn instructions_that_give_two_states() {
    let instructions: Vec<_> = Arm64::<User>::list_instructions().collect();
    let differing: Vec<String> = thread::scope(|scope| {
        let runs: Vec<_> = (0..AGREEMENT_EXECUTORS)
            .map(|share| {
                let instructions = &instructions;
                scope.spawn(move || {
                    let generator = CriticalValueCut::default();
                    let mut executor = WasmtimeExecutor::new(WasmtimeTranslatorConfig::default());
                    let mut differing = Vec::new();
                    for instruction in instructions.iter().skip(share).step_by(AGREEMENT_EXECUTORS)
                    {
                        let mut count = 0;
                        let mut first = None;
                        for before in generator.generate_states(*instruction) {
                            let (Ok(once), Ok(twice)) = (
                                executor.execute_single_instruction(before.clone()),
                                executor.execute_single_instruction(before),
                            ) else {
                                continue;
                            };
                            if once != twice {
                                count += 1;
                                first.get_or_insert_with(|| {
                                    format!("{:?}\n    {:?}", once.processor, twice.processor)
                                });
                            }
                        }
                        if let Some(first) = first {
                            differing.push(format!(
                                "{instruction:?}: {count} states; the first:\n    {first}"
                            ));
                        }
                    }
                    differing
                })
            })
            .collect();
        runs.into_iter()
            .flat_map(|run| run.join().expect("a probe run ends"))
            .collect()
    });
    for line in &differing {
        eprintln!("DIFFERS {line}");
    }
    eprintln!("{} instructions give two states", differing.len());
}
