//! KVM's ioctls on this host's `/dev/kvm`, through kvm-ioctls where it is sound and marked raw ioctls where it is not.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::collections::BTreeMap;
use std::ffi::{CStr, c_ulong, c_void};
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::ptr;
use std::sync::{Arc, Mutex, PoisonError};

use kvm_bindings::{
    KVM_ARM_FEATURE_ID_RANGE_SIZE, KVM_CAP_DIRTY_LOG_RING, KVM_CAP_DIRTY_LOG_RING_ACQ_REL,
    KVM_DIRTY_LOG_PAGE_OFFSET, KVM_IRQFD_FLAG_RESAMPLE, KVMIO, KvmIrqRouting, RegList,
    kvm_arm_copy_mte_tags, kvm_arm_counter_offset, kvm_arm_device_addr, kvm_clear_dirty_log,
    kvm_clear_dirty_log__bindgen_ty_1, kvm_create_device, kvm_device_attr, kvm_ioeventfd,
    kvm_irq_level, kvm_irqfd, kvm_reg_list, kvm_signal_mask, kvm_smccc_filter,
    kvm_userspace_memory_region, kvm_userspace_memory_region2, reg_mask_range,
};
use kvm_ioctls::{DeviceFd, IoEventAddress, VcpuFd, VmFd};
use memmap2::{MmapOptions, MmapRaw};
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};
use vmm_sys_util::fam::FamStruct;
use vmm_sys_util::ioctl::{
    _IOC_NONE, _IOC_READ, _IOC_WRITE, ioctl, ioctl_expr, ioctl_with_mut_ref, ioctl_with_ptr,
    ioctl_with_ref,
};
use wcontainers_kvm::{
    CreateGuestMemfd, EnableCap, Errno, GuestDebug, IrqRoutingEntry, Kvm, MTE_GRANULE_BYTES,
    MemoryAttributes, MpState, Msi, PreFaultMemory, VcpuEvents, VcpuInit,
};
use wcontainers_volatile_bytes::{StandaloneVolatileBytes, VolatileBytes};
use wcontainers_volatile_bytes_pointer::{PointerAndLength, PointerBytes};
use wcontainers_volatile_bytes_shared_mapping::{MapError, Mapped, SharedMapping};

/// The number of `KVM_RUN`, which kvm-ioctls keeps to itself.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_RUN: c_ulong = ioctl_expr(_IOC_NONE, KVMIO, 0x80, u32::MIN);

/// The number of `KVM_IRQFD`, which kvm-ioctls keeps to itself.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_IRQFD: c_ulong = ioctl_expr(_IOC_WRITE, KVMIO, 0x76, size_of::<kvm_irqfd>() as u32);

/// The number of `KVM_IOEVENTFD`, which kvm-ioctls keeps to itself.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_IOEVENTFD: c_ulong =
    ioctl_expr(_IOC_WRITE, KVMIO, 0x79, size_of::<kvm_ioeventfd>() as u32);

/// The number of `KVM_SET_DEVICE_ATTR`, which kvm-ioctls keeps to itself.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_SET_DEVICE_ATTR: c_ulong =
    ioctl_expr(_IOC_WRITE, KVMIO, 0xe1, size_of::<kvm_device_attr>() as u32);

/// The number of `KVM_GET_DEVICE_ATTR`, which kvm-ioctls keeps to itself.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_GET_DEVICE_ATTR: c_ulong =
    ioctl_expr(_IOC_WRITE, KVMIO, 0xe2, size_of::<kvm_device_attr>() as u32);

/// The number of `KVM_ARM_SET_COUNTER_OFFSET`, which kvm-ioctls does not have.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_ARM_SET_COUNTER_OFFSET: c_ulong = ioctl_expr(
    _IOC_WRITE,
    KVMIO,
    0xb5,
    size_of::<kvm_arm_counter_offset>() as u32,
);

/// The number of `KVM_IRQ_LINE_STATUS`, which kvm-ioctls does not have.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_IRQ_LINE_STATUS: c_ulong = ioctl_expr(
    _IOC_READ | _IOC_WRITE,
    KVMIO,
    0x67,
    size_of::<kvm_irq_level>() as u32,
);

/// The number of `KVM_ARM_SET_DEVICE_ADDR`, which kvm-ioctls does not have.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_ARM_SET_DEVICE_ADDR: c_ulong = ioctl_expr(
    _IOC_WRITE,
    KVMIO,
    0xab,
    size_of::<kvm_arm_device_addr>() as u32,
);

/// The number of `KVM_ARM_MTE_COPY_TAGS`, which kvm-ioctls does not have.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_ARM_MTE_COPY_TAGS: c_ulong = ioctl_expr(
    _IOC_READ,
    KVMIO,
    0xb4,
    size_of::<kvm_arm_copy_mte_tags>() as u32,
);

/// The number of `KVM_ARM_GET_REG_WRITABLE_MASKS`, which kvm-ioctls does not have.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_ARM_GET_REG_WRITABLE_MASKS: c_ulong =
    ioctl_expr(_IOC_READ, KVMIO, 0xb6, size_of::<reg_mask_range>() as u32);

/// The number of `KVM_SET_SIGNAL_MASK`, which kvm-ioctls does not have.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_SET_SIGNAL_MASK: c_ulong =
    ioctl_expr(_IOC_WRITE, KVMIO, 0x8b, size_of::<kvm_signal_mask>() as u32);

/// The number of `KVM_CLEAR_DIRTY_LOG`, which kvm-ioctls does not have.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_CLEAR_DIRTY_LOG: c_ulong = ioctl_expr(
    _IOC_READ | _IOC_WRITE,
    KVMIO,
    0xc0,
    size_of::<kvm_clear_dirty_log>() as u32,
);

/// The number of `KVM_RESET_DIRTY_RINGS`, which kvm-ioctls does not have.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_RESET_DIRTY_RINGS: c_ulong = ioctl_expr(_IOC_NONE, KVMIO, 0xc7, u32::MIN);

/// The number of `KVM_GET_STATS_FD`, which kvm-ioctls does not have.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_GET_STATS_FD: c_ulong = ioctl_expr(_IOC_NONE, KVMIO, 0xce, u32::MIN);

/// The number of `KVM_PRE_FAULT_MEMORY`, which kvm-ioctls does not have.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const KVM_PRE_FAULT_MEMORY: c_ulong = ioctl_expr(
    _IOC_READ | _IOC_WRITE,
    KVMIO,
    0xd5,
    size_of::<PreFaultMemory>() as u32,
);

/// The bytes every attribute value is passed in: no attribute an ARM64 device documents is longer than the SMCCC filter.
///
/// https://docs.kernel.org/virt/kvm/devices/vm.html#attribute-kvm-arm-vm-smccc-filter-w-only
const ATTRIBUTE_VALUE_BYTES: usize = size_of::<kvm_smccc_filter>();

/// The name each memory file is made with, which shows in `/proc/<pid>/maps`.
const USER_MEMORY_NAME: &CStr = c"wcontainers-kvm-user-memory";

/// The buffer an attribute's value is passed to KVM in, as long as the longest value.
type AttributeValue = [u8; ATTRIBUTE_VALUE_BYTES];

/// `kvm_irq_level`, with its union of the line's number and the status KVM writes back as one plain field.
#[repr(C)]
struct IrqLevel {
    irq_or_status: u32,
    level: u32,
}

const _: () = assert!(size_of::<IrqLevel>() == size_of::<kvm_irq_level>());

/// KVM, called on this host through `/dev/kvm`.
pub struct DirectKvm;

/// An open `/dev/kvm`.
pub struct KvmSystem(kvm_ioctls::Kvm);

/// A VM, which keeps every memory it holds in a slot mapped while it, a vCPU or a device of it lives.
pub struct KvmVm(Arc<Machine>);

/// A vCPU, with views of the pages it shares with KVM.
pub struct KvmVcpu {
    fd: VcpuFd,
    run: SharedPages,
    dirty_ring: Option<SharedPages>,
    _machine: Arc<Machine>,
}

/// A device of a VM.
pub struct KvmDevice {
    fd: DeviceFd,
    _machine: Arc<Machine>,
}

/// A VM's descriptor, its slots, and the length of each vCPU's dirty ring once dirty rings are on.
struct Machine {
    fd: VmFd,
    slots: Mutex<BTreeMap<u32, Slot>>,
    dirty_ring_bytes: Mutex<Option<usize>>,
}

/// A slot's length and a view of the memory it holds.
struct Slot {
    length: u64,
    _memory: SharedMapping,
}

/// Pages of a vCPU's file that KVM writes while they are mapped, and a view of them; the view ends first.
struct SharedPages {
    bytes: PointerBytes,
    _mapping: MmapRaw,
}

impl SharedPages {
    /// Maps the pages of the vCPU's file at the offset, and returns a view of them.
    fn map(vcpu: &VcpuFd, offset: u64, length: usize) -> Result<SharedPages, Errno> {
        let mapping = MmapOptions::new()
            .offset(offset)
            .len(length)
            .map_raw(vcpu)
            .map_err(|error| io_errno(&error))?;
        #[expect(
            unsafe_code,
            reason = "the promise of a vCPU's shared pages, which the vCPU keeps beside the view"
        )]
        // SAFETY: the view is declared before the mapping, so it ends first, and both end with the vCPU; `MmapRaw` maps
        // the pages readable and writable and hands out only the address; KVM backs every page of the vCPU file it maps,
        // so reaching them never traps; kvm-ioctls' `run` and `map_coalesced_mmio_ring`, which form references, are never
        // called.
        let pointer = unsafe { PointerAndLength::new(mapping.as_mut_ptr(), mapping.len()) };
        Ok(SharedPages {
            bytes: PointerBytes::new(pointer),
            _mapping: mapping,
        })
    }
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

/// Returns the attribute naming the group, the attribute and a value buffer, which the caller keeps alive for the call.
fn attribute(group: u32, attr: u64, buffer: &mut AttributeValue) -> kvm_device_attr {
    kvm_device_attr {
        flags: u32::MIN,
        group,
        attr,
        addr: buffer.as_mut_ptr() as u64,
    }
}

/// Sets an attribute of a VM, a vCPU or a device from the value, copied to the start of a whole attribute buffer.
fn set_attribute<F: AsRawFd>(fd: &F, group: u32, attr: u64, value: &[u8]) -> Result<(), Errno> {
    let mut buffer: AttributeValue = [u8::MIN; ATTRIBUTE_VALUE_BYTES];
    buffer
        .get_mut(..value.len())
        .ok_or(Errno::TOOBIG)?
        .copy_from_slice(value);
    let attribute = attribute(group, attr, &mut buffer);
    #[expect(
        unsafe_code,
        reason = "KVM_SET_DEVICE_ATTR reads the value at the attribute's address"
    )]
    // SAFETY: the address is a live buffer as long as any ARM64 attribute's value, which is all KVM reads.
    let result = unsafe { ioctl_with_ref(fd, KVM_SET_DEVICE_ATTR, &attribute) };
    returned(result).map(drop)
}

/// Reads an attribute of a VM, a vCPU or a device through a whole attribute buffer, and returns the buffer.
fn read_attribute<F: AsRawFd>(fd: &F, group: u32, attr: u64) -> Result<AttributeValue, Errno> {
    let mut buffer: AttributeValue = [u8::MIN; ATTRIBUTE_VALUE_BYTES];
    let attribute = attribute(group, attr, &mut buffer);
    #[expect(
        unsafe_code,
        reason = "KVM_GET_DEVICE_ATTR writes the value at the attribute's address"
    )]
    // SAFETY: the address is a live buffer as long as any ARM64 attribute's value, which is all KVM writes.
    let result = unsafe { ioctl_with_ref(fd, KVM_GET_DEVICE_ATTR, &attribute) };
    returned(result)?;
    Ok(buffer)
}

/// Reads an attribute of a VM, a vCPU or a device into the value, from the start of a whole attribute buffer.
fn get_attribute<F: AsRawFd>(fd: &F, group: u32, attr: u64, value: &mut [u8]) -> Result<(), Errno> {
    if value.len() > ATTRIBUTE_VALUE_BYTES {
        return Err(Errno::TOOBIG);
    }
    let buffer = read_attribute(fd, group, attr)?;
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

/// Returns the statistics file of a VM or a vCPU.
fn stats_fd<F: AsRawFd>(fd: &F) -> Result<OwnedFd, Errno> {
    #[expect(
        unsafe_code,
        reason = "KVM_GET_STATS_FD, which kvm-ioctls does not offer"
    )]
    // SAFETY: the ioctl takes no argument.
    let result = unsafe { ioctl(fd, KVM_GET_STATS_FD) };
    let stats: RawFd = returned(result)?;
    #[expect(
        unsafe_code,
        reason = "adopts the new file's descriptor, which the ioctl returns raw"
    )]
    // SAFETY: KVM_GET_STATS_FD succeeded, so the descriptor is a new one that nothing else owns.
    let stats = unsafe { OwnedFd::from_raw_fd(stats) };
    Ok(stats)
}

impl Machine {
    /// Changes a slot through the call given its memory's address, and keeps a view of the memory while KVM holds it.
    fn set_slot(
        &self,
        slot: u32,
        memory_size: u64,
        memory: Option<&SharedMapping>,
        set: impl FnOnce(u64) -> Result<(), kvm_ioctls::Error>,
    ) -> Result<(), Errno> {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        let kept = match memory {
            Some(memory) => Some(memory.part(u64::MIN..memory_size).ok_or(Errno::INVAL)?),
            None if memory_size == u64::MIN => None,
            None => return Err(Errno::INVAL),
        };
        set(kept.as_ref().map_or(u64::MIN, |kept| kept.address() as u64)).map_err(errno)?;
        match kept {
            Some(kept) if memory_size > u64::MIN => slots.insert(
                slot,
                Slot {
                    length: memory_size,
                    _memory: kept,
                },
            ),
            _ => slots.remove(&slot),
        };
        Ok(())
    }
}

impl Kvm for DirectKvm {
    type System = KvmSystem;
    type Vm = KvmVm;
    type Vcpu = KvmVcpu;
    type Device = KvmDevice;
    type UserMemory = SharedMapping;
    type RunPage = PointerBytes;
    type DirtyRing = PointerBytes;

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
            dirty_ring_bytes: Mutex::new(None),
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
        let memory_size = memory.map_or(u64::MIN, VolatileBytes::len);
        vm.0.set_slot(slot, memory_size, memory, |userspace_addr| {
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
            // SAFETY: the addresses are a view of `memory_size` bytes, or none for size 0; the view goes into the VM's
            // slots once KVM took it, and leaves only when a later call for the slot succeeds or when the VM, its vCPUs
            // and its devices have all ended, after which nothing reaches it.
            unsafe {
                vm.0.fd.set_user_memory_region(region)
            }
        })
    }

    /// Refuses, with `EINVAL`, a size larger than the memory, or a size other than 0 with no memory.
    fn set_user_memory_region2(
        &self,
        vm: &KvmVm,
        slot: u32,
        flags: u32,
        guest_phys_addr: u64,
        memory_size: u64,
        memory: Option<&SharedMapping>,
        guest_memfd: Option<BorrowedFd<'_>>,
        guest_memfd_offset: u64,
    ) -> Result<(), Errno> {
        vm.0.set_slot(slot, memory_size, memory, |userspace_addr| {
            let region = kvm_userspace_memory_region2 {
                slot,
                flags,
                guest_phys_addr,
                memory_size,
                userspace_addr,
                guest_memfd_offset,
                guest_memfd: guest_memfd.map_or(u32::MIN, |fd| fd.as_raw_fd() as u32),
                ..Default::default()
            };
            #[expect(
                unsafe_code,
                reason = "KVM keeps the slot's host addresses until the slot changes or the VM ends"
            )]
            // SAFETY: as for `set_user_memory_region`; KVM takes its own hold of the guest_memfd's file, so it cannot
            // outlive the borrowed descriptor's file.
            unsafe {
                vm.0.fd.set_user_memory_region2(region)
            }
        })
    }

    /// Returns `ENOENT` for a slot this VM was not given memory for, whose length it does not know.
    fn get_dirty_log(&self, vm: &KvmVm, slot: u32) -> Result<Vec<u64>, Errno> {
        let slots = vm.0.slots.lock().unwrap_or_else(PoisonError::into_inner);
        let length = slots.get(&slot).ok_or(Errno::NOENT)?.length;
        vm.0.fd.get_dirty_log(slot, length as usize).map_err(errno)
    }

    /// Refuses, with `EINVAL`, a bitmap with fewer bits than pages.
    fn clear_dirty_log(
        &self,
        vm: &KvmVm,
        slot: u32,
        first_page: u64,
        num_pages: u32,
        dirty_bitmap: &[u64],
    ) -> Result<(), Errno> {
        if dirty_bitmap.len() < (num_pages as usize).div_ceil(u64::BITS as usize) {
            return Err(Errno::INVAL);
        }
        let mut log = kvm_clear_dirty_log {
            slot,
            num_pages,
            first_page,
            __bindgen_anon_1: kvm_clear_dirty_log__bindgen_ty_1 {
                dirty_bitmap: dirty_bitmap.as_ptr().cast_mut().cast::<c_void>(),
            },
        };
        #[expect(
            unsafe_code,
            reason = "KVM_CLEAR_DIRTY_LOG, which kvm-ioctls does not offer"
        )]
        // SAFETY: KVM reads one bit a page of the bitmap, which holds at least that many bits, and keeps nothing of it.
        let result = unsafe { ioctl_with_mut_ref(&vm.0.fd, KVM_CLEAR_DIRTY_LOG, &mut log) };
        returned(result).map(drop)
    }

    fn reset_dirty_rings(&self, vm: &KvmVm) -> Result<i32, Errno> {
        #[expect(
            unsafe_code,
            reason = "KVM_RESET_DIRTY_RINGS, which kvm-ioctls does not offer"
        )]
        // SAFETY: the ioctl takes no argument; the rings it reads are reached here only as volatile bytes.
        let result = unsafe { ioctl(&vm.0.fd, KVM_RESET_DIRTY_RINGS) };
        returned(result)
    }

    fn create_vcpu(&self, vm: &KvmVm, vcpu_id: u64) -> Result<KvmVcpu, Errno> {
        let fd = vm.0.fd.create_vcpu(vcpu_id).map_err(errno)?;
        let run = SharedPages::map(&fd, u64::MIN, vm.0.fd.run_size())?;
        let ring_bytes = *vm
            .0
            .dirty_ring_bytes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let ring_offset = u64::from(KVM_DIRTY_LOG_PAGE_OFFSET) * rustix::param::page_size() as u64;
        let dirty_ring = ring_bytes
            .map(|length| SharedPages::map(&fd, ring_offset, length))
            .transpose()?;
        Ok(KvmVcpu {
            fd,
            run,
            dirty_ring,
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

    fn create_irq_chip(&self, vm: &KvmVm) -> Result<(), Errno> {
        vm.0.fd.create_irq_chip().map_err(errno)
    }

    fn arm_set_device_addr(&self, vm: &KvmVm, id: u64, addr: u64) -> Result<(), Errno> {
        let device = kvm_arm_device_addr { id, addr };
        #[expect(
            unsafe_code,
            reason = "KVM_ARM_SET_DEVICE_ADDR, which kvm-ioctls does not offer"
        )]
        // SAFETY: KVM reads the structure, alive for the call, and keeps nothing of it.
        let result = unsafe { ioctl_with_ref(&vm.0.fd, KVM_ARM_SET_DEVICE_ADDR, &device) };
        returned(result).map(drop)
    }

    fn irq_line(&self, vm: &KvmVm, irq: u32, level: bool) -> Result<(), Errno> {
        vm.0.fd.set_irq_line(irq, level).map_err(errno)
    }

    fn irq_line_status(&self, vm: &KvmVm, irq: u32, level: bool) -> Result<i32, Errno> {
        let mut line = IrqLevel {
            irq_or_status: irq,
            level: level.into(),
        };
        #[expect(
            unsafe_code,
            reason = "KVM_IRQ_LINE_STATUS, which kvm-ioctls does not offer"
        )]
        // SAFETY: KVM reads and writes back the structure, alive for the call and laid out as `kvm_irq_level`.
        let result = unsafe { ioctl_with_mut_ref(&vm.0.fd, KVM_IRQ_LINE_STATUS, &mut line) };
        returned(result)?;
        Ok(line.irq_or_status.cast_signed())
    }

    /// Refuses, with `EINVAL`, `KVM_IRQFD_FLAG_RESAMPLE` without a resample file.
    fn irqfd(
        &self,
        vm: &KvmVm,
        eventfd: BorrowedFd<'_>,
        gsi: u32,
        flags: u32,
        resamplefd: Option<BorrowedFd<'_>>,
    ) -> Result<(), Errno> {
        if flags & KVM_IRQFD_FLAG_RESAMPLE != u32::MIN && resamplefd.is_none() {
            return Err(Errno::INVAL);
        }
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

    /// Passes the arguments on as they are, since no ARM64 capability reads an address from them; keeps a dirty ring's length.
    fn enable_cap(&self, vm: &KvmVm, cap: &EnableCap) -> Result<(), Errno> {
        let mut ring_bytes =
            vm.0.dirty_ring_bytes
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
        vm.0.fd.enable_cap(cap).map_err(errno)?;
        if [KVM_CAP_DIRTY_LOG_RING, KVM_CAP_DIRTY_LOG_RING_ACQ_REL].contains(&cap.cap) {
            *ring_bytes = Some(usize::try_from(cap.args[0]).map_err(|_| Errno::INVAL)?);
        }
        Ok(())
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

    /// Refuses, with `EINVAL`, tags for more guest bytes than a 64-bit length holds.
    fn arm_mte_copy_tags(
        &self,
        vm: &KvmVm,
        guest_ipa: u64,
        flags: u64,
        tags: &mut [u8],
    ) -> Result<u64, Errno> {
        let length = (tags.len() as u64)
            .checked_mul(MTE_GRANULE_BYTES)
            .ok_or(Errno::INVAL)?;
        let copy = kvm_arm_copy_mte_tags {
            guest_ipa,
            length,
            addr: tags.as_mut_ptr().cast::<c_void>(),
            flags,
            reserved: [u64::MIN; 2],
        };
        #[expect(
            unsafe_code,
            reason = "KVM_ARM_MTE_COPY_TAGS, which kvm-ioctls does not offer"
        )]
        // SAFETY: KVM reads or writes one byte a granule of the length, which is the tags' whole length, alive and
        // borrowed mutably for the call, and keeps nothing of it.
        let result = unsafe { ioctl_with_ref(&vm.0.fd, KVM_ARM_MTE_COPY_TAGS, &copy) };
        returned(result).map(|copied| copied as u64)
    }

    fn arm_get_reg_writable_masks(&self, vm: &KvmVm, range: u32) -> Result<Vec<u64>, Errno> {
        let mut masks = vec![u64::MIN; KVM_ARM_FEATURE_ID_RANGE_SIZE as usize];
        let request = reg_mask_range {
            addr: masks.as_mut_ptr() as u64,
            range,
            ..Default::default()
        };
        #[expect(
            unsafe_code,
            reason = "KVM_ARM_GET_REG_WRITABLE_MASKS, which kvm-ioctls does not offer"
        )]
        // SAFETY: KVM writes `KVM_ARM_FEATURE_ID_RANGE_SIZE` masks at the address, the live array of that many.
        let result = unsafe { ioctl_with_ref(&vm.0.fd, KVM_ARM_GET_REG_WRITABLE_MASKS, &request) };
        returned(result)?;
        Ok(masks)
    }

    fn vm_get_stats_fd(&self, vm: &KvmVm) -> Result<OwnedFd, Errno> {
        stats_fd(&vm.0.fd)
    }

    /// Refuses, with `E2BIG`, a value longer than any ARM64 attribute's.
    fn vm_set_device_attr(
        &self,
        vm: &KvmVm,
        group: u32,
        attr: u64,
        value: &[u8],
    ) -> Result<(), Errno> {
        set_attribute(&vm.0.fd, group, attr, value)
    }

    /// Refuses, with `E2BIG`, a value longer than any ARM64 attribute's.
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
        &vcpu.run.bytes
    }

    fn vcpu_dirty_ring<'vcpu>(&self, vcpu: &'vcpu KvmVcpu) -> Option<&'vcpu PointerBytes> {
        vcpu.dirty_ring.as_ref().map(|ring| &ring.bytes)
    }

    fn run(&self, vcpu: &KvmVcpu) -> Result<(), Errno> {
        #[expect(
            unsafe_code,
            reason = "KVM_RUN, which kvm-ioctls offers only while it holds a reference over the run page"
        )]
        // SAFETY: KVM reads and writes only the vCPU's own shared pages and guest memory, which this crate reaches only as
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

    /// Refuses, with `E2BIG`, a set longer than a `u32` counts.
    fn set_signal_mask(&self, vcpu: &KvmVcpu, sigset: Option<&[u8]>) -> Result<(), Errno> {
        let mask = sigset
            .map(|sigset| {
                let len = u32::try_from(sigset.len()).map_err(|_| Errno::TOOBIG)?;
                Ok([&len.to_ne_bytes()[..], sigset].concat())
            })
            .transpose()?;
        let argument = mask.as_ref().map_or(ptr::null(), |mask| mask.as_ptr());
        #[expect(
            unsafe_code,
            reason = "KVM_SET_SIGNAL_MASK, which kvm-ioctls does not offer"
        )]
        // SAFETY: the argument is none, or a live `kvm_signal_mask` whose length is followed by that many bytes; KVM reads
        // the set only when its length is the kernel's, and keeps nothing of it.
        let result = unsafe { ioctl_with_ptr(&vcpu.fd, KVM_SET_SIGNAL_MASK, argument) };
        returned(result).map(drop)
    }

    fn pre_fault_memory(&self, vcpu: &KvmVcpu, range: &mut PreFaultMemory) -> Result<(), Errno> {
        #[expect(
            unsafe_code,
            reason = "KVM_PRE_FAULT_MEMORY, which kvm-ioctls does not offer"
        )]
        // SAFETY: KVM reads and writes back the structure, alive for the call, and keeps nothing of it.
        let result = unsafe { ioctl_with_mut_ref(&vcpu.fd, KVM_PRE_FAULT_MEMORY, range) };
        returned(result).map(drop)
    }

    fn vcpu_get_stats_fd(&self, vcpu: &KvmVcpu) -> Result<OwnedFd, Errno> {
        stats_fd(&vcpu.fd)
    }

    /// Refuses, with `E2BIG`, a value longer than any ARM64 attribute's.
    fn vcpu_set_device_attr(
        &self,
        vcpu: &KvmVcpu,
        group: u32,
        attr: u64,
        value: &[u8],
    ) -> Result<(), Errno> {
        set_attribute(&vcpu.fd, group, attr, value)
    }

    /// Refuses, with `E2BIG`, a value longer than any ARM64 attribute's.
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

    /// Refuses, with `E2BIG`, a value longer than any ARM64 attribute's.
    fn set_device_attr(
        &self,
        device: &KvmDevice,
        group: u32,
        attr: u64,
        value: &[u8],
    ) -> Result<(), Errno> {
        set_attribute(&device.fd, group, attr, value)
    }

    /// Refuses, with `E2BIG`, a value longer than any ARM64 attribute's.
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
