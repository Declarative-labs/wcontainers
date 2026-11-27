//! Form I's pool: every slot taken once, refused when none is free, and given back unmapped. A process of its own, so
//! no other test takes slots meanwhile.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::collections::BTreeSet;

use rustix::process::{Resource, Rlimit};
use wcontainers_arm64::way::{IN_PROCESS_SLOTS, SLOT_BITS};
use wcontainers_executor_in_process::memory::{CreateMemoryError, InProcessPool};

/// Whether this process maps the host address, read from its own `/proc/self/maps`.
fn is_mapped(address: u64) -> bool {
    let maps = std::fs::read_to_string("/proc/self/maps").expect("the process's own maps");
    maps.lines().any(|line| {
        let range = line.split_whitespace().next().unwrap_or_default();
        let Some((start, end)) = range.split_once('-') else {
            return false;
        };
        let parse = |text: &str| u64::from_str_radix(text, 16).unwrap_or_default();
        (parse(start)..parse(end)).contains(&address)
    })
}

#[test]
fn every_free_slot_is_taken_once_then_refused_and_given_back_unmapped() {
    // Each memory holds its own file open: more than the usual soft limit of open files.
    let files = rustix::process::getrlimit(Resource::Nofile);
    rustix::process::setrlimit(
        Resource::Nofile,
        Rlimit {
            current: files.maximum,
            maximum: files.maximum,
        },
    )
    .expect("the soft limit of open files rises to the hard one");
    let pool = InProcessPool::new().expect("a host with the guest's pages");
    let mut memories = Vec::new();
    let error = loop {
        match pool.create_memory() {
            Ok(memory) => memories.push(memory),
            Err(error) => break error,
        }
    };
    assert!(matches!(error, CreateMemoryError::NoSlot), "{error}");
    let slots: BTreeSet<u16> = memories.iter().map(|(memory, _)| memory.slot()).collect();
    assert_eq!(slots.len(), memories.len(), "a slot taken twice");
    assert!(
        slots
            .iter()
            .all(|slot| (1..=IN_PROCESS_SLOTS).contains(slot))
    );
    // Only slots something else of this process maps are left out.
    let left_out = usize::from(IN_PROCESS_SLOTS) - slots.len();
    println!("slots taken {}, left out {left_out}", slots.len());
    assert!(
        left_out < usize::from(IN_PROCESS_SLOTS) / 2,
        "{left_out} slots left out"
    );

    let (memory, table) = memories.pop().expect("at least one memory");
    let slot = memory.slot();
    let base = memory.window_base();
    drop(memory);
    assert!(
        is_mapped(base),
        "a memory's slot is held while its table is"
    );
    drop(table);
    assert!(!is_mapped(base), "a dropped memory's slot is unmapped");
    assert!(!is_mapped(base + (1 << SLOT_BITS) - 1));
    let (again, _again_table) = pool.create_memory().expect("the slot given back");
    assert_eq!(again.slot(), slot);
}
