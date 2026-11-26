//! Conversions between ARM64 processor states and register blocks, and single-step runs on a CPU side.
//!
//! An execution state's memory and processor become a memory and thread there, run for exactly one instruction.
//!
//! What an exit leaves out, such as whether a faulting page was mapped, is read from the state before.

use std::collections::BTreeMap;

use aarch64_cpu::registers::ESR_EL1;
use wcontainers_abi::{Access, Cpu, Exit, PAGE_SIZE, Permissions, RegisterBlock, ThreadId};
use wcontainers_processor_architecture::MemoryDelta;
use wcontainers_processor_architecture_arm64::encoding::IMM16;
use wcontainers_processor_architecture_arm64::memory::report_changed_bytes;
use wcontainers_processor_architecture_arm64::syndrome::{
    FaultStatus, WRITE_NOT_READ, encode_syndrome,
};
use wcontainers_processor_architecture_arm64::{
    Arm64, ConditionFlags, DataAccess, Exception, INSTRUCTION_BYTES, InstructionFetch,
    PagePermission, Processor,
};

/// Runs the state's instruction on a CPU side, where `run_one` must stop a thread after exactly one instruction.
pub fn execute_on_cpu<Side: Cpu>(
    cpu: &mut Side,
    processor: &mut Processor,
    memory: &[MemoryDelta<'_, Arm64>],
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, Arm64>),
    run_one: impl FnOnce(&mut Side, ThreadId) -> Exit,
) {
    let space = cpu
        .memory_new()
        .expect("the CPU side has room for one more memory");
    let mut pages: BTreeMap<u64, PagePermission> = BTreeMap::new();
    for delta in memory {
        match delta {
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => {
                assert!(
                    addresses.start.is_multiple_of(PAGE_SIZE)
                        && addresses.end.is_multiple_of(PAGE_SIZE),
                    "a permission is given to whole pages"
                );
                cpu.set_permissions(
                    space,
                    addresses.start,
                    addresses.end - addresses.start,
                    to_permissions(*permission),
                )
                .expect("the pages are inside the CPU side's address space");
                for page in addresses.clone().step_by(PAGE_SIZE as usize) {
                    pages.insert(page, *permission);
                }
            }
            MemoryDelta::ValueAssignment { start, bytes } => cpu
                .write(space, *start, bytes)
                .expect("the bytes are inside the CPU side's address space"),
        }
    }
    let pc = processor.program_counter;
    let mut word = [u8::MIN; INSTRUCTION_BYTES as usize];
    cpu.read(space, pc, &mut word)
        .expect("the program counter is inside the CPU side's address space");
    let word = u32::from_le_bytes(word);
    let writable: Vec<u64> = pages
        .iter()
        .filter(|(_, permission)| {
            matches!(
                permission,
                PagePermission::Mapped {
                    data_access: DataAccess::ReadWrite,
                    ..
                }
            )
        })
        .map(|(page, _)| *page)
        .collect();
    let read_page = |cpu: &mut Side, page: u64| {
        let mut bytes = vec![u8::MIN; PAGE_SIZE as usize];
        cpu.read(space, page, &mut bytes)
            .expect("a page the state gave a permission");
        bytes
    };
    let before: Vec<Vec<u8>> = writable.iter().map(|page| read_page(cpu, *page)).collect();
    let thread = cpu
        .thread_new(space)
        .expect("the CPU side has room for one more thread");
    let mut registers = to_register_block(processor);
    cpu.set_registers(thread, &registers);
    let exit = run_one(cpu, thread);
    cpu.registers(thread, &mut registers);
    // The abi leaves the program counter after a system call; an exception leaves it on the instruction.
    if let Exit::Syscall(_) = exit {
        registers.pc = pc;
    }
    let exception = to_exception(exit, word, pc, |address| {
        pages
            .get(&(address - address % PAGE_SIZE))
            .copied()
            .unwrap_or(PagePermission::Unmapped)
    });
    *processor = to_processor(&registers, exception);
    for (page, before) in writable.iter().zip(before) {
        let after = read_page(cpu, *page);
        report_changed_bytes(*page, &before, &after, &mut on_memory_mutated);
    }
    cpu.thread_drop(thread);
    cpu.memory_drop(space);
}

fn to_permissions(permission: PagePermission) -> Permissions {
    match permission {
        PagePermission::Unmapped => Permissions::empty(),
        PagePermission::Mapped {
            data_access,
            instruction_fetch,
        } => {
            let data = match data_access {
                DataAccess::Forbidden => Permissions::empty(),
                DataAccess::ReadOnly => Permissions::READ,
                DataAccess::ReadWrite => Permissions::READ | Permissions::WRITE,
            };
            let fetch = match instruction_fetch {
                InstructionFetch::Forbidden => Permissions::empty(),
                InstructionFetch::Allowed => Permissions::EXECUTE,
            };
            data | fetch
        }
    }
}

pub(crate) fn to_register_block(processor: &Processor) -> RegisterBlock {
    RegisterBlock {
        x: processor.general_purpose_registers,
        sp: processor.stack_pointer,
        pc: processor.program_counter,
        pstate: u64::from(processor.condition_flags.bits()),
        v: processor
            .vector_registers
            .map(|value| [value as u64, (value >> u64::BITS) as u64]),
        fpsr: u32::try_from(processor.floating_point_status)
            .expect("FPSR's upper half is reserved as zero"),
        fpcr: u32::try_from(processor.floating_point_control)
            .expect("FPCR's upper half is reserved as zero"),
        tpidr_el0: processor.software_thread_id,
    }
}

pub(crate) fn to_processor(registers: &RegisterBlock, exception: Option<Exception>) -> Processor {
    Processor {
        general_purpose_registers: registers.x,
        stack_pointer: registers.sp,
        program_counter: registers.pc,
        condition_flags: ConditionFlags::from_bits_truncate(registers.pstate as u32),
        vector_registers: registers
            .v
            .map(|[low, high]| u128::from(low) | u128::from(high) << u64::BITS),
        floating_point_control: u64::from(registers.fpcr),
        floating_point_status: u64::from(registers.fpsr),
        software_thread_id: registers.tpidr_el0,
        exception,
    }
}

/// Returns the exception an exit is, or none when the instruction completed.
pub(crate) fn to_exception(
    exit: Exit,
    word: u32,
    pc: u64,
    permission_at: impl Fn(u64) -> PagePermission,
) -> Option<Exception> {
    let fault_status = |address| match permission_at(address) {
        PagePermission::Unmapped => FaultStatus::TranslationFaultLevel3 as u64,
        PagePermission::Mapped { .. } => FaultStatus::PermissionFaultLevel3 as u64,
    };
    let (class, specific, fault_address) = match exit {
        Exit::Stop => return None,
        Exit::Syscall(_) => (ESR_EL1::EC::Value::SVC64, u64::from(IMM16.read(word)), None),
        Exit::MemoryFault {
            address,
            access: Access::Execute,
        } => (
            ESR_EL1::EC::Value::InstrAbortLowerEL,
            fault_status(address),
            Some(address),
        ),
        Exit::MemoryFault {
            address,
            access: Access::Read,
        } => (
            ESR_EL1::EC::Value::DataAbortLowerEL,
            fault_status(address),
            Some(address),
        ),
        Exit::MemoryFault {
            address,
            access: Access::Write,
        } => (
            ESR_EL1::EC::Value::DataAbortLowerEL,
            WRITE_NOT_READ | fault_status(address),
            Some(address),
        ),
        Exit::AlignmentFault { address } if address == pc => {
            (ESR_EL1::EC::Value::PCAlignmentFault, u64::MIN, Some(pc))
        }
        Exit::AlignmentFault { .. } => (ESR_EL1::EC::Value::SPAlignmentFault, u64::MIN, None),
        Exit::IllegalInstruction => (ESR_EL1::EC::Value::Unknown, u64::MIN, None),
        Exit::Breakpoint { comment } => (ESR_EL1::EC::Value::Brk64, u64::from(comment), None),
        Exit::MemoryFaultWithoutRegisters => {
            panic!("the instruction at {pc:#x} reached beyond the CPU side's address space")
        }
    };
    Some(Exception {
        syndrome: encode_syndrome(class, specific),
        fault_address,
    })
}
