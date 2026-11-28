//! A mapping with no page mapped just before it or just after it, so that a copy reaching one byte outside faults and
//! ends this test binary: a binary of its own, so that no other test maps pages into the gaps meanwhile.
#![cfg(all(target_os = "linux", not(miri)))]

use std::os::fd::{AsFd, OwnedFd};

use memmap2::{MmapOptions, MmapRaw};
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_shared_mapping::{Mapped, SharedMapping};

fn memory_file(name: &str, length: usize) -> OwnedFd {
    let file = memfd_create(name, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING).expect("a memory file");
    ftruncate(&file, length as u64).expect("the file grows");
    fcntl_add_seals(&file, SealFlags::SHRINK).expect("the file takes the seal");
    file
}

/// Returns the start and end address of every mapping of this process, and the path it maps.
fn mappings() -> Vec<(usize, usize, String)> {
    let maps = std::fs::read_to_string("/proc/self/maps").expect("the process's map");
    maps.lines()
        .map(|line| {
            let mut fields = line.split_whitespace();
            let (start, end) = fields.next().and_then(|range| range.split_once('-')).expect("an address range");
            let address = |text| usize::from_str_radix(text, 16).expect("a hexadecimal address");
            (address(start), address(end), fields.nth(4).unwrap_or_default().to_owned())
        })
        .collect()
}

fn mapped(address: usize) -> bool {
    mappings().iter().any(|&(start, end, _)| (start..end).contains(&address))
}

#[test]
fn copies_at_both_edges_of_a_mapping_with_no_page_mapped_beside_it() {
    let page = rustix::param::page_size();
    let length = 2 * page;
    let neighbor = memory_file("guard-neighbor", page);
    let mut kept = Vec::new();
    for attempt in 0..64 {
        let name = format!("guard-memory-{attempt}-");
        let after: MmapRaw = MmapOptions::new().len(page).map_raw(&neighbor).expect("a page");
        let file = memory_file(&name, length);
        let memory = SharedMapping::new(Mapped::file(file.as_fd(), 0, length).expect("the host maps the file"));
        let before: MmapRaw = MmapOptions::new().len(page).map_raw(&neighbor).expect("a page");
        let found: Vec<_> = mappings().into_iter().filter(|(_, _, path)| path.contains(&name)).collect();
        let [(start, end, _)] = found[..] else {
            panic!("one mapping of the memory file: {found:?}");
        };
        if after.as_ptr() as usize != end || before.as_ptr() as usize + page != start {
            // Kept mapped, so that they fill the gaps a next try could land in.
            kept.push((after, memory, before));
            continue;
        }
        drop((after, before));
        assert!(!mapped(start - 1) && !mapped(end), "the pages beside the mapping are unmapped");
        let pattern: Vec<u8> = (0..length).map(|index| (index * 7) as u8 | 1).collect();
        let mut buffer = vec![0; length];
        for count in (0..=257).chain([511, 512, 513, 4095, 4096, 4097, length - 1, length]) {
            for at in [0, length - count] {
                assert_eq!(memory.write_at(at as u64, &pattern[..count]), count);
                assert_eq!(memory.read_at(at as u64, &mut buffer[..count]), count);
                assert_eq!(buffer[..count], pattern[..count], "{count} bytes at {at}");
            }
        }
        assert!(!mapped(start - 1) && !mapped(end), "the pages beside the mapping stayed unmapped");
        return;
    }
    panic!("the host never placed the three mappings next to each other");
}
