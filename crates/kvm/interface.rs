//! [KVM](https://docs.kernel.org/virt/kvm/api.html), the Linux kernel's virtual machines, on an ARM64 host.
#![cfg(target_arch = "aarch64")]

use std::os::fd::{BorrowedFd, OwnedFd};

use kvm_bindings::{
    kvm_create_guest_memfd, kvm_enable_cap, kvm_guest_debug, kvm_irq_routing_entry,
    kvm_memory_attributes, kvm_mp_state, kvm_msi, kvm_pre_fault_memory, kvm_vcpu_events,
    kvm_vcpu_init,
};
use wcontainers_volatile_bytes::VolatileBytes;

/// One method for each ARM64 ioctl of [the API](https://docs.kernel.org/virt/kvm/api.html), on the object it is made on.
#[allow(
    reviewable::explicit_shared_error::Errno,
    reason = "every KVM ioctl fails the same way, with the kernel's error number"
)]
pub trait Kvm {
    type System;
    type Vm;
    type Vcpu;
    type Device;
    type UserMemory: VolatileBytes;
    /// A vCPU's [`kvm_run`](https://docs.kernel.org/virt/kvm/api.html#the-kvm-run-structure) page and [coalesced MMIO ring](https://docs.kernel.org/virt/kvm/api.html#kvm-un-register-coalesced-mmio).
    type RunPage: VolatileBytes;
    /// A vCPU's [dirty ring](https://docs.kernel.org/virt/kvm/api.html#kvm-cap-dirty-log-ring-kvm-cap-dirty-log-ring-acq-rel).
    type DirtyRing: VolatileBytes;

    /// Opens [`/dev/kvm`](https://docs.kernel.org/virt/kvm/api.html#general-description).
    fn open(&self) -> Result<Self::System, Errno>;

    /// Returns [the API's version](https://docs.kernel.org/virt/kvm/api.html#kvm-get-api-version).
    fn get_api_version(&self, system: &Self::System) -> Result<i32, Errno>;

    /// Returns [the system's answer for an extension](https://docs.kernel.org/virt/kvm/api.html#kvm-check-extension).
    fn check_extension(&self, system: &Self::System, extension: u32) -> Result<i32, Errno>;

    /// Returns [the length of a vCPU's run page](https://docs.kernel.org/virt/kvm/api.html#kvm-get-vcpu-mmap-size).
    fn get_vcpu_mmap_size(&self, system: &Self::System) -> Result<usize, Errno>;

    /// Makes [a VM](https://docs.kernel.org/virt/kvm/api.html#kvm-create-vm) of the machine type, on ARM64 its address size.
    fn create_vm(&self, system: &Self::System, machine_type: u64) -> Result<Self::Vm, Errno>;

    /// Returns [the VM's answer for an extension](https://docs.kernel.org/virt/kvm/api.html#kvm-check-extension).
    fn vm_check_extension(&self, vm: &Self::Vm, extension: u32) -> Result<i32, Errno>;

    /// Maps zeroed memory of the length, in whole pages, for a slot.
    fn create_user_memory(&self, length: usize) -> Result<Self::UserMemory, Errno>;

    /// Gives the VM [the memory as a slot](https://docs.kernel.org/virt/kvm/api.html#kvm-set-user-memory-region); none takes it away.
    fn set_user_memory_region(
        &self,
        vm: &Self::Vm,
        slot: u32,
        flags: u32,
        guest_phys_addr: u64,
        memory: Option<&Self::UserMemory>,
    ) -> Result<(), Errno>;

    /// Gives the VM [the memory and a guest_memfd as a slot](https://docs.kernel.org/virt/kvm/api.html#kvm-set-user-memory-region2); size 0 takes it away.
    #[expect(
        clippy::too_many_arguments,
        reason = "one argument for each field of kvm_userspace_memory_region2"
    )]
    fn set_user_memory_region2(
        &self,
        vm: &Self::Vm,
        slot: u32,
        flags: u32,
        guest_phys_addr: u64,
        memory_size: u64,
        memory: Option<&Self::UserMemory>,
        guest_memfd: Option<BorrowedFd<'_>>,
        guest_memfd_offset: u64,
    ) -> Result<(), Errno>;

    /// Returns [the pages written in a slot](https://docs.kernel.org/virt/kvm/api.html#kvm-get-dirty-log) since the last call.
    fn get_dirty_log(&self, vm: &Self::Vm, slot: u32) -> Result<Vec<u64>, Errno>;

    /// Marks [the pages of a slot clean](https://docs.kernel.org/virt/kvm/api.html#kvm-clear-dirty-log) whose bits are set.
    fn clear_dirty_log(
        &self,
        vm: &Self::Vm,
        slot: u32,
        first_page: u64,
        num_pages: u32,
        dirty_bitmap: &[u64],
    ) -> Result<(), Errno>;

    /// Gives [the dirty rings](https://docs.kernel.org/virt/kvm/api.html#kvm-cap-dirty-log-ring-kvm-cap-dirty-log-ring-acq-rel) back the entries marked reset, and returns the number of entries reset.
    fn reset_dirty_rings(&self, vm: &Self::Vm) -> Result<i32, Errno>;

    /// Makes [a vCPU](https://docs.kernel.org/virt/kvm/api.html#kvm-create-vcpu) of the VM.
    fn create_vcpu(&self, vm: &Self::Vm, vcpu_id: u64) -> Result<Self::Vcpu, Errno>;

    /// Makes [a device](https://docs.kernel.org/virt/kvm/api.html#kvm-create-device) of the type in the VM.
    fn create_device(&self, vm: &Self::Vm, device_type: u32) -> Result<Self::Device, Errno>;

    /// Makes [the VM's interrupt controller](https://docs.kernel.org/virt/kvm/api.html#kvm-create-irqchip), on ARM64 a GICv2.
    fn create_irq_chip(&self, vm: &Self::Vm) -> Result<(), Errno>;

    /// Places [an interrupt controller device](https://docs.kernel.org/virt/kvm/api.html#kvm-arm-set-device-addr-deprecated) at the address.
    fn arm_set_device_addr(&self, vm: &Self::Vm, id: u64, addr: u64) -> Result<(), Errno>;

    /// Sets [an interrupt line's level](https://docs.kernel.org/virt/kvm/api.html#kvm-irq-line).
    fn irq_line(&self, vm: &Self::Vm, irq: u32, level: bool) -> Result<(), Errno>;

    /// Sets [an interrupt line's level](https://docs.kernel.org/virt/kvm/api.html#kvm-irq-line); returns the status KVM writes back.
    fn irq_line_status(&self, vm: &Self::Vm, irq: u32, level: bool) -> Result<i32, Errno>;

    /// Ties [an event file to an interrupt](https://docs.kernel.org/virt/kvm/api.html#kvm-irqfd), or unties it.
    fn irqfd(
        &self,
        vm: &Self::Vm,
        eventfd: BorrowedFd<'_>,
        gsi: u32,
        flags: u32,
        resamplefd: Option<BorrowedFd<'_>>,
    ) -> Result<(), Errno>;

    /// Ties [an event file to guest writes at an address](https://docs.kernel.org/virt/kvm/api.html#kvm-ioeventfd), or unties it.
    fn ioeventfd(
        &self,
        vm: &Self::Vm,
        eventfd: BorrowedFd<'_>,
        addr: u64,
        len: u32,
        datamatch: u64,
        flags: u32,
    ) -> Result<(), Errno>;

    /// Replaces [the VM's interrupt routes](https://docs.kernel.org/virt/kvm/api.html#kvm-set-gsi-routing) with the entries.
    fn set_gsi_routing(&self, vm: &Self::Vm, entries: &[IrqRoutingEntry]) -> Result<(), Errno>;

    /// Sends [a message signaled interrupt](https://docs.kernel.org/virt/kvm/api.html#kvm-signal-msi); returns whether the guest took it.
    fn signal_msi(&self, vm: &Self::Vm, msi: &Msi) -> Result<i32, Errno>;

    /// Gathers [guest writes to a range](https://docs.kernel.org/virt/kvm/api.html#kvm-un-register-coalesced-mmio).
    fn register_coalesced_mmio(
        &self,
        vm: &Self::Vm,
        addr: u64,
        size: u32,
        pio: bool,
    ) -> Result<(), Errno>;

    /// Ends [the gathering of writes to a range](https://docs.kernel.org/virt/kvm/api.html#kvm-un-register-coalesced-mmio).
    fn unregister_coalesced_mmio(
        &self,
        vm: &Self::Vm,
        addr: u64,
        size: u32,
        pio: bool,
    ) -> Result<(), Errno>;

    /// Turns on [a capability of the VM](https://docs.kernel.org/virt/kvm/api.html#kvm-enable-cap).
    fn enable_cap(&self, vm: &Self::Vm, cap: &EnableCap) -> Result<(), Errno>;

    /// Sets [the attributes of guest memory](https://docs.kernel.org/virt/kvm/api.html#kvm-set-memory-attributes) in a range.
    fn set_memory_attributes(
        &self,
        vm: &Self::Vm,
        attributes: &MemoryAttributes,
    ) -> Result<(), Errno>;

    /// Makes [guest memory the host cannot map](https://docs.kernel.org/virt/kvm/api.html#kvm-create-guest-memfd).
    fn create_guest_memfd(
        &self,
        vm: &Self::Vm,
        guest_memfd: &CreateGuestMemfd,
    ) -> Result<OwnedFd, Errno>;

    /// Returns [the vCPU target and features](https://docs.kernel.org/virt/kvm/api.html#kvm-arm-preferred-target) best for this host.
    fn arm_preferred_target(&self, vm: &Self::Vm) -> Result<VcpuInit, Errno>;

    /// Sets [what the VM's counters lag the host's by](https://docs.kernel.org/virt/kvm/api.html#kvm-arm-set-counter-offset).
    fn arm_set_counter_offset(&self, vm: &Self::Vm, counter_offset: u64) -> Result<(), Errno>;

    /// Copies [the tags of guest memory](https://docs.kernel.org/virt/kvm/api.html#kvm-arm-mte-copy-tags), a byte a granule, as the flags say; returns the number of guest bytes covered.
    fn arm_mte_copy_tags(
        &self,
        vm: &Self::Vm,
        guest_ipa: u64,
        flags: u64,
        tags: &mut [u8],
    ) -> Result<u64, Errno>;

    /// Returns [the bits the host may write](https://docs.kernel.org/virt/kvm/api.html#kvm-arm-get-reg-writable-masks) of each register in the range.
    fn arm_get_reg_writable_masks(&self, vm: &Self::Vm, range: u32) -> Result<Vec<u64>, Errno>;

    /// Returns [the VM's statistics](https://docs.kernel.org/virt/kvm/api.html#kvm-get-stats-fd) as a file.
    fn vm_get_stats_fd(&self, vm: &Self::Vm) -> Result<OwnedFd, Errno>;

    /// Sets [an attribute of the VM](https://docs.kernel.org/virt/kvm/devices/vm.html) to the value.
    fn vm_set_device_attr(
        &self,
        vm: &Self::Vm,
        group: u32,
        attr: u64,
        value: &[u8],
    ) -> Result<(), Errno>;

    /// Reads [an attribute of the VM](https://docs.kernel.org/virt/kvm/devices/vm.html) into the value.
    fn vm_get_device_attr(
        &self,
        vm: &Self::Vm,
        group: u32,
        attr: u64,
        value: &mut [u8],
    ) -> Result<(), Errno>;

    /// Returns whether [the VM has the attribute](https://docs.kernel.org/virt/kvm/api.html#kvm-has-device-attr).
    fn vm_has_device_attr(&self, vm: &Self::Vm, group: u32, attr: u64) -> Result<bool, Errno>;

    fn vcpu_run_page<'vcpu>(&self, vcpu: &'vcpu Self::Vcpu) -> &'vcpu Self::RunPage;

    /// Returns the vCPU's dirty ring, if dirty rings were on.
    fn vcpu_dirty_ring<'vcpu>(&self, vcpu: &'vcpu Self::Vcpu) -> Option<&'vcpu Self::DirtyRing>;

    /// Runs [the vCPU](https://docs.kernel.org/virt/kvm/api.html#kvm-run) until it exits.
    fn run(&self, vcpu: &Self::Vcpu) -> Result<(), Errno>;

    /// Reads [a register](https://docs.kernel.org/virt/kvm/api.html#kvm-get-one-reg) into bytes of the length its id says.
    fn get_one_reg(&self, vcpu: &Self::Vcpu, id: u64, value: &mut [u8]) -> Result<(), Errno>;

    /// Sets [a register](https://docs.kernel.org/virt/kvm/api.html#kvm-set-one-reg) from bytes of the length its id says.
    fn set_one_reg(&self, vcpu: &Self::Vcpu, id: u64, value: &[u8]) -> Result<(), Errno>;

    /// Returns [the ids of the vCPU's registers](https://docs.kernel.org/virt/kvm/api.html#kvm-get-reg-list).
    fn get_reg_list(&self, vcpu: &Self::Vcpu) -> Result<Vec<u64>, Errno>;

    /// Resets [the vCPU to the target and features](https://docs.kernel.org/virt/kvm/api.html#kvm-arm-vcpu-init).
    fn arm_vcpu_init(&self, vcpu: &Self::Vcpu, init: &VcpuInit) -> Result<(), Errno>;

    /// Fixes [a feature's configuration](https://docs.kernel.org/virt/kvm/api.html#kvm-arm-vcpu-finalize) before the first run.
    fn arm_vcpu_finalize(&self, vcpu: &Self::Vcpu, feature: i32) -> Result<(), Errno>;

    /// Returns [whether the vCPU runs or stops](https://docs.kernel.org/virt/kvm/api.html#kvm-get-mp-state).
    fn get_mp_state(&self, vcpu: &Self::Vcpu) -> Result<MpState, Errno>;

    /// Sets [whether the vCPU runs or stops](https://docs.kernel.org/virt/kvm/api.html#kvm-set-mp-state).
    fn set_mp_state(&self, vcpu: &Self::Vcpu, mp_state: &MpState) -> Result<(), Errno>;

    /// Returns [the vCPU's pending exceptions](https://docs.kernel.org/virt/kvm/api.html#kvm-get-vcpu-events).
    fn get_vcpu_events(&self, vcpu: &Self::Vcpu) -> Result<VcpuEvents, Errno>;

    /// Sets [the vCPU's pending exceptions](https://docs.kernel.org/virt/kvm/api.html#kvm-set-vcpu-events).
    fn set_vcpu_events(&self, vcpu: &Self::Vcpu, events: &VcpuEvents) -> Result<(), Errno>;

    /// Sets [how the host debugs the vCPU](https://docs.kernel.org/virt/kvm/api.html#kvm-set-guest-debug).
    fn set_guest_debug(&self, vcpu: &Self::Vcpu, debug: &GuestDebug) -> Result<(), Errno>;

    /// Sets [the signals blocked while the vCPU runs](https://docs.kernel.org/virt/kvm/api.html#kvm-set-signal-mask).
    fn set_signal_mask(&self, vcpu: &Self::Vcpu, sigset: Option<&[u8]>) -> Result<(), Errno>;

    /// Maps [guest memory ahead of the guest](https://docs.kernel.org/virt/kvm/api.html#kvm-pre-fault-memory).
    fn pre_fault_memory(&self, vcpu: &Self::Vcpu, range: &mut PreFaultMemory) -> Result<(), Errno>;

    /// Returns [the vCPU's statistics](https://docs.kernel.org/virt/kvm/api.html#kvm-get-stats-fd) as a file.
    fn vcpu_get_stats_fd(&self, vcpu: &Self::Vcpu) -> Result<OwnedFd, Errno>;

    /// Sets [an attribute of the vCPU](https://docs.kernel.org/virt/kvm/devices/vcpu.html) to the value.
    fn vcpu_set_device_attr(
        &self,
        vcpu: &Self::Vcpu,
        group: u32,
        attr: u64,
        value: &[u8],
    ) -> Result<(), Errno>;

    /// Reads [an attribute of the vCPU](https://docs.kernel.org/virt/kvm/devices/vcpu.html) into the value.
    fn vcpu_get_device_attr(
        &self,
        vcpu: &Self::Vcpu,
        group: u32,
        attr: u64,
        value: &mut [u8],
    ) -> Result<(), Errno>;

    /// Returns whether [the vCPU has the attribute](https://docs.kernel.org/virt/kvm/api.html#kvm-has-device-attr).
    fn vcpu_has_device_attr(&self, vcpu: &Self::Vcpu, group: u32, attr: u64)
    -> Result<bool, Errno>;

    /// Sets [an attribute of the device](https://docs.kernel.org/virt/kvm/api.html#kvm-set-device-attr-kvm-get-device-attr) to the value.
    fn set_device_attr(
        &self,
        device: &Self::Device,
        group: u32,
        attr: u64,
        value: &[u8],
    ) -> Result<(), Errno>;

    /// Reads [an attribute of the device](https://docs.kernel.org/virt/kvm/api.html#kvm-set-device-attr-kvm-get-device-attr) into the value.
    fn get_device_attr(
        &self,
        device: &Self::Device,
        group: u32,
        attr: u64,
        value: &mut [u8],
    ) -> Result<(), Errno>;

    /// Returns whether [the device has the attribute](https://docs.kernel.org/virt/kvm/api.html#kvm-has-device-attr).
    fn has_device_attr(&self, device: &Self::Device, group: u32, attr: u64) -> Result<bool, Errno>;
}

pub type Errno = rustix::io::Errno;

pub type IrqRoutingEntry = kvm_irq_routing_entry;

pub type Msi = kvm_msi;

pub type EnableCap = kvm_enable_cap;

pub type MemoryAttributes = kvm_memory_attributes;

pub type CreateGuestMemfd = kvm_create_guest_memfd;

pub type VcpuInit = kvm_vcpu_init;

pub type MpState = kvm_mp_state;

pub type VcpuEvents = kvm_vcpu_events;

pub type GuestDebug = kvm_guest_debug;

pub type PreFaultMemory = kvm_pre_fault_memory;

/// The bytes of guest memory each [allocation tag](https://docs.kernel.org/virt/kvm/api.html#:~:text=granules%20in%20MTE%20are%2016%20bytes%20long) covers.
pub const MTE_GRANULE_BYTES: u64 = 16;
