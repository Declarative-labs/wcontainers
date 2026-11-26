// Written once for both guests; each guest's module in `changes.rs` includes it.

pub(super) fn plan() -> impl Strategy<Value = Plan> {
    (
        any::<u64>(),
        prop::sample::select(vec![1u32, 4, 32, 256]),
        prop::sample::select(vec![1.0, 1.0, 1.0, 0.5, 0.1, 0.0]),
    )
        .prop_map(|(seed, every, stops)| Plan { seed, every, stops })
}
/// Counts of what the runs did, printed so a run says how strong it was.
#[derive(Default)]
pub(super) struct Tally {
    ran: u64,
    changes: u64,
    handled: u64,
    faulted: u64,
    translated: u64,
    left: u64,
}
impl Tally {
    pub(super) fn add(&mut self, compared: Compared) {
        self.ran += 1;
        self.changes += compared.changes as u64;
        self.handled += compared.handled as u64;
        self.translated += compared.translated;
        self.left += compared.left;
        if matches!(compared.stopped, Stopped::Exit(Exit::MemoryFault { .. })) {
            self.faulted += 1;
        }
    }

    fn report(&self, what: &str) {
        eprintln!(
            "changes: {} {what}; {} changes made, {} faults handled, {} ended by a fault; at full {} pieces \
             translated at run time, {} instructions run alone after a check failed",
            self.ran, self.changes, self.handled, self.faulted, self.translated, self.left
        );
    }
}
pub(super) fn random_programs_with_changes_match_none() {
    let cases = knob("WC_CHANGES_CASES", 64);
    let seed = knob("WC_CHANGES_SEED", 1);
    let mut runner = TestRunner::new_with_rng(
        Config {
            cases: cases as u32,
            failure_persistence: None,
            ..Config::default()
        },
        generator(seed, &[], ""),
    );
    let tally = std::cell::RefCell::new(Tally::default());
    let result = runner.run(&(program(), plan()), |(program, plan)| {
        let blob = part::assemble(&program);
        tally.borrow_mut().add(compare(&blob, plan));
        Ok(())
    });
    tally.borrow().report(&format!(
        "random programs (shrinking included), seed {seed}"
    ));
    if let Err(error) = result {
        panic!("{error}");
    }
}
pub(super) fn programs_with_changes_match_none() {
    let seeds = knob("WC_CHANGES_PROGRAM_SEEDS", 3);
    let mut tally = Tally::default();
    for (name, optimisation) in [
        ("walks", "-O0"),
        ("walks", "-O2"),
        ("integers", "-O2"),
        ("tables", "-O2"),
        ("bench", "-O0"),
        ("bench", "-O2"),
    ] {
        let blob = build(name, optimisation, Library::None);
        for seed in 0..seeds {
            for every in [16, 1024] {
                let plan = Plan {
                    seed,
                    every,
                    stops: 1.0,
                };
                eprintln!("{name} {optimisation}: {plan:?}");
                tally.add(compare(&blob, plan));
            }
        }
    }
    tally.report("program runs");
}
/// At the first system call, or at the first stop where the change's pc and field are reached, the change's
/// page is made to allow nothing; full must fault exactly as none does. With a stop, none stops at every
/// instruction and full at every stop check up to that point.
pub(super) fn denied_at_the_first(change: &Change) {
    let Change { blob, at, page } = change;
    let (at, page) = (*at, *page);
    let run = |look_ahead| {
        let mut machine = ready(blob, look_ahead, Omit::default());
        let mut first = true;
        machine.on_pause(move |pause, paused| {
            let reached = match at {
                None => pause == Pause::Syscall,
                Some((pc, field, value)) => {
                    let state = fields(&paused.registers()).0;
                    let reached = pause == Pause::Stop
                        && state[part::PC_FIELD] == u128::from(pc)
                        && state[field] == value;
                    // Full stops at every check until the point, then runs on without stopping.
                    if first && !reached {
                        paused.raise_stop_flag();
                    }
                    reached
                }
            };
            if reached && std::mem::take(&mut first) {
                paused.set_permissions(page..page + PAGE_SIZE, Permissions::empty());
            }
        });
        if at.is_some() {
            if look_ahead == LookAhead::None {
                machine.stop_at_every_check();
            } else {
                machine.raise_stop_flag();
            }
        }
        machine.run()
    };
    let none = run(LookAhead::None);
    assert!(
        matches!(none.0, Stopped::Exit(Exit::MemoryFault { address, .. }) if address / PAGE_SIZE == page / PAGE_SIZE),
        "{}: none {:?}",
        blob.name,
        none.0
    );
    let full = run(LookAhead::Full);
    assert_eq!(full.0, none.0, "{}: why it stopped", blob.name);
    assert_eq!(full.1, none.1, "{}: registers", blob.name);
}
/// The guest's hand-made programs (`GuestTest::changes`): groups and loops across system calls and calls,
/// accesses below sp once a frame is popped, the longest accesses onto a page changed at a stop.
pub(super) fn hand_made_changes_match_none() {
    let changes = part::changes();
    std::thread::scope(|scope| {
        for change in &changes {
            scope.spawn(move || denied_at_the_first(change));
        }
    });
    eprintln!("changes: {} hand-made programs", changes.len());
}
