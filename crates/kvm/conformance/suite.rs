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
    };
}
