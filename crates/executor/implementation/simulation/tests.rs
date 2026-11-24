use wcontainers_abi::{LOWEST_ADDRESS, PAGE_SIZE, Permissions, WORD};
use wcontainers_executor::Executor;
use wcontainers_executor::memory_space::{
    CopyMemoryError, InvalidAddressError, MapMemoryError, MemorySpace, VolatileBytes,
};
use wcontainers_precise_instruction_set_apple_m5::AppleM5;
use wcontainers_processor_architecture_arm64::syndrome::UNDEFINED_INSTRUCTION;
use wcontainers_processor_architecture_arm64::{Processor, ZERO_PROCESSOR};

use crate::Simulation;
use crate::memory::SimulatedMemory;

const PAGE: u64 = LOWEST_ADDRESS;

#[test]
fn word_no_listed_instruction_matches_is_undefined() {
    let mut chip = Simulation::new(AppleM5);
    let memory = SimulatedMemory::default();
    let allowed = Permissions::READ | Permissions::EXECUTE;
    assert_eq!(
        memory.set_permissions(PAGE..PAGE + PAGE_SIZE, allowed),
        Ok(())
    );
    let start = Processor {
        program_counter: PAGE,
        ..ZERO_PROCESSOR
    };
    let deltas = memory.to_memory_deltas();
    let mut stopped = start.clone();
    chip.execute_single_instruction_in_place(&mut stopped, &deltas, |_| {});
    assert_eq!(
        stopped,
        Processor {
            exception: Some(UNDEFINED_INSTRUCTION),
            ..start
        }
    );
}

#[test]
fn page_methods_refuse_addresses_inside_a_page() {
    let memory = SimulatedMemory::default();
    let other = SimulatedMemory::default();
    let inside = PAGE + WORD..PAGE + PAGE_SIZE;
    assert_eq!(
        memory.set_permissions(inside.clone(), Permissions::READ),
        Err(InvalidAddressError)
    );
    assert_eq!(
        memory.map_from(inside.clone(), &other, PAGE + WORD),
        Err(MapMemoryError::InvalidAddress(InvalidAddressError))
    );
    assert_eq!(memory.unmap_from(inside.clone()), Err(InvalidAddressError));
    assert_eq!(
        memory.copy_from(PAGE..PAGE + PAGE_SIZE, &other, PAGE + WORD),
        Err(CopyMemoryError::InvalidAddress(InvalidAddressError))
    );
}

#[test]
fn unmap_from_leaves_a_page_no_mapping_shares() {
    let memory = SimulatedMemory::default();
    let written = u64::MAX.to_le_bytes();
    let bytes = memory.borrow(PAGE..PAGE + WORD).expect("inside the space");
    assert_eq!(bytes.write(&written), written.len());
    assert_eq!(memory.unmap_from(PAGE..PAGE + PAGE_SIZE), Ok(()));
    let mut read = [u8::MIN; WORD as usize];
    assert_eq!(bytes.read(&mut read), read.len());
    assert_eq!(read, written);
}

#[test]
fn unmap_from_leaves_a_page_another_space_maps() {
    let owner = SimulatedMemory::default();
    let other = SimulatedMemory::default();
    let written = u64::MAX.to_le_bytes();
    let whole = PAGE..PAGE + PAGE_SIZE;
    let bytes = owner.borrow(PAGE..PAGE + WORD).expect("inside the space");
    assert_eq!(bytes.write(&written), written.len());
    assert_eq!(other.map_from(whole.clone(), &owner, PAGE), Ok(()));
    assert_eq!(owner.unmap_from(whole), Ok(()));
    let mut read = [u8::MIN; WORD as usize];
    assert_eq!(bytes.read(&mut read), read.len());
    assert_eq!(read, written);
    let mapped = other.borrow(PAGE..PAGE + WORD).expect("inside the space");
    assert_eq!(mapped.read(&mut read), read.len());
    assert_eq!(read, written);
}
