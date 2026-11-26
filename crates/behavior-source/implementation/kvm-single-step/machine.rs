//! The one virtual machine with one virtual CPU, made and run through the KVM API crate, with SVE where the host has it.

use std::mem::offset_of;

use kvm_bindings::{
    KVM_ARM_VCPU_SVE, KVM_ARM_VM_SMCCC_CTRL, KVM_ARM_VM_SMCCC_FILTER, KVM_CAP_ARM_SVE,
    KVM_DEV_ARM_VGIC_CTRL_INIT, KVM_DEV_ARM_VGIC_GRP_ADDR, KVM_DEV_ARM_VGIC_GRP_CTRL,
    KVM_EXIT_HYPERCALL, KVM_REG_ARM_CORE, KVM_REG_ARM64, KVM_REG_ARM64_SYSREG,
    KVM_REG_ARM64_SYSREG_CRM_SHIFT, KVM_REG_ARM64_SYSREG_CRN_SHIFT, KVM_REG_ARM64_SYSREG_OP0_SHIFT,
    KVM_REG_ARM64_SYSREG_OP1_SHIFT, KVM_REG_ARM64_SYSREG_OP2_SHIFT, KVM_REG_SIZE_U64,
    KVM_VGIC_V3_ADDR_TYPE_DIST, KVM_VGIC_V3_ADDR_TYPE_REDIST,
    kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V3, kvm_regs, kvm_run,
    kvm_run__bindgen_ty_1__bindgen_ty_8, kvm_smccc_filter,
    kvm_smccc_filter_action_KVM_SMCCC_FILTER_FWD_TO_USER, user_pt_regs,
};
use wcontainers_kvm::{Errno, Kvm};
use wcontainers_kvm_direct::DirectKvm;
use wcontainers_volatile_bytes::VolatileBytes;

use crate::guest_layout::{DOORBELL_FUNCTION, GIC_DISTRIBUTOR, GIC_REDISTRIBUTORS};

/// Why the machine could not be made or run, in KVM's words.
#[derive(Debug)]
pub struct MachineError(pub String);

/// The guest memory KVM gives the machine.
pub(crate) type GuestMemory = <DirectKvm as Kvm>::UserMemory;

/// The machine, its interrupt controller and its virtual CPU, which drop before it.
pub(crate) struct Machine {
    cpu: <DirectKvm as Kvm>::Vcpu,
    _interrupt_controller: <DirectKvm as Kvm>::Device,
    _machine: <DirectKvm as Kvm>::Vm,
    _system: <DirectKvm as Kvm>::System,
}

/// A register the single step machine reads or writes on its virtual CPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Register {
    Pc,
    Pstate,
    StackEl1,
    Mair,
    Tcr,
    Sctlr,
    Cpacr,
    Ttbr1,
    /// ID_AA64MMFR0_EL1.
    MemoryModelFeatures,
    /// ID_AA64MMFR2_EL1.
    MemoryModelFeatures2,
    /// ID_AA64PFR0_EL1.
    ProcessorFeatures,
    /// ID_AA64PFR1_EL1.
    ProcessorFeatures1,
    /// VBAR_EL1: the handler's exception vectors.
    Vectors,
    /// MDSCR_EL1: the debug controls, software step among them.
    DebugControl,
    /// TPIDR_EL1: the handler's thread register, which names the slot it runs.
    HandlerThread,
    /// ZCR_EL1: the SVE vector length of EL1 and EL0.
    SveControl,
}

/// The SMCCC filter that forwards the doorbell's function, alone, to this program.
const DOORBELL_FILTER: kvm_smccc_filter = kvm_smccc_filter {
    base: DOORBELL_FUNCTION,
    nr_functions: 1,
    action: kvm_smccc_filter_action_KVM_SMCCC_FILTER_FWD_TO_USER as u8,
    pad: [0; 15],
};

/// Returns the filter's bytes as KVM reads them.
fn filter_bytes(filter: &kvm_smccc_filter) -> Vec<u8> {
    let mut bytes = vec![u8::MIN; size_of::<kvm_smccc_filter>()];
    let fields = [filter.base.to_ne_bytes(), filter.nr_functions.to_ne_bytes()].concat();
    bytes[..fields.len()].copy_from_slice(&fields);
    bytes[offset_of!(kvm_smccc_filter, action)] = filter.action;
    bytes
}

/// Makes the machine with its memory at the guest-physical address, and its one virtual CPU.
///
/// KVM refuses a virtual CPU once the interrupt controller is set up, so the CPU is made first.
pub(crate) fn create_machine(
    memory_start: u64,
    memory_bytes: usize,
) -> Result<(Machine, GuestMemory), MachineError> {
    let refused = |what: &str, error: Errno| MachineError(format!("{what}: {error}"));
    let kvm = DirectKvm;
    let system = kvm.open().map_err(|error| refused("opening KVM", error))?;
    let machine = kvm
        .create_vm(&system, u64::MIN)
        .map_err(|error| refused("making the machine", error))?;
    kvm.vm_set_device_attr(
        &machine,
        KVM_ARM_VM_SMCCC_CTRL,
        KVM_ARM_VM_SMCCC_FILTER.into(),
        &filter_bytes(&DOORBELL_FILTER),
    )
    .map_err(|error| refused("forwarding the doorbell", error))?;
    let memory = kvm
        .create_user_memory(memory_bytes)
        .map_err(|error| refused("mapping memory", error))?;
    kvm.set_user_memory_region(&machine, u32::MIN, u32::MIN, memory_start, Some(&memory))
        .map_err(|error| refused("mapping memory", error))?;
    let mut init = kvm
        .arm_preferred_target(&machine)
        .map_err(|error| refused("the preferred CPU", error))?;
    let scalable_vectors = kvm
        .check_extension(&system, KVM_CAP_ARM_SVE)
        .is_ok_and(|answer| answer > 0);
    if scalable_vectors {
        init.features[0] |= 1 << KVM_ARM_VCPU_SVE;
    }
    let cpu = kvm
        .create_vcpu(&machine, u64::MIN)
        .map_err(|error| refused("making the virtual CPU", error))?;
    kvm.arm_vcpu_init(&cpu, &init)
        .map_err(|error| refused("starting the virtual CPU", error))?;
    if scalable_vectors {
        kvm.arm_vcpu_finalize(&cpu, KVM_ARM_VCPU_SVE as i32)
            .map_err(|error| refused("fixing the virtual CPU's SVE", error))?;
    }
    let interrupt_controller = kvm
        .create_device(&machine, kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V3)
        .map_err(|error| refused("making the interrupt controller", error))?;
    for (kind, address) in [
        (KVM_VGIC_V3_ADDR_TYPE_DIST, GIC_DISTRIBUTOR),
        (KVM_VGIC_V3_ADDR_TYPE_REDIST, GIC_REDISTRIBUTORS),
    ] {
        kvm.set_device_attr(
            &interrupt_controller,
            KVM_DEV_ARM_VGIC_GRP_ADDR,
            kind.into(),
            &address.to_ne_bytes(),
        )
        .map_err(|error| refused("placing the interrupt controller", error))?;
    }
    kvm.set_device_attr(
        &interrupt_controller,
        KVM_DEV_ARM_VGIC_GRP_CTRL,
        KVM_DEV_ARM_VGIC_CTRL_INIT.into(),
        &[],
    )
    .map_err(|error| refused("setting up the interrupt controller", error))?;
    let made = Machine {
        cpu,
        _interrupt_controller: interrupt_controller,
        _machine: machine,
        _system: system,
    };
    Ok((made, memory))
}

/// Returns the little-endian word of the length at the offset in the bytes.
fn word(bytes: &[u8], at: usize, length: usize) -> u64 {
    bytes[at..at + length]
        .iter()
        .rev()
        .fold(u64::MIN, |all, &byte| all << u8::BITS | u64::from(byte))
}

impl Machine {
    /// Runs the virtual CPU until the guest rings the doorbell.
    pub(crate) fn run(&mut self) -> Result<(), MachineError> {
        let kvm = DirectKvm;
        loop {
            match kvm.run(&self.cpu) {
                Ok(()) => break,
                // A signal ends a run too: then the CPU runs on.
                Err(Errno::INTR) => {}
                Err(error) => return Err(MachineError(format!("running: {error}"))),
            }
        }
        let mut run = vec![u8::MIN; size_of::<kvm_run>()];
        kvm.vcpu_run_page(&self.cpu).read(&mut run);
        let reason = word(&run, offset_of!(kvm_run, exit_reason), size_of::<u32>());
        let call = word(
            &run,
            offset_of!(kvm_run, __bindgen_anon_1)
                + offset_of!(kvm_run__bindgen_ty_1__bindgen_ty_8, nr),
            size_of::<u64>(),
        );
        if reason == u64::from(KVM_EXIT_HYPERCALL) && call == u64::from(DOORBELL_FUNCTION) {
            Ok(())
        } else {
            Err(MachineError(format!(
                "unexpected exit {reason}, function {call:#x}"
            )))
        }
    }

    pub(crate) fn get(&self, register: Register) -> Result<u64, MachineError> {
        let mut bytes = [u8::MIN; size_of::<u64>()];
        DirectKvm
            .get_one_reg(&self.cpu, to_id(register), &mut bytes)
            .map_err(|error| MachineError(format!("{register:?}: {error}")))?;
        Ok(u64::from_le_bytes(bytes))
    }

    pub(crate) fn set(&mut self, register: Register, value: u64) -> Result<(), MachineError> {
        DirectKvm
            .set_one_reg(&self.cpu, to_id(register), &value.to_le_bytes())
            .map_err(|error| MachineError(format!("{register:?}: {error}")))
    }
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

// The encodings of the system registers, from https://developer.arm.com/documentation/ddi0601/latest/Index-by-Encoding.
const SCTLR_EL1: SystemRegister = system(3, 0, 1, 0, 0);
const CPACR_EL1: SystemRegister = system(3, 0, 1, 0, 2);
const TTBR1_EL1: SystemRegister = system(3, 0, 2, 0, 1);
const TCR_EL1: SystemRegister = system(3, 0, 2, 0, 2);
const MAIR_EL1: SystemRegister = system(3, 0, 10, 2, 0);
const ID_AA64MMFR0_EL1: SystemRegister = system(3, 0, 0, 7, 0);
const ID_AA64MMFR2_EL1: SystemRegister = system(3, 0, 0, 7, 2);
const ID_AA64PFR0_EL1: SystemRegister = system(3, 0, 0, 4, 0);
const ID_AA64PFR1_EL1: SystemRegister = system(3, 0, 0, 4, 1);
const VBAR_EL1: SystemRegister = system(3, 0, 12, 0, 0);
const MDSCR_EL1: SystemRegister = system(2, 0, 0, 2, 2);
const TPIDR_EL1: SystemRegister = system(3, 0, 13, 0, 4);
const ZCR_EL1: SystemRegister = system(3, 0, 1, 2, 0);

/// Returns KVM's number of the register: a core register by its offset in `kvm_regs`, or a system register by its
/// encoding.
fn to_id(register: Register) -> u64 {
    let core = |offset: usize| {
        KVM_REG_ARM64
            | KVM_REG_SIZE_U64
            | u64::from(KVM_REG_ARM_CORE)
            | (offset / size_of::<u32>()) as u64
    };
    let regs = offset_of!(kvm_regs, regs);
    let system_register = match register {
        Register::Pc => return core(regs + offset_of!(user_pt_regs, pc)),
        Register::Pstate => return core(regs + offset_of!(user_pt_regs, pstate)),
        Register::StackEl1 => return core(offset_of!(kvm_regs, sp_el1)),
        Register::Mair => MAIR_EL1,
        Register::Tcr => TCR_EL1,
        Register::Sctlr => SCTLR_EL1,
        Register::Cpacr => CPACR_EL1,
        Register::Ttbr1 => TTBR1_EL1,
        Register::MemoryModelFeatures => ID_AA64MMFR0_EL1,
        Register::MemoryModelFeatures2 => ID_AA64MMFR2_EL1,
        Register::ProcessorFeatures => ID_AA64PFR0_EL1,
        Register::ProcessorFeatures1 => ID_AA64PFR1_EL1,
        Register::Vectors => VBAR_EL1,
        Register::DebugControl => MDSCR_EL1,
        Register::HandlerThread => TPIDR_EL1,
        Register::SveControl => ZCR_EL1,
    };
    let SystemRegister {
        op0,
        op1,
        crn,
        crm,
        op2,
    } = system_register;
    KVM_REG_ARM64
        | KVM_REG_SIZE_U64
        | u64::from(KVM_REG_ARM64_SYSREG)
        | op0 << KVM_REG_ARM64_SYSREG_OP0_SHIFT
        | op1 << KVM_REG_ARM64_SYSREG_OP1_SHIFT
        | crn << KVM_REG_ARM64_SYSREG_CRN_SHIFT
        | crm << KVM_REG_ARM64_SYSREG_CRM_SHIFT
        | op2 << KVM_REG_ARM64_SYSREG_OP2_SHIFT
}
