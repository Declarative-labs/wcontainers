//! Safe callers, and one unsafe caller in another crate, that the compiler must refuse; each case is built alone with
//! `RUSTFLAGS='--cfg refused="<case>"' cargo test --no-run --features host --test refused`, and each build must fail.
#![cfg(unix)]
#![allow(dead_code, unused_imports)]

use wcontainers_shared_memory::{HostMapping, SharedMemory};

/// Returns a page of anonymous memory.
fn memory() -> HostMapping {
    HostMapping::anonymous(4096).expect("the host maps a page")
}

/// A part that outlives its memory: E0505.
#[cfg(refused = "outlives")]
#[test]
fn part_outlives_memory() {
    let memory = memory();
    let part = memory.borrow(0..8).expect("inside");
    drop(memory);
    let _ = part;
}

/// A sub-range of a sub-range: E0599.
#[cfg(refused = "part_of_part")]
#[test]
fn part_of_a_part() {
    let memory = memory();
    let part = memory.borrow(0..8).expect("inside");
    let _ = part.borrow(0..4);
}

/// The host mapping's own memory, other than through the trait: E0616.
#[cfg(refused = "field")]
#[test]
fn reach_the_mapping() {
    let memory = memory();
    let _ = &memory.map;
}

/// A word of another crate's own, with every bound a word has but the seal; the seal named: E0603.
#[cfg(refused = "seal_named")]
mod seal_named {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Mine(u64);

    impl From<Mine> for u64 {
        fn from(mine: Mine) -> u64 {
            mine.0
        }
    }

    impl TryFrom<u64> for Mine {
        type Error = ();

        fn try_from(value: u64) -> Result<Mine, ()> {
            Ok(Mine(value))
        }
    }

    impl wcontainers_shared_memory::seal::Sealed for Mine {}

    impl wcontainers_shared_memory::Word for Mine {}
}

/// The same word without the seal: E0277.
#[cfg(refused = "seal_missing")]
mod seal_missing {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Mine(u64);

    impl From<Mine> for u64 {
        fn from(mine: Mine) -> u64 {
            mine.0
        }
    }

    impl TryFrom<u64> for Mine {
        type Error = ();

        fn try_from(value: u64) -> Result<Mine, ()> {
            Ok(Mine(value))
        }
    }

    impl wcontainers_shared_memory::Word for Mine {}
}

/// A word of a size the processor cannot reach at once, through a method: E0277.
#[cfg(refused = "byte_word")]
#[test]
fn byte_word() {
    let _ = memory().load::<u8>(0);
}

/// Production code naming the heap's test double: E0433, since it exists only in a build with `--cfg miri` or
/// `--cfg kani`. That is a convention, not a guarantee: any build can set either flag (`RUSTFLAGS="--cfg miri"`),
/// and then this compiles.
#[cfg(refused = "heap")]
#[test]
fn heap_double() {
    let _ = wcontainers_shared_memory::HeapMapping::new(1);
}

/// Pointing the descriptor at another file while `HostMapping::file` holds it borrowed, so that the checks and the
/// mapping could see two files: E0502, since a safe `dup2` needs the descriptor it replaces mutably.
#[cfg(all(refused = "redirect", target_os = "linux"))]
#[test]
fn redirect_the_descriptor() {
    use std::os::fd::AsFd;

    use rustix::fs::{MemfdFlags, memfd_create};

    let mut file = memfd_create("words", MemfdFlags::ALLOW_SEALING).expect("a memory file");
    let other = memfd_create("other", MemfdFlags::empty()).expect("a memory file");
    let borrowed = file.as_fd();
    rustix::io::dup2(&other, &mut file).expect("the descriptor points at the other file");
    let _ = HostMapping::file(borrowed, 0, 8);
}
