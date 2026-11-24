//! [`Hypervisor`] on KVM (Linux, ARM64), through kvm-ioctls.

use std::collections::VecDeque;
use std::ffi::c_int;
use std::mem::offset_of;
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicI32, AtomicU8, AtomicU32, Ordering, fence};
use std::sync::{Arc, Mutex, Once};

use aarch64_cpu::registers::{CNTVCT_EL0, Read};
use arm_gic::IntId;
use kvm_bindings::{
    KVM_ARM_IRQ_TYPE_SHIFT, KVM_ARM_IRQ_TYPE_SPI, KVM_ARM_VCPU_SVE, KVM_ARM_VCPU_TIMER_CTRL,
    KVM_ARM_VCPU_TIMER_IRQ_VTIMER, KVM_ARM_VM_SMCCC_CTRL, KVM_ARM_VM_SMCCC_FILTER,
    KVM_DEV_ARM_VGIC_CTRL_INIT, KVM_DEV_ARM_VGIC_GRP_ADDR, KVM_DEV_ARM_VGIC_GRP_CTRL,
    KVM_REG_ARM_CORE, KVM_REG_ARM64, KVM_REG_ARM64_SYSREG, KVM_REG_ARM64_SYSREG_CRM_SHIFT,
    KVM_REG_ARM64_SYSREG_CRN_SHIFT, KVM_REG_ARM64_SYSREG_OP0_SHIFT, KVM_REG_ARM64_SYSREG_OP1_SHIFT,
    KVM_REG_ARM64_SYSREG_OP2_SHIFT, KVM_REG_SIZE_U32, KVM_REG_SIZE_U64, KVM_REG_SIZE_U128,
    KVM_SPSR_EL1, KVM_VGIC_V3_ADDR_TYPE_DIST, KVM_VGIC_V3_ADDR_TYPE_REDIST, KVMIO,
    kvm_arm_counter_offset, kvm_create_device, kvm_device_attr,
    kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V3, kvm_regs, kvm_run, kvm_smccc_filter,
    kvm_smccc_filter_action_KVM_SMCCC_FILTER_FWD_TO_USER, kvm_userspace_memory_region,
    kvm_vcpu_init, user_fpsimd_state, user_pt_regs,
};
use kvm_ioctls::{Cap, DeviceFd, Kvm, VcpuExit, VcpuFd, VmFd};
use vmm_sys_util::ioctl::ioctl_with_ref;
use vmm_sys_util::ioctl_iow_nr;
use vmm_sys_util::signal::SIGRTMIN;
use wcontainers_executor_hardware_virtualization_inside::layout::{
    GIC_DISTRIBUTOR, GIC_REDISTRIBUTORS,
};
use wcontainers_executor_hardware_virtualization_inside::link::DOORBELL_FUNCTION;

use crate::{Error, Exit, Hypervisor, Interrupts, Memory, Register, Result};

/// The number of KVM_ARM_SET_COUNTER_OFFSET, which kvm-ioctls does not wrap.
const SET_COUNTER_OFFSET: u32 = 0xb5;

/// The number of KVM_GET_DEVICE_ATTR, which kvm-ioctls does not wrap for a virtual CPU.
const GET_DEVICE_ATTR: u32 = 0xe2;

ioctl_iow_nr!(
    KVM_ARM_SET_COUNTER_OFFSET,
    KVMIO,
    SET_COUNTER_OFFSET,
    kvm_arm_counter_offset
);
ioctl_iow_nr!(KVM_GET_DEVICE_ATTR, KVMIO, GET_DEVICE_ATTR, kvm_device_attr);

/// KVM.
pub struct KernelVirtualMachine;

/// The process's machine, with its interrupt controller and the virtual CPUs not yet taken.
pub struct KvmVirtualMachine {
    vm: VmFd,
    _gic: DeviceFd,
    cpus: Mutex<VecDeque<VcpuFd>>,
    /// The memory slot the next mapping takes.
    next_slot: AtomicU32,
    counter_offset: u64,
    timer: u32,
}

/// A virtual CPU, run by one thread at a time.
pub struct KvmCpu {
    vcpu: VcpuFd,
    remote: Arc<Remote>,
    counter_offset: u64,
}

/// What another thread stops a virtual CPU through: its own mapping of the CPU's run page, and the CPU's thread.
struct Remote {
    run: NonNull<kvm_run>,
    bytes: usize,
    /// The thread that ran the CPU last, [`NOT_RUN_BY_A_THREAD`] before its first run and once it is gone.
    thread: AtomicI32,
}

/// The thread of a virtual CPU that has never run or is gone: no thread has a negative number.
const NOT_RUN_BY_A_THREAD: i32 = libc::pid_t::MIN;

thread_local! {
    /// The calling thread's number, which the stop signal is sent to.
    static THREAD: i32 = rustix::thread::gettid().as_raw_nonzero().get();
}

// SAFETY: the mapping is the kernel's shared run page, which is written only through atomics here.
#[expect(
    unsafe_code,
    reason = "UNSOUND: promises Remote may move to another thread, from which it writes immediate_exit through its own second mapping of the run page while kvm-ioctls holds &mut kvm_run over that page on the CPU's thread (kvm-ioctls vcpu.rs:1690), the same fault as the Sync below; Kick replaces it"
)]
unsafe impl Send for Remote {}
// SAFETY: as above.
#[expect(
    unsafe_code,
    reason = "UNSOUND: lets another thread write the run page's immediate_exit while kvm-ioctls holds &mut kvm_run over the same page (kvm-ioctls vcpu.rs:1690); Kick replaces it"
)]
unsafe impl Sync for Remote {}

/// Stops a virtual CPU: sets its run page's `immediate_exit`, which KVM_RUN reads when it starts, then signals its
/// thread, which ends a KVM_RUN in progress.
#[derive(Clone)]
pub struct KvmStopper {
    remote: Arc<Remote>,
}

/// KVM's refusal, in words.
fn refused(what: std::fmt::Arguments) -> Error {
    Error::Hypervisor(what.to_string())
}

/// The signal that ends a virtual CPU's run.
fn stop_signal() -> c_int {
    SIGRTMIN()
}

/// The stop signal's handler does nothing: delivering it is what ends KVM_RUN.
extern "C" fn on_stop_signal(_: c_int) {}

/// Installs the stop signal's handler once per process. Calls the signal interrupts restart where SA_RESTART
/// applies; others, such as poll, fail with EINTR.
fn install_stop_signal() -> Result<()> {
    static INSTALLED: Once = Once::new();
    let mut outcome = Ok(());
    INSTALLED.call_once(|| {
        // SAFETY: a zeroed `sigaction` is a valid start, and every field the call reads is set.
        #[expect(
            unsafe_code,
            reason = "a zeroed libc sigaction, valid for this plain C struct, before the fields the call reads are set; Kick, by signal-hook, replaces it"
        )]
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = on_stop_signal as extern "C" fn(c_int) as usize;
        action.sa_flags = libc::SA_RESTART;
        // SAFETY: a handler that touches nothing, for a signal of the application's own range.
        #[expect(
            unsafe_code,
            reason = "installs, once per process, a handler that does nothing, with SA_RESTART, for the real-time stop signal; Kick, by signal-hook, replaces it"
        )]
        let installed = unsafe { libc::sigaction(stop_signal(), &action, ptr::null_mut()) };
        if installed != 0 {
            outcome = Err(Error::Host(std::io::Error::last_os_error()));
        }
    });
    outcome
}

/// Sets the machine's counter offset: its virtual CPUs' counters read the host's counter less it.
fn set_counter_offset(vm: &VmFd, offset: u64) -> Result<()> {
    let offset = kvm_arm_counter_offset {
        counter_offset: offset,
        ..Default::default()
    };
    // SAFETY: the machine's descriptor, with the argument the call reads.
    #[expect(
        unsafe_code,
        reason = "calls KVM_ARM_SET_COUNTER_OFFSET on the machine's own descriptor with the struct the call reads; kvm-ioctls has no wrapper, so it stays a one-off"
    )]
    let done = unsafe { ioctl_with_ref(vm, KVM_ARM_SET_COUNTER_OFFSET(), &offset) };
    if done < 0 {
        return Err(refused(format_args!(
            "setting the counter offset: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

/// The interrupt number of a virtual CPU's virtual timer.
fn timer_interrupt(vcpu: &VcpuFd) -> Result<u32> {
    let mut timer = 0u32;
    let attribute = kvm_device_attr {
        group: KVM_ARM_VCPU_TIMER_CTRL,
        attr: KVM_ARM_VCPU_TIMER_IRQ_VTIMER.into(),
        addr: (&raw mut timer) as u64,
        ..Default::default()
    };
    // SAFETY: a virtual CPU's descriptor, with an attribute whose value the call writes into `timer`.
    #[expect(
        unsafe_code,
        reason = "calls KVM_GET_DEVICE_ATTR on a virtual CPU's descriptor, which writes one u32 into the live local the attribute points to; kvm-ioctls has no safe getter; setting it with set_device_attr may remove it"
    )]
    let done = unsafe { ioctl_with_ref(vcpu, KVM_GET_DEVICE_ATTR(), &attribute) };
    if done < 0 {
        return Err(refused(format_args!(
            "reading the timer's interrupt: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(timer)
}

/// Makes the interrupt controller, a GICv3 with its distributor and redistributors where the inside finds them.
/// It is set up once every virtual CPU is made.
fn interrupt_controller(vm: &VmFd) -> Result<DeviceFd> {
    let mut device = kvm_create_device {
        type_: kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V3,
        ..Default::default()
    };
    let gic = vm
        .create_device(&mut device)
        .map_err(|e| refused(format_args!("making the interrupt controller: {e}")))?;
    for (kind, address) in [
        (KVM_VGIC_V3_ADDR_TYPE_DIST, GIC_DISTRIBUTOR),
        (KVM_VGIC_V3_ADDR_TYPE_REDIST, GIC_REDISTRIBUTORS),
    ] {
        gic.set_device_attr(&kvm_device_attr {
            group: KVM_DEV_ARM_VGIC_GRP_ADDR,
            attr: kind.into(),
            addr: (&raw const address) as u64,
            ..Default::default()
        })
        .map_err(|e| refused(format_args!("placing the interrupt controller: {e}")))?;
    }
    Ok(gic)
}

fn set_up(gic: &DeviceFd) -> Result<()> {
    gic.set_device_attr(&kvm_device_attr {
        group: KVM_DEV_ARM_VGIC_GRP_CTRL,
        attr: KVM_DEV_ARM_VGIC_CTRL_INIT.into(),
        ..Default::default()
    })
    .map_err(|e| refused(format_args!("setting up the interrupt controller: {e}")))
}

/// Whether a machine's virtual CPUs have the host's [SVE](https://docs.kernel.org/arch/arm64/sve.html#:~:text=KVM%20guests), which KVM hides unless asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalableVectors {
    Hidden,
    /// Shown where the host has it, at every vector length the host has.
    Shown,
}

impl Hypervisor for KernelVirtualMachine {
    type VirtualMachine = KvmVirtualMachine;

    fn machine(cpus: u64) -> Result<KvmVirtualMachine> {
        Self::machine_with(cpus, ScalableVectors::Hidden)
    }
}

impl KernelVirtualMachine {
    /// Makes the machine as [`Hypervisor::machine`] does, with SVE as asked.
    ///
    /// KVM refuses a virtual CPU once the interrupt controller is set up, so every one is made here.
    pub fn machine_with(cpus: u64, scalable_vectors: ScalableVectors) -> Result<KvmVirtualMachine> {
        install_stop_signal()?;
        let kvm = Kvm::new().map_err(|e| refused(format_args!("opening KVM: {e}")))?;
        let vm = kvm
            .create_vm()
            .map_err(|e| refused(format_args!("making the machine: {e}")))?;
        let counter_offset = CNTVCT_EL0.get();
        set_counter_offset(&vm, counter_offset)?;
        let filter = DOORBELL_FILTER;
        vm.set_device_attr(&kvm_device_attr {
            group: KVM_ARM_VM_SMCCC_CTRL,
            attr: KVM_ARM_VM_SMCCC_FILTER.into(),
            addr: (&raw const filter) as u64,
            ..Default::default()
        })
        .map_err(|e| refused(format_args!("forwarding the doorbell: {e}")))?;
        let mut init = kvm_vcpu_init::default();
        vm.get_preferred_target(&mut init)
            .map_err(|e| refused(format_args!("the preferred CPU: {e}")))?;
        let sve = scalable_vectors == ScalableVectors::Shown && kvm.check_extension(Cap::ArmSve);
        if sve {
            init.features[0] |= 1 << KVM_ARM_VCPU_SVE;
        }
        let mut made = VecDeque::new();
        for id in 0..cpus {
            let vcpu = vm
                .create_vcpu(id)
                .map_err(|e| refused(format_args!("making a virtual CPU: {e}")))?;
            vcpu.vcpu_init(&init)
                .map_err(|e| refused(format_args!("starting a virtual CPU: {e}")))?;
            if sve {
                vcpu.vcpu_finalize(&(KVM_ARM_VCPU_SVE as c_int))
                    .map_err(|e| refused(format_args!("fixing a virtual CPU's SVE: {e}")))?;
            }
            made.push_back(vcpu);
        }
        let gic = interrupt_controller(&vm)?;
        set_up(&gic)?;
        let timer = match made.front() {
            Some(vcpu) => timer_interrupt(vcpu)?,
            None => return Err(refused(format_args!("a machine with no CPU"))),
        };
        Ok(KvmVirtualMachine {
            vm,
            _gic: gic,
            cpus: Mutex::new(made),
            next_slot: AtomicU32::new(0),
            counter_offset,
            timer,
        })
    }
}

/// The SMCCC filter that forwards the doorbell's function, alone, to the monitor.
const DOORBELL_FILTER: kvm_smccc_filter = kvm_smccc_filter {
    base: DOORBELL_FUNCTION,
    nr_functions: 1,
    action: kvm_smccc_filter_action_KVM_SMCCC_FILTER_FWD_TO_USER as u8,
    pad: [0; 15],
};

impl crate::VirtualMachine for KvmVirtualMachine {
    type Cpu = KvmCpu;

    #[expect(
        unsafe_code,
        reason = "UNSOUND: registers host memory as guest memory from a safe trait method that takes any pointer and length, so nothing keeps the range alive while it is mapped; Lend replaces it"
    )]
    fn map(&self, host: NonNull<u8>, at: u64, length: u64) -> Result<()> {
        let region = kvm_userspace_memory_region {
            slot: self.next_slot.fetch_add(1, Ordering::Relaxed),
            flags: 0,
            guest_phys_addr: at,
            memory_size: length,
            userspace_addr: host.as_ptr() as u64,
        };
        // SAFETY: the host range is the caller's memory and outlives the mapping.
        unsafe { self.vm.set_user_memory_region(region) }
            .map_err(|e| refused(format_args!("mapping memory: {e}")))
    }

    /// Private anonymous memory reads as zeros once its pages are dropped, and KVM drops its own mappings of
    /// them with them.
    fn give_back(&self, memory: &Memory, at: u64, length: u64) -> Result<()> {
        if !memory.contains(at, length) {
            return Err(refused(format_args!(
                "giving back {length:#x} bytes at {at:#x}, which are not all guest memory"
            )));
        }
        let host = memory.host_bytes(at, length);
        // SAFETY: the whole range is guest memory, which the caller no longer needs.
        #[expect(
            unsafe_code,
            reason = "UNEXPLAINED: drops the host pages of a range give_back checked is all guest memory, which then reads as zeros, from a safe method any thread may call while virtual CPUs or the monitor's plain copies reach that range; only the inside's request says nothing uses it; a hole punch in the memfd replaces it in step one"
        )]
        let done = unsafe { libc::madvise(host.cast(), length as usize, libc::MADV_DONTNEED) };
        if done != 0 {
            return Err(Error::Host(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    fn raise(&self, interrupt: u32, level: bool) -> Result<()> {
        self.vm
            .set_irq_line(
                KVM_ARM_IRQ_TYPE_SPI << KVM_ARM_IRQ_TYPE_SHIFT | interrupt,
                level,
            )
            .map_err(|e| refused(format_args!("raising an interrupt: {e}")))
    }

    fn cpu(&self) -> Result<KvmCpu> {
        let vcpu = self
            .cpus
            .lock()
            .expect("the virtual CPUs")
            .pop_front()
            .ok_or_else(|| refused(format_args!("every virtual CPU of the machine is taken")))?;
        let bytes = self.vm.run_size();
        // SAFETY: a shared mapping of the virtual CPU's run page, as KVM gives it to every mapper.
        #[expect(
            unsafe_code,
            reason = "UNSOUND: maps the virtual CPU's run page a second time, so immediate_exit is written through it while kvm-ioctls holds &mut kvm_run over the page; Kick's flag and repeated signal replace it"
        )]
        let run = unsafe {
            libc::mmap(
                ptr::null_mut(),
                bytes,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                std::os::fd::AsRawFd::as_raw_fd(&vcpu),
                0,
            )
        };
        if run == libc::MAP_FAILED {
            return Err(Error::Host(std::io::Error::last_os_error()));
        }
        let remote = Arc::new(Remote {
            run: NonNull::new(run.cast()).expect("mmap gave null"),
            bytes,
            thread: AtomicI32::new(NOT_RUN_BY_A_THREAD),
        });
        Ok(KvmCpu {
            vcpu,
            remote,
            counter_offset: self.counter_offset,
        })
    }

    fn interrupts(&self) -> Result<Interrupts> {
        Ok(Interrupts {
            timer: self.timer,
            first_raised: IntId::spi(0).into(),
        })
    }
}

impl Remote {
    #[expect(
        unsafe_code,
        reason = "UNSOUND: an atomic view of immediate_exit in the second mapping of the run page, while kvm-ioctls holds &mut kvm_run over it; Kick's flag and repeated signal replace it"
    )]
    fn immediate_exit(&self) -> &AtomicU8 {
        // SAFETY: the field lies in the mapping, which lives as long as `self`; the kernel and every writer here
        // reach it only through atomics or the kernel's own reads.
        unsafe { AtomicU8::from_ptr(&raw mut (*self.run.as_ptr()).immediate_exit) }
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        // SAFETY: the mapping is this value's own.
        #[expect(
            unsafe_code,
            reason = "unmaps the mapping of the run page this value made and alone holds, once nothing reaches it; Kick replaces it"
        )]
        unsafe { libc::munmap(self.run.as_ptr().cast(), self.bytes) };
    }
}

impl Drop for KvmCpu {
    fn drop(&mut self) {
        self.remote.thread.store(NOT_RUN_BY_A_THREAD, Ordering::SeqCst);
    }
}

/// Where a register is in KVM's register numbering, and its size there.
enum Place {
    /// A field of `kvm_regs`, at this byte offset.
    Core(usize, u64),
    /// A system register by its encoding.
    System(SystemRegister),
}

/// A system register's encoding.
#[derive(Clone, Copy)]
struct SystemRegister {
    op0: u64,
    op1: u64,
    crn: u64,
    crm: u64,
    op2: u64,
}

const fn system(op0: u64, op1: u64, crn: u64, crm: u64, op2: u64) -> SystemRegister {
    SystemRegister {
        op0,
        op1,
        crn,
        crm,
        op2,
    }
}

const SCTLR_EL1: SystemRegister = system(3, 0, 1, 0, 0);
const CPACR_EL1: SystemRegister = system(3, 0, 1, 0, 2);
const TTBR1_EL1: SystemRegister = system(3, 0, 2, 0, 1);
const TCR_EL1: SystemRegister = system(3, 0, 2, 0, 2);
const MAIR_EL1: SystemRegister = system(3, 0, 10, 2, 0);
const MPIDR_EL1: SystemRegister = system(3, 0, 0, 0, 5);
const ID_AA64MMFR0_EL1: SystemRegister = system(3, 0, 0, 7, 0);
const ID_AA64PFR0_EL1: SystemRegister = system(3, 0, 0, 4, 0);
const ID_AA64PFR1_EL1: SystemRegister = system(3, 0, 0, 4, 1);
const ESR_EL1: SystemRegister = system(3, 0, 5, 2, 0);
const FAR_EL1: SystemRegister = system(3, 0, 6, 0, 0);
const VBAR_EL1: SystemRegister = system(3, 0, 12, 0, 0);
const MDSCR_EL1: SystemRegister = system(2, 0, 0, 2, 2);
const TPIDR_EL0: SystemRegister = system(3, 3, 13, 0, 2);
const TPIDR_EL1: SystemRegister = system(3, 0, 13, 0, 4);

/// The byte offsets in `kvm_regs` of the general registers, the float status registers and the vectors.
const GENERAL: usize = offset_of!(kvm_regs, regs) + offset_of!(user_pt_regs, regs);
const FLOAT: usize = offset_of!(kvm_regs, fp_regs);
const VECTORS: usize = FLOAT + offset_of!(user_fpsimd_state, vregs);

fn core(offset: usize, size: u64) -> Place {
    Place::Core(offset, size)
}

fn place(register: Register) -> Place {
    let regs = offset_of!(kvm_regs, regs);
    match register {
        Register::Pc => core(regs + offset_of!(user_pt_regs, pc), KVM_REG_SIZE_U64),
        Register::Pstate => core(regs + offset_of!(user_pt_regs, pstate), KVM_REG_SIZE_U64),
        Register::X(n) => core(
            GENERAL + usize::from(n) * size_of::<u64>(),
            KVM_REG_SIZE_U64,
        ),
        Register::StackEl0 => core(regs + offset_of!(user_pt_regs, sp), KVM_REG_SIZE_U64),
        Register::StackEl1 => core(offset_of!(kvm_regs, sp_el1), KVM_REG_SIZE_U64),
        Register::ExceptionLink => core(offset_of!(kvm_regs, elr_el1), KVM_REG_SIZE_U64),
        Register::SavedState => core(
            offset_of!(kvm_regs, spsr) + KVM_SPSR_EL1 as usize * size_of::<u64>(),
            KVM_REG_SIZE_U64,
        ),
        Register::FloatStatus => core(
            FLOAT + offset_of!(user_fpsimd_state, fpsr),
            KVM_REG_SIZE_U32,
        ),
        Register::FloatControl => core(
            FLOAT + offset_of!(user_fpsimd_state, fpcr),
            KVM_REG_SIZE_U32,
        ),
        Register::Mair => Place::System(MAIR_EL1),
        Register::Tcr => Place::System(TCR_EL1),
        Register::Sctlr => Place::System(SCTLR_EL1),
        Register::Cpacr => Place::System(CPACR_EL1),
        Register::Ttbr1 => Place::System(TTBR1_EL1),
        Register::Affinity => Place::System(MPIDR_EL1),
        Register::MemoryModelFeatures => Place::System(ID_AA64MMFR0_EL1),
        Register::ProcessorFeatures => Place::System(ID_AA64PFR0_EL1),
        Register::ProcessorFeatures1 => Place::System(ID_AA64PFR1_EL1),
        Register::Syndrome => Place::System(ESR_EL1),
        Register::FaultAddress => Place::System(FAR_EL1),
        Register::Vectors => Place::System(VBAR_EL1),
        Register::DebugControl => Place::System(MDSCR_EL1),
        Register::ThreadPointer => Place::System(TPIDR_EL0),
        Register::InsideThread => Place::System(TPIDR_EL1),
    }
}

/// A core register's number: its offset in `kvm_regs` counted in 32-bit words.
fn core_id(offset: usize, size: u64) -> u64 {
    KVM_REG_ARM64 | size | u64::from(KVM_REG_ARM_CORE) | (offset / size_of::<u32>()) as u64
}

fn id(place: &Place) -> u64 {
    match *place {
        Place::Core(offset, size) => core_id(offset, size),
        Place::System(r) => {
            KVM_REG_ARM64
                | KVM_REG_SIZE_U64
                | u64::from(KVM_REG_ARM64_SYSREG)
                | r.op0 << KVM_REG_ARM64_SYSREG_OP0_SHIFT
                | r.op1 << KVM_REG_ARM64_SYSREG_OP1_SHIFT
                | r.crn << KVM_REG_ARM64_SYSREG_CRN_SHIFT
                | r.crm << KVM_REG_ARM64_SYSREG_CRM_SHIFT
                | r.op2 << KVM_REG_ARM64_SYSREG_OP2_SHIFT
        }
    }
}

fn vector_id(n: u8) -> u64 {
    core_id(
        VECTORS + usize::from(n) * size_of::<u128>(),
        KVM_REG_SIZE_U128,
    )
}

impl crate::VirtualCpu for KvmCpu {
    type Stopper = KvmStopper;

    /// Records the calling thread first, so that a stop signals the thread that runs it now.
    fn run(&mut self) -> Result<Exit> {
        self.remote.thread.store(THREAD.with(|thread| *thread), Ordering::SeqCst);
        // Against the stopper's own fence: either KVM_RUN reads its `immediate_exit`, or it signals this thread.
        fence(Ordering::SeqCst);
        loop {
            match self.vcpu.run() {
                Ok(VcpuExit::Hypercall(call)) if call.nr == u64::from(DOORBELL_FUNCTION) => {
                    return Ok(Exit::Doorbell);
                }
                Ok(other) => {
                    return Err(refused(format_args!("unexpected exit {other:?}")));
                }
                Err(e) if e.errno() == libc::EINTR => {
                    // A signal that was not a stop ends a run too: then the CPU runs on.
                    if self.remote.immediate_exit().swap(0, Ordering::Acquire) != 0 {
                        return Ok(Exit::Stopped);
                    }
                }
                Err(e) => return Err(refused(format_args!("running: {e}"))),
            }
        }
    }

    fn get(&self, register: Register) -> Result<u64> {
        let place = place(register);
        let mut bytes = [0u8; size_of::<u64>()];
        let size = match place {
            Place::Core(_, KVM_REG_SIZE_U32) => size_of::<u32>(),
            _ => size_of::<u64>(),
        };
        self.vcpu
            .get_one_reg(id(&place), &mut bytes[..size])
            .map_err(|e| refused(format_args!("{register:?}: {e}")))?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn set(&mut self, register: Register, value: u64) -> Result<()> {
        let place = place(register);
        let bytes = value.to_le_bytes();
        let size = match place {
            Place::Core(_, KVM_REG_SIZE_U32) => size_of::<u32>(),
            _ => size_of::<u64>(),
        };
        self.vcpu
            .set_one_reg(id(&place), &bytes[..size])
            .map(drop)
            .map_err(|e| refused(format_args!("{register:?}: {e}")))
    }

    fn vector(&self, n: u8) -> Result<u128> {
        let mut bytes = [0u8; size_of::<u128>()];
        self.vcpu
            .get_one_reg(vector_id(n), &mut bytes)
            .map_err(|e| refused(format_args!("v{n}: {e}")))?;
        Ok(u128::from_le_bytes(bytes))
    }

    fn set_vector(&mut self, n: u8, value: u128) -> Result<()> {
        self.vcpu
            .set_one_reg(vector_id(n), &value.to_le_bytes())
            .map(drop)
            .map_err(|e| refused(format_args!("v{n}: {e}")))
    }

    fn stopper(&self) -> KvmStopper {
        KvmStopper {
            remote: Arc::clone(&self.remote),
        }
    }

    /// KVM has no register that reads the offset back, so the machine keeps the one it set.
    fn counter_offset(&self) -> Result<u64> {
        Ok(self.counter_offset)
    }
}

impl crate::Stopper for KvmStopper {
    /// Signals no thread when the CPU has never run or is gone: its next run, if any, reads `immediate_exit`.
    fn stop(&self) -> Result<()> {
        self.remote.immediate_exit().store(u8::from(true), Ordering::SeqCst);
        fence(Ordering::SeqCst);
        let thread = self.remote.thread.load(Ordering::SeqCst);
        if thread == NOT_RUN_BY_A_THREAD {
            return Ok(());
        }
        // SAFETY: a signal whose handler does nothing, to a thread of this process.
        #[expect(
            unsafe_code,
            reason = "UNEXPLAINED: sends the stop signal, whose handler does nothing, to the thread id of this process that last ran the CPU; that thread may have ended and its id gone to another thread of this process, where the signal can make a blocking call such as poll fail with EINTR even under SA_RESTART, which nothing shows that thread's code handles; Kick replaces it"
        )]
        let sent = unsafe { libc::tgkill(libc::getpid(), thread, stop_signal()) };
        if sent != 0 {
            return Err(Error::Host(std::io::Error::last_os_error()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use wcontainers_executor_hardware_virtualization_inside::layout::MEMORY_BASE;

    use super::KernelVirtualMachine;
    use crate::{Hypervisor, Memory, VirtualMachine};

    /// Bytes of each reservation: whole pages whether the host's pages are 4 KiB or 64 KiB.
    const RESERVATION: u64 = 1 << 16;

    /// Reservations made at most before two are neighbors.
    const TRIES: usize = 64;

    /// A range given back that runs past guest memory is refused, and the host memory after it keeps its bytes.
    #[test]
    fn giving_back_past_guest_memory_is_refused() {
        let machine = KernelVirtualMachine::machine(1).expect("a machine");
        // The host maps a new reservation next to an earlier one, unless other tests' mappings come between.
        let after = |memory: &Memory| memory.host(MEMORY_BASE).wrapping_add(RESERVATION as usize);
        let mut reserved: Vec<Memory> = Vec::new();
        let (lower, mut upper) = loop {
            assert!(reserved.len() < TRIES, "two reservations are neighbors");
            let memory = Memory::reserve(RESERVATION).expect("reserving memory");
            if let Some(i) = (reserved.iter()).position(|m| after(&memory) == m.host(MEMORY_BASE)) {
                break (memory, reserved.swap_remove(i));
            }
            if let Some(i) = (reserved.iter()).position(|m| after(m) == memory.host(MEMORY_BASE)) {
                break (reserved.swap_remove(i), memory);
            }
            reserved.push(memory);
        };
        upper.bytes_mut(MEMORY_BASE, 1)[0] = 0xa5;
        let given = machine.give_back(&lower, MEMORY_BASE, 2 * RESERVATION);
        assert_eq!(
            upper.bytes(MEMORY_BASE, 1)[0],
            0xa5,
            "the neighbor's byte survives"
        );
        assert!(given.is_err(), "the range is refused");
    }
}
