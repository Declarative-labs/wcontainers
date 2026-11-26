// Written once for both guests; each guest's module in `layer1.rs` includes it.

pub(super) fn sample_matches_the_cpu_in_chrome() {
    let count = setting("WC_LAYER1_CHROME", 1);
    let seed = setting("WC_LAYER1_SEED", 1);
    let mut sample: Vec<(String, Case)> = Vec::new();
    for form in part::forms() {
        let name = part::form_name(form);
        let mut runner = generator(seed, name, Config::default());
        let strategy = part::cases(form);
        for _ in 0..count {
            let tree = strategy.new_tree(&mut runner).expect("a case");
            sample.push((name.to_string(), tree.current()));
        }
    }
    let all: Vec<Case> = sample.iter().map(|(_, case)| case.clone()).collect();
    let outcomes = translated_in_chrome(&all);
    let mut reference = Reference::start(&part::REFERENCE);
    let mut failures = Vec::new();
    let mut known = Known::default();
    for ((name, case), outcome) in sample.iter().zip(outcomes) {
        match compare_outcome(&mut reference, case, outcome) {
            Err(error) => {
                eprintln!("{name} in Chrome: {error}");
                failures.push(name.clone());
            }
            Ok(agreement) => known.add(name, agreement),
        }
    }
    eprintln!(
        "layer 1 in Chrome: {} cases ({count} a form), seed {seed}, {} failing: {failures:?}",
        sample.len(),
        failures.len()
    );
    known.report("layer 1 in Chrome");
    assert!(failures.is_empty());
}
/// The known failures of the reference met, counted by form and reason.
#[derive(Default)]
pub(super) struct Known(BTreeMap<(String, &'static str), u64>);
impl Known {
    pub(super) fn add(&mut self, form: &str, agreement: Agreement) {
        if let Agreement::KnownFailure(reason) = agreement {
            *self.0.entry((form.to_string(), reason)).or_default() += 1;
        }
    }

    /// One line for each form and reason, and the total.
    fn report(&self, what: &str) {
        for ((form, reason), n) in &self.0 {
            eprintln!("{what}: {form}: {n} known failures: {reason}");
        }
        eprintln!(
            "{what}: {} known failures in all",
            self.0.values().sum::<u64>()
        );
    }
}
/// Runs `count` cases of each form chosen by `WC_LAYER1_FORMS`, `WC_LAYER1_JOBS` forms at once, each case
/// through `check` with a reference runner of its own; prints the summary, its first words `layer 1` and
/// `list`, and the known failures met, and returns the failing forms' names, sorted.
pub(super) fn run_forms(
    forms: &[part::Form],
    count: u32,
    list: &str,
    check: impl Fn(&mut Reference, &part::Form, &Case) -> Result<Agreement, String> + Sync,
) -> Vec<String> {
    let seed = setting("WC_LAYER1_SEED", 1);
    let jobs = setting("WC_LAYER1_JOBS", 1).max(1);
    let only: Option<Vec<String>> = std::env::var("WC_LAYER1_FORMS")
        .ok()
        .map(|list| list.split(',').map(String::from).collect());
    let chosen: Vec<_> = forms
        .iter()
        .filter(|f| {
            only.as_ref()
                .is_none_or(|names| names.iter().any(|n| n == part::form_name(f)))
        })
        .collect();
    let failures = Mutex::new(Vec::new());
    let known = Mutex::new(Known::default());
    // Reference runners not in use; a form takes one, or starts one when none is free.
    let free = Mutex::new(Vec::new());
    at_once(jobs, chosen.len() as u64, |i| {
        let form = chosen[i as usize];
        let name = part::form_name(form);
        let taken = free.lock().unwrap().pop();
        let reference = RefCell::new(taken.unwrap_or_else(|| Reference::start(&part::REFERENCE)));
        let mut config = Config::with_cases(count);
        config.failure_persistence = None;
        config.max_global_rejects = REJECTS_PER_CASE * count;
        let mut runner = generator(seed, name, config);
        let result = runner.run(&part::cases(form), |case| {
            let agreement =
                check(&mut reference.borrow_mut(), form, &case).map_err(TestCaseError::fail)?;
            known.lock().unwrap().add(name, agreement);
            Ok(())
        });
        free.lock().unwrap().push(reference.into_inner());
        if let Err(error) = result {
            eprintln!("{name}: {error}");
            failures
                .lock()
                .unwrap()
                .push((part::form_group(form).to_string(), name.to_string()));
        }
    });
    let failing = failures.into_inner().unwrap();
    let mut groups = BTreeMap::<&str, usize>::new();
    for (group, _) in &failing {
        *groups.entry(group).or_default() += 1;
    }
    let mut failures: Vec<String> = failing.iter().map(|(_, name)| name.clone()).collect();
    failures.sort();
    eprintln!(
        "layer 1{list}: {} forms, {count} cases each, seed {seed}, {jobs} at once, {} failing: {failures:?}",
        chosen.len(),
        failures.len()
    );
    if !groups.is_empty() {
        eprintln!("layer 1 failing by group: {groups:?}");
    }
    known
        .into_inner()
        .unwrap()
        .report(&format!("layer 1{list}"));
    failures
}
pub(super) fn every_form_matches_the_cpu() {
    let count = setting("WC_LAYER1_CASES", 64) as u32;
    let failures = run_forms(part::forms(), count, "", |reference, _, case| {
        compare(reference, case)
    });
    assert!(
        failures.is_empty(),
        "{} forms differ from the CPU",
        failures.len()
    );
}
/// The guest's hand-made cases: each compared with the CPU unless answered by design, and checked for
/// what it must do.
pub(super) fn hand_cases_do_what_they_must() {
    let mut reference = Reference::start(&part::REFERENCE);
    let all = part::hand_cases();
    for designed in &all {
        let name = designed.name;
        if designed.compared {
            let agreement = compare(&mut reference, &designed.case)
                .unwrap_or_else(|report| panic!("{name}: {report}"));
            assert_eq!(agreement, Agreement::Same, "{name}");
        }
        let (outcome, machine) = translated_on(&designed.case, |_| {});
        if let Some(tracking) = designed.tracking {
            assert_eq!(
                part::tracked(machine.stopped_state()),
                tracking,
                "{name}: whether the float flags are kept"
            );
        }
        for &(reason, count) in &designed.counted {
            assert_eq!(machine.count_named(reason), count, "{name}: {reason}");
        }
        if let Some(stop) = designed.stop {
            assert_eq!(outcome.stop, stop, "{name}");
        }
        for &(field, value) in &designed.fields {
            let field_name = part::REFERENCE.fields[field];
            assert_eq!(
                outcome.state.0[field], value,
                "{name}: {field_name} is {:#x}, must be {value:#x}",
                outcome.state.0[field]
            );
        }
    }
    eprintln!("layer 1: {} hand-made cases", all.len());
}
/// Bytes in no form of the guest's, for any feature, stop ours as they stop the CPU. Random cases,
/// `WC_LAYER1_CASES` of them (default 64), seeded from `WC_LAYER1_SEED`.
pub(super) fn bytes_in_no_form_stop_as_on_the_cpu() {
    let count = setting("WC_LAYER1_CASES", 64) as u32;
    let seed = setting("WC_LAYER1_SEED", 1);
    let mut config = Config::with_cases(count);
    config.failure_persistence = None;
    config.max_local_rejects = REJECTS_PER_CASE * count;
    let mut runner = TestRunner::new_with_rng(config, seeded::generator(seed, b"no form ", ""));
    let reference = RefCell::new(Reference::start(&part::REFERENCE));
    let known = RefCell::new(Known::default());
    let result = runner.run(&part::no_form(seed), |case| {
        let agreement = compare(&mut reference.borrow_mut(), &case).map_err(TestCaseError::fail)?;
        known
            .borrow_mut()
            .add(&part::disassemble(case.code.code()), agreement);
        Ok(())
    });
    eprintln!("layer 1: {count} cases in no form, seed {seed}");
    known.borrow().report("layer 1, no form");
    result.unwrap();
}
/// What the CPU offers is what a program is told it has.
pub(super) fn offered_features_are_the_ones_programs_are_told() {
    let (mut offered, mut told) = (part::offered(), part::told());
    offered.sort();
    told.sort();
    assert_eq!(offered, told);
}
