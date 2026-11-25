//! Runs every planted bug of the architecture's `planted::PLANTED_BUGS` against the chip: each modeled form's
//! generated states run on the chip once, then each bug's simulation of this chip's behavior is compared with the
//! chip on every state of every form of the bug's family. Prints the states and forms that catch each bug, the bugs
//! no state catches, and a greedy cover: form by form, the fewest states that keep every pair of known behaviors
//! the form's states tell apart told apart and every bug they catch caught.
//!
//! `cargo run --release --features planted-bugs -p wcontainers-behavior-source-kvm-single-step-x86-64 --example
//! planted`. The chip's behavior of each form is the one the provided `measure_instruction` measures with no bug on. `X86_PROBE_FORMS` restricts the forms and `X86_PLANTED_BUGS` the bugs, each a comma-separated list of
//! names.

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
    use std::collections::{BTreeMap, BinaryHeap};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use wcontainers_behavior_source::BehaviorSource;
    use wcontainers_behavior_source_kvm_single_step_x86_64::{
        EntryPath, MemoryLoading, Options, SingleStepMachine, Stepping,
    };
    use wcontainers_distinguishing_state_generator::DistinguishingStateGenerator;
    use wcontainers_distinguishing_state_generator_x86_64::CriticalValueCut;
    use wcontainers_instruction_input_domain_x86_64::X86_64InputDomain;
    use wcontainers_processor_architecture::{ExecutionState, ProcessorArchitecture, Simulator};
    use wcontainers_processor_architecture_x86_64::planted::{PLANTED_BUGS, PlantedBug, activate};
    use wcontainers_processor_architecture_x86_64::{Instruction, PreciseInstruction, X86_64};

    type State = ExecutionState<'static, X86_64>;

    const GENERATOR: CriticalValueCut = CriticalValueCut::new(X86_64InputDomain);
    const PARKED: Options = Options {
        loading: MemoryLoading::Full,
        entry: EntryPath::Parked,
        stepping: Stepping::TrapFlag,
    };
    const THREADS: usize = 32;

    fn read_load() -> String {
        std::fs::read_to_string("/proc/loadavg")
            .map(|load| load.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
            .unwrap_or_default()
    }

    fn is_same(_before: &State, first: &State, second: &State) -> bool {
        first == second
    }

    /// The behavior's result, or `None` where its simulation panics, as a planted bug's may.
    fn simulate(behavior: PreciseInstruction, before: &State) -> Option<State> {
        catch_unwind(AssertUnwindSafe(|| {
            let Ok(simulated) = behavior.simulate(before.clone());
            simulated
        }))
        .ok()
    }

    /// The indices of the states on which the behavior's simulation differs from the chip.
    fn find_disagreements(behavior: PreciseInstruction, states: &[State], executed: &[State]) -> Vec<usize> {
        let chunk = states.len().div_ceil(THREADS).max(1);
        std::thread::scope(|scope| {
            let handles: Vec<_> = states
                .chunks(chunk)
                .zip(executed.chunks(chunk))
                .enumerate()
                .map(|(part, (befores, afters))| {
                    scope.spawn(move || {
                        befores
                            .iter()
                            .zip(afters)
                            .enumerate()
                            .filter(|(_, (before, after))| {
                                simulate(behavior, before).is_none_or(|simulated| !is_same(before, &simulated, after))
                            })
                            .map(|(index, _)| part * chunk + index)
                            .collect::<Vec<usize>>()
                    })
                })
                .collect();
            handles.into_iter().flat_map(|handle| handle.join().expect("a judging thread")).collect()
        })
    }

    /// For each state, each behavior's group: behaviors in one group give the same result on it.
    fn partition(behaviors: &[PreciseInstruction], states: &[State]) -> Vec<Vec<u16>> {
        let chunk = states.len().div_ceil(THREADS).max(1);
        std::thread::scope(|scope| {
            let handles: Vec<_> = states
                .chunks(chunk)
                .map(|part| {
                    scope.spawn(move || {
                        part.iter()
                            .map(|before| {
                                let mut representatives: Vec<State> = Vec::new();
                                behaviors
                                    .iter()
                                    .map(|behavior| {
                                        let after = simulate(*behavior, before).expect("a known behavior simulates");
                                        let group = representatives.iter().position(|other| is_same(before, other, &after));
                                        group.unwrap_or_else(|| {
                                            representatives.push(after);
                                            representatives.len() - 1
                                        }) as u16
                                    })
                                    .collect::<Vec<u16>>()
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles.into_iter().flat_map(|handle| handle.join().expect("a partitioning thread")).collect()
        })
    }

    /// The pairs of behaviors a state's groups tell apart, as indices into a square of side `count`.
    fn list_pairs(groups: &[u16]) -> Vec<usize> {
        let count = groups.len();
        let mut pairs = Vec::new();
        for first in 0..count {
            for second in first + 1..count {
                if groups[first] != groups[second] {
                    pairs.push(first * count + second);
                }
            }
        }
        pairs
    }

    /// The size of a greedy cover of the requirements: each state's pairs, then the bugs it catches, numbered after
    /// every pair.
    fn count_cover(requirements: &[Vec<usize>], universe: usize) -> usize {
        let mut uncovered = vec![false; universe];
        for list in requirements {
            for &requirement in list {
                uncovered[requirement] = true;
            }
        }
        let gain = |index: usize, uncovered: &[bool]| requirements[index].iter().filter(|&&r| uncovered[r]).count();
        let mut heap: BinaryHeap<(usize, usize)> =
            (0..requirements.len()).map(|index| (gain(index, &uncovered), index)).collect();
        let mut kept = 0;
        while let Some((stale, index)) = heap.pop() {
            if stale == 0 {
                break;
            }
            let fresh = gain(index, &uncovered);
            if heap.peek().is_some_and(|(next, _)| fresh < *next) {
                heap.push((fresh, index));
                continue;
            }
            if fresh == 0 {
                break;
            }
            kept += 1;
            for &requirement in &requirements[index] {
                uncovered[requirement] = false;
            }
        }
        kept
    }

    fn list_chosen(variable: &str) -> Option<Vec<String>> {
        std::env::var(variable).ok().map(|names| names.split(',').map(str::to_string).collect())
    }

    /// What one bug's runs found.
    #[derive(Default)]
    struct Catches {
        family_forms: usize,
        states: usize,
        forms: Vec<String>,
    }

    pub fn main() {
        println!("load at start: {}", read_load());
        let chosen_forms = list_chosen("X86_PROBE_FORMS");
        let chosen_bugs = list_chosen("X86_PLANTED_BUGS");
        let forms: Vec<Instruction> = X86_64::list_instructions()
            .filter(|instruction| chosen_forms.as_ref().is_none_or(|names| names.contains(&format!("{instruction:?}"))))
            .collect();
        let bugs: Vec<&PlantedBug> = PLANTED_BUGS
            .iter()
            .filter(|row| chosen_bugs.as_ref().is_none_or(|names| names.contains(&format!("{:?}", row.bug))))
            .collect();
        let mut machine = SingleStepMachine::with_options(PARKED).unwrap_or_else(|error| panic!("no machine: {error:?}"));
        let mut catches: BTreeMap<usize, Catches> = BTreeMap::new();
        let (mut total_states, mut clean_disagreements, mut total_cover, mut total_requirements) = (0, 0, 0, 0);
        for &instruction in &forms {
            let states: Vec<State> = GENERATOR.generate_states(instruction).collect();
            let Ok(chosen) = machine.measure_instruction(instruction, &GENERATOR) else {
                panic!("{instruction:?} shows no one known behavior on this chip");
            };
            let executed: Vec<State> = states.iter().map(|state| machine.execute_single_instruction(state.clone())).collect();
            let clean = find_disagreements(chosen, &states, &executed).len();
            let behaviors: Vec<PreciseInstruction> = X86_64::list_known_behaviors(instruction).collect();
            let groups = partition(&behaviors, &states);
            let universe = behaviors.len() * behaviors.len();
            let mut requirements: Vec<Vec<usize>> = groups.iter().map(|groups| list_pairs(groups)).collect();
            let pairs_told_apart = {
                let mut seen = vec![false; universe];
                requirements.iter().flatten().for_each(|&pair| seen[pair] = true);
                seen.iter().filter(|seen| **seen).count()
            };
            let mut bugs_caught = 0;
            for (number, row) in bugs.iter().enumerate() {
                if !row.contains(instruction) {
                    continue;
                }
                activate(Some(row.bug));
                let caught = find_disagreements(chosen, &states, &executed);
                activate(None);
                let entry = catches.entry(number).or_default();
                entry.family_forms += 1;
                if !caught.is_empty() {
                    bugs_caught += 1;
                    entry.states += caught.len();
                    entry.forms.push(format!("{instruction:?}"));
                }
                for index in caught {
                    requirements[index].push(universe + number);
                }
            }
            let cover = count_cover(&requirements, universe + bugs.len());
            total_states += states.len();
            clean_disagreements += clean;
            total_cover += cover;
            total_requirements += pairs_told_apart + bugs_caught;
            println!(
                "FORM {instruction:?} | states {} | chosen disagrees {clean} | pairs told apart {pairs_told_apart} of {} | bugs caught {bugs_caught} | cover {cover}",
                states.len(),
                behaviors.len() * (behaviors.len() - 1) / 2,
            );
        }
        let mut missed = Vec::new();
        for (number, row) in bugs.iter().enumerate() {
            let found = catches.remove(&number).unwrap_or_default();
            if found.states == 0 {
                missed.push(format!("{:?}", row.bug));
            }
            println!(
                "BUG {:?} | {} | {:?} | family forms {} | states {} | forms {} | {}",
                row.bug,
                row.family_name,
                row.origin,
                found.family_forms,
                found.states,
                found.forms.len(),
                found.forms.iter().take(3).cloned().collect::<Vec<_>>().join(","),
            );
        }
        println!(
            "TOTAL forms {} | states {total_states} | chosen disagreements {clean_disagreements} | bugs caught {} of {} | missed {missed:?} | requirements {total_requirements} | cover {total_cover}",
            forms.len(),
            bugs.len() - missed.len(),
            bugs.len(),
        );
        println!("load at end: {}", read_load());
    }
}
