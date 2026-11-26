//! Measures every listed form on the chip with the provided `measure_instruction`, form by form, and prints for each
//! form its states, its known behaviors and the outcome; where no behavior agrees on a modeled form, the first states
//! on which the closest disagrees, and on a form whose one behavior is #UD, what the chip did. Then it counts the
//! outcomes. With `X86_PROBE_SET` set it measures every form with the provided `measure_instructions`, prints where
//! that stops, builds the set from the forms measured one by one and prints the modeled forms' behaviors.
//!
//! `cargo run --release -p wcontainers-behavior-source-kvm-single-step-x86-64 --example probe`. With `X86_PROBE_FORMS`
//! set to a comma-separated list of form names it runs those alone, and with `X86_PROBE_MODELED` set the modeled forms
//! alone; with `X86_PROBE_TIMING` set to a number of passes it then times every state on the parked machine, the
//! parked machine with incremental loading and the plain one, interleaved; with `X86_PROBE_SPLIT` set it prints where
//! the parked machine spends its time.

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
    use std::cell::Cell;
    use std::time::{Duration, Instant};

    use wcontainers_behavior_source::{BehaviorSource, MeasureInstructionError};
    use wcontainers_behavior_source_kvm_single_step_x86_64::{
        EntryPath, MemoryLoading, Options, Refusal, SingleStepMachine,
    };
    use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
    use wcontainers_distinguishing_state_generator_x86_64::CriticalValueCut;
    use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;
    use wcontainers_precise_instruction_set::PreciseInstructionSet;
    use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
    use wcontainers_processor_architecture::{
        ExecutionState, ExecutionStateExpectation, MemoryDelta, ProcessorArchitecture, Simulator,
    };
    use wcontainers_processor_architecture_x86_64::integer::find_integer_form;
    use wcontainers_processor_architecture_x86_64::{
        Instruction, PreciseInstruction, Processor, ProcessorExpectation, RegisterValueExpectation, X86_64,
        list_modeled_forms,
    };

    const GENERATOR: CriticalValueCut = CriticalValueCut::new(X86_64InputDomain);
    /// The chosen path: full loading, the CPU parked between states, the trap flag.
    const PARKED: Options = Options {
        loading: MemoryLoading::Full,
        entry: EntryPath::Parked,
    };
    /// The chosen path with incremental loading.
    const PARKED_INCREMENTAL: Options = Options {
        loading: MemoryLoading::Incremental,
        entry: EntryPath::Parked,
    };

    type State = ExecutionState<'static, X86_64>;
    type Expectation = ExecutionStateExpectation<'static, X86_64>;

    /// The generator, counting the states it gives.
    struct Counted {
        states: Cell<usize>,
    }

    impl DistinguishingStateGenerator<X86_64> for Counted {
        type States<'generator> = Box<dyn Iterator<Item = State> + 'generator>;

        fn generate_states(&self, instruction: Instruction) -> Self::States<'_> {
            Box::new(GENERATOR.generate_states(instruction).inspect(|_| self.states.set(self.states.get() + 1)))
        }
    }

    fn read_load() -> String {
        std::fs::read_to_string("/proc/loadavg")
            .map(|load| load.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
            .unwrap_or_default()
    }

    fn make_machine(options: Options) -> SingleStepMachine {
        SingleStepMachine::with_options(options).unwrap_or_else(|error| panic!("no single step machine: {error:?}"))
    }

    /// The state after, or the refusal's text in its place.
    fn execute(machine: &mut SingleStepMachine, before: &State) -> Result<State, String> {
        machine.execute_single_instruction(before.clone()).map_err(|refusal| format!("{refusal:?}"))
    }

    fn simulate(behavior: PreciseInstruction, before: &State) -> Expectation {
        let Ok(simulated) = behavior.simulate_state(before.clone());
        simulated
    }

    /// The registers, and of the vector registers those that hold other than the complement of their number.
    fn render(processor: &Processor) -> String {
        let vectors: Vec<String> = processor
            .vector_registers
            .iter()
            .enumerate()
            .filter(|(number, value)| **value != !(*number as u128))
            .map(|(number, value)| format!("xmm{number} {value:032x}"))
            .collect();
        format!(
            "{:x?} ip {:#x} {:?} {:?} fs {:#x} gs {:#x} mxcsr {:#x} {}",
            processor.general_purpose_registers,
            processor.instruction_pointer,
            processor.flags,
            processor.exception,
            processor.fs_base,
            processor.gs_base,
            processor.mxcsr,
            vectors.join(" "),
        ) + &render_x87(&processor.x87)
    }

    /// The x87 state, when it is not the initial one.
    fn render_x87(x87: &wcontainers_processor_architecture_x86_64::X87) -> String {
        if *x87 == wcontainers_processor_architecture_x86_64::X87_INITIAL {
            return String::new();
        }
        format!(
            " x87 fcw {:#x} fsw {:#x} tags {:#x} fop {:#x} fip {:#x} fdp {:#x} {:x?}",
            x87.control, x87.status, x87.tags, x87.opcode, x87.instruction_pointer, x87.data_pointer, x87.registers
        )
    }

    /// The expected registers as `render` prints a processor, a register not compared as `?`.
    fn render_expected(processor: &ProcessorExpectation) -> String {
        let registers: Vec<String> = processor
            .general_purpose_registers
            .iter()
            .map(|register| match register {
                RegisterValueExpectation::Deterministic(value) => format!("{value:x}"),
                RegisterValueExpectation::NonDeterministic => "?".to_owned(),
            })
            .collect();
        let vectors: Vec<String> = processor
            .vector_registers
            .iter()
            .enumerate()
            .filter(|(number, value)| **value != !(*number as u128))
            .map(|(number, value)| format!("xmm{number} {value:032x}"))
            .collect();
        format!(
            "[{}] ip {:#x} {:?} {:?} fs {:#x} gs {:#x} mxcsr {:#x} {}",
            registers.join(", "),
            processor.instruction_pointer,
            processor.flags,
            processor.exception,
            processor.fs_base,
            processor.gs_base,
            processor.mxcsr,
            vectors.join(" "),
        ) + &render_x87(&processor.x87)
    }

    /// Each ZMM register, all 512 bits, and each opmask register, by name.
    fn list_avx512(
        low: &[u128; 16],
        middle: &[u128; 16],
        upper: &[[u128; 2]; 16],
        high: &[[u128; 4]; 16],
        opmask: &[u64; 8],
    ) -> Vec<(String, String)> {
        let zmm = (0..32).map(|number| {
            let [first, second, third, fourth] =
                if number < 16 { [low[number], middle[number], upper[number][0], upper[number][1]] } else { high[number - 16] };
            (format!("zmm{number}"), format!("{fourth:032x}_{third:032x}_{second:032x}_{first:032x}"))
        });
        zmm.chain(opmask.iter().enumerate().map(|(number, value)| (format!("k{number}"), format!("{value:016x}")))).collect()
    }

    /// The ZMM and opmask registers where the simulation and the chip differ: before, simulated, executed.
    fn render_avx512_differences(before: &Processor, simulated: &ProcessorExpectation, executed: Option<&Processor>) -> String {
        let Some(executed) = executed else {
            return String::new();
        };
        let of = |processor: &Processor| {
            list_avx512(
                &processor.vector_registers,
                &processor.upper_vector_halves,
                &processor.zmm_upper_halves,
                &processor.high_vector_registers,
                &processor.opmask_registers,
            )
        };
        let expected = list_avx512(
            &simulated.vector_registers,
            &simulated.upper_vector_halves,
            &simulated.zmm_upper_halves,
            &simulated.high_vector_registers,
            &simulated.opmask_registers,
        );
        of(before)
            .into_iter()
            .zip(expected)
            .zip(of(executed))
            .filter(|((_, (_, simulated)), (_, executed))| simulated != executed)
            .map(|(((name, before), (_, simulated)), (_, executed))| {
                format!("\n    {name}: before {before}\n      simulated {simulated}\n      executed  {executed}")
            })
            .collect()
    }

    fn summarize(changes: &[MemoryDelta<'_, X86_64>]) -> Vec<String> {
        changes
            .iter()
            .map(|change| match change {
                MemoryDelta::ValueAssignment { start, bytes } => {
                    format!("{start:#x}: {:02x?}", &bytes[..bytes.len().min(16)])
                }
                MemoryDelta::PermissionAssignment { addresses, permission } => {
                    format!("{addresses:#x?}: {permission:?}")
                }
            })
            .collect()
    }

    fn describe(behavior: PreciseInstruction, before: &State, executed: &Result<State, String>) -> String {
        let simulated = simulate(behavior, before);
        let start = before.memory_mutations.len();
        let code = match before.memory_mutations.get(1) {
            Some(MemoryDelta::ValueAssignment { bytes, .. }) => format!("{:02x?}", &bytes[..]),
            _ => String::new(),
        };
        let differences = render_avx512_differences(&before.processor, &simulated.processor, executed.as_ref().ok().map(|state| &state.processor));
        let executed = match executed {
            Ok(executed) => format!(
                "{}\n      {:x?}",
                render(&executed.processor),
                summarize(&executed.memory_mutations[start..])
            ),
            Err(refusal) => format!("refused: {refusal}"),
        };
        format!(
            "    code {code}\n    before:    {}\n      {:x?}\n    simulated: {}\n      {:x?}\n    executed:  {executed}{differences}",
            render(&before.processor),
            summarize(&before.memory_mutations[2.min(start)..]),
            render_expected(&simulated.processor),
            summarize(&simulated.memory_mutations[start..]),
        )
    }

    fn list_forms() -> Vec<Instruction> {
        let chosen = std::env::var("X86_PROBE_FORMS").ok();
        let all: Vec<Instruction> = if std::env::var("X86_PROBE_MODELED").is_ok() {
            list_modeled_forms()
        } else {
            X86_64::list_instructions().collect()
        };
        all.into_iter()
            .filter(|instruction| {
                chosen
                    .as_deref()
                    .is_none_or(|names| names.split(',').any(|name| format!("{instruction:?}") == name))
            })
            .collect()
    }

    fn to_family(instruction: Instruction) -> String {
        match find_integer_form(instruction) {
            Some(form) => format!("{:?}", form.operation),
            None => format!("{instruction:?}"),
        }
    }

    /// Prints, for a form no behavior agrees with, each behavior's disagreements and the first states of the closest.
    fn explain(machine: &mut SingleStepMachine, instruction: Instruction, behaviors: &[PreciseInstruction]) {
        let states: Vec<State> = GENERATOR.generate_states(instruction).collect();
        let executed: Vec<Result<State, String>> = states.iter().map(|state| execute(machine, state)).collect();
        let differs = |behavior: PreciseInstruction, before: &State, after: &Result<State, String>| {
            !after.as_ref().is_ok_and(|after| after.meets_expectations(&simulate(behavior, before)))
        };
        let disagreements: Vec<usize> = behaviors
            .iter()
            .map(|behavior| states.iter().zip(&executed).filter(|(before, after)| differs(*behavior, before, after)).count())
            .collect();
        println!("  DISAGREEING {disagreements:?} of {} states", states.len());
        let Some((closest, _)) = behaviors.iter().zip(&disagreements).min_by_key(|(_, count)| **count) else {
            return;
        };
        for (before, after) in
            states.iter().zip(&executed).filter(|(before, after)| differs(*closest, before, after)).take(4)
        {
            println!("{}", describe(*closest, before, after));
        }
    }

    /// The outcome of measuring one form, as the counts group them.
    fn to_outcome(measured: &Result<PreciseInstruction, MeasureInstructionError<X86_64, Refusal>>) -> &'static str {
        match measured {
            Ok(PreciseInstruction::Undefined(_)) => "behavior: undefined instruction",
            Ok(_) => "behavior: modeled",
            Err(MeasureInstructionError::NoKnownBehavior { .. }) => "no known behavior",
            Err(MeasureInstructionError::NoBehaviorAgrees { .. }) => "no behavior agrees",
            Err(MeasureInstructionError::ExecutionFailed { .. }) => "refused by the chip source",
            Err(MeasureInstructionError::IndistinguishableBehaviors { .. }) => "behaviors indistinguishable",
        }
    }

    /// Prints what the chip did on the one state of a form whose one known behavior is #UD and that the chip runs.
    fn report_chip_runs(instruction: Instruction, observed: &State) {
        println!(
            "  CHIP RUNS {instruction:?}: exception {:?} ip {:#x}",
            observed.processor.exception, observed.processor.instruction_pointer
        );
    }

    pub fn main() {
        println!("load at start: {}", read_load());
        let forms = list_forms();
        let mut machine = make_machine(PARKED);
        let counted = Counted { states: Cell::new(0) };
        let (mut total_states, mut agreeing, mut several, mut time_measuring) = (0, 0, 0, Duration::ZERO);
        let mut outcomes: std::collections::BTreeMap<&'static str, usize> = std::collections::BTreeMap::new();
        for instruction in &forms {
            counted.states.set(0);
            let behaviors: Vec<PreciseInstruction> = X86_64::list_known_behaviors(*instruction).collect();
            let started = Instant::now();
            let measured = machine.measure_instruction(*instruction, &counted);
            let elapsed = started.elapsed();
            time_measuring += elapsed;
            let states = counted.states.get();
            total_states += states;
            several += usize::from(behaviors.len() > 1);
            *outcomes.entry(to_outcome(&measured)).or_default() += 1;
            let verdict = match &measured {
                Ok(behavior) => {
                    agreeing += 1;
                    let rank = behaviors.iter().position(|known| known == behavior).unwrap_or(usize::MAX);
                    format!("measured behavior {rank}")
                }
                Err(MeasureInstructionError::NoBehaviorAgrees { .. }) => "no behavior agrees".to_owned(),
                Err(MeasureInstructionError::NoKnownBehavior { .. }) => "no known behavior".to_owned(),
                Err(MeasureInstructionError::ExecutionFailed { error, .. }) => format!("refused: {error:?}"),
                Err(MeasureInstructionError::IndistinguishableBehaviors { behaviors, .. }) => {
                    format!("{} behaviors indistinguishable", behaviors.len())
                }
            };
            println!(
                "FORM {instruction:?} | {} | states {states} | behaviors {} | {verdict} | {} ns",
                to_family(*instruction),
                behaviors.len(),
                elapsed.as_nanos(),
            );
            let only_undefined = behaviors == [PreciseInstruction::Undefined(*instruction)];
            match &measured {
                Err(MeasureInstructionError::NoBehaviorAgrees { observed, .. }) if only_undefined => {
                    report_chip_runs(*instruction, observed);
                }
                Err(MeasureInstructionError::NoBehaviorAgrees { .. } | MeasureInstructionError::IndistinguishableBehaviors { .. }) => {
                    explain(&mut machine, *instruction, &behaviors);
                }
                _ => {}
            }
        }
        println!(
            "{agreeing} of {} forms show one known behavior on every state; {several} have more than one; {total_states} states; measuring took {:.3} s, {:.0} ns per state",
            forms.len(),
            time_measuring.as_secs_f64(),
            time_measuring.as_nanos() as f64 / total_states as f64
        );
        for (outcome, count) in &outcomes {
            println!("OUTCOME {outcome}: {count}");
        }
        if std::env::var("X86_PROBE_SET").is_ok() {
            measure_set();
        }
        if let Some(passes) = std::env::var("X86_PROBE_TIMING").ok().and_then(|passes| passes.parse().ok()) {
            time_paths(&forms, passes);
        }
        if std::env::var("X86_PROBE_SPLIT").is_ok() {
            split_time(&forms);
        }
        println!("load at end: {}", read_load());
    }

    /// Measures every form with the provided `measure_instructions` and prints where it stops; then builds the set
    /// from the forms that measure one by one and prints each modeled form's behavior.
    fn measure_set() {
        let started = Instant::now();
        match make_machine(PARKED).measure_instructions(&GENERATOR) {
            Ok(measurements) => println!("SET measure_instructions: every one of {} forms", measurements.len()),
            Err(error) => {
                let (kind, instruction) = match &error {
                    MeasureInstructionError::NoKnownBehavior { instruction } => ("no known behavior", instruction),
                    MeasureInstructionError::ExecutionFailed { instruction, .. } => ("refused by the chip source", instruction),
                    MeasureInstructionError::NoBehaviorAgrees { instruction, .. } => ("no behavior agrees", instruction),
                    MeasureInstructionError::IndistinguishableBehaviors { instruction, .. } => {
                        ("behaviors indistinguishable", instruction)
                    }
                };
                let position = X86_64::list_instructions().position(|listed| listed == *instruction).unwrap_or(usize::MAX);
                println!("SET measure_instructions: stops at listed form {position}, {instruction:?}: {kind}");
            }
        }
        println!("SET measure_instructions took {:.3} s", started.elapsed().as_secs_f64());
        let mut machine = make_machine(PARKED);
        let measurements: Vec<(Instruction, PreciseInstruction)> = X86_64::list_instructions()
            .filter_map(|instruction| machine.measure_instruction(instruction, &GENERATOR).ok().map(|precise| (instruction, precise)))
            .collect();
        let set = MeasuredInstructionSet::<X86_64>::from_measurements(&measurements);
        println!("SET built from {} measurements in {:.3} s", measurements.len(), started.elapsed().as_secs_f64());
        for instruction in list_modeled_forms() {
            println!("CHOSEN {instruction:?} {:?}", set.to_precise(instruction));
        }
    }

    /// Times every state on the parked and the plain machine, the two interleaved pass by pass, generating each
    /// form's states outside the timing; prints each form's median on the parked machine, then the totals.
    fn time_paths(forms: &[Instruction], passes: usize) {
        let mut machines = [
            ("parked", make_machine(PARKED)),
            ("parked incremental", make_machine(PARKED_INCREMENTAL)),
            ("plain", make_machine(Options::PLAIN)),
        ];
        let mut times: Vec<Vec<Duration>> = vec![Vec::new(); machines.len()];
        let mut form_times: Vec<Vec<Duration>> = vec![Vec::new(); forms.len()];
        let (mut count, mut refused) = (0, 0);
        for pass in 0..passes {
            let mut pass_times = vec![Duration::ZERO; machines.len()];
            count = 0;
            for (instruction, form_time) in forms.iter().zip(&mut form_times) {
                let states: Vec<State> = GENERATOR.generate_states(*instruction).collect();
                count += states.len();
                for (index, ((_, machine), time)) in machines.iter_mut().zip(&mut pass_times).enumerate() {
                    let started = Instant::now();
                    for state in &states {
                        refused += usize::from(machine.execute_single_instruction(state.clone()).is_err());
                    }
                    let elapsed = started.elapsed();
                    *time += elapsed;
                    if index == 0 {
                        form_time.push(elapsed);
                    }
                }
            }
            for (all, time) in times.iter_mut().zip(pass_times) {
                all.push(time);
            }
            println!("timing pass {pass}: load {}; refused so far {refused}", read_load());
        }
        for (instruction, mut all) in forms.iter().zip(form_times) {
            all.sort();
            println!(
                "FORMTIME {instruction:?} | {} | parked median {} ns",
                to_family(*instruction),
                all[all.len() / 2].as_nanos()
            );
        }
        for ((label, _), mut all) in machines.iter().zip(times) {
            all.sort();
            let median = all[all.len() / 2];
            println!(
                "TIMING {label}: {count} states, median of {passes} passes {:.3} s, {:.0} ns per state; passes {:?}",
                median.as_secs_f64(),
                median.as_nanos() as f64 / count as f64,
                all.iter().map(|time| format!("{:.3}", time.as_secs_f64())).collect::<Vec<_>>(),
            );
        }
    }

    /// Prints where the parked machine spends a state's time: loading, setting registers, in the guest, reading back,
    /// and the rest; then a bare halt's KVM_RUN alone.
    fn split_time(forms: &[Instruction]) {
        let mut machine = make_machine(PARKED);
        let states: Vec<State> = forms.iter().flat_map(|instruction| GENERATOR.generate_states(*instruction)).collect();
        let mut refused = 0;
        for state in &states {
            refused += usize::from(machine.execute_single_instruction(state.clone()).is_err());
        }
        machine.reset_times();
        let started = Instant::now();
        for state in &states {
            refused += usize::from(machine.execute_single_instruction(state.clone()).is_err());
        }
        let whole = started.elapsed();
        let count = states.len() as f64;
        let per_state = |time: Duration| time.as_nanos() as f64 / count;
        let parts = [
            ("loading", machine.time_loading),
            ("setting registers", machine.time_setting_registers),
            ("in the guest", machine.time_in_guest),
            ("reading back", machine.time_reading_back),
        ];
        let accounted: Duration = parts.iter().map(|(_, time)| *time).sum();
        for (label, time) in parts {
            println!("SPLIT {label}: {:.0} ns", per_state(time));
        }
        println!("SPLIT the rest (checks, decoding, cloning, the hook): {:.0} ns", per_state(whole - accounted));
        println!("SPLIT whole: {:.0} ns per state over {} states, {refused} refused", per_state(whole), states.len());
        let runs = 200_000;
        let bare = machine.time_bare_runs(runs);
        println!("SPLIT a bare halt's KVM_RUN: {:.0} ns", bare.as_nanos() as f64 / f64::from(runs));
    }
}
