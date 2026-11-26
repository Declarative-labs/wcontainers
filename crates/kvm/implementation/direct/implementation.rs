//! The KVM calls kvm-ioctls leaves unsafe, made on fresh anonymous memory that is never unmapped.

use std::mem;

use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::VmFd;
use thiserror::Error;
use wcontainers_kvm::Kvm;
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_shared_mapping::{MapError, Mapped, SharedMapping};

/// KVM, called directly through kvm-ioctls.
pub struct DirectKvm;

#[derive(Debug, Error)]
pub enum SetUserMemoryRegionError {
    #[error("the host refused the memory: {0}")]
    Mapping(#[from] MapError),
    #[error("KVM refused the memory slot: {0}")]
    Refused(#[from] kvm_ioctls::Error),
}

/// Why KVM refused memory given again.
#[derive(Debug, Error)]
#[error("KVM refused the memory slot: {0}")]
pub struct SetUserMemoryRegionAgainError(#[from] kvm_ioctls::Error);

impl Kvm for DirectKvm {
    type GuestMemory = SharedMapping;
    type SetUserMemoryRegionError = SetUserMemoryRegionError;
    type SetUserMemoryRegionAgainError = SetUserMemoryRegionAgainError;

    /// Maps fresh anonymous memory and keeps one view of it for good, so that it is never unmapped.
    fn set_user_memory_region(
        machine: &VmFd,
        slot: u32,
        guest_physical_address: u64,
        length: usize,
    ) -> Result<SharedMapping, SetUserMemoryRegionError> {
        let memory = SharedMapping::new(Mapped::anonymous(length)?);
        let region = kvm_userspace_memory_region {
            slot,
            flags: 0,
            guest_phys_addr: guest_physical_address,
            memory_size: memory.len(),
            userspace_addr: memory.address() as u64,
        };
        // The machine, and every virtual CPU made from it, may reach the memory until the process ends.
        mem::forget(memory.part(0..memory.len()));
        register(machine, region)?;
        Ok(memory)
    }

    /// Keeps one more view of the memory for good, so that it stays mapped whatever made it, and registers it.
    fn set_user_memory_region_again(
        machine: &VmFd,
        slot: u32,
        guest_physical_address: u64,
        memory: &SharedMapping,
    ) -> Result<(), SetUserMemoryRegionAgainError> {
        let region = kvm_userspace_memory_region {
            slot,
            flags: 0,
            guest_phys_addr: guest_physical_address,
            memory_size: memory.len(),
            userspace_addr: memory.address() as u64,
        };
        mem::forget(memory.part(0..memory.len()));
        register(machine, region)?;
        Ok(())
    }
}

/// Gives the machine the region; a view of its memory was forgotten, so the memory is never unmapped.
#[expect(
    unsafe_code,
    reason = "KVM keeps the host addresses for as long as the machine lives, so they must stay mapped"
)]
fn register(machine: &VmFd, region: kvm_userspace_memory_region) -> Result<(), kvm_ioctls::Error> {
    // SAFETY: the region's host addresses are a mapping of which a view was forgotten, so it stays mapped, readable and
    // writable until the process ends, longer than any machine can use it.
    unsafe { machine.set_user_memory_region(region) }
}
