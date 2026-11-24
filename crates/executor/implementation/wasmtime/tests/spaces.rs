//! A space's own pages that another space maps stay its own, and a space covers every address ARM64 has.

use wcontainers_abi::LOWEST_ADDRESS as OWN;
use wcontainers_abi::PAGE_SIZE;
use wcontainers_executor::Executor;
use wcontainers_executor::memory_space::{MemorySpace, VolatileBytes};
use wcontainers_executor_wasmtime::{Arm64Executor, TranslatorConfig};
use wcontainers_processor_architecture::ProcessorArchitecture;
use wcontainers_processor_architecture_arm64::Arm64;

type Space = <Arm64Executor as Executor>::GuestMemory;

/// The bytes the owner writes on its own page.
const OWNED: [u8; size_of::<u64>()] = [u8::MAX; size_of::<u64>()];
/// The bytes written on a copy of the owner's page.
const COPIED: [u8; size_of::<u64>()] = [0x5a; size_of::<u64>()];

/// Returns the executor, an owner holding `OWNED` on its page at `OWN`, and a space that maps that page.
fn lent_page() -> (Arm64Executor, Space, Space) {
    let executor = Arm64Executor::new(TranslatorConfig::default());
    let owner = executor.create_guest_memory().expect("a space");
    let mapper = executor.create_guest_memory().expect("a space");
    write(&owner, OWN, &OWNED);
    mapper.map_from(page(), &owner, OWN).expect("the mapping");
    (executor, owner, mapper)
}

/// Returns the addresses of the owner's page.
fn page() -> std::ops::Range<u64> {
    OWN..OWN + PAGE_SIZE
}

fn write(space: &Space, at: u64, bytes: &[u8]) {
    let end = at + bytes.len() as u64;
    assert_eq!(
        space.borrow(at..end).expect("inside").write(bytes),
        bytes.len()
    );
}

/// Returns the bytes at the address, as many as `OWNED` holds.
fn read(space: &Space, at: u64) -> [u8; size_of::<u64>()] {
    let mut into = [u8::MIN; size_of::<u64>()];
    let end = at + into.len() as u64;
    assert_eq!(
        space.borrow(at..end).expect("inside").read(&mut into),
        into.len()
    );
    into
}

#[test]
fn unmap_from_leaves_the_owners_page_another_space_maps() {
    let (_executor, owner, mapper) = lent_page();
    owner.unmap_from(page()).expect("the unmapping");
    assert_eq!(
        read(&owner, OWN),
        OWNED,
        "the owner's own page is no mapping"
    );
    assert_eq!(read(&mapper, OWN), OWNED, "the mapper keeps the page");
    write(&owner, OWN, &COPIED);
    assert_eq!(read(&mapper, OWN), COPIED, "the page is still shared");
}

#[test]
fn unmap_from_on_the_mapping_side_zeroes_it() {
    let (_executor, owner, mapper) = lent_page();
    mapper.unmap_from(page()).expect("the unmapping");
    assert_eq!(read(&mapper, OWN), [u8::MIN; size_of::<u64>()]);
    assert_eq!(read(&owner, OWN), OWNED, "the owner keeps its page");
}

#[test]
fn a_copy_of_a_page_another_space_maps_is_private() {
    let (executor, owner, mapper) = lent_page();
    let copy = executor.create_guest_memory().expect("a space");
    copy.copy_from(page(), &owner, OWN).expect("the copy");
    assert_eq!(read(&copy, OWN), OWNED, "the copy holds the owner's bytes");
    write(&copy, OWN, &COPIED);
    assert_eq!(
        read(&owner, OWN),
        OWNED,
        "the owner unchanged by a write to its copy"
    );
    assert_eq!(
        read(&mapper, OWN),
        OWNED,
        "the mapper unchanged by a write to the copy"
    );
}

#[test]
fn a_copy_of_the_mapping_side_stays_a_mapping() {
    let (executor, owner, mapper) = lent_page();
    let copy = executor.create_guest_memory().expect("a space");
    copy.copy_from(page(), &mapper, OWN).expect("the copy");
    write(&copy, OWN, &COPIED);
    assert_eq!(
        read(&owner, OWN),
        COPIED,
        "a write to the copied mapping reaches its owner"
    );
    assert_eq!(read(&mapper, OWN), COPIED);
}

#[test]
fn the_top_page_of_the_architecture_lies_in_the_space() {
    let executor = Arm64Executor::new(TranslatorConfig::default());
    let space = executor.create_guest_memory().expect("a space");
    let end = Arm64::ADDRESSABLE.end;
    let top = end - PAGE_SIZE..end;
    assert!(
        space.borrow(top.clone()).is_ok(),
        "the top page {top:#x?} was refused"
    );
}
