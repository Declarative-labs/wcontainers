//! A memory that ends exactly where its mapping ends, with no page mapped just before it or just after it, so that a copy
//! or word access reaching one byte outside faults and ends this test binary. It is a binary of its own, with one test,
//! so that no other test maps pages into the gaps meanwhile.
#![cfg(all(target_os = "linux", not(miri)))]

use std::os::fd::{AsFd, OwnedFd};

use memmap2::{MmapOptions, MmapRaw};
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};
use wcontainers_shared_memory::{HostMapping, SharedMemory};

/// The start of the name of each memory file the test copies at, named so that the process's map lists it.
const NAME: &str = "guard-test-memory";

/// The most tries at getting the three mappings placed next to each other.
const TRIES: usize = 64;

/// The longest copy at each edge.
const LONGEST: usize = 4 * 64 + 1;

/// Returns a memory file of the length, sealed against shrinking.
fn memory_file(name: &str, length: usize) -> OwnedFd {
    let file =
        memfd_create(name, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING).expect("a memory file");
    ftruncate(&file, length as u64).expect("the file grows");
    fcntl_add_seals(&file, SealFlags::SHRINK).expect("the file takes the seal");
    file
}

/// Returns the start and end address of every mapping of this process, and the path each maps, if any.
fn mappings() -> Vec<(usize, usize, String)> {
    std::fs::read_to_string("/proc/self/maps")
        .expect("the process's map")
        .lines()
        .map(|line| {
            let mut fields = line.split_whitespace();
            let range = fields.next().expect("an address range");
            let (start, end) = range.split_once('-').expect("a start and an end");
            let path = fields.nth(4).unwrap_or_default().to_owned();
            let address = |text| usize::from_str_radix(text, 16).expect("a hexadecimal address");
            (address(start), address(end), path)
        })
        .collect()
}

/// Returns whether a mapping of this process covers the address.
fn mapped(address: usize) -> bool {
    mappings()
        .iter()
        .any(|&(start, end, _)| (start..end).contains(&address))
}

#[test]
fn copies_and_words_at_the_edges_of_a_memory_with_no_page_mapped_before_or_after_it() {
    let page = rustix::param::page_size();
    let length = 2 * page;
    let neighbor = memory_file("guard-test-neighbor", page);
    let mut kept = Vec::new();
    for attempt in 0..TRIES {
        let name = format!("{NAME}-{attempt}-");
        let after: MmapRaw = MmapOptions::new()
            .len(page)
            .map_raw(&neighbor)
            .expect("the host maps a page");
        let file = memory_file(&name, length);
        let memory = HostMapping::file(file.as_fd(), 0, length).expect("the host maps the file");
        let before: MmapRaw = MmapOptions::new()
            .len(page)
            .map_raw(&neighbor)
            .expect("the host maps a page");
        let found: Vec<(usize, usize)> = mappings()
            .into_iter()
            .filter(|(_, _, path)| path.contains(&name))
            .map(|(start, end, _)| (start, end))
            .collect();
        let [(start, end)] = found[..] else {
            panic!("one mapping of the memory file: {found:?}");
        };
        assert_eq!(end - start, length);
        if after.as_ptr() as usize != end || before.as_ptr() as usize + page != start {
            // Kept mapped, so that they fill the gaps a next try could otherwise land in.
            kept.push((after, memory, before));
            continue;
        }
        let mut buffer = vec![0_u8; LONGEST];
        let pattern: Vec<u8> = (0..LONGEST).map(|index| index as u8 | 1).collect();
        drop((after, before));
        assert!(
            !mapped(start - 1) && !mapped(end),
            "the pages just before and just after the memory are unmapped"
        );
        for count in 0..=LONGEST {
            for at in [0, length - count] {
                memory.write(at as u64, &pattern[..count]).expect("inside");
                memory
                    .read(at as u64, &mut buffer[..count])
                    .expect("inside");
                assert_eq!(buffer[..count], pattern[..count], "{count} bytes at {at}");
            }
        }
        let last = (length - size_of::<u64>()) as u64;
        let half = (length - size_of::<u32>()) as u64;
        for at in [0, last] {
            memory.store(at, u64::MAX).expect("inside");
            assert_eq!(memory.swap(at, 1_u64), Ok(u64::MAX));
            assert_eq!(memory.compare_exchange(at, 1_u64, 2), Ok(1));
            assert_eq!(memory.load::<u64>(at), Ok(2));
        }
        for at in [0, half] {
            memory.store(at, u32::MAX).expect("inside");
            assert_eq!(memory.swap(at, 1_u32), Ok(u32::MAX));
            assert_eq!(memory.compare_exchange(at, 1_u32, 2), Ok(1));
            assert_eq!(memory.load::<u32>(at), Ok(2));
        }
        assert!(
            !mapped(start - 1) && !mapped(end),
            "the pages just before and just after the memory stayed unmapped"
        );
        eprintln!(
            "copied every length up to {LONGEST} bytes at both edges of {length} bytes at {start:#x}, after {} tries",
            kept.len() + 1
        );
        return;
    }
    panic!("the host never placed the three mappings next to each other in {TRIES} tries");
}
