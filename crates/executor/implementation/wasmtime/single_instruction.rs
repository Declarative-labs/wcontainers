//! One ARM64 instruction of an execution state, run on a CPU side of the abi.
//!
//! The state's memory becomes a new memory of the CPU side and its processor a new thread; the CPU side runs
//! the thread for one instruction, and its exit becomes the exception the architecture names. The abi says
//! less than the architecture (a memory fault does not say whether the page was mapped), so what it leaves
//! out is read from the state before, as the page tables an EL1 would have made from it say.

use std::borrow::Cow;
use std::collections::BTreeMap;

use aarch64_cpu::registers::ESR_EL1;
use wcontainers_abi::{Access, Cpu, Exit, PAGE_SIZE, Permissions, RegisterBlock, ThreadId};
use wcontainers_precise_instruction_set::PreciseInstructionSet;
use wcontainers_processor_architecture::{MemoryDelta, ProcessorArchitecture};
use wcontainers_processor_architecture_arm64::{
    Arm64, ConditionFlags, DataAccess, Exception, InstructionFetch, PagePermission,
    PreciseInstruction, Processor,
};

const INSTRUCTION_BYTES: u64 = size_of::<u32>() as u64;
/// The [WnR](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=WnR%2C%20bit%20%5B6%5D) bit of a data abort's syndrome.
const WRITE_NOT_READ: u64 = 1 << 6;
/// The [DFSC](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=DFSC%2C%20bits%20%5B5%3A0%5D) and IFSC of a translation fault at level 3, where every page of an executor's tables is.
const TRANSLATION_FAULT_LEVEL_3: u64 = 0b00_0111;
/// The DFSC and IFSC of a permission fault at level 3.
const PERMISSION_FAULT_LEVEL_3: u64 = 0b00_1111;

/// A precise instruction an executor was asked to honor and cannot, because its back end runs another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub requested: PreciseInstruction,
    pub executed: PreciseInstruction,
}

/// Fails with every instruction `requested` gives another behavior than the back end's set, `executed`.
pub fn check_honored(
    requested: &impl PreciseInstructionSet<Arm64>,
    executed: &impl PreciseInstructionSet<Arm64>,
) -> Result<(), Vec<Refusal>> {
    let refusals: Vec<Refusal> = Arm64::list_instructions()
        .map(|instruction| Refusal {
            requested: requested.to_precise(instruction),
            executed: executed.to_precise(instruction),
        })
        .filter(|refusal| refusal.requested != refusal.executed)
        .collect();
    if refusals.is_empty() {
        Ok(())
    } else {
        Err(refusals)
    }
}

/// What becomes of the bytes an instruction wrote before a write of it faulted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultingWrite {
    /// They stay as the CPU side left them.
    Kept,
    /// They are put back as they were, so the instruction writes nothing.
    Undone,
}

/// Runs the instruction at the program counter on the CPU side, with `run_one`, which runs exactly one
/// instruction of the thread it is given and returns how it ended.
pub fn execute_on_cpu<C: Cpu>(
    cpu: &mut C,
    faulting_write: FaultingWrite,
    processor: &mut Processor,
    memory: &[MemoryDelta<'_, Arm64>],
    mut on_memory_mutated: impl FnMut(MemoryDelta<'_, Arm64>),
    run_one: impl FnOnce(&mut C, ThreadId) -> Exit,
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
    let mut word = [0; INSTRUCTION_BYTES as usize];
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
    let read_page = |cpu: &mut C, page: u64| {
        let mut bytes = vec![0; PAGE_SIZE as usize];
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
    if let Exit::MemoryFault {
        access: Access::Write,
        ..
    } = exit
        && faulting_write == FaultingWrite::Undone
    {
        for (page, bytes) in writable.iter().zip(&before) {
            cpu.write(space, *page, bytes)
                .expect("a page the state gave a permission");
        }
    }
    cpu.registers(thread, &mut registers);
    let exception = to_exception(exit, word, pc, &mut registers, |address| {
        pages
            .get(&(address - address % PAGE_SIZE))
            .copied()
            .unwrap_or(PagePermission::Unmapped)
    });
    *processor = to_processor(&registers, exception);
    for (page, before) in writable.iter().zip(before) {
        let after = read_page(cpu, *page);
        let mut offset = 0;
        while offset < after.len() {
            if after[offset] == before[offset] {
                offset += 1;
                continue;
            }
            let start = offset;
            while offset < after.len() && after[offset] != before[offset] {
                offset += 1;
            }
            on_memory_mutated(MemoryDelta::ValueAssignment {
                start: page + start as u64,
                bytes: Cow::Borrowed(&after[start..offset]),
            });
        }
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

fn to_register_block(processor: &Processor) -> RegisterBlock {
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

fn to_processor(registers: &RegisterBlock, exception: Option<Exception>) -> Processor {
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

/// The exception an exit is, with the program counter put back on the instruction that raised it.
fn to_exception(
    exit: Exit,
    word: u32,
    pc: u64,
    registers: &mut RegisterBlock,
    permission_at: impl Fn(u64) -> PagePermission,
) -> Option<Exception> {
    let fault_status = |address| match permission_at(address) {
        PagePermission::Unmapped => TRANSLATION_FAULT_LEVEL_3,
        PagePermission::Mapped { .. } => PERMISSION_FAULT_LEVEL_3,
    };
    let (class, specific, fault_address) = match exit {
        Exit::Stop => return None,
        // The abi leaves the program counter after a system call.
        Exit::Syscall(_) => {
            registers.pc = pc;
            let comment = u64::from(word >> 5 & u32::from(u16::MAX));
            (ESR_EL1::EC::Value::SVC64, comment, None)
        }
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
            (ESR_EL1::EC::Value::PCAlignmentFault, 0, Some(pc))
        }
        Exit::AlignmentFault { .. } => (ESR_EL1::EC::Value::SPAlignmentFault, 0, None),
        Exit::IllegalInstruction => (ESR_EL1::EC::Value::Unknown, 0, None),
        Exit::Breakpoint { comment } => (ESR_EL1::EC::Value::Brk64, u64::from(comment), None),
        Exit::MemoryFaultWithoutRegisters => {
            panic!("the instruction at {pc:#x} reached beyond the CPU side's address space")
        }
    };
    Some(Exception {
        syndrome: ESR_EL1::EC.val(class as u64).value
            | ESR_EL1::IL::SET.value
            | ESR_EL1::ISS.val(specific).value,
        fault_address,
    })
}
