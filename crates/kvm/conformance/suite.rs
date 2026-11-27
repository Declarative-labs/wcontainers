//! The tests every implementation of wcontainers-kvm runs, written against its traits alone.

use kvm_ioctls::Kvm as KvmSystem;
use wcontainers_kvm::Kvm;
use wcontainers_volatile_bytes::VolatileBytes;

/// The guest-physical address and the length of the memory each test gives a machine.
const GUEST_MEMORY: (u64, usize) = (1 << 30, 1 << 21);

/// Memory given to a machine is zeroed, as long as asked, and holds what is written into it.
pub fn gives_zeroed_memory_that_holds_what_is_written<T: Kvm>(_: T) {
    let Ok(machine) = KvmSystem::new().and_then(|system| system.create_vm()) else {
        panic!("this host has no KVM");
    };
    let (start, length) = GUEST_MEMORY;
    let Ok(memory) = T::set_user_memory_region(&machine, 0, start, length) else {
        panic!("KVM refused the memory");
    };
    assert_eq!(memory.len(), length as u64);
    let mut read = vec![u8::MAX; length];
    assert_eq!(memory.read(&mut read), length);
    assert!(read.iter().all(|byte| *byte == 0), "fresh memory is zeroed");
    let written: Vec<u8> = (0..length).map(|offset| offset as u8).collect();
    assert_eq!(memory.write(&written), length);
    assert_eq!(memory.read(&mut read), length);
    assert!(read == written);
}

/// A second slot over the same guest-physical addresses is refused.
pub fn refuses_overlapping_memory<T: Kvm>(_: T) {
    let Ok(machine) = KvmSystem::new().and_then(|system| system.create_vm()) else {
        panic!("this host has no KVM");
    };
    let (start, length) = GUEST_MEMORY;
    assert!(T::set_user_memory_region(&machine, 0, start, length).is_ok());
    assert!(T::set_user_memory_region(&machine, 1, start, length).is_err());
}

/// Memory given again to another machine holds what was written into it, and the second machine takes it.
pub fn gives_memory_again_to_another_machine<T: Kvm>(_: T) {
    let make_machine = || match KvmSystem::new().and_then(|system| system.create_vm()) {
        Ok(machine) => machine,
        Err(_) => panic!("this host has no KVM"),
    };
    let (start, length) = GUEST_MEMORY;
    let first = make_machine();
    let Ok(memory) = T::set_user_memory_region(&first, 0, start, length) else {
        panic!("KVM refused the memory");
    };
    let written: Vec<u8> = (0..length).map(|offset| !(offset as u8)).collect();
    assert_eq!(memory.write(&written), length);
    drop(first);
    let second = make_machine();
    assert!(T::set_user_memory_region_again(&second, 0, start, &memory).is_ok());
    let mut read = vec![u8::MIN; length];
    assert_eq!(memory.read(&mut read), length);
    assert!(read == written, "memory given again holds what it held");
}

/// The entries asked of KVM's supported CPUID, KVM's own most.
const SUPPORTED_ENTRIES: usize = 80;
/// XCR0's bits of AMX's TILECFG and TILEDATA.
const TILE_COMPONENTS: u32 = 1 << 17 | 1 << 18;

/// KVM offers the tile components in the CPUID it supports once the guest tile permission is granted, and not where it
/// is refused.
pub fn offers_tile_components_once_permitted<T: Kvm>(_: T) {
    let granted = T::request_guest_tile_permission().is_ok();
    let Ok(system) = KvmSystem::new() else {
        panic!("this host has no KVM");
    };
    let Ok(cpuid) = system.get_supported_cpuid(SUPPORTED_ENTRIES) else {
        panic!("KVM gives no supported CPUID");
    };
    let components = cpuid
        .as_slice()
        .iter()
        .find(|entry| entry.function == 0xD && entry.index == 0)
        .map_or(0, |entry| entry.eax);
    assert_eq!(components & TILE_COMPONENTS == TILE_COMPONENTS, granted);
}

/// Runs each suite function above as its own test on a value made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn gives_zeroed_memory_that_holds_what_is_written() {
            $crate::gives_zeroed_memory_that_holds_what_is_written($value);
        }

        #[test]
        fn refuses_overlapping_memory() {
            $crate::refuses_overlapping_memory($value);
        }

        #[test]
        fn gives_memory_again_to_another_machine() {
            $crate::gives_memory_again_to_another_machine($value);
        }

        #[test]
        fn offers_tile_components_once_permitted() {
            $crate::offers_tile_components_once_permitted($value);
        }
    };
}
