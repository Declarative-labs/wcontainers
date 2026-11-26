//! The generated hostile C caller over every table and export of the serving glue, which serves the hostile
//! implementation: a short honest run, and the fuzzing run (ignored by default) of rounds of calls, each round under a
//! lie the hostile implementation tells, picked at random (one round in three honest). Each run counts the calls of each
//! case on its own object and fails when a case or export is never reached, but for the cases `UNREACHABLE` names.

use std::sync::Arc;
use std::time::Instant;

use wcontainers_component_object_model::structures::Hmodule;
use wcontainers_direct3d_11::Direct3D11 as _;
use wcontainers_direct3d_11::enumerations::CreateDeviceFlag;
use wcontainers_direct3d_11_direct_c_caller::{CALLED, CASES, COUNTED, EXPORT_NAMES, UNREACHABLE, hostile_exports, run};
use wcontainers_direct3d_11_hostile::{self as hostile, Lie, Objects, Site, Way, When};
use wcontainers_direct3d_common::enumerations::{DriverType, FeatureLevel};

/// The feature levels a re-entering lie asks for.
const LEVELS: [FeatureLevel; 1] = [FeatureLevel(0xb000)];

/// The SDK version Direct3D 11's header names.
const SDK_VERSION: u32 = 7;

/// Calls per round, each round under one lie.
const ROUND: u64 = 50_000;

fn number(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().and_then(|value| value.parse().ok()).unwrap_or(default)
}

fn quiet() {
    std::panic::set_hook(Box::new(|_| {}));
}

fn printed(counted: &[u64]) -> String {
    COUNTED.iter().zip(counted).map(|(name, value)| format!("{name} {value}")).collect::<Vec<_>>().join(", ")
}

/// The count named `name` of `COUNTED`.
fn count(counted: &[u64], name: &str) -> u64 {
    COUNTED.iter().position(|counted_name| *counted_name == name).map_or(0, |at| counted[at])
}

/// Checks what a run reached: every case and export but `UNREACHABLE`'s called on its own object, and none of those.
/// Returns the line to print, or what was missed.
fn reach(reached: &[u64]) -> Result<String, String> {
    let names: Vec<&str> = CASES.iter().chain(EXPORT_NAMES.iter()).copied().collect();
    assert_eq!(names.len(), reached.len(), "one count per case and export");
    let excluded: Vec<&str> = UNREACHABLE.iter().map(|(name, _)| *name).collect();
    let never: Vec<&str> = names.iter().zip(reached).filter(|(_, times)| **times == 0).map(|(name, _)| *name).collect();
    let missed: Vec<&str> = never.iter().filter(|name| !excluded.contains(name)).copied().collect();
    let reached_excluded: Vec<&str> = names.iter().zip(reached).filter(|(name, times)| **times > 0 && excluded.contains(name)).map(|(name, _)| *name).collect();
    let cases_reached = reached[..CASES.len()].iter().filter(|times| **times > 0).count();
    let exports_reached = reached[CASES.len()..].iter().filter(|times| **times > 0).count();
    let line = format!(
        "C CALLER REACHED: cases {cases_reached} of {}, exports {exports_reached} of {}; never reached {} = unreachable listed {}{}",
        CASES.len(),
        EXPORT_NAMES.len(),
        never.len(),
        UNREACHABLE.len(),
        if excluded.is_empty() { String::new() } else { format!(" ({})", excluded.join(", ")) }
    );
    if !missed.is_empty() {
        return Err(format!("{line}\nNEVER REACHED {}: {}", missed.len(), missed.join(", ")));
    }
    if !reached_excluded.is_empty() {
        return Err(format!("{line}\nREACHED THOUGH LISTED UNREACHABLE {}: {}", reached_excluded.len(), reached_excluded.join(", ")));
    }
    if never.len() != UNREACHABLE.len() {
        return Err(format!("{line}\nthe cases never reached are not exactly the {} listed", UNREACHABLE.len()));
    }
    Ok(line)
}

#[test]
fn an_honest_short_run_calls_every_kind_of_entry() {
    quiet();
    hostile::plan(None);
    let exports = hostile_exports();
    let result = run(1, 20_000, 2, &exports).expect("the C caller took its arguments");
    let counted = result.counts;
    println!("C CALLER honest: entries and exports {CALLED}; {}", printed(&counted));
    let reached = reach(&result.reached);
    println!("{}", reached.as_ref().unwrap_or_else(|missed| missed));
    assert!(count(&counted, "succeeded") > 0, "no call succeeded: {}", printed(&counted));
    assert!(count(&counted, "adopted") > 0, "no object was handed out: {}", printed(&counted));
    assert_eq!(count(&counted, "query_left_pointer"), 0, "a failed QueryInterface left a pointer: {}", printed(&counted));
    assert_eq!(count(&counted, "refused_wrong"), 0, "a refusing entry answered other than E_NOTIMPL or wrote other than null and zeros: {}", printed(&counted));
    if let Err(missed) = reached {
        panic!("the honest run did not reach every entry: {missed}");
    }
}

#[test]
#[ignore = "long: run by name with FUZZ_SEED and FUZZ_CALLS"]
fn fuzz_every_table() {
    quiet();
    let seed = number("FUZZ_SEED", 1);
    let calls = number("FUZZ_CALLS", 1_000_000);
    let threads = u32::try_from(number("FUZZ_THREADS", 4)).unwrap_or(4);
    let exports = hostile_exports();
    let mut lies = seed ^ 0x5eed_c0de;
    let mut roll = |bound: u64| {
        lies = lies.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (lies >> 33) % bound
    };
    hostile::on_reentry(Some(Arc::new(|| {
        let objects = Objects::new();
        let _ = objects.api.create_device(None, DriverType(1), Hmodule(0), CreateDeviceFlag(0), &LEVELS, SDK_VERSION);
    })));
    let before = hostile::live();
    let started = Instant::now();
    let mut total = vec![0_u64; COUNTED.len()];
    let mut reached = vec![0_u64; CASES.len() + EXPORT_NAMES.len()];
    let mut told = 0;
    let mut done = 0;
    let mut round = 0_u64;
    while done < calls {
        let these = (calls - done).min(ROUND * u64::from(threads)) / u64::from(threads);
        if roll(3) == 0 {
            hostile::plan(None);
        } else {
            let site = [Site::Method, Site::Drop, Site::Mapped][usize::try_from(roll(3)).unwrap_or(0)];
            let way = [Way::Panic, Way::Reenter][usize::try_from(roll(2)).unwrap_or(0)];
            hostile::plan(Some(Lie { site, way, when: When::Every(7 + roll(500)) }));
        }
        let counted = run(seed.wrapping_mul(1_000_003).wrapping_add(round), these.max(1), threads, &exports).expect("the C caller took its arguments");
        for (sum, value) in total.iter_mut().zip(counted.counts) {
            *sum += value;
        }
        for (sum, value) in reached.iter_mut().zip(counted.reached) {
            *sum += value;
        }
        told += hostile::lies_told();
        done += these.max(1) * u64::from(threads);
        round += 1;
    }
    hostile::plan(None);
    hostile::on_reentry(None);
    println!(
        "C CALLER FUZZ seed {seed}: {done} calls on {threads} threads in {:?}, {round} rounds; lies told {told}; implementation objects alive {} (before {before}); entries and exports {CALLED}; {}",
        started.elapsed(),
        hostile::live(),
        printed(&total)
    );
    let reach = reach(&reached);
    println!("{}", reach.as_ref().unwrap_or_else(|missed| missed));
    assert!(count(&total, "succeeded") > 0 && count(&total, "adopted") > 0, "nothing succeeded or was handed out");
    assert_eq!(count(&total, "query_left_pointer"), 0, "a failed QueryInterface left a pointer");
    assert_eq!(count(&total, "refused_wrong"), 0, "a refusing entry answered other than E_NOTIMPL or wrote other than null and zeros");
    assert_eq!(hostile::live(), before, "implementation objects outlived the run: a reference was leaked");
    if let Err(missed) = reach {
        panic!("the fuzzing run did not reach every entry: {missed}");
    }
}
