//! The calls of [KVM](https://docs.kernel.org/virt/kvm/api.html) that kvm-ioctls leaves unsafe, made safe.

use kvm_ioctls::VmFd;
use wcontainers_volatile_bytes::VolatileBytes;

/// [KVM](https://docs.kernel.org/virt/kvm/api.html), whose guest memory stays mapped until the process ends once a machine is given it.
pub trait Kvm {
    type GuestMemory: VolatileBytes;
    type SetUserMemoryRegionError;
    type SetUserMemoryRegionAgainError;

    /// Returns zeroed memory that the machine sees from the guest-physical address on, as its [memory slot](https://docs.kernel.org/virt/kvm/api.html#kvm-set-user-memory-region).
    fn set_user_memory_region(
        machine: &VmFd,
        slot: u32,
        guest_physical_address: u64,
        length: usize,
    ) -> Result<Self::GuestMemory, Self::SetUserMemoryRegionError>;

    /// Gives the machine, as its memory slot, memory an earlier machine was given, which then stays mapped until the process ends.
    ///
    /// The memory holds what it held; a machine that is gone reaches it no more.
    fn set_user_memory_region_again(
        machine: &VmFd,
        slot: u32,
        guest_physical_address: u64,
        memory: &Self::GuestMemory,
    ) -> Result<(), Self::SetUserMemoryRegionAgainError>;
}
