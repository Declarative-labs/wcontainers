//! KVM's ioctls on this host's `/dev/kvm`, through kvm-ioctls where it is sound and marked raw ioctls where it is not.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::collections::BTreeMap;
use std::ffi::{CStr, c_ulong};
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::sync::{Arc, Mutex, PoisonError};

use kvm_bindings::{
    KVMIO, KvmIrqRouting, RegList, kvm_arm_counter_offset, kvm_create_device, kvm_device_attr,
    kvm_ioeventfd, kvm_irqfd, kvm_reg_list, kvm_smccc_filter, kvm_userspace_memory_region,
};
use kvm_ioctls::{DeviceFd, IoEventAddress, VcpuFd, VmFd};
use memmap2::{MmapOptions, MmapRaw};
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};
use vmm_sys_util::fam::FamStruct;
use vmm_sys_util::ioctl::{_IOC_NONE, _IOC_WRITE, ioctl, ioctl_expr, ioctl_with_ref};
use wcontainers_kvm::{
    CreateGuestMemfd, EnableCap, Errno, GuestDebug, IrqRoutingEntry, Kvm, MemoryAttributes,
    MpState, Msi, VcpuEvents, VcpuInit,
};
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_pointer::{PointerAndLength, PointerBytes};
use wcontainers_volatile_bytes_shared_mapping::{MapError, Mapped, SharedMapping};

/// The number of `KVM_RUN`, which kvm-ioctls keeps to itself.
///
/// https://github.com/torvalds/linux/blob/v6.12/include/uapi/linux/kvm.h
const KVM_RUN: c_ulong = ioctl_expr(_IOC_NONE, KVMIO, 0x80, u32::MIN);

/// The number of `KVM_IRQFD`, which kvm-ioctls keeps to itself.
///
/// https://github.com/torvalds/linux/blob/v6.12/include/uapi/linux/kvm.h
const KVM_IRQFD: c_ulong = ioctl_expr(_IOC_WRITE, KVMIO, 0x76, size_of::<kvm_irqfd>() as u32);

/// The number of `KVM_IOEVENTFD`, which kvm-ioctls keeps to itself.
///
/// https://github.com/torvalds/linux/blob/v6.12/include/uapi/linux/kvm.h
const KVM_IOEVENTFD: c_ulong =
    ioctl_expr(_IOC_WRITE, KVMIO, 0x79, size_of::<kvm_ioeventfd>() as u32);

/// The number of `KVM_SET_DEVICE_ATTR`, which kvm-ioctls keeps to itself.
///
/// https://github.com/torvalds/linux/blob/v6.12/include/uapi/linux/kvm.h
const KVM_SET_DEVICE_ATTR: c_ulong =
    ioctl_expr(_IOC_WRITE, KVMIO, 0xe1, size_of::<kvm_device_attr>() as u32);

/// The number of `KVM_GET_DEVICE_ATTR`, which kvm-ioctls keeps to itself.
///
/// https://github.com/torvalds/linux/blob/v6.12/include/uapi/linux/kvm.h
const KVM_GET_DEVICE_ATTR: c_ulong =
    ioctl_expr(_IOC_WRITE, KVMIO, 0xe2, size_of::<kvm_device_attr>() as u32);

/// The number of `KVM_ARM_SET_COUNTER_OFFSET`, which kvm-ioctls does not have.
///
/// https://github.com/torvalds/linux/blob/v6.12/include/uapi/linux/kvm.h
const KVM_ARM_SET_COUNTER_OFFSET: c_ulong = ioctl_expr(
    _IOC_WRITE,
    KVMIO,
    0xb5,
    size_of::<kvm_arm_counter_offset>() as u32,
);

/// The bytes every attribute value is passed in: no attribute an ARM64 device documents is longer than the SMCCC filter.
///
/// https://docs.kernel.org/virt/kvm/devices/vm.html#attribute-kvm-arm-vm-smccc-filter-w-only
const ATTRIBUTE_VALUE_BYTES: usize = size_of::<kvm_smccc_filter>();

/// The name each memory file is made with, which shows in `/proc/<pid>/maps`.
const USER_MEMORY_NAME: &CStr = c"wcontainers-kvm-user-memory";

/// KVM, called on this host through `/dev/kvm`.
pub struct DirectKvm;

/// An open `/dev/kvm`.
pub struct KvmSystem(kvm_ioctls::Kvm);

/// A VM, which keeps every memory it holds in a slot mapped while it, a vCPU or a device of it lives.
pub struct KvmVm(Arc<Machine>);

/// A vCPU, with a view of its run page.
pub struct KvmVcpu {
    fd: VcpuFd,
    run: PointerBytes,
    _run_mapping: MmapRaw,
    _machine: Arc<Machine>,
}

/// A device of a VM.
pub struct KvmDevice {
    fd: DeviceFd,
    _machine: Arc<Machine>,
}

/// A VM's descriptor and a view of the memory each of its slots holds.
struct Machine {
    fd: VmFd,
    slots: Mutex<BTreeMap<u32, SharedMapping>>,
}

/// Returns the kernel's error number in kvm-ioctls' error.
fn errno(error: kvm_ioctls::Error) -> Errno {
    Errno::from_raw_os_error(error.errno())
}

/// Returns the error number in an error of the standard library.
fn io_errno(error: &io::Error) -> Errno {
    Errno::from_io_error(error).unwrap_or(Errno::IO)
}

/// Returns the ioctl's result, or the error number it left when it failed.
fn returned(result: i32) -> Result<i32, Errno> {
    if result < 0 {
        Err(io_errno(&io::Error::last_os_error()))
    } else {
        Ok(result)
    }
}

/// Returns a view of a vCPU's whole run page.
fn run_page_bytes(map: &MmapRaw) -> PointerBytes {
    #[expect(
        unsafe_code,
        reason = "the promise of a run page's mapping, which the vCPU keeps beside the view"
    )]
    // SAFETY: the vCPU holds the mapping next to the view and ends both together, so the bytes stay mapped while the view
    // lives; `MmapRaw` maps them readable and writable and hands out only the address; the page is the vCPU file's whole
    // mapping, so reaching it never traps; no reference over it is formed, since kvm-ioctls' `run` is never called.
    let pointer = unsafe { PointerAndLength::new(map.as_mut_ptr(), map.len()) };
    PointerBytes::new(pointer)
}

/// Returns the attribute naming the group, the attribute and a value buffer, which the caller keeps alive for the call.
fn attribute(group: u32, attr: u64, buffer: &mut [u8]) -> kvm_device_attr {
    kvm_device_attr {
        flags: u32::MIN,
        group,
        attr,
        addr: buffer.as_mut_ptr() as u64,
    }
}

/// Returns a buffer at least as long as any attribute's value, holding the value at its start.
fn buffer(value: &[u8]) -> Vec<u8> {
    let mut buffer = vec![u8::MIN; value.len().max(ATTRIBUTE_VALUE_BYTES)];
    buffer[..value.len()].copy_from_slice(value);
    buffer
}

/// Sets an attribute of a VM, a vCPU or a device from a buffer at least as long as any attribute's value.
fn set_attribute<F: AsRawFd>(fd: &F, group: u32, attr: u64, value: &[u8]) -> Result<(), Errno> {
    let mut buffer = buffer(value);
    let attribute = attribute(group, attr, &mut buffer);
    #[expect(
        unsafe_code,
        reason = "KVM_SET_DEVICE_ATTR reads the value at the attribute's address"
    )]
    // SAFETY: the address is a live buffer at least as long as any ARM64 attribute's value, which is all KVM reads.
    let result = unsafe { ioctl_with_ref(fd, KVM_SET_DEVICE_ATTR, &attribute) };
    returned(result).map(drop)
}

/// Reads an attribute of a VM, a vCPU or a device through a buffer at least as long as any attribute's value.
fn get_attribute<F: AsRawFd>(fd: &F, group: u32, attr: u64, value: &mut [u8]) -> Result<(), Errno> {
    let mut buffer = buffer(value);
    let attribute = attribute(group, attr, &mut buffer);
    #[expect(
        unsafe_code,
        reason = "KVM_GET_DEVICE_ATTR writes the value at the attribute's address"
    )]
    // SAFETY: the address is a live buffer at least as long as any ARM64 attribute's value, which is all KVM writes.
    let result = unsafe { ioctl_with_ref(fd, KVM_GET_DEVICE_ATTR, &attribute) };
    returned(result)?;
    value.copy_from_slice(&buffer[..value.len()]);
    Ok(())
}

/// Returns whether an attribute exists, from kvm-ioctls' answer.
fn has_attribute(answer: Result<(), kvm_ioctls::Error>) -> Result<bool, Errno> {
    match answer {
        Ok(()) => Ok(true),
        Err(error) if errno(error) == Errno::NXIO => Ok(false),
        Err(error) => Err(errno(error)),
    }
}

/// Returns the attribute `KVM_HAS_DEVICE_ATTR` asks about, which names no value.
fn asked(group: u32, attr: u64) -> kvm_device_attr {
    kvm_device_attr {
        flags: u32::MIN,
        group,
        attr,
        addr: u64::MIN,
    }
}

/// Returns the address `KVM_REGISTER_COALESCED_MMIO` names.
fn coalesced(addr: u64, pio: bool) -> IoEventAddress {
    if pio {
        IoEventAddress::Pio(addr)
    } else {
        IoEventAddress::Mmio(addr)
    }
}

impl Kvm for DirectKvm {
    type System = KvmSystem;
    type Vm = KvmVm;
    type Vcpu = KvmVcpu;
    type Device = KvmDevice;
    type UserMemory = SharedMapping;
    type RunPage = PointerBytes;

    fn open(&self) -> Result<KvmSystem, Errno> {
        kvm_ioctls::Kvm::new().map(KvmSystem).map_err(errno)
    }

    fn get_api_version(&self, system: &KvmSystem) -> Result<i32, Errno> {
        returned(system.0.get_api_version())
    }

    fn check_extension(&self, system: &KvmSystem, extension: u32) -> Result<i32, Errno> {
        returned(system.0.check_extension_raw(extension.into()))
    }

    fn get_vcpu_mmap_size(&self, system: &KvmSystem) -> Result<usize, Errno> {
        system.0.get_vcpu_mmap_size().map_err(errno)
    }

    fn create_vm(&self, system: &KvmSystem, machine_type: u64) -> Result<KvmVm, Errno> {
        let fd = system.0.create_vm_with_type(machine_type).map_err(errno)?;
        Ok(KvmVm(Arc::new(Machine {
            fd,
            slots: Mutex::new(BTreeMap::new()),
        })))
    }

    fn vm_check_extension(&self, vm: &KvmVm, extension: u32) -> Result<i32, Errno> {
        returned(vm.0.fd.check_extension_raw(extension.into()))
    }

    /// Makes a memory file of the length, sealed against shrinking, and maps all of it shared.
    fn create_user_memory(&self, length: usize) -> Result<SharedMapping, Errno> {
        if length == usize::MIN || !length.is_multiple_of(rustix::param::page_size()) {
            return Err(Errno::INVAL);
        }
        let file = memfd_create(
            USER_MEMORY_NAME,
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )?;
        ftruncate(&file, length as u64)?;
        fcntl_add_seals(&file, SealFlags::SHRINK)?;
        let mapped = Mapped::file(file.as_fd(), u64::MIN, length).map_err(|error| match error {
            MapError::Refused(error) => io_errno(&error),
            _ => Errno::INVAL,
        })?;
        Ok(SharedMapping::new(mapped))
    }

    fn set_user_memory_region(
        &self,
        vm: &KvmVm,
        slot: u32,
        flags: u32,
        guest_phys_addr: u64,
        memory: Option<&SharedMapping>,
    ) -> Result<(), Errno> {
        let mut slots = vm.0.slots.lock().unwrap_or_else(PoisonError::into_inner);
        let kept = memory
            .map(|memory| memory.part(u64::MIN..memory.len()).ok_or(Errno::INVAL))
            .transpose()?;
        let (userspace_addr, memory_size) = kept.as_ref().map_or((u64::MIN, u64::MIN), |kept| {
            (kept.address() as u64, kept.len())
        });
        let region = kvm_userspace_memory_region {
            slot,
            flags,
            guest_phys_addr,
            memory_size,
            userspace_addr,
        };
        #[expect(
            unsafe_code,
            reason = "KVM keeps the slot's host addresses until the slot changes or the VM ends"
        )]
        // SAFETY: a view of the memory goes into the VM's slots below once KVM took it, and leaves only when a later call for
        // the slot succeeds or when the VM, its vCPUs and its devices have all ended, after which nothing reaches it.
        let set = unsafe { vm.0.fd.set_user_memory_region(region) };
        set.map_err(errno)?;
        match kept {
            Some(kept) => slots.insert(slot, kept),
            None => slots.remove(&slot),
        };
        Ok(())
    }

    /// Returns `ENOENT` for a slot this VM was not given memory for, whose length it does not know.
    fn get_dirty_log(&self, vm: &KvmVm, slot: u32) -> Result<Vec<u64>, Errno> {
        let slots = vm.0.slots.lock().unwrap_or_else(PoisonError::into_inner);
        let length = slots.get(&slot).ok_or(Errno::NOENT)?.len();
        vm.0.fd.get_dirty_log(slot, length as usize).map_err(errno)
    }

    fn create_vcpu(&self, vm: &KvmVm, vcpu_id: u64) -> Result<KvmVcpu, Errno> {
        let fd = vm.0.fd.create_vcpu(vcpu_id).map_err(errno)?;
        let mapping = MmapOptions::new()
            .len(vm.0.fd.run_size())
            .map_raw(&fd)
            .map_err(|error| io_errno(&error))?;
        Ok(KvmVcpu {
            fd,
            run: run_page_bytes(&mapping),
            _run_mapping: mapping,
            _machine: Arc::clone(&vm.0),
        })
    }

    fn create_device(&self, vm: &KvmVm, device_type: u32) -> Result<KvmDevice, Errno> {
        let mut device = kvm_create_device {
            type_: device_type,
            fd: u32::MIN,
            flags: u32::MIN,
        };
        let fd = vm.0.fd.create_device(&mut device).map_err(errno)?;
        Ok(KvmDevice {
            fd,
            _machine: Arc::clone(&vm.0),
        })
    }

    fn irq_line(&self, vm: &KvmVm, irq: u32, level: bool) -> Result<(), Errno> {
        vm.0.fd.set_irq_line(irq, level).map_err(errno)
    }

    fn irqfd(
        &self,
        vm: &KvmVm,
        eventfd: BorrowedFd<'_>,
        gsi: u32,
        flags: u32,
        resamplefd: Option<BorrowedFd<'_>>,
    ) -> Result<(), Errno> {
        let irqfd = kvm_irqfd {
            fd: eventfd.as_raw_fd() as u32,
            gsi,
            flags,
            resamplefd: resamplefd.map_or(u32::MIN, |fd| fd.as_raw_fd() as u32),
            ..Default::default()
        };
        #[expect(
            unsafe_code,
            reason = "KVM_IRQFD, which kvm-ioctls offers only for its own event type and no flags"
        )]
        // SAFETY: KVM reads the structure, alive for the call, and takes its own hold of each file the borrowed
        // descriptors name, so nothing it keeps can outlive them.
        let result = unsafe { ioctl_with_ref(&vm.0.fd, KVM_IRQFD, &irqfd) };
        returned(result).map(drop)
    }

    fn ioeventfd(
        &self,
        vm: &KvmVm,
        eventfd: BorrowedFd<'_>,
        addr: u64,
        len: u32,
        datamatch: u64,
        flags: u32,
    ) -> Result<(), Errno> {
        let ioeventfd = kvm_ioeventfd {
            datamatch,
            addr,
            len,
            fd: eventfd.as_raw_fd(),
            flags,
            ..Default::default()
        };
        #[expect(
            unsafe_code,
            reason = "KVM_IOEVENTFD, which kvm-ioctls offers only for its own event type"
        )]
        // SAFETY: KVM reads the structure, alive for the call, and takes its own hold of the file the borrowed descriptor
        // names.
        let result = unsafe { ioctl_with_ref(&vm.0.fd, KVM_IOEVENTFD, &ioeventfd) };
        returned(result).map(drop)
    }

    fn set_gsi_routing(&self, vm: &KvmVm, entries: &[IrqRoutingEntry]) -> Result<(), Errno> {
        let routing = KvmIrqRouting::from_entries(entries).map_err(|_| Errno::TOOBIG)?;
        vm.0.fd.set_gsi_routing(&routing).map_err(errno)
    }

    fn signal_msi(&self, vm: &KvmVm, msi: &Msi) -> Result<i32, Errno> {
        vm.0.fd.signal_msi(*msi).map_err(errno)
    }

    fn register_coalesced_mmio(
        &self,
        vm: &KvmVm,
        addr: u64,
        size: u32,
        pio: bool,
    ) -> Result<(), Errno> {
        vm.0.fd
            .register_coalesced_mmio(coalesced(addr, pio), size)
            .map_err(errno)
    }

    fn unregister_coalesced_mmio(
        &self,
        vm: &KvmVm,
        addr: u64,
        size: u32,
        pio: bool,
    ) -> Result<(), Errno> {
        vm.0.fd
            .unregister_coalesced_mmio(coalesced(addr, pio), size)
            .map_err(errno)
    }

    /// Passes the arguments through kvm-ioctls as they are, since no capability an ARM64 VM enables reads an address from them.
    fn enable_cap(&self, vm: &KvmVm, cap: &EnableCap) -> Result<(), Errno> {
        vm.0.fd.enable_cap(cap).map_err(errno)
    }

    fn set_memory_attributes(
        &self,
        vm: &KvmVm,
        attributes: &MemoryAttributes,
    ) -> Result<(), Errno> {
        vm.0.fd.set_memory_attributes(*attributes).map_err(errno)
    }

    fn create_guest_memfd(
        &self,
        vm: &KvmVm,
        guest_memfd: &CreateGuestMemfd,
    ) -> Result<OwnedFd, Errno> {
        let fd = vm.0.fd.create_guest_memfd(*guest_memfd).map_err(errno)?;
        #[expect(
            unsafe_code,
            reason = "adopts the new file's descriptor, which kvm-ioctls returns raw"
        )]
        // SAFETY: KVM_CREATE_GUEST_MEMFD succeeded, so the descriptor is a new one that nothing else owns.
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        Ok(fd)
    }

    fn arm_preferred_target(&self, vm: &KvmVm) -> Result<VcpuInit, Errno> {
        let mut init = VcpuInit::default();
        vm.0.fd.get_preferred_target(&mut init).map_err(errno)?;
        Ok(init)
    }

    fn arm_set_counter_offset(&self, vm: &KvmVm, counter_offset: u64) -> Result<(), Errno> {
        let offset = kvm_arm_counter_offset {
            counter_offset,
            reserved: u64::MIN,
        };
        #[expect(
            unsafe_code,
            reason = "KVM_ARM_SET_COUNTER_OFFSET, which kvm-ioctls does not offer"
        )]
        // SAFETY: KVM reads the structure, alive for the call, and keeps nothing of it.
        let result = unsafe { ioctl_with_ref(&vm.0.fd, KVM_ARM_SET_COUNTER_OFFSET, &offset) };
        returned(result).map(drop)
    }

    fn vm_set_device_attr(
        &self,
        vm: &KvmVm,
        group: u32,
        attr: u64,
        value: &[u8],
    ) -> Result<(), Errno> {
        set_attribute(&vm.0.fd, group, attr, value)
    }

    fn vm_get_device_attr(
        &self,
        vm: &KvmVm,
        group: u32,
        attr: u64,
        value: &mut [u8],
    ) -> Result<(), Errno> {
        get_attribute(&vm.0.fd, group, attr, value)
    }

    fn vm_has_device_attr(&self, vm: &KvmVm, group: u32, attr: u64) -> Result<bool, Errno> {
        has_attribute(vm.0.fd.has_device_attr(&asked(group, attr)))
    }

    fn vcpu_run_page<'vcpu>(&self, vcpu: &'vcpu KvmVcpu) -> &'vcpu PointerBytes {
        &vcpu.run
    }

    fn run(&self, vcpu: &KvmVcpu) -> Result<(), Errno> {
        #[expect(
            unsafe_code,
            reason = "KVM_RUN, which kvm-ioctls offers only while it holds a reference over the run page"
        )]
        // SAFETY: KVM reads and writes only the vCPU's own run page and guest memory, which this crate reaches only as
        // volatile bytes; kvm-ioctls' own mapping of the page is never read, because its `run` is never called.
        let result = unsafe { ioctl(&vcpu.fd, KVM_RUN) };
        returned(result).map(drop)
    }

    /// Refuses, through kvm-ioctls, bytes shorter than the id says, with `EINVAL`.
    fn get_one_reg(&self, vcpu: &KvmVcpu, id: u64, value: &mut [u8]) -> Result<(), Errno> {
        vcpu.fd.get_one_reg(id, value).map(drop).map_err(errno)
    }

    /// Refuses, through kvm-ioctls, bytes shorter than the id says, with `EINVAL`.
    fn set_one_reg(&self, vcpu: &KvmVcpu, id: u64, value: &[u8]) -> Result<(), Errno> {
        vcpu.fd.set_one_reg(id, value).map(drop).map_err(errno)
    }

    fn get_reg_list(&self, vcpu: &KvmVcpu) -> Result<Vec<u64>, Errno> {
        let mut list = RegList::new(kvm_reg_list::max_len()).map_err(|_| Errno::NOMEM)?;
        vcpu.fd.get_reg_list(&mut list).map_err(errno)?;
        Ok(list.as_slice().to_vec())
    }

    fn arm_vcpu_init(&self, vcpu: &KvmVcpu, init: &VcpuInit) -> Result<(), Errno> {
        vcpu.fd.vcpu_init(init).map_err(errno)
    }

    fn arm_vcpu_finalize(&self, vcpu: &KvmVcpu, feature: i32) -> Result<(), Errno> {
        vcpu.fd.vcpu_finalize(&feature).map_err(errno)
    }

    fn get_mp_state(&self, vcpu: &KvmVcpu) -> Result<MpState, Errno> {
        vcpu.fd.get_mp_state().map_err(errno)
    }

    fn set_mp_state(&self, vcpu: &KvmVcpu, mp_state: &MpState) -> Result<(), Errno> {
        vcpu.fd.set_mp_state(*mp_state).map_err(errno)
    }

    fn get_vcpu_events(&self, vcpu: &KvmVcpu) -> Result<VcpuEvents, Errno> {
        vcpu.fd.get_vcpu_events().map_err(errno)
    }

    fn set_vcpu_events(&self, vcpu: &KvmVcpu, events: &VcpuEvents) -> Result<(), Errno> {
        vcpu.fd.set_vcpu_events(events).map_err(errno)
    }

    fn set_guest_debug(&self, vcpu: &KvmVcpu, debug: &GuestDebug) -> Result<(), Errno> {
        vcpu.fd.set_guest_debug(debug).map_err(errno)
    }

    fn vcpu_set_device_attr(
        &self,
        vcpu: &KvmVcpu,
        group: u32,
        attr: u64,
        value: &[u8],
    ) -> Result<(), Errno> {
        set_attribute(&vcpu.fd, group, attr, value)
    }

    fn vcpu_get_device_attr(
        &self,
        vcpu: &KvmVcpu,
        group: u32,
        attr: u64,
        value: &mut [u8],
    ) -> Result<(), Errno> {
        get_attribute(&vcpu.fd, group, attr, value)
    }

    fn vcpu_has_device_attr(&self, vcpu: &KvmVcpu, group: u32, attr: u64) -> Result<bool, Errno> {
        has_attribute(vcpu.fd.has_device_attr(&asked(group, attr)))
    }

    fn set_device_attr(
        &self,
        device: &KvmDevice,
        group: u32,
        attr: u64,
        value: &[u8],
    ) -> Result<(), Errno> {
        set_attribute(&device.fd, group, attr, value)
    }

    fn get_device_attr(
        &self,
        device: &KvmDevice,
        group: u32,
        attr: u64,
        value: &mut [u8],
    ) -> Result<(), Errno> {
        get_attribute(&device.fd, group, attr, value)
    }

    fn has_device_attr(&self, device: &KvmDevice, group: u32, attr: u64) -> Result<bool, Errno> {
        has_attribute(device.fd.has_device_attr(&asked(group, attr)))
    }
}
