//! The KVM calls kvm-ioctls leaves unsafe, made on fresh anonymous memory that is never unmapped.

use std::io;
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

/// Why Linux refused the guest tile permission.
#[derive(Debug, Error)]
#[error("Linux refused the guest tile permission: {0}")]
pub struct RequestGuestTilePermissionError(#[from] io::Error);

impl Kvm for DirectKvm {
    type GuestMemory = SharedMapping;
    type SetUserMemoryRegionError = SetUserMemoryRegionError;
    type SetUserMemoryRegionAgainError = SetUserMemoryRegionAgainError;
    type RequestGuestTilePermissionError = RequestGuestTilePermissionError;

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

    fn request_guest_tile_permission() -> Result<(), RequestGuestTilePermissionError> {
        Ok(request_guest_permission(TILE_DATA_COMPONENT)?)
    }
}

/// XSAVE's component of AMX's tile data, the last of the tile components, which asks for TILECFG too.
const TILE_DATA_COMPONENT: u64 = 18;

/// The `arch_prctl` request that lets the virtual CPUs a process makes hold the state components up to one.
#[cfg(target_arch = "x86_64")]
const REQUEST_GUEST_PERMISSION: u64 = 0x1025;

/// Asks Linux to let the virtual CPUs this process makes hold the state components up to `component`.
#[cfg(target_arch = "x86_64")]
#[expect(
    unsafe_code,
    reason = "arch_prctl changes one permission of this process only, which state components its later virtual CPUs may hold; it reads and writes no memory of the process"
)]
fn request_guest_permission(component: u64) -> io::Result<()> {
    // SAFETY: this request takes two numbers by value and neither reads nor writes the process's memory.
    let result = unsafe { libc::syscall(libc::SYS_arch_prctl, REQUEST_GUEST_PERMISSION, component) };
    if result == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

/// No other architecture has tile state to give.
#[cfg(not(target_arch = "x86_64"))]
fn request_guest_permission(_component: u64) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
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
