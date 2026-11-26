// Written once for both guests; each guest's module in `layer3.rs` includes it.

/// The signals a program whose access or jump went astray ends by, depending on what lies where it went.
/// A stray access raises SIGSEGV or SIGBUS (no page, or a file's page past the file's end, which an
/// ordinary executable has and a blob does not). A stray jump into data faults in an ordinary executable,
/// whose data cannot be run, but runs the data in a blob, which is all one region that can be (see
/// `placement`), and ends by whatever the data decodes to: an undefined instruction, a breakpoint or a
/// stray access.
pub(super) const ASTRAY: [u32; 4] = [SIGSEGV, SIGBUS, SIGILL, SIGTRAP];
/// Whether two exit statuses end the same way, every signal of a program gone astray being one.
pub(super) fn same_ending(one: i32, other: i32) -> bool {
    let astray = |status: i32| {
        u32::try_from(status - SIGNALLED).is_ok_and(|signal| ASTRAY.contains(&signal))
    };
    one == other || (astray(one) && astray(other))
}
/// Run a blob at none and full in both settings; compare with Linux, and full with none in registers and
/// memory. Gives Linux's run, or nothing when the program did not end in time there. `ordinary` runs the
/// program built as an ordinary Linux executable.
pub(super) fn compare(blob: &Blob, ordinary: impl Fn() -> Ran) -> Result<Option<Ran>, String> {
    let Some(theirs) = native_in_time(blob) else {
        return Ok(None);
    };
    // A blob that Linux ends by a signal is compared only when the program, built as an ordinary
    // executable, ends the same way (`same_ending`): otherwise both sides could agree on a blob that is broken (a
    // moved blob that did not relocate itself, say). What it wrote before may differ, as a program that
    // faults may have read through a wild pointer, whose target depends on where things are.
    if theirs.status >= SIGNALLED {
        let program = ordinary();
        if !same_ending(program.status, theirs.status) {
            // Whether Linux does it again tells a fault of the blob from one of the reference.
            let again: Vec<i32> = (0..3).map(|_| native(blob).status).collect();
            return Err(format!(
                "{}: Linux ended the blob with status {}, then {again:?} when run again, the ordinary executable with status {}",
                blob.name, theirs.status, program.status
            ));
        }
    }
    let mut finals = Vec::new();
    for (look_ahead, optimise, stopping) in [
        (LookAhead::None, Optimise::Speed, false),
        (LookAhead::Full, Optimise::Speed, false),
        (LookAhead::Full, Optimise::Size, false),
        (LookAhead::Full, Optimise::Speed, true),
        (LookAhead::Full, Optimise::Size, true),
    ] {
        let how = (look_ahead, optimise);
        let (ours, mut machine, registers) = if stopping {
            translated_stopping_everywhere(blob, how)
        } else {
            translated_for(blob, how)
        };
        if ours != theirs {
            return Err(format!(
                "{} at {look_ahead:?} for {optimise:?}, stopping everywhere {stopping}: ours {:?} status {}, Linux {:?} status {}",
                blob.name,
                String::from_utf8_lossy(&ours.stdout),
                ours.status,
                String::from_utf8_lossy(&theirs.stdout),
                theirs.status
            ));
        }
        if machine.count(Reason::UnsupportedInstructionReached) != 0 {
            return Err(format!("{}: unsupported instruction reached", blob.name));
        }
        finals.push((registers, reachable_memory(&mut machine, blob)));
    }
    if finals[1..].iter().any(|full| *full != finals[0]) {
        return Err(format!("{}: full differs from none at the end", blob.name));
    }
    // Bounded of an N and pieces mixed with events injected, both from the blob's own hash.
    let seed = {
        use std::hash::Hasher;
        let mut hasher = rustc_hash::FxHasher::default();
        hasher.write(&blob.bytes);
        hasher.finish()
    };
    let bound = 2 + (seed % 63) as u32;
    let look_ahead = LookAhead::Bounded(bound);
    let mut machine = ready(blob, look_ahead, Default::default());
    machine.translate_at_run_time(move || look_ahead);
    let (stopped, registers) = machine.run();
    let ours = outcome(blob, &machine, stopped, &registers);
    if ours != theirs || (registers, reachable_memory(&mut machine, blob)) != finals[0] {
        return Err(format!(
            "{} at bounded {bound} differs from none",
            blob.name
        ));
    }
    let (ours, mut machine, registers, events) = mixed(blob, seed);
    if ours != theirs || (registers, reachable_memory(&mut machine, blob)) != finals[0] {
        return Err(format!(
            "{} mixed from {seed} ({events:?}) differs from none",
            blob.name
        ));
    }
    Ok(Some(theirs))
}
/// Run a blob in Chrome at none and at full for speed and for size, and compare with Linux's run.
pub(super) fn compare_in_chrome(blob: &Blob, theirs: &Ran) -> Result<(), String> {
    for how in [
        (LookAhead::None, Optimise::Speed),
        (LookAhead::Full, Optimise::Speed),
        (LookAhead::Full, Optimise::Size),
    ] {
        let ours = browser::translated_with(blob, how, Omit::default());
        if ours.ran != *theirs {
            return Err(format!(
                "{} in Chrome at {how:?}: ours {:?} status {}, Linux {:?} status {}",
                blob.name,
                String::from_utf8_lossy(&ours.ran.stdout),
                ours.ran.status,
                String::from_utf8_lossy(&theirs.stdout),
                theirs.status
            ));
        }
    }
    Ok(())
}
pub(super) fn csmith_programs_match_linux() {
    let cases = setting("WC_LAYER3_CASES", 4);
    let seed = setting("WC_LAYER3_SEED", 1);
    let from = setting("WC_LAYER3_FROM", 0);
    let chrome = setting("WC_LAYER3_CHROME", 1);
    let jobs = setting("WC_LAYER3_JOBS", 1).max(1);
    let chrome_every = cases.div_ceil(chrome.max(1));
    let in_chrome = AtomicU64::new(0);
    let (ran, too_long, done) = (AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0));
    // Programs whose floating point runs on the exact path: they read FPSR.
    let exact = AtomicU64::new(0);
    let failures = Mutex::new(Vec::new());
    at_once(jobs, cases, |i| {
        let number = seed.wrapping_mul(LAYER3_CASE_STRIDE).wrapping_add(from + i);
        let (source, compiler, level, position) = csmith_case(number);
        let blob = build(&source, compiler, level, position);
        if blob.reads_float_status() {
            exact.fetch_add(1, Ordering::Relaxed);
        }
        let own = || ordinary(&source, compiler, level, position);
        let checked = compare(&blob, own).and_then(|theirs| match theirs {
            Some(theirs) if chrome > 0 && i.is_multiple_of(chrome_every) => {
                in_chrome.fetch_add(1, Ordering::Relaxed);
                compare_in_chrome(&blob, &theirs).map(|()| Some(theirs))
            }
            other => Ok(other),
        });
        match checked {
            Ok(Some(_)) => ran.fetch_add(1, Ordering::Relaxed),
            Ok(None) => too_long.fetch_add(1, Ordering::Relaxed),
            Err(report) => {
                eprintln!("csmith {number} (case {}): {report}", from + i);
                failures.lock().unwrap().push(number);
                0
            }
        };
        let done = done.fetch_add(1, Ordering::Relaxed) + 1;
        if done.is_multiple_of(1000) {
            let failing = failures.lock().unwrap().len();
            eprintln!("layer 3: {done} of {cases} done, {failing} failing so far");
        }
    });
    let mut failures = failures.into_inner().unwrap();
    failures.sort();
    eprintln!(
        "layer 3: {cases} Csmith programs from case {from}, seed {seed}, {jobs} at once: {} compared ({} also in Chrome), {} did not end in time on Linux, {} read FPSR, {} failing: {failures:?}",
        ran.into_inner(),
        in_chrome.into_inner(),
        too_long.into_inner(),
        exact.into_inner(),
        failures.len()
    );
    assert!(failures.is_empty());
}
/// The hand-picked programs `names` with the C library (number printing, sorting and, for ARM64, the
/// differences of section 8), with both compilers, every level and both positions.
pub(super) fn hand_picked_programs_match_linux(names: &[&str]) {
    let mut failures = Vec::new();
    for &name in names {
        for compiler in [Compiler::Clang, Compiler::Gcc] {
            for level in LEVELS {
                for position in [Position::Fixed, Position::Independent] {
                    let source = harness::programs::source(name);
                    let blob = build(&source, compiler, level, position);
                    let own = || ordinary(&source, compiler, level, position);
                    match compare(&blob, own) {
                        Ok(Some(_)) => {}
                        Ok(None) => failures.push(format!("{}: did not end on Linux", blob.name)),
                        Err(report) => failures.push(report),
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
/// The reference runs ordinary executables, which tell a blob's signal ending to be the program's own.
pub(super) fn ordinary_executables_run_on_the_reference() {
    check_ordinary_programs(part::C_TARGET, native_program);
}
