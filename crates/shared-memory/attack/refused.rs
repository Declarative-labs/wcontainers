//! Safe callers, and one unsafe caller in another crate, that the compiler must refuse; each case is built alone with
//! `RUSTFLAGS='--cfg refused="<case>"' cargo test --no-run --features host --test refused`, and each build must fail.
#![cfg(unix)]
#![allow(dead_code, unused_imports)]

use std::sync::Arc;

use wcontainers_shared_memory::{HostMapping, Words};

/// Returns words over a page of anonymous memory.
fn words() -> Words {
    Words::new(HostMapping::anonymous(4096).expect("the host maps a page"))
}

/// A part that outlives its words: E0505.
#[cfg(refused = "outlives")]
#[test]
fn part_outlives_words() {
    let words = words();
    let part = words.part(0..8).expect("inside");
    drop(words);
    let _ = part;
}

/// A sub-range of a sub-range: E0599.
#[cfg(refused = "part_of_part")]
#[test]
fn part_of_a_part() {
    let words = words();
    let part = words.part(0..8).expect("inside");
    let _ = part.part(0..4);
}

/// The host mapping's own memory, other than through `Words`: E0616.
#[cfg(refused = "field")]
#[test]
fn reach_the_mapping() {
    let mapping = HostMapping::anonymous(4096).expect("the host maps a page");
    let _ = &mapping.map;
}

/// Replacing the mapping under live words: E0507.
#[cfg(refused = "replace")]
#[test]
fn replace_under_words() {
    let mut mapping = HostMapping::anonymous(4096).expect("the host maps a page");
    let words = Words::new(mapping.clone());
    *Arc::get_mut(&mut mapping).expect("one holder") =
        *HostMapping::anonymous(8192).expect("the host maps two pages");
    let _ = words;
}

/// A mapping of another crate's own, naming the seal: E0603, whatever the lint allows.
#[cfg(refused = "seal_named")]
#[allow(unsafe_code)]
mod seal_named {
    use core::ptr::NonNull;

    struct Mine(Vec<u64>);

    impl wcontainers_shared_memory::seal::Sealed for Mine {}

    unsafe impl wcontainers_shared_memory::Mapped for Mine {
        fn base(&self) -> NonNull<u64> {
            NonNull::new(self.0.as_ptr().cast_mut()).expect("not null")
        }

        fn len(&self) -> usize {
            4096
        }
    }
}

/// A mapping of another crate's own, without the seal: E0277, whatever the lint allows.
#[cfg(refused = "seal_missing")]
#[allow(unsafe_code)]
mod seal_missing {
    use core::ptr::NonNull;

    struct Mine(Vec<u64>);

    unsafe impl wcontainers_shared_memory::Mapped for Mine {
        fn base(&self) -> NonNull<u64> {
            NonNull::new(self.0.as_ptr().cast_mut()).expect("not null")
        }

        fn len(&self) -> usize {
            4096
        }
    }
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
