//! [`Hypervisor`] on Hypervisor.framework (macOS, ARM64).

use std::ptr::NonNull;

use aarch64_cpu::registers::ESR_EL2;
use applevisor::prelude::*;
use applevisor_sys::{
    HV_MEMORY_EXEC, HV_MEMORY_READ, HV_MEMORY_WRITE, hv_vcpu_t, hv_vcpus_exit, hv_vm_map,
    hv_vm_unmap,
};
use wcontainers_inside::GENERAL_REGISTERS;
use wcontainers_inside::layout::{DOORBELL, GIC_DISTRIBUTOR, GIC_REDISTRIBUTORS};

/// Bytes of an ARM64 instruction: the store that rang the doorbell is passed over.
const INSTRUCTION_BYTES: u64 = size_of::<u32>() as u64;

use crate::{Error, Exit, Hypervisor, Interrupts, Register, Result};

/// Hypervisor.framework.
pub struct HypervisorFramework;

/// The process's machine, with its interrupt controller.
pub struct HvfMachine {
    vm: VirtualMachineInstance<GicEnabled>,
}

/// A virtual CPU, bound to the thread that made it.
pub struct HvfCpu {
    vcpu: Vcpu,
}

/// Stops a virtual CPU by its identifier.
#[derive(Clone, Copy)]
pub struct HvfStopper {
    id: hv_vcpu_t,
}

fn check(result: std::result::Result<(), HypervisorError>) -> Result<()> {
    result.map_err(|e| Error::Hypervisor(format!("{e:?}")))
}

fn code(code: i32) -> Result<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(Error::Hypervisor(format!("{code:#x}")))
    }
}

impl Hypervisor for HypervisorFramework {
    type Machine = HvfMachine;

    fn machine(el2: bool) -> Result<HvfMachine> {
        let mut config = VirtualMachineConfig::new();
        check(config.set_el2_enabled(el2))?;
        let mut gic = GicConfig::new();
        check(gic.set_distributor_base(GIC_DISTRIBUTOR))?;
        check(gic.set_redistributor_base(GIC_REDISTRIBUTORS))?;
        let vm = VirtualMachine::with_gic(config, gic)
            .map_err(|e| Error::Hypervisor(format!("{e:?}")))?;
        Ok(HvfMachine { vm })
    }
}

impl crate::Machine for HvfMachine {
    type Cpu = HvfCpu;

    fn map(&self, host: NonNull<u8>, at: u64, length: u64) -> Result<()> {
        let flags = HV_MEMORY_READ | HV_MEMORY_WRITE | HV_MEMORY_EXEC;
        // SAFETY: the host range is the caller's memory and outlives the mapping.
        code(unsafe { hv_vm_map(host.as_ptr().cast(), at, length as usize, flags) })
    }

    fn unmap(&self, at: u64, length: u64) -> Result<()> {
        // SAFETY: unmapping guest-physical memory the caller mapped.
        code(unsafe { hv_vm_unmap(at, length as usize) })
    }

    fn raise(&self, interrupt: u32, level: bool) -> Result<()> {
        check(self.vm.gic_set_spi(interrupt, level))
    }

    fn cpu(&self) -> Result<HvfCpu> {
        let vcpu = self
            .vm
            .vcpu_create()
            .map_err(|e| Error::Hypervisor(format!("{e:?}")))?;
        Ok(HvfCpu { vcpu })
    }

    fn interrupts(&self) -> Result<Interrupts> {
        let id =
            |which| GicConfig::get_intid(which).map_err(|e| Error::Hypervisor(format!("{e:?}")));
        let (first_raised, _) = GicConfig::get_spi_interrupt_range()
            .map_err(|e| Error::Hypervisor(format!("{e:?}")))?;
        Ok(Interrupts {
            timer: id(GicIntId::EL1_VIRTUAL_TIMER)?,
            counter: id(GicIntId::PERFORMANCE_MONITOR)?,
            first_raised,
        })
    }

    fn max_cpus(&self) -> Result<u64> {
        Vcpu::get_max_count()
            .map(u64::from)
            .map_err(|e| Error::Hypervisor(format!("{e:?}")))
    }
}

/// The general registers in order.
const GENERAL: [Reg; GENERAL_REGISTERS] = [
    Reg::X0,
    Reg::X1,
    Reg::X2,
    Reg::X3,
    Reg::X4,
    Reg::X5,
    Reg::X6,
    Reg::X7,
    Reg::X8,
    Reg::X9,
    Reg::X10,
    Reg::X11,
    Reg::X12,
    Reg::X13,
    Reg::X14,
    Reg::X15,
    Reg::X16,
    Reg::X17,
    Reg::X18,
    Reg::X19,
    Reg::X20,
    Reg::X21,
    Reg::X22,
    Reg::X23,
    Reg::X24,
    Reg::X25,
    Reg::X26,
    Reg::X27,
    Reg::X28,
    Reg::X29,
    Reg::X30,
];

enum Which {
    General(Reg),
    System(SysReg),
}

fn which(register: Register) -> Which {
    match register {
        Register::Pc => Which::General(Reg::PC),
        Register::Pstate => Which::General(Reg::CPSR),
        Register::X(n) => Which::General(GENERAL[usize::from(n)]),
        Register::StackEl1 => Which::System(SysReg::SP_EL1),
        Register::Mair => Which::System(SysReg::MAIR_EL1),
        Register::Tcr => Which::System(SysReg::TCR_EL1),
        Register::Sctlr => Which::System(SysReg::SCTLR_EL1),
        Register::Cpacr => Which::System(SysReg::CPACR_EL1),
        Register::Ttbr1 => Which::System(SysReg::TTBR1_EL1),
        Register::Affinity => Which::System(SysReg::MPIDR_EL1),
        Register::MemoryModelFeatures => Which::System(SysReg::ID_AA64MMFR0_EL1),
        Register::DebugFeatures => Which::System(SysReg::ID_AA64DFR0_EL1),
        Register::HypervisorControl => Which::System(SysReg::HCR_EL2),
    }
}

impl crate::VirtualCpu for HvfCpu {
    type Stopper = HvfStopper;

    fn run(&mut self) -> Result<Exit> {
        check(self.vcpu.run())?;
        let exit = self.vcpu.get_exit_info();
        match exit.reason {
            ExitReason::CANCELED => Ok(Exit::Stopped),
            ExitReason::EXCEPTION
                if ESR_EL2::EC.read(exit.exception.syndrome)
                    == ESR_EL2::EC::Value::HVC64 as u64 =>
            {
                Ok(Exit::Doorbell)
            }
            ExitReason::EXCEPTION
                if ESR_EL2::EC.read(exit.exception.syndrome)
                    == ESR_EL2::EC::Value::DataAbortLowerEL as u64
                    && exit.exception.physical_address == DOORBELL =>
            {
                let pc = self.get(Register::Pc)?;
                self.set(Register::Pc, pc + INSTRUCTION_BYTES)?;
                Ok(Exit::Doorbell)
            }
            ExitReason::EXCEPTION => Ok(Exit::Exception {
                syndrome: exit.exception.syndrome,
                address: exit.exception.virtual_address,
                physical: exit.exception.physical_address,
            }),
            other => Err(Error::Hypervisor(format!("unexpected exit {other:?}"))),
        }
    }

    fn get(&self, register: Register) -> Result<u64> {
        let value = match which(register) {
            Which::General(r) => self.vcpu.get_reg(r),
            Which::System(r) => self.vcpu.get_sys_reg(r),
        };
        value.map_err(|e| Error::Hypervisor(format!("{register:?}: {e:?}")))
    }

    fn set(&mut self, register: Register, value: u64) -> Result<()> {
        let done = match which(register) {
            Which::General(r) => self.vcpu.set_reg(r, value),
            Which::System(r) => self.vcpu.set_sys_reg(r, value),
        };
        done.map_err(|e| Error::Hypervisor(format!("{register:?}: {e:?}")))
    }

    fn stopper(&self) -> HvfStopper {
        HvfStopper { id: self.vcpu.id() }
    }

    fn counter_offset(&self) -> Result<u64> {
        self.vcpu
            .get_vtimer_offset()
            .map_err(|e| Error::Hypervisor(format!("{e:?}")))
    }
}

impl crate::Stopper for HvfStopper {
    fn stop(&self) -> Result<()> {
        // SAFETY: one valid identifier; stopping a CPU that has gone is refused by the framework, not unsound.
        code(unsafe { hv_vcpus_exit(&self.id, 1) })
    }
}
