//! The spike's attacks through raw COM calls, as tests that must pass: a stale pointer once more objects of its kind
//! were made, a release of the last reference inside a call on the same object or from another thread while it is
//! in progress, writes to mapped memory, and counts the output merger may not trust. Each runs under a time limit,
//! so a hang fails it.
#![forbid(unsafe_code)]

use std::sync::mpsc::channel;
use std::time::Duration;

use wcontainers_direct3d_11_direct_harness::front::Front;
use wcontainers_direct3d_11_direct_harness::{Attack, Entry};

/// How many later releases a stale pointer is known to find nothing for (objects.rs: `QUARANTINE`).
#[cfg(not(feature = "small-table"))]
const QUARANTINE: u32 = 1 << 16;
#[cfg(feature = "small-table")]
const QUARANTINE: u32 = 1 << 10;

fn targets() -> [(&'static str, Entry); 2] {
    [("front", Entry::served(Front)), ("pass-through", Entry::pass_through())]
}

/// Runs the attack on a thread of its own, and returns its outcome, or none when it has not returned in 10 s.
fn within_time(entry: Entry, attack: Attack) -> Option<(i32, String)> {
    let (sender, receiver) = channel();
    std::thread::spawn(move || drop(sender.send(entry.attack(attack))));
    receiver.recv_timeout(Duration::from_secs(10)).ok()
}

fn each_target(attack: Attack) -> Vec<Option<(i32, String)>> {
    targets()
        .into_iter()
        .map(|(name, entry)| {
            let outcome = within_time(entry, attack);
            println!("ATTACK {attack:?} {name}: {outcome:?}");
            outcome
        })
        .collect()
}

#[test]
fn a_stale_pointer_does_not_reach_the_next_object_of_its_kind() {
    let reused: Vec<i32> = each_target(Attack::StaleReuse).into_iter().map(|outcome| outcome.map_or(-1, |o| o.0)).collect();
    assert_eq!(reused, [0, 0], "a stale pointer is the newer buffer's pointer");
}

#[test]
fn a_stale_pointer_finds_nothing_for_the_quarantine() {
    let found: Vec<i32> = each_target(Attack::StaleBound(QUARANTINE)).into_iter().map(|outcome| outcome.map_or(-1, |o| o.0)).collect();
    assert_eq!(found, [0, 0], "a stale pointer found an object within the quarantine");
}

#[test]
fn a_release_of_the_last_reference_inside_a_call_on_the_same_object() {
    let returned: Vec<bool> = each_target(Attack::ReleaseInsideCall).iter().map(Option::is_some).collect();
    assert_eq!(returned, [true, true], "hung for 10 s");
}

#[test]
fn a_release_of_the_last_reference_from_another_thread_during_a_call() {
    let outcomes = each_target(Attack::ReleaseFromOtherThread);
    let returned: Vec<bool> = outcomes.iter().map(Option::is_some).collect();
    assert_eq!(returned, [true, true], "hung for 10 s");
}

#[test]
fn writes_to_mapped_memory_stay_inside_it() {
    for width in [1, 16, 4096, 1 << 20] {
        let returned: Vec<bool> = each_target(Attack::WriteMapped(width)).iter().map(Option::is_some).collect();
        assert_eq!(returned, [true, true], "width {width}");
    }
    for side in [1, 4, 64, 1024] {
        let returned: Vec<bool> = each_target(Attack::WriteMappedTexture(side)).iter().map(Option::is_some).collect();
        assert_eq!(returned, [true, true], "side {side}");
    }
}

#[test]
fn counts_out_of_range_and_keep_values_with_garbage_arrays() {
    let returned: Vec<bool> = each_target(Attack::OutputMerger).iter().map(Option::is_some).collect();
    assert_eq!(returned, [true, true]);
}
