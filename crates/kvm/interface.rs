//! The calls of [KVM](https://docs.kernel.org/virt/kvm/api.html) that kvm-ioctls leaves unsafe, made safe.

use kvm_ioctls::VmFd;
use wcontainers_volatile_bytes::VolatileBytes;

/// [KVM](https://docs.kernel.org/virt/kvm/api.html), whose guest memory stays mapped until the process ends once a machine is given it.
pub trait Kvm {
    type GuestMemory: VolatileBytes;
    type SetUserMemoryRegionError;

    /// Returns zeroed memory that the machine sees from the guest-physical address on, as its [memory slot](https://docs.kernel.org/virt/kvm/api.html#kvm-set-user-memory-region).
    fn set_user_memory_region(
        machine: &VmFd,
        slot: u32,
        guest_physical_address: u64,
        length: usize,
    ) -> Result<Self::GuestMemory, Self::SetUserMemoryRegionError>;
}
