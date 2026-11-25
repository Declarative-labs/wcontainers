//! Measures every modeled form on the chip with the provided `measure_instruction`, form by form, and prints for each
//! form its states, its known behaviors and the one measured; where none agrees, the first states on which the closest
//! behavior disagrees. Then it measures the whole set with `MeasuredInstructionSet` and prints each form's behavior.
//!
//! `cargo run --release -p wcontainers-behavior-source-kvm-single-step-x86-64 --example probe`. With `X86_PROBE_FORMS`
//! set to a comma-separated list of form names it runs those alone; with `X86_PROBE_TIMING` set to a number of passes it
//! then times every state on the parked machine and on the plain one, the two interleaved; with `X86_PROBE_SPLIT` set
//! it prints where the parked machine spends its time.

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
        EntryPath, MemoryLoading, Options, SingleStepMachine,
    };
    use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
    use wcontainers_distinguishing_state_generator_x86_64::CriticalValueCut;
    use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;
    use wcontainers_precise_instruction_set::PreciseInstructionSet;
    use wcontainers_precise_instruction_set_measured::MeasuredInstructionSet;
    use wcontainers_processor_architecture::{ExecutionState, MemoryDelta, ProcessorArchitecture, Simulator};
    use wcontainers_processor_architecture_x86_64::integer::find_integer_form;
    use wcontainers_processor_architecture_x86_64::{Instruction, PreciseInstruction, Processor, X86_64};

    const GENERATOR: CriticalValueCut = CriticalValueCut::new(X86_64InputDomain);
    /// The chosen path: full loading, the CPU parked between states, the trap flag.
    const PARKED: Options = Options {
        loading: MemoryLoading::Full,
        entry: EntryPath::Parked,
    };

    type State = ExecutionState<'static, X86_64>;

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

    fn simulate(behavior: PreciseInstruction, before: &State) -> State {
        let Ok(simulated) = behavior.simulate(before.clone());
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
        )
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

    fn describe(behavior: PreciseInstruction, before: &State, executed: &State) -> String {
        let simulated = simulate(behavior, before);
        let start = before.memory_mutations.len();
        let code = match before.memory_mutations.get(1) {
            Some(MemoryDelta::ValueAssignment { bytes, .. }) => format!("{:02x?}", &bytes[..]),
            _ => String::new(),
        };
        format!(
            "    code {code}\n    before:    {}\n      {:x?}\n    simulated: {}\n      {:x?}\n    executed:  {}\n      {:x?}",
            render(&before.processor),
            summarize(&before.memory_mutations[2.min(start)..]),
            render(&simulated.processor),
            summarize(&simulated.memory_mutations[start..]),
            render(&executed.processor),
            summarize(&executed.memory_mutations[start..]),
        )
    }

    fn list_forms() -> Vec<Instruction> {
        let chosen = std::env::var("X86_PROBE_FORMS").ok();
        X86_64::list_instructions()
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
        let executed: Vec<State> = states.iter().map(|state| machine.execute_single_instruction(state.clone())).collect();
        let disagreements: Vec<usize> = behaviors
            .iter()
            .map(|behavior| {
                states.iter().zip(&executed).filter(|(before, after)| simulate(*behavior, before) != **after).count()
            })
            .collect();
        println!("  DISAGREEING {disagreements:?} of {} states", states.len());
        let Some((closest, _)) = behaviors.iter().zip(&disagreements).min_by_key(|(_, count)| **count) else {
            return;
        };
        for (before, after) in
            states.iter().zip(&executed).filter(|(before, after)| simulate(*closest, before) != **after).take(4)
        {
            println!("{}", describe(*closest, before, after));
        }
    }

    pub fn main() {
        println!("load at start: {}", read_load());
        let forms = list_forms();
        let mut machine = make_machine(PARKED);
        let counted = Counted { states: Cell::new(0) };
        let (mut total_states, mut agreeing, mut several, mut time_measuring) = (0, 0, 0, Duration::ZERO);
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
            let verdict = match &measured {
                Ok(behavior) => {
                    agreeing += 1;
                    let rank = behaviors.iter().position(|known| known == behavior).unwrap_or(usize::MAX);
                    format!("measured behavior {rank}")
                }
                Err(MeasureInstructionError::NoBehaviorAgrees { .. }) => "no behavior agrees".to_owned(),
                Err(MeasureInstructionError::NoKnownBehavior { .. }) => "no known behavior".to_owned(),
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
            if measured.is_err() {
                explain(&mut machine, *instruction, &behaviors);
            }
        }
        println!(
            "{agreeing} of {} forms show one known behavior on every state; {several} have more than one; {total_states} states; measuring took {:.3} s, {:.0} ns per state",
            forms.len(),
            time_measuring.as_secs_f64(),
            time_measuring.as_nanos() as f64 / total_states as f64
        );
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

    /// Measures the whole set with `MeasuredInstructionSet` and prints each form's behavior.
    fn measure_set() {
        let started = Instant::now();
        let measured = <MeasuredInstructionSet<SingleStepMachine, CriticalValueCut>>::measure(make_machine(PARKED));
        let Ok(set) = measured else {
            panic!("the measured set fails on this chip");
        };
        println!("SET measured in {:.3} s", started.elapsed().as_secs_f64());
        for instruction in X86_64::list_instructions() {
            println!("CHOSEN {instruction:?} {:?}", set.to_precise(instruction));
        }
    }

    /// Times every state on the parked and the plain machine, the two interleaved pass by pass, generating each
    /// form's states outside the timing; prints each form's median on the parked machine, then the totals.
    fn time_paths(forms: &[Instruction], passes: usize) {
        let mut machines = [("parked", make_machine(PARKED)), ("plain", make_machine(Options::PLAIN))];
        let mut times: Vec<Vec<Duration>> = vec![Vec::new(); machines.len()];
        let mut form_times: Vec<Vec<Duration>> = vec![Vec::new(); forms.len()];
        let mut count = 0;
        for pass in 0..passes {
            let mut pass_times = vec![Duration::ZERO; machines.len()];
            count = 0;
            for (instruction, form_time) in forms.iter().zip(&mut form_times) {
                let states: Vec<State> = GENERATOR.generate_states(*instruction).collect();
                count += states.len();
                for (index, ((_, machine), time)) in machines.iter_mut().zip(&mut pass_times).enumerate() {
                    let started = Instant::now();
                    for state in &states {
                        machine.execute_single_instruction(state.clone());
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
            println!("timing pass {pass}: load {}", read_load());
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
        for state in &states {
            machine.execute_single_instruction(state.clone());
        }
        machine.reset_times();
        let started = Instant::now();
        for state in &states {
            machine.execute_single_instruction(state.clone());
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
        println!("SPLIT whole: {:.0} ns per state over {} states", per_state(whole), states.len());
        let runs = 200_000;
        let bare = machine.time_bare_runs(runs);
        println!("SPLIT a bare halt's KVM_RUN: {:.0} ns", bare.as_nanos() as f64 / f64::from(runs));
    }
}
