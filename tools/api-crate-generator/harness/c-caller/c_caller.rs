//! The generated hostile C caller over every table and export of the serving glue, which serves the hostile
//! implementation: a short honest run, and the fuzzing run (ignored by default) of rounds of calls, each round under a
//! lie the hostile implementation tells, picked at random (one round in three honest).

use std::sync::Arc;
use std::time::Instant;

use wcontainers_component_object_model::structures::Hmodule;
use wcontainers_direct3d_11::Direct3D11 as _;
use wcontainers_direct3d_11::enumerations::CreateDeviceFlag;
use wcontainers_direct3d_11_direct_c_caller::{CALLED, COUNTED, hostile_exports, run};
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

#[test]
fn an_honest_short_run_calls_every_kind_of_entry() {
    quiet();
    hostile::plan(None);
    let exports = hostile_exports();
    let counted = run(1, 20_000, 2, &exports).expect("the C caller took its arguments");
    println!("C CALLER honest: entries and exports {CALLED}; {}", printed(&counted));
    assert!(counted[3] > 0, "no call succeeded: {}", printed(&counted));
    assert!(counted[5] > 0, "no object was handed out: {}", printed(&counted));
    assert_eq!(counted[16], 0, "a failed QueryInterface left a pointer: {}", printed(&counted));
    assert_eq!(counted[18], 0, "a refusing entry answered other than E_NOTIMPL: {}", printed(&counted));
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
        for (sum, value) in total.iter_mut().zip(counted) {
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
    assert!(total[3] > 0 && total[5] > 0, "nothing succeeded or was handed out");
    assert_eq!(total[16], 0, "a failed QueryInterface left a pointer");
    assert_eq!(total[18], 0, "a refusing entry answered other than E_NOTIMPL");
}
