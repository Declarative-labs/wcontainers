//! The tests every implementation of wcontainers-kvm runs on a host with KVM, written against its trait alone.
#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use std::fs::File;
use std::io::Read;
use std::mem::offset_of;
use std::os::fd::AsFd;
use std::thread;

use kvm_bindings::{
    KVM_API_VERSION, KVM_ARM_DEVICE_ID_SHIFT, KVM_ARM_DEVICE_VGIC_V2, KVM_ARM_FEATURE_ID_RANGE,
    KVM_ARM_FEATURE_ID_RANGE_SIZE, KVM_ARM_IRQ_TYPE_SHIFT, KVM_ARM_IRQ_TYPE_SPI,
    KVM_ARM_TAGS_FROM_GUEST, KVM_ARM_VCPU_SVE, KVM_ARM_VCPU_TIMER_CTRL,
    KVM_ARM_VCPU_TIMER_IRQ_VTIMER, KVM_ARM_VM_SMCCC_CTRL, KVM_ARM_VM_SMCCC_FILTER,
    KVM_CAP_ARM_NISV_TO_USER, KVM_CAP_ARM_SUPPORTED_REG_MASK_RANGES, KVM_CAP_COUNTER_OFFSET,
    KVM_CAP_DIRTY_LOG_RING_ACQ_REL, KVM_CAP_GUEST_MEMFD, KVM_CAP_MANUAL_DIRTY_LOG_PROTECT2,
    KVM_CAP_MEMORY_ATTRIBUTES, KVM_CAP_PRE_FAULT_MEMORY, KVM_CAP_USER_MEMORY, KVM_CAP_USER_MEMORY2,
    KVM_DEV_ARM_VGIC_CTRL_INIT, KVM_DEV_ARM_VGIC_GRP_ADDR, KVM_DEV_ARM_VGIC_GRP_CTRL,
    KVM_DIRTY_LOG_MANUAL_PROTECT_ENABLE, KVM_EXIT_MMIO, KVM_GUESTDBG_ENABLE,
    KVM_GUESTDBG_SINGLESTEP, KVM_IRQFD_FLAG_DEASSIGN, KVM_IRQFD_FLAG_RESAMPLE,
    KVM_MEM_LOG_DIRTY_PAGES, KVM_MP_STATE_STOPPED, KVM_REG_ARM_CORE, KVM_REG_ARM64,
    KVM_REG_SIZE_U64, KVM_VGIC_V2_ADDR_TYPE_DIST, KVM_VGIC_V3_ADDR_TYPE_DIST,
    KVM_VGIC_V3_ADDR_TYPE_REDIST, PSR_A_BIT, PSR_D_BIT, PSR_F_BIT, PSR_I_BIT, PSR_MODE_EL1h,
    kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V3, kvm_dirty_gfn, kvm_ioeventfd_flag_nr_deassign,
    kvm_regs, kvm_run, kvm_run__bindgen_ty_1__bindgen_ty_6, kvm_smccc_filter,
    kvm_smccc_filter_action_KVM_SMCCC_FILTER_FWD_TO_USER, kvm_stats_header, user_pt_regs,
};
use rustix::event::{EventfdFlags, eventfd};
use wcontainers_kvm::{
    CreateGuestMemfd, EnableCap, Errno, GuestDebug, Kvm, MTE_GRANULE_BYTES, MemoryAttributes,
    MpState, Msi, PreFaultMemory,
};
use wcontainers_volatile_bytes::VolatileBytes;

/// The guest-physical address of the memory each test gives a VM, inside the smallest address size KVM offers.
///
/// https://docs.kernel.org/virt/kvm/api.html#kvm-create-vm
const GUEST_MEMORY: u64 = 1 << 30;

/// The guest-physical address of the device the guest writes to: below the memory and in no slot, so the write exits.
///
/// https://docs.kernel.org/virt/kvm/api.html#kvm-exit-mmio
const DEVICE: u64 = GUEST_MEMORY / 2;

/// Where in its memory the guest stores, past its two instructions.
const STORED_AT: usize = 2 * size_of::<u32>();

/// A slot beside slot 0, which every test gives memory first.
///
/// https://docs.kernel.org/virt/kvm/api.html#kvm-set-user-memory-region
const SECOND_SLOT: u32 = 1;

/// The general registers the guest's two stores name: the memory's address, the value, and the device's address.
///
/// https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--immediate---Store-Register--immediate--
const MEMORY_REGISTER: usize = 0;

/// The register holding the value both stores write.
const VALUE_REGISTER: usize = MEMORY_REGISTER + 1;

/// The register holding the device's address.
const DEVICE_REGISTER: usize = VALUE_REGISTER + 1;

/// The number of functions the SMCCC filter test forwards, starting at function 0.
///
/// https://docs.kernel.org/virt/kvm/devices/vm.html#attribute-kvm-arm-vm-smccc-filter-w-only
const FILTERED_FUNCTIONS: u32 = 1;

/// The value the guest stores in its memory and writes to the device.
const STORED: u64 = u64::MAX;

/// `str x1, [x0]`: STR (immediate), 64-bit, unsigned offset 0, Rn 0, Rt 1.
///
/// https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--immediate---Store-Register--immediate--
const STORE_TO_MEMORY: u32 = 0xf900_0001;

/// `str x1, [x2]`: STR (immediate), 64-bit, unsigned offset 0, Rn 2, Rt 1.
///
/// https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--immediate---Store-Register--immediate--
const STORE_TO_DEVICE: u32 = 0xf900_0041;

/// The interrupt controller's distributor, placed above the memory.
///
/// https://docs.kernel.org/virt/kvm/devices/arm-vgic-v3.html
const DISTRIBUTOR: u64 = 2 * GUEST_MEMORY;

/// The length of a GICv3 distributor's region, which KVM_VGIC_V3_ADDR_TYPE_DIST names.
///
/// https://docs.kernel.org/virt/kvm/devices/arm-vgic-v3.html#:~:text=64K
const DISTRIBUTOR_LENGTH: u64 = 1 << 16;

/// The interrupt controller's redistributors, just past the distributor.
const REDISTRIBUTORS: u64 = DISTRIBUTOR + DISTRIBUTOR_LENGTH;

/// The first shared peripheral interrupt of a GIC.
///
/// https://developer.arm.com/documentation/198123/0302/Arm-GIC-fundamentals#:~:text=SPI
const FIRST_SHARED_INTERRUPT: u32 = 32;

/// The pages of the large slot, two words of the dirty log.
///
/// https://docs.kernel.org/virt/kvm/api.html#kvm-get-dirty-log
const LARGE_SLOT_PAGES: usize = 2 * u64::BITS as usize;

/// The entries of each vCPU's dirty ring: a power of two, a whole number of pages of entries.
///
/// https://docs.kernel.org/virt/kvm/api.html#kvm-cap-dirty-log-ring-kvm-cap-dirty-log-ring-acq-rel
const DIRTY_RING_ENTRIES: usize = 1 << 12;

/// `KVM_DIRTY_GFN_F_DIRTY`: KVM wrote the entry.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const DIRTY_GFN_DIRTY: u32 = 1;

/// `KVM_DIRTY_GFN_F_RESET`: the host collected the entry.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/kvm.h
const DIRTY_GFN_RESET: u32 = 1 << 1;

/// The bytes of the kernel's signal set on ARM64, `_NSIG` bits.
///
/// https://github.com/torvalds/linux/blob/v7.0/include/uapi/asm-generic/signal.h
const KERNEL_SIGNAL_SET_BYTES: usize = size_of::<u64>();

/// A first page inside a word of the dirty log, which KVM refuses to clear from.
///
/// https://docs.kernel.org/virt/kvm/api.html#kvm-clear-dirty-log
const FIRST_PAGE_INSIDE_A_WORD: u64 = 1;

/// Returns the host's page size, in which KVM gives memory.
fn page() -> usize {
    rustix::param::page_size()
}

/// Returns the id of the 64-bit core register at the byte offset in `kvm_regs`.
///
/// https://docs.kernel.org/virt/kvm/api.html#kvm-set-one-reg
fn core_register(offset: usize) -> u64 {
    KVM_REG_ARM64
        | KVM_REG_SIZE_U64
        | u64::from(KVM_REG_ARM_CORE)
        | (offset / size_of::<u32>()) as u64
}

/// Returns the id of the general register `x<number>`.
fn general_register(number: usize) -> u64 {
    core_register(
        offset_of!(kvm_regs, regs) + offset_of!(user_pt_regs, regs) + number * size_of::<u64>(),
    )
}

/// Returns the id of the program counter.
fn program_counter() -> u64 {
    core_register(offset_of!(kvm_regs, regs) + offset_of!(user_pt_regs, pc))
}

/// Returns the id of the processor state.
fn processor_state() -> u64 {
    core_register(offset_of!(kvm_regs, regs) + offset_of!(user_pt_regs, pstate))
}

/// Opens the system and makes a VM of the default type.
fn create_vm<K: Kvm>(kvm: &K) -> (K::System, K::Vm) {
    let system = kvm.open().expect("this host must have KVM");
    let vm = kvm
        .create_vm(&system, u64::MIN)
        .expect("KVM must make a VM of the default type");
    (system, vm)
}

/// Makes a vCPU of the VM reset to the host's preferred target.
fn create_vcpu<K: Kvm>(kvm: &K, vm: &K::Vm) -> K::Vcpu {
    let init = kvm
        .arm_preferred_target(vm)
        .expect("KVM must name its preferred target");
    let vcpu = kvm.create_vcpu(vm, u64::MIN).expect("KVM must make a vCPU");
    kvm.arm_vcpu_init(&vcpu, &init)
        .expect("KVM must reset the vCPU to its preferred target");
    vcpu
}

/// Gives the VM one page of memory, holding the guest's two instructions, in slot 0.
fn give_program<K: Kvm>(kvm: &K, vm: &K::Vm) -> K::UserMemory {
    give_program_with_flags(kvm, vm, u32::MIN)
}

/// Gives the VM one page of memory, holding the guest's two instructions, in slot 0 with the flags.
fn give_program_with_flags<K: Kvm>(kvm: &K, vm: &K::Vm, flags: u32) -> K::UserMemory {
    let memory = kvm
        .create_user_memory(page())
        .expect("the host must map a page");
    let program: Vec<u8> = [STORE_TO_MEMORY, STORE_TO_DEVICE]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    assert_eq!(memory.write(&program), program.len());
    kvm.set_user_memory_region(vm, u32::MIN, flags, GUEST_MEMORY, Some(&memory))
        .expect("KVM must take the memory as a slot");
    memory
}

/// Sets the vCPU to run the program at EL1 with every exception masked, storing `STORED` to memory and the device.
fn point_at_program<K: Kvm>(kvm: &K, vcpu: &K::Vcpu) {
    let state = u64::from(PSR_MODE_EL1h | PSR_D_BIT | PSR_A_BIT | PSR_I_BIT | PSR_F_BIT);
    for (id, value) in [
        (program_counter(), GUEST_MEMORY),
        (processor_state(), state),
        (
            general_register(MEMORY_REGISTER),
            GUEST_MEMORY + STORED_AT as u64,
        ),
        (general_register(VALUE_REGISTER), STORED),
        (general_register(DEVICE_REGISTER), DEVICE),
    ] {
        kvm.set_one_reg(vcpu, id, &value.to_le_bytes())
            .expect("KVM must set a core register");
    }
}

/// Returns the exit reason and the device address in a run page.
fn exit<K: Kvm>(kvm: &K, vcpu: &K::Vcpu) -> (u32, u64) {
    let page = kvm.vcpu_run_page(vcpu);
    let mut bytes = vec![u8::MIN; size_of::<kvm_run>()];
    assert_eq!(
        page.read(&mut bytes),
        bytes.len(),
        "the run page holds a whole kvm_run"
    );
    let reason = offset_of!(kvm_run, exit_reason);
    let address = offset_of!(kvm_run, __bindgen_anon_1)
        + offset_of!(kvm_run__bindgen_ty_1__bindgen_ty_6, phys_addr);
    let word = |at: usize, length: usize| {
        bytes[at..at + length]
            .iter()
            .rev()
            .fold(u64::MIN, |all, &byte| all << u8::BITS | u64::from(byte))
    };
    (
        word(reason, size_of::<u32>()) as u32,
        word(address, size_of::<u64>()),
    )
}

/// Returns the eight bytes from the start of the memory, past `STORED_AT`.
fn stored(memory: &impl VolatileBytes) -> u64 {
    let mut bytes = [u8::MIN; STORED_AT + size_of::<u64>()];
    assert_eq!(memory.read(&mut bytes), bytes.len());
    u64::from_le_bytes(bytes[STORED_AT..].try_into().expect("eight bytes"))
}

/// Checks that the system answers at the API's version, names an extension it has, and has a run page to map.
pub fn opens_the_system_at_its_version<K: Kvm>(kvm: K) {
    let system = kvm.open().expect("this host must have KVM");
    assert_eq!(kvm.get_api_version(&system), Ok(KVM_API_VERSION as i32));
    assert!(
        kvm.check_extension(&system, KVM_CAP_USER_MEMORY)
            .expect("an answer")
            > i32::default()
    );
    assert_eq!(
        kvm.check_extension(&system, u32::MAX),
        Ok(i32::default()),
        "no extension has the largest number"
    );
    assert!(kvm.get_vcpu_mmap_size(&system).expect("a size") >= size_of::<kvm_run>());
}

/// Checks that a guest stores to its memory and then exits for a device, both seen from the host.
pub fn runs_a_guest_that_stores_and_exits_for_a_device<K: Kvm>(kvm: K) {
    let (system, vm) = create_vm(&kvm);
    let memory = give_program(&kvm, &vm);
    let vcpu = create_vcpu(&kvm, &vm);
    assert!(
        kvm.get_reg_list(&vcpu)
            .expect("a register list")
            .contains(&program_counter())
    );
    point_at_program(&kvm, &vcpu);
    let mut pc = [u8::MIN; size_of::<u64>()];
    kvm.get_one_reg(&vcpu, program_counter(), &mut pc)
        .expect("the program counter");
    assert_eq!(u64::from_le_bytes(pc), GUEST_MEMORY);
    let size = kvm.get_vcpu_mmap_size(&system).expect("a size");
    assert_eq!(kvm.vcpu_run_page(&vcpu).len(), size as u64);
    kvm.run(&vcpu).expect("the guest runs");
    assert_eq!(exit(&kvm, &vcpu), (KVM_EXIT_MMIO, DEVICE));
    assert_eq!(
        stored(&memory),
        STORED,
        "the host sees what the guest stored"
    );
}

/// Checks that a vCPU still runs on its memory once the VM and the memory's own handle are dropped.
pub fn keeps_memory_mapped_while_a_vcpu_lives<K: Kvm>(kvm: K) {
    let (system, vm) = create_vm(&kvm);
    let memory = give_program(&kvm, &vm);
    let vcpu = create_vcpu(&kvm, &vm);
    point_at_program(&kvm, &vcpu);
    drop((memory, vm, system));
    kvm.run(&vcpu)
        .expect("the guest runs on memory the VM still holds");
    assert_eq!(exit(&kvm, &vcpu), (KVM_EXIT_MMIO, DEVICE));
}

/// Checks that a vCPU made on one thread runs on another.
pub fn runs_a_vcpu_on_another_thread<K: Kvm + Sync>(kvm: K)
where
    K::Vcpu: Sync,
{
    let (_system, vm) = create_vm(&kvm);
    let memory = give_program(&kvm, &vm);
    let vcpu = create_vcpu(&kvm, &vm);
    point_at_program(&kvm, &vcpu);
    thread::scope(|scope| {
        scope
            .spawn(|| kvm.run(&vcpu).expect("the guest runs"))
            .join()
            .expect("the thread ends");
    });
    assert_eq!(stored(&memory), STORED);
}

/// Checks that a slot over taken addresses is refused, and that a slot taken away frees them.
pub fn refuses_overlapping_slots_until_one_is_taken_away<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let memory = kvm
        .create_user_memory(page())
        .expect("the host must map a page");
    assert_eq!(stored(&memory), u64::MIN, "fresh memory is zeroed");
    let second = SECOND_SLOT;
    kvm.set_user_memory_region(&vm, u32::MIN, u32::MIN, GUEST_MEMORY, Some(&memory))
        .expect("a slot");
    assert_eq!(
        kvm.set_user_memory_region(&vm, second, u32::MIN, GUEST_MEMORY, Some(&memory)),
        Err(Errno::EXIST)
    );
    kvm.set_user_memory_region(&vm, u32::MIN, u32::MIN, GUEST_MEMORY, None)
        .expect("the slot is taken away");
    kvm.set_user_memory_region(
        &vm,
        second,
        KVM_MEM_LOG_DIRTY_PAGES,
        GUEST_MEMORY,
        Some(&memory),
    )
    .expect("a slot");
    let pages_a_word = u64::BITS as usize;
    assert_eq!(
        kvm.get_dirty_log(&vm, second).map(|log| log.len()),
        Ok(page().div_ceil(page() * pages_a_word))
    );
    assert_eq!(
        kvm.get_dirty_log(&vm, u32::MIN),
        Err(Errno::NOENT),
        "slot 0 holds nothing now"
    );
}

/// Checks that memory of no length, or not of whole pages, is refused.
pub fn refuses_memory_not_of_whole_pages<K: Kvm>(kvm: K) {
    assert!(kvm.create_user_memory(usize::MIN).is_err());
    assert!(kvm.create_user_memory(page() + 1).is_err());
    let memory = kvm.create_user_memory(page()).expect("a page");
    assert_eq!(memory.len(), page() as u64);
}

/// Checks that bytes shorter than a register are refused both ways.
pub fn refuses_register_bytes_shorter_than_the_register<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let vcpu = create_vcpu(&kvm, &vm);
    let mut short = [u8::MIN; size_of::<u32>()];
    assert_eq!(
        kvm.get_one_reg(&vcpu, program_counter(), &mut short),
        Err(Errno::INVAL)
    );
    assert_eq!(
        kvm.set_one_reg(&vcpu, program_counter(), &short),
        Err(Errno::INVAL)
    );
}

/// Checks the interrupt controller's attributes, values of every length included, and the VM's interrupt calls.
pub fn sets_up_the_interrupt_controller_and_its_interrupts<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let vcpu = create_vcpu(&kvm, &vm);
    let gic = kvm
        .create_device(&vm, kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V3)
        .expect("a GICv3");
    let distributor = u64::from(KVM_VGIC_V3_ADDR_TYPE_DIST);
    assert_eq!(
        kvm.has_device_attr(&gic, KVM_DEV_ARM_VGIC_GRP_ADDR, distributor),
        Ok(true)
    );
    assert_eq!(kvm.has_device_attr(&gic, u32::MAX, distributor), Ok(false));
    let mut long = vec![u8::MIN; size_of::<kvm_smccc_filter>() + 1];
    long[..size_of::<u64>()].copy_from_slice(&DISTRIBUTOR.to_le_bytes());
    assert_eq!(
        kvm.set_device_attr(&gic, KVM_DEV_ARM_VGIC_GRP_ADDR, distributor, &long),
        Err(Errno::TOOBIG),
        "a value longer than any"
    );
    assert_eq!(
        kvm.get_device_attr(&gic, KVM_DEV_ARM_VGIC_GRP_ADDR, distributor, &mut long),
        Err(Errno::TOOBIG)
    );
    kvm.set_device_attr(
        &gic,
        KVM_DEV_ARM_VGIC_GRP_ADDR,
        distributor,
        &long[..size_of::<kvm_smccc_filter>()],
    )
    .expect("a value as long as the longest");
    let mut read = [u8::MIN; size_of::<u64>()];
    kvm.get_device_attr(&gic, KVM_DEV_ARM_VGIC_GRP_ADDR, distributor, &mut read)
        .expect("the address");
    assert_eq!(u64::from_le_bytes(read), DISTRIBUTOR);
    let mut one = [u8::MIN; size_of::<u8>()];
    kvm.get_device_attr(&gic, KVM_DEV_ARM_VGIC_GRP_ADDR, distributor, &mut one)
        .expect("a value shorter than any");
    assert_eq!(one, DISTRIBUTOR.to_le_bytes()[..size_of::<u8>()]);
    let redistributors = u64::from(KVM_VGIC_V3_ADDR_TYPE_REDIST);
    kvm.set_device_attr(
        &gic,
        KVM_DEV_ARM_VGIC_GRP_ADDR,
        redistributors,
        &REDISTRIBUTORS.to_le_bytes(),
    )
    .expect("placed");
    let init = u64::from(KVM_DEV_ARM_VGIC_CTRL_INIT);
    kvm.set_device_attr(&gic, KVM_DEV_ARM_VGIC_GRP_CTRL, init, &[])
        .expect("the GIC is set up");
    kvm.irq_line(
        &vm,
        KVM_ARM_IRQ_TYPE_SPI << KVM_ARM_IRQ_TYPE_SHIFT | FIRST_SHARED_INTERRUPT,
        true,
    )
    .expect("raised");
    let line = KVM_ARM_IRQ_TYPE_SPI << KVM_ARM_IRQ_TYPE_SHIFT | FIRST_SHARED_INTERRUPT;
    assert!(
        kvm.irq_line_status(&vm, line, false).expect("lowered") >= i32::default(),
        "the status KVM writes back"
    );
    let event = eventfd(u32::MIN, EventfdFlags::CLOEXEC).expect("an event file");
    let gsi = u32::MIN;
    kvm.irqfd(&vm, event.as_fd(), gsi, u32::MIN, None)
        .expect("tied");
    kvm.irqfd(&vm, event.as_fd(), gsi, KVM_IRQFD_FLAG_DEASSIGN, None)
        .expect("untied");
    assert_eq!(
        kvm.irqfd(&vm, event.as_fd(), gsi, KVM_IRQFD_FLAG_RESAMPLE, None),
        Err(Errno::INVAL),
        "resampling with no resample file"
    );
    let resample = eventfd(u32::MIN, EventfdFlags::CLOEXEC).expect("an event file");
    kvm.irqfd(
        &vm,
        event.as_fd(),
        gsi,
        KVM_IRQFD_FLAG_RESAMPLE,
        Some(resample.as_fd()),
    )
    .expect("tied with its resample file");
    kvm.irqfd(&vm, event.as_fd(), gsi, KVM_IRQFD_FLAG_DEASSIGN, None)
        .expect("untied");
    kvm.set_gsi_routing(&vm, &[]).expect("no routes");
    assert!(
        kvm.signal_msi(&vm, &Msi::default()).is_err(),
        "a VM with no ITS takes no message"
    );
    let width = size_of::<u32>() as u32;
    kvm.ioeventfd(&vm, event.as_fd(), DEVICE, width, u64::MIN, u32::MIN)
        .expect("tied");
    let deassign = 1 << kvm_ioeventfd_flag_nr_deassign;
    kvm.ioeventfd(&vm, event.as_fd(), DEVICE, width, u64::MIN, deassign)
        .expect("untied");
    let zone = page() as u32;
    kvm.register_coalesced_mmio(&vm, DEVICE, zone, false)
        .expect("gathered");
    kvm.unregister_coalesced_mmio(&vm, DEVICE, zone, false)
        .expect("no longer gathered");
    drop(vcpu);
}

/// Checks the vCPU's state calls: its run state, its pending exceptions, its debugging, and a feature it lacks.
pub fn reads_and_sets_the_vcpu_state<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let vcpu = create_vcpu(&kvm, &vm);
    let stopped = MpState {
        mp_state: KVM_MP_STATE_STOPPED,
    };
    kvm.set_mp_state(&vcpu, &stopped).expect("stopped");
    assert_eq!(
        kvm.get_mp_state(&vcpu).map(|state| state.mp_state),
        Ok(KVM_MP_STATE_STOPPED)
    );
    let events = kvm.get_vcpu_events(&vcpu).expect("the pending exceptions");
    kvm.set_vcpu_events(&vcpu, &events)
        .expect("the same exceptions set back");
    let debug = GuestDebug {
        control: KVM_GUESTDBG_ENABLE | KVM_GUESTDBG_SINGLESTEP,
        ..Default::default()
    };
    kvm.set_guest_debug(&vcpu, &debug).expect("stepping");
    assert!(
        kvm.arm_vcpu_finalize(&vcpu, KVM_ARM_VCPU_SVE as i32)
            .is_err(),
        "SVE was not asked for"
    );
}

/// Checks the attributes of a vCPU and of a VM, and that a write-only attribute is not read.
pub fn reads_and_sets_vcpu_and_vm_attributes<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let vcpu = create_vcpu(&kvm, &vm);
    let timer = u64::from(KVM_ARM_VCPU_TIMER_IRQ_VTIMER);
    assert_eq!(
        kvm.vcpu_has_device_attr(&vcpu, KVM_ARM_VCPU_TIMER_CTRL, timer),
        Ok(true)
    );
    let mut interrupt = [u8::MIN; size_of::<u32>()];
    kvm.vcpu_get_device_attr(&vcpu, KVM_ARM_VCPU_TIMER_CTRL, timer, &mut interrupt)
        .expect("the timer's interrupt");
    assert!(u32::from_le_bytes(interrupt) > u32::MIN);
    let _gic = kvm
        .create_device(&vm, kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V3)
        .expect("a GICv3, which the timer needs");
    kvm.vcpu_set_device_attr(&vcpu, KVM_ARM_VCPU_TIMER_CTRL, timer, &interrupt)
        .expect("set back");
    let filter_attr = u64::from(KVM_ARM_VM_SMCCC_FILTER);
    if kvm.vm_has_device_attr(&vm, KVM_ARM_VM_SMCCC_CTRL, filter_attr) == Ok(true) {
        let filter = kvm_smccc_filter {
            base: u32::MIN,
            nr_functions: FILTERED_FUNCTIONS,
            action: kvm_smccc_filter_action_KVM_SMCCC_FILTER_FWD_TO_USER as u8,
            ..Default::default()
        };
        let mut bytes = vec![u8::MIN; size_of::<kvm_smccc_filter>()];
        let fields = [filter.base.to_le_bytes(), filter.nr_functions.to_le_bytes()].concat();
        bytes[..fields.len()].copy_from_slice(&fields);
        bytes[fields.len()] = filter.action;
        kvm.vm_set_device_attr(&vm, KVM_ARM_VM_SMCCC_CTRL, filter_attr, &bytes)
            .expect("the filter");
        let mut read = vec![u8::MIN; size_of::<kvm_smccc_filter>()];
        assert!(
            kvm.vm_get_device_attr(&vm, KVM_ARM_VM_SMCCC_CTRL, filter_attr, &mut read)
                .is_err(),
            "write only"
        );
    }
}

/// Checks that each capability call succeeds exactly where the VM says it has the capability.
pub fn enables_what_the_vm_has_and_refuses_the_rest<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let has =
        |extension| kvm.vm_check_extension(&vm, extension).expect("an answer") > i32::default();
    let nisv = EnableCap {
        cap: KVM_CAP_ARM_NISV_TO_USER,
        ..Default::default()
    };
    assert_eq!(
        kvm.enable_cap(&vm, &nisv).is_ok(),
        has(KVM_CAP_ARM_NISV_TO_USER)
    );
    assert_eq!(
        kvm.arm_set_counter_offset(&vm, u64::MIN).is_ok(),
        has(KVM_CAP_COUNTER_OFFSET)
    );
    let attributes = MemoryAttributes {
        address: GUEST_MEMORY,
        size: page() as u64,
        ..Default::default()
    };
    if !has(KVM_CAP_MEMORY_ATTRIBUTES) {
        assert!(kvm.set_memory_attributes(&vm, &attributes).is_err());
    }
    let guest_memfd = CreateGuestMemfd {
        size: page() as u64,
        ..Default::default()
    };
    assert_eq!(
        kvm.create_guest_memfd(&vm, &guest_memfd).is_ok(),
        has(KVM_CAP_GUEST_MEMFD)
    );
}

/// Checks that the dirty log of a slot of many pages is as long as the slot asks, one bit a page.
pub fn sizes_the_dirty_log_from_a_large_slot<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let memory = kvm
        .create_user_memory(page() * LARGE_SLOT_PAGES)
        .expect("the host must map the pages");
    kvm.set_user_memory_region(
        &vm,
        u32::MIN,
        KVM_MEM_LOG_DIRTY_PAGES,
        GUEST_MEMORY,
        Some(&memory),
    )
    .expect("a slot");
    assert_eq!(
        kvm.get_dirty_log(&vm, u32::MIN).map(|log| log.len()),
        Ok(LARGE_SLOT_PAGES / u64::BITS as usize)
    );
}

/// Checks that a change KVM refuses leaves the slot as it was.
pub fn keeps_the_slot_through_a_refused_change<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let first = kvm.create_user_memory(page()).expect("a page");
    let larger = kvm.create_user_memory(page() * 2).expect("two pages");
    kvm.set_user_memory_region(
        &vm,
        u32::MIN,
        KVM_MEM_LOG_DIRTY_PAGES,
        GUEST_MEMORY,
        Some(&first),
    )
    .expect("a slot");
    assert!(
        kvm.set_user_memory_region(
            &vm,
            u32::MIN,
            KVM_MEM_LOG_DIRTY_PAGES,
            GUEST_MEMORY,
            Some(&larger)
        )
        .is_err(),
        "KVM changes no slot's size"
    );
    assert!(
        kvm.get_dirty_log(&vm, u32::MIN).is_ok(),
        "the slot is still known"
    );
}

/// Checks that a vCPU never reset to a target is refused a run, with KVM's own error.
pub fn refuses_to_run_a_vcpu_not_yet_initialized<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let vcpu = kvm.create_vcpu(&vm, u64::MIN).expect("a vCPU");
    assert_eq!(kvm.run(&vcpu), Err(Errno::NOEXEC));
}

/// Checks the second kind of slot: its size, the refusals of a size the memory does not cover, and its removal.
pub fn gives_memory_as_a_slot_of_the_second_kind<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let memory = kvm
        .create_user_memory(page() * LARGE_SLOT_PAGES)
        .expect("the host must map the pages");
    let size = memory.len();
    let flags = KVM_MEM_LOG_DIRTY_PAGES;
    let set = |memory_size, memory| {
        kvm.set_user_memory_region2(
            &vm,
            u32::MIN,
            flags,
            GUEST_MEMORY,
            memory_size,
            memory,
            None,
            u64::MIN,
        )
    };
    if kvm.vm_check_extension(&vm, KVM_CAP_USER_MEMORY2) == Ok(i32::default()) {
        assert!(set(size, Some(&memory)).is_err());
        return;
    }
    assert_eq!(set(size + page() as u64, Some(&memory)), Err(Errno::INVAL));
    assert_eq!(set(size, None), Err(Errno::INVAL));
    set(size / 2, Some(&memory)).expect("a slot of half the memory");
    assert_eq!(
        kvm.get_dirty_log(&vm, u32::MIN).map(|log| log.len()),
        Ok(LARGE_SLOT_PAGES / 2 / u64::BITS as usize)
    );
    set(u64::MIN, None).expect("the slot is taken away");
    assert_eq!(kvm.get_dirty_log(&vm, u32::MIN), Err(Errno::NOENT));
}

/// Checks that a slot's dirty bits are cleared for whole words of pages, and that a bitmap too short is refused.
pub fn clears_the_dirty_log_of_the_pages_asked<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    if kvm
        .vm_check_extension(&vm, KVM_CAP_MANUAL_DIRTY_LOG_PROTECT2)
        .is_ok_and(|answer| answer > i32::default())
    {
        let manual = EnableCap {
            cap: KVM_CAP_MANUAL_DIRTY_LOG_PROTECT2,
            args: [
                u64::from(KVM_DIRTY_LOG_MANUAL_PROTECT_ENABLE),
                u64::MIN,
                u64::MIN,
                u64::MIN,
            ],
            ..Default::default()
        };
        kvm.enable_cap(&vm, &manual).expect("manual protection");
    }
    let memory = kvm
        .create_user_memory(page() * LARGE_SLOT_PAGES)
        .expect("the host must map the pages");
    kvm.set_user_memory_region(
        &vm,
        u32::MIN,
        KVM_MEM_LOG_DIRTY_PAGES,
        GUEST_MEMORY,
        Some(&memory),
    )
    .expect("a slot");
    let pages = LARGE_SLOT_PAGES as u32;
    let words = LARGE_SLOT_PAGES / u64::BITS as usize;
    let all = vec![u64::MAX; words];
    kvm.clear_dirty_log(&vm, u32::MIN, u64::MIN, pages, &all)
        .expect("cleared");
    assert_eq!(
        kvm.clear_dirty_log(&vm, u32::MIN, u64::MIN, pages, &all[1..]),
        Err(Errno::INVAL),
        "a bitmap with fewer bits than pages"
    );
    assert!(
        kvm.clear_dirty_log(&vm, u32::MIN, FIRST_PAGE_INSIDE_A_WORD, u64::BITS, &all)
            .is_err(),
        "a first page inside a word"
    );
}

/// Checks the dirty ring: absent until turned on, then the page the guest wrote, which KVM takes back once reset.
pub fn keeps_a_dirty_ring_of_the_pages_written<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    assert!(kvm.reset_dirty_rings(&vm).is_err(), "no ring is on");
    let bytes = DIRTY_RING_ENTRIES * size_of::<kvm_dirty_gfn>();
    let ring = EnableCap {
        cap: KVM_CAP_DIRTY_LOG_RING_ACQ_REL,
        args: [bytes as u64, u64::MIN, u64::MIN, u64::MIN],
        ..Default::default()
    };
    let has_ring = kvm
        .vm_check_extension(&vm, KVM_CAP_DIRTY_LOG_RING_ACQ_REL)
        .is_ok_and(|largest| largest as usize >= bytes);
    assert_eq!(kvm.enable_cap(&vm, &ring).is_ok(), has_ring);
    let memory = give_program_with_flags(&kvm, &vm, KVM_MEM_LOG_DIRTY_PAGES);
    let vcpu = create_vcpu(&kvm, &vm);
    let Some(entries) = kvm.vcpu_dirty_ring(&vcpu) else {
        assert!(
            !has_ring,
            "a vCPU made after the ring was turned on has one"
        );
        return;
    };
    assert_eq!(entries.len(), bytes as u64);
    point_at_program(&kvm, &vcpu);
    kvm.run(&vcpu).expect("the guest runs");
    assert_eq!(stored(&memory), STORED);
    let mut first = [u8::MIN; size_of::<kvm_dirty_gfn>()];
    assert_eq!(entries.read(&mut first), first.len());
    let word = |at: usize| {
        u32::from_le_bytes(
            first[at..at + size_of::<u32>()]
                .try_into()
                .expect("four bytes"),
        )
    };
    assert_eq!(word(offset_of!(kvm_dirty_gfn, flags)), DIRTY_GFN_DIRTY);
    assert_eq!(word(offset_of!(kvm_dirty_gfn, slot)), u32::MIN);
    assert_eq!(
        entries.write(&DIRTY_GFN_RESET.to_le_bytes()),
        size_of::<u32>()
    );
    let pages = i32::try_from(memory.len() / page() as u64).expect("a page count");
    assert_eq!(
        kvm.reset_dirty_rings(&vm),
        Ok(pages),
        "the guest wrote each page of its memory"
    );
}

/// Checks the legacy interrupt controller: made once where the host can show a GICv2, and placed only once made.
pub fn makes_the_legacy_interrupt_controller_where_the_host_has_one<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let distributor =
        u64::from(KVM_ARM_DEVICE_VGIC_V2 << KVM_ARM_DEVICE_ID_SHIFT | KVM_VGIC_V2_ADDR_TYPE_DIST);
    match kvm.create_irq_chip(&vm) {
        Ok(()) => {
            assert_eq!(kvm.create_irq_chip(&vm), Err(Errno::EXIST));
            kvm.arm_set_device_addr(&vm, distributor, DISTRIBUTOR)
                .expect("placed");
        }
        Err(error) => {
            assert_eq!(error, Errno::NODEV, "a host that cannot show a GICv2");
            assert!(
                kvm.arm_set_device_addr(&vm, distributor, DISTRIBUTOR)
                    .is_err()
            );
        }
    }
}

/// Checks the VM's ARM64 calls of memory tags and writable register bits against what it says it has.
pub fn answers_tags_and_writable_masks_as_the_vm_has_them<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let _memory = give_program(&kvm, &vm);
    let mut tags = vec![u8::MIN; page() / MTE_GRANULE_BYTES as usize];
    assert_eq!(
        kvm.arm_mte_copy_tags(
            &vm,
            GUEST_MEMORY,
            u64::from(KVM_ARM_TAGS_FROM_GUEST),
            &mut tags
        ),
        Err(Errno::INVAL),
        "a VM with tags not turned on has none"
    );
    let ranges = kvm
        .vm_check_extension(&vm, KVM_CAP_ARM_SUPPORTED_REG_MASK_RANGES)
        .expect("an answer");
    let masks = kvm.arm_get_reg_writable_masks(&vm, KVM_ARM_FEATURE_ID_RANGE);
    if ranges & 1 << KVM_ARM_FEATURE_ID_RANGE == i32::default() {
        assert!(masks.is_err());
    } else {
        let masks = masks.expect("the masks");
        assert_eq!(masks.len(), KVM_ARM_FEATURE_ID_RANGE_SIZE as usize);
        assert!(
            masks.iter().any(|mask| *mask != u64::MIN),
            "some bit is writable"
        );
    }
}

/// Checks that the VM and the vCPU give statistics files that start with their header.
pub fn gives_statistics_files<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let vcpu = create_vcpu(&kvm, &vm);
    for stats in [kvm.vm_get_stats_fd(&vm), kvm.vcpu_get_stats_fd(&vcpu)] {
        let mut header = [u8::MIN; size_of::<kvm_stats_header>()];
        File::from(stats.expect("a statistics file"))
            .read_exact(&mut header)
            .expect("the header");
        let at = offset_of!(kvm_stats_header, num_desc);
        let descriptors = u32::from_le_bytes(
            header[at..at + size_of::<u32>()]
                .try_into()
                .expect("four bytes"),
        );
        assert!(descriptors > u32::MIN, "some statistic is described");
    }
}

/// Checks the vCPU's signal mask, set, refused when not the kernel's length, and taken away, and its memory pre-faulted.
pub fn sets_the_signal_mask_and_pre_faults_memory<K: Kvm>(kvm: K) {
    let (_system, vm) = create_vm(&kvm);
    let _memory = give_program(&kvm, &vm);
    let vcpu = create_vcpu(&kvm, &vm);
    kvm.set_signal_mask(&vcpu, Some(&[u8::MIN; KERNEL_SIGNAL_SET_BYTES][..]))
        .expect("nothing blocked");
    assert_eq!(
        kvm.set_signal_mask(&vcpu, Some(&[u8::MIN; KERNEL_SIGNAL_SET_BYTES / 2][..])),
        Err(Errno::INVAL)
    );
    kvm.set_signal_mask(&vcpu, None)
        .expect("the thread's own mask");
    let mut range = PreFaultMemory {
        gpa: GUEST_MEMORY,
        size: page() as u64,
        ..Default::default()
    };
    let pre_faulted = kvm.pre_fault_memory(&vcpu, &mut range);
    if kvm.vm_check_extension(&vm, KVM_CAP_PRE_FAULT_MEMORY) == Ok(i32::default()) {
        assert!(pre_faulted.is_err());
    } else if pre_faulted.is_ok() {
        assert_eq!(range.size, u64::MIN, "nothing is left to map");
    }
}

/// Runs each suite function above as its own test on the value an implementation gives, made anew for every test.
#[macro_export]
macro_rules! tests {
    ($value:expr) => {
        #[test]
        fn opens_the_system_at_its_version() {
            $crate::opens_the_system_at_its_version($value);
        }

        #[test]
        fn runs_a_guest_that_stores_and_exits_for_a_device() {
            $crate::runs_a_guest_that_stores_and_exits_for_a_device($value);
        }

        #[test]
        fn keeps_memory_mapped_while_a_vcpu_lives() {
            $crate::keeps_memory_mapped_while_a_vcpu_lives($value);
        }

        #[test]
        fn runs_a_vcpu_on_another_thread() {
            $crate::runs_a_vcpu_on_another_thread($value);
        }

        #[test]
        fn refuses_overlapping_slots_until_one_is_taken_away() {
            $crate::refuses_overlapping_slots_until_one_is_taken_away($value);
        }

        #[test]
        fn refuses_memory_not_of_whole_pages() {
            $crate::refuses_memory_not_of_whole_pages($value);
        }

        #[test]
        fn refuses_register_bytes_shorter_than_the_register() {
            $crate::refuses_register_bytes_shorter_than_the_register($value);
        }

        #[test]
        fn sets_up_the_interrupt_controller_and_its_interrupts() {
            $crate::sets_up_the_interrupt_controller_and_its_interrupts($value);
        }

        #[test]
        fn reads_and_sets_the_vcpu_state() {
            $crate::reads_and_sets_the_vcpu_state($value);
        }

        #[test]
        fn reads_and_sets_vcpu_and_vm_attributes() {
            $crate::reads_and_sets_vcpu_and_vm_attributes($value);
        }

        #[test]
        fn enables_what_the_vm_has_and_refuses_the_rest() {
            $crate::enables_what_the_vm_has_and_refuses_the_rest($value);
        }

        #[test]
        fn sizes_the_dirty_log_from_a_large_slot() {
            $crate::sizes_the_dirty_log_from_a_large_slot($value);
        }

        #[test]
        fn keeps_the_slot_through_a_refused_change() {
            $crate::keeps_the_slot_through_a_refused_change($value);
        }

        #[test]
        fn refuses_to_run_a_vcpu_not_yet_initialized() {
            $crate::refuses_to_run_a_vcpu_not_yet_initialized($value);
        }

        #[test]
        fn gives_memory_as_a_slot_of_the_second_kind() {
            $crate::gives_memory_as_a_slot_of_the_second_kind($value);
        }

        #[test]
        fn clears_the_dirty_log_of_the_pages_asked() {
            $crate::clears_the_dirty_log_of_the_pages_asked($value);
        }

        #[test]
        fn keeps_a_dirty_ring_of_the_pages_written() {
            $crate::keeps_a_dirty_ring_of_the_pages_written($value);
        }

        #[test]
        fn makes_the_legacy_interrupt_controller_where_the_host_has_one() {
            $crate::makes_the_legacy_interrupt_controller_where_the_host_has_one($value);
        }

        #[test]
        fn answers_tags_and_writable_masks_as_the_vm_has_them() {
            $crate::answers_tags_and_writable_masks_as_the_vm_has_them($value);
        }

        #[test]
        fn gives_statistics_files() {
            $crate::gives_statistics_files($value);
        }

        #[test]
        fn sets_the_signal_mask_and_pre_faults_memory() {
            $crate::sets_the_signal_mask_and_pre_faults_memory($value);
        }
    };
}
