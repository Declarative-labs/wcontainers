//! The host branch tally (docs/native.md section 4, "Can the processor keep the count"): the stepping test's
//! functions stepped at every host instruction, at every look-ahead, with the count and without it, with exits and
//! jump tables unlinked and linked and with last targets never made and made as the inside makes them. Each host
//! instruction is put to the guest instruction the position map resolves it to (an entry slot's jump to the
//! instruction that jumped there), and every guest instruction run gets the host branches it ran, in order. The
//! guest's path must be the same in every variant; what the host branches add to the guest's own differs where the
//! translation's own branches depend on the variant. Prints, for every guest instruction, each sequence of host
//! branches it ran and the variants it ran it in. Runs itself as a copy signed with the hypervisor entitlement.
//!
//! `WCONTAINERS_BRANCHES_SEED` and `WCONTAINERS_BRANCHES_STARTS` set the starting states' seed and how many are
//! drawn per case.

#[cfg(target_os = "macos")]
mod common;

#[cfg(target_os = "macos")]
#[path = "../../hardware-virtualization-inside/bin/native-cases/protocol.rs"]
mod protocol;

#[cfg(target_os = "macos")]
mod runner;

#[cfg(target_os = "macos")]
fn main() {
    macos::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("the host branch tally needs Hypervisor.framework; nothing to run on this host");
}

#[cfg(target_os = "macos")]
mod macos {
    use std::collections::BTreeMap;
    use std::time::Instant;

    use wcontainers_arm64::carry::call_word;
    use wcontainers_arm64::{Arm64, decode};
    use wcontainers_guest_cpu::{FlowKind, Guest, InsideCall, Native, Route, counted};
    use wcontainers_translator::LookAhead;
    use wcontainers_translator_test::layout::CODE;

    use crate::common::{be_signed, load_average, setting};
    use crate::runner::Runner;
    use crate::runner::function::{Links, Run};
    use crate::runner::functions::{DEFAULT_SEED, DEFAULT_STARTS, LOOK_AHEADS, RETURNS, functions};

    const SEED_VARIABLE: &str = "WCONTAINERS_BRANCHES_SEED";
    const STARTS_VARIABLE: &str = "WCONTAINERS_BRANCHES_STARTS";
    /// What an entry slot runs that this runner's unmapped slots do not: its one jump, after its moves, when its
    /// target has a piece; an empty slot's `udf`, a call into the inside, when not.
    const SLOT_JUMP: &str = "slot Br";
    const SLOT_EMPTY: &str = "call: empty slot";
    /// What a call into the inside's name starts with: an exception, which an every-branch event does not count.
    const CALL: &str = "call: ";

    /// One way of translating and loading a function.
    #[derive(Clone, Copy)]
    struct Variant {
        look_ahead: LookAhead,
        exact_replay: bool,
        links: Links,
    }

    impl Variant {
        /// A short name: the look-ahead, `c` with the count, then `e`, `t` and `l` for linked exits, linked tables
        /// and last targets made.
        fn label(&self) -> String {
            let look_ahead = match self.look_ahead {
                LookAhead::None => "N".to_string(),
                LookAhead::Bounded(n) => n.to_string(),
                LookAhead::Full => "F".to_string(),
            };
            let flag = |on: bool, c: char| if on { c } else { '-' };
            format!(
                "{look_ahead}{}{}{}{}",
                flag(self.exact_replay, 'c'),
                flag(self.links.exits, 'e'),
                flag(self.links.tables, 't'),
                flag(self.links.last_targets, 'l')
            )
        }
    }

    fn variants() -> Vec<Variant> {
        let mut all = Vec::new();
        for look_ahead in LOOK_AHEADS {
            for exact_replay in [true, false] {
                for (exits, tables) in [(false, false), (true, false), (true, true)] {
                    for last_targets in [false, true] {
                        all.push(Variant {
                            look_ahead,
                            exact_replay,
                            links: Links {
                                exits,
                                tables,
                                last_targets,
                            },
                        });
                    }
                }
            }
        }
        all
    }

    /// Whether a word is a branch, as an every-branch event counts: any direct or indirect transfer, taken or not.
    fn is_branch(word: u32) -> bool {
        let flow = Arm64::flow(&decode(word));
        flow.target.is_some() || !matches!(flow.kind, FlowKind::Plain)
    }

    /// A host word's name when it is a branch or a call into the inside: the decoder's form, or the call.
    fn named(word: u32) -> Option<String> {
        if let Some(call) = InsideCall::ALL.into_iter().find(|&c| call_word(c) == word) {
            return Some(format!("{CALL}{call:?}"));
        }
        if !is_branch(word) {
            return None;
        }
        let form = format!("{:?}", decode(word));
        let name: String = form.chars().take_while(|c| c.is_alphanumeric()).collect();
        Some(match form.find("op: ") {
            Some(at) => format!(
                "{name} {}",
                form[at + "op: ".len()..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric())
                    .collect::<String>()
            ),
            None => name,
        })
    }

    /// One guest instruction as run: its address and the host branches and calls into the inside it ran.
    struct Instance {
        pc: u64,
        host: Vec<String>,
    }

    /// The run's guest instructions, each with what it ran: a host instruction belongs to the guest instruction
    /// whose host code it is, an entry slot's to the jump that went there.
    fn instances(run: &Run, words: &[u32]) -> Vec<Instance> {
        let mut all: Vec<Instance> = Vec::new();
        let mut last = None;
        for landing in &run.landed {
            let name = match landing.word {
                Some(word) => named(word),
                None if guest(words, landing.state.pc).is_some() => Some(SLOT_JUMP.to_string()),
                None => Some(SLOT_EMPTY.to_string()),
            };
            if let Some(owner) = landing.owner
                && last != Some(owner)
            {
                all.push(Instance {
                    pc: owner,
                    host: Vec::new(),
                });
                last = Some(owner);
            }
            if let (Some(name), Some(instance)) = (name, all.last_mut()) {
                instance.host.push(name);
            }
        }
        all
    }

    /// A run's guest path, less the instructions that carry no host code in a longer piece.
    fn path(all: &[Instance], words: &[u32]) -> Vec<u64> {
        all.iter()
            .map(|i| i.pc)
            .filter(|&pc| {
                guest(words, pc)
                    .is_none_or(|(word, ..)| Arm64::route(&decode(word)) != Route::Nothing)
            })
            .collect()
    }

    /// The guest word at `pc` and whether it is a branch and an event of the count.
    fn guest(words: &[u32], pc: u64) -> Option<(u32, bool, bool)> {
        let index = usize::try_from(pc.checked_sub(CODE)? / Arm64::MAX_LENGTH).ok()?;
        let word = *words.get(index)?;
        Some((
            word,
            is_branch(word),
            counted(pc, &Arm64::flow(&decode(word))),
        ))
    }

    /// The totals of one variant's run.
    #[derive(Default)]
    struct Totals {
        guest: u64,
        guest_branches: u64,
        events: u64,
        host_branches: u64,
        calls: u64,
    }

    pub fn main() {
        be_signed();
        let started = Instant::now();
        let seed = setting(SEED_VARIABLE, DEFAULT_SEED);
        let starts = setting(STARTS_VARIABLE, DEFAULT_STARTS);
        let mut runner = Runner::start();
        let mut seeds = fastrand::Rng::with_seed(seed);
        let variants = variants();
        let mut cases = 0;
        let mut differing_paths = 0;
        for draw in 0..starts {
            let draw_seed = if draw == 0 { seed } else { seeds.u64(..) };
            for (name, words, function_cases) in functions(draw_seed) {
                for case in &function_cases {
                    cases += 1;
                    let mut paths: Vec<Vec<u64>> = Vec::new();
                    // (guest address, host sequence) -> instances in each variant.
                    let mut seen: BTreeMap<(u64, Vec<String>), BTreeMap<String, u64>> =
                        BTreeMap::new();
                    let mut lines = Vec::new();
                    for v in &variants {
                        let mut function =
                            runner.load_linked(words, v.look_ahead, v.exact_replay, v.links);
                        let run = runner.run_function(&mut function, case, RETURNS, Some(u64::MAX));
                        runner.unload();
                        let all = instances(&run, words);
                        let mut totals = Totals::default();
                        for i in &all {
                            let (_, branch, event) = guest(words, i.pc).unwrap_or_default();
                            totals.guest += 1;
                            totals.guest_branches += u64::from(branch);
                            totals.events += u64::from(event);
                            totals.calls +=
                                i.host.iter().filter(|h| h.starts_with(CALL)).count() as u64;
                            totals.host_branches +=
                                i.host.iter().filter(|h| !h.starts_with(CALL)).count() as u64;
                            let host: Vec<String> = i
                                .host
                                .iter()
                                .filter(|h| !h.starts_with(CALL))
                                .cloned()
                                .collect();
                            *seen
                                .entry((i.pc, host))
                                .or_default()
                                .entry(v.label())
                                .or_default() += 1;
                        }
                        lines.push(format!(
                            "  {}: {:?}, guest {} branches {} events {}, host branches {} (+{}), calls into the inside {}",
                            v.label(),
                            run.stop,
                            totals.guest,
                            totals.guest_branches,
                            totals.events,
                            totals.host_branches,
                            totals.host_branches - totals.guest_branches,
                            totals.calls
                        ));
                        paths.push(path(&all, words));
                    }
                    let same = paths.windows(2).all(|w| w[0] == w[1]);
                    if !same {
                        differing_paths += 1;
                    }
                    println!(
                        "{name} (starting state {:#x}){}",
                        case.seed,
                        if same { "" } else { ": PATHS DIFFER" }
                    );
                    for line in lines {
                        println!("{line}");
                    }
                    for ((pc, host), counts) in &seen {
                        let (word, branch, event) = guest(words, *pc).unwrap_or_default();
                        let form = format!("{:?}", decode(word));
                        let short: String =
                            form.chars().take_while(|c| c.is_alphanumeric()).collect();
                        let extra = host.len() as i64 - i64::from(branch);
                        let mut by_count: BTreeMap<u64, Vec<&str>> = BTreeMap::new();
                        for (label, n) in counts {
                            by_count.entry(*n).or_default().push(label);
                        }
                        let runs: Vec<String> = by_count
                            .iter()
                            .map(|(n, labels)| format!("{n}x in {}", labels.join(" ")))
                            .collect();
                        println!(
                            "    {:+#x} {short}{}{}: [{}] (+{extra}) {}",
                            pc.wrapping_sub(CODE) as i64,
                            if branch { " branch" } else { "" },
                            if event { " event" } else { "" },
                            host.join(", "),
                            runs.join("; ")
                        );
                    }
                }
            }
        }
        runner.finish();
        println!(
            "host branch tally: {cases} cases over {} variants (seed {seed:#x}, {starts} starting states), {differing_paths} with differing guest paths, in {:?}, load {}",
            variants.len(),
            started.elapsed(),
            load_average()
        );
        assert_eq!(
            differing_paths, 0,
            "the guest's path must not depend on the translation"
        );
    }
}
