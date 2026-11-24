//! An [`Executor`] that runs ARM64 code at EL0 of a virtual machine, under an EL1 handler that saves its registers.
//!
//! Each state is a slot in guest memory. A batch of single steps, each slot with tables of its own, costs one exit;
//! `execute` runs from one slot on a [`GuestMemorySpace`]'s tables until a trap.

use std::collections::BTreeMap;
use std::mem::offset_of;
use std::ops::Range;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aarch64_cpu::registers::{
    CPACR_EL1, ESR_EL1, ESR_EL2, ID_AA64MMFR0_EL1, ID_AA64PFR0_EL1, MDSCR_EL1, SCTLR_EL1, SPSR_EL1,
    TTBR0_EL1,
};
use aarch64_paging::descriptor::El1Attributes;
use wcontainers_abi::{
    Access, PAGE_SIZE, SYSTEM_CALL_ARGUMENT_COUNT, SYSTEM_CALL_NUMBER_REGISTER, SystemCallNumber,
    SystemCallRequest,
};
use wcontainers_arm64::INSTRUCTION_BYTES;
use wcontainers_executor::{
    ExecutionInterrupter, Executor, InterruptResponse, MemoryFaultResponse, SystemCallResponse,
    TrapHandler,
};

use crate::Stopper;
use wcontainers_executor_hardware_virtualization_inside::boot::{
    BOOT_MAP_BYTES, BootRegisters, boot_registers, inside_attributes, write_boot_map,
};
use wcontainers_executor_hardware_virtualization_inside::layout::{
    ATTRIBUTE_NORMAL, ENTRIES, ENTRY_BYTES, LEAF_LEVEL, MEMORY_BASE, TOP_LEVEL, VECTOR_BYTES,
    VECTOR_TABLE_BYTES, level_index, linear,
};
use wcontainers_executor_hardware_virtualization_inside::link::DOORBELL_FUNCTION;
use wcontainers_processor_architecture::{ExecutionState, MemoryDelta};
use wcontainers_processor_architecture_arm64::memory::report_changed_bytes;
use wcontainers_executor_hardware_virtualization_inside::syndrome::{
    CACHE_MAINTENANCE, FAULT_STATUS,
};
use wcontainers_processor_architecture_arm64::syndrome::{FaultStatus, WRITE_NOT_READ};
use wcontainers_processor_architecture_arm64::{
    Arm64, ConditionFlags, DataAccess, Exception, GENERAL_PURPOSE_REGISTERS, InstructionFetch,
    PagePermission, Processor, VECTOR_REGISTERS,
};

use crate::memory_space::{GuestMemorySpace, OutOfFrames, POOL_BASE, POOL_BYTES, Pool};
use crate::{Exit, Host, Hypervisor, Memory, Register, Result, VirtualCpu, VirtualMachine};

/// Bytes of the machine's own guest-physical memory: one block of the boot map, as large as the devices' below it.
pub(crate) const MEMORY_BYTES: u64 = MEMORY_BASE;

// Where each part lies in guest-physical memory.
const BOOT_MAP: u64 = MEMORY_BASE;
const HANDLER: u64 = BOOT_MAP + BOOT_MAP_BYTES;
const HANDLER_STACK: u64 = HANDLER + PAGE_SIZE;
/// One table for each level below the top whose entries all lead to an invalid entry at the last level.
const UNMAPPED_TABLES: u64 = HANDLER_STACK + PAGE_SIZE;
/// The slot `execute` runs its guest from.
const EXECUTE_SLOT: u64 = UNMAPPED_TABLES + LEAF_LEVEL as u64 * PAGE_SIZE;
/// What the slots of a batch take their pages from.
const ARENA: Range<u64> = EXECUTE_SLOT + PAGE_SIZE..MEMORY_BASE + MEMORY_BYTES;

/// The fault statuses of a page that does not allow an access: every table leads to a last-level entry, and every
/// entry is marked accessed, so no other level or access flag fault comes.
const PAGE_FAULTS: [FaultStatus; 2] = [
    FaultStatus::TranslationFaultLevel3,
    FaultStatus::PermissionFaultLevel3,
];

// Bytes of one register of each kind.
const REGISTER_BYTES: usize = size_of::<u64>();
const VECTOR_REGISTER_BYTES: usize = size_of::<u128>();

/// [FnV](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ESR-EL1--Exception-Syndrome-Register--EL1-#:~:text=FnV%2C%20bit%20%5B10%5D), set when an abort's FAR_EL1 is not valid.
const FAULT_ADDRESS_NOT_VALID: u64 = 1 << 10;

/// [SME](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64PFR1-EL1--AArch64-Processor-Feature-Register-1#:~:text=SME%2C%20bits%20%5B27%3A24%5D), not zero when the chip has SME.
const SME_IMPLEMENTED: u64 = 0b1111 << 24;

/// [SMEN](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CPACR-EL1--Architectural-Feature-Access-Control-Register#:~:text=SMEN%2C%20bits%20%5B25%3A24%5D) at 0b11: EL0 may use SME, as Linux lets a program once it has used it.
const SME_TRAPS_NOTHING: u64 = 0b11 << 24;

/// What a program at EL0 holds, in the order the handler loads and saves it.
#[repr(C)]
#[derive(Clone, Copy)]
struct Registers {
    general_purpose: [u64; GENERAL_PURPOSE_REGISTERS],
    /// SP_EL0.
    stack_pointer: u64,
    /// ELR_EL1: where the instruction is, and after, where the exception that ended it returns to.
    program_counter: u64,
    /// SPSR_EL1: EL0's PSTATE.
    saved_program_status: u64,
    /// TPIDR_EL0.
    software_thread_id: u64,
    floating_point_control: u64,
    floating_point_status: u64,
    /// ESR_EL1 after.
    syndrome: u64,
    /// FAR_EL1 after.
    fault_address: u64,
    vector: [u128; VECTOR_REGISTERS],
}

/// One state in guest memory, as the handler reads and writes it.
#[repr(C)]
#[derive(Clone, Copy)]
struct Slot {
    /// TTBR0_EL1 while the instruction runs: the slot's top-level table and its ASID.
    translation_table: u64,
    /// The linear address of the next slot of the batch, zero after the last.
    next: u64,
    /// An [`Ending`], which the handler writes.
    ending: u64,
    before: Registers,
    after: Registers,
}

/// What ended a slot's run.
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ending {
    NotRun,
    /// An exception from EL0: the software step after the instruction, or the instruction's own.
    ExceptionFromEl0,
    /// Any other exception, which the handler does not expect: the batch stops there.
    Unexpected,
}

/// Where the handler's code lies in this program.
#[repr(C)]
struct HandlerCode {
    /// Its exception vectors, where it starts.
    vectors: *const u8,
    /// Where the host enters it to run the slot TPIDR_EL1 names.
    entry: *const u8,
    /// Where it starts to save a slot after an exception from EL0; from the entry to here it runs no guest code.
    save: *const u8,
    end: *const u8,
}

/// Gives where the handler's code lies; the code follows the return and never runs in this program.
///
/// Every exception but a synchronous one from EL0 is unexpected. The handler keeps the current slot in
/// TPIDR_EL1, and leaves the guest by the doorbell's hypervisor call once a slot's `next` is zero.
#[unsafe(naked)]
extern "C" fn find_handler_code() -> HandlerCode {
    core::arch::naked_asm!(
        "adr x9, 2f",
        "str x9, [x8, #{vectors_field}]",
        "adr x9, 4f",
        "str x9, [x8, #{entry_field}]",
        "adr x9, 5f",
        "str x9, [x8, #{save_field}]",
        "adr x9, 9f",
        "str x9, [x8, #{end_field}]",
        "ret",
        // The vectors: from EL1 (with SP_EL0, then SP_EL1), then from EL0 in AArch64, then in AArch32; each
        // synchronous, IRQ, FIQ and SError.
        ".balign {vector_table_bytes}",
        "2:",
        ".rept 8",
        "b 8f",
        ".balign {vector_bytes}",
        ".endr",
        "b 5f",
        ".balign {vector_bytes}",
        ".rept 7",
        "b 8f",
        ".balign {vector_bytes}",
        ".endr",
        // The entry: unlock the OS lock, which stops software step, forget the translations and instructions of
        // the frames the last batch used, then run the slot. Within a batch each slot has its own ASID and frames.
        "4:",
        "msr oslar_el1, xzr",
        "tlbi vmalle1is",
        "ic ialluis",
        "dsb ish",
        "isb",
        "3:",
        "mrs x0, tpidr_el1",
        "ldr x1, [x0, #{translation_table}]",
        "msr ttbr0_el1, x1",
        "isb",
        "add x1, x0, #{before}",
        "ldp x2, x3, [x1, #{stack_pointer}]",
        "msr sp_el0, x2",
        "msr elr_el1, x3",
        "ldp x2, x3, [x1, #{saved_program_status}]",
        "msr spsr_el1, x2",
        "msr tpidr_el0, x3",
        "ldp x2, x3, [x1, #{floating_point_control}]",
        "msr fpcr, x2",
        "msr fpsr, x3",
        "add x2, x1, #{vector}",
        "ldp q0, q1, [x2, #0]",
        "ldp q2, q3, [x2, #32]",
        "ldp q4, q5, [x2, #64]",
        "ldp q6, q7, [x2, #96]",
        "ldp q8, q9, [x2, #128]",
        "ldp q10, q11, [x2, #160]",
        "ldp q12, q13, [x2, #192]",
        "ldp q14, q15, [x2, #224]",
        "ldp q16, q17, [x2, #256]",
        "ldp q18, q19, [x2, #288]",
        "ldp q20, q21, [x2, #320]",
        "ldp q22, q23, [x2, #352]",
        "ldp q24, q25, [x2, #384]",
        "ldp q26, q27, [x2, #416]",
        "ldp q28, q29, [x2, #448]",
        "ldp q30, q31, [x2, #480]",
        "ldp x2, x3, [x1, #16]",
        "ldp x4, x5, [x1, #32]",
        "ldp x6, x7, [x1, #48]",
        "ldp x8, x9, [x1, #64]",
        "ldp x10, x11, [x1, #80]",
        "ldp x12, x13, [x1, #96]",
        "ldp x14, x15, [x1, #112]",
        "ldp x16, x17, [x1, #128]",
        "ldp x18, x19, [x1, #144]",
        "ldp x20, x21, [x1, #160]",
        "ldp x22, x23, [x1, #176]",
        "ldp x24, x25, [x1, #192]",
        "ldp x26, x27, [x1, #208]",
        "ldp x28, x29, [x1, #224]",
        "ldr x30, [x1, #240]",
        "ldp x0, x1, [x1, #0]",
        "eret",
        // A synchronous exception from EL0: save everything into the slot, then run the next one.
        "5:",
        "str x0, [sp, #-16]!",
        "mrs x0, tpidr_el1",
        "add x0, x0, #{after}",
        "stp x2, x3, [x0, #16]",
        "stp x4, x5, [x0, #32]",
        "stp x6, x7, [x0, #48]",
        "stp x8, x9, [x0, #64]",
        "stp x10, x11, [x0, #80]",
        "stp x12, x13, [x0, #96]",
        "stp x14, x15, [x0, #112]",
        "stp x16, x17, [x0, #128]",
        "stp x18, x19, [x0, #144]",
        "stp x20, x21, [x0, #160]",
        "stp x22, x23, [x0, #176]",
        "stp x24, x25, [x0, #192]",
        "stp x26, x27, [x0, #208]",
        "stp x28, x29, [x0, #224]",
        "str x30, [x0, #240]",
        "ldr x2, [sp], #16",
        "stp x2, x1, [x0, #0]",
        "mrs x2, sp_el0",
        "mrs x3, elr_el1",
        "stp x2, x3, [x0, #{stack_pointer}]",
        "mrs x2, spsr_el1",
        "mrs x3, tpidr_el0",
        "stp x2, x3, [x0, #{saved_program_status}]",
        "mrs x2, fpcr",
        "mrs x3, fpsr",
        "stp x2, x3, [x0, #{floating_point_control}]",
        "mrs x2, esr_el1",
        "mrs x3, far_el1",
        "stp x2, x3, [x0, #{syndrome}]",
        "add x2, x0, #{vector}",
        "stp q0, q1, [x2, #0]",
        "stp q2, q3, [x2, #32]",
        "stp q4, q5, [x2, #64]",
        "stp q6, q7, [x2, #96]",
        "stp q8, q9, [x2, #128]",
        "stp q10, q11, [x2, #160]",
        "stp q12, q13, [x2, #192]",
        "stp q14, q15, [x2, #224]",
        "stp q16, q17, [x2, #256]",
        "stp q18, q19, [x2, #288]",
        "stp q20, q21, [x2, #320]",
        "stp q22, q23, [x2, #352]",
        "stp q24, q25, [x2, #384]",
        "stp q26, q27, [x2, #416]",
        "stp q28, q29, [x2, #448]",
        "stp q30, q31, [x2, #480]",
        "mrs x0, tpidr_el1",
        "mov x1, #{exception_from_el0}",
        "str x1, [x0, #{ending}]",
        "ldr x1, [x0, #{next}]",
        "cbz x1, 6f",
        "msr tpidr_el1, x1",
        "b 3b",
        // Any other exception: record what the syndrome registers say, and stop the batch.
        "8:",
        "mrs x0, tpidr_el1",
        "mov x1, #{unexpected}",
        "str x1, [x0, #{ending}]",
        "add x0, x0, #{after}",
        "mrs x2, esr_el1",
        "mrs x3, far_el1",
        "stp x2, x3, [x0, #{syndrome}]",
        "mrs x2, spsr_el1",
        "str x2, [x0, #{saved_program_status}]",
        "mrs x2, elr_el1",
        "str x2, [x0, #{program_counter}]",
        "6:",
        "mov x0, #{doorbell}",
        "hvc #0",
        "b 6b",
        "9:",
        vectors_field = const offset_of!(HandlerCode, vectors),
        entry_field = const offset_of!(HandlerCode, entry),
        save_field = const offset_of!(HandlerCode, save),
        end_field = const offset_of!(HandlerCode, end),
        vector_table_bytes = const VECTOR_TABLE_BYTES,
        vector_bytes = const VECTOR_BYTES,
        translation_table = const offset_of!(Slot, translation_table),
        next = const offset_of!(Slot, next),
        ending = const offset_of!(Slot, ending),
        before = const offset_of!(Slot, before),
        after = const offset_of!(Slot, after),
        stack_pointer = const offset_of!(Registers, stack_pointer),
        program_counter = const offset_of!(Registers, program_counter),
        saved_program_status = const offset_of!(Registers, saved_program_status),
        floating_point_control = const offset_of!(Registers, floating_point_control),
        syndrome = const offset_of!(Registers, syndrome),
        vector = const offset_of!(Registers, vector),
        exception_from_el0 = const Ending::ExceptionFromEl0 as u64,
        unexpected = const Ending::Unexpected as u64,
        doorbell = const DOORBELL_FUNCTION,
    );
}

// The handler moves registers in pairs, and the general registers from the start of the block.
const _: () = {
    assert!(offset_of!(Registers, general_purpose) == 0);
    assert!(
        offset_of!(Registers, program_counter)
            == offset_of!(Registers, stack_pointer) + REGISTER_BYTES
    );
    assert!(
        offset_of!(Registers, software_thread_id)
            == offset_of!(Registers, saved_program_status) + REGISTER_BYTES
    );
    assert!(
        offset_of!(Registers, floating_point_status)
            == offset_of!(Registers, floating_point_control) + REGISTER_BYTES
    );
    assert!(
        offset_of!(Registers, fault_address) == offset_of!(Registers, syndrome) + REGISTER_BYTES
    );
    assert!(VECTOR_REGISTERS * VECTOR_REGISTER_BYTES == size_of::<[u128; VECTOR_REGISTERS]>());
    assert!(size_of::<Slot>() as u64 <= PAGE_SIZE);
};

/// A slot loaded for the next run, and the pages of it that EL0 may write.
struct LoadedSlot {
    slot: u64,
    writable: Range<usize>,
}

/// Consecutive pages EL0 may write, on consecutive frames.
struct WritableRun {
    /// Its first virtual address.
    start: u64,
    frame: u64,
    /// Where the bytes before the run lie in [`SingleStepMachine::bytes_before`].
    before: Range<usize>,
}

/// The executor: one virtual machine with one virtual CPU, bound to the thread that made it.
///
/// The fields drop in order: the CPU before its machine, the machine before the memory it maps.
pub struct SingleStepMachine {
    cpu: <<Host as Hypervisor>::VirtualMachine as VirtualMachine>::Cpu,
    _machine: <Host as Hypervisor>::VirtualMachine,
    memory: Memory,
    /// The frames of the spaces it makes, which it maps at [`POOL_BASE`].
    pool: Arc<Mutex<Pool>>,
    /// The handler's entry, at its linear address.
    entry: u64,
    /// Where the handler starts to save a slot, at its linear address.
    save: u64,
    /// PSTATE as the host enters the handler.
    entry_pstate: u64,
    next_free_page: u64,
    /// How many ASIDs the chip has, and so how many slots a batch holds at most.
    asid_count: u64,
    /// The slots of the batch so far, which is also the next one's ASID.
    slot_count: u64,
    first_slot: Option<u64>,
    last_slot: Option<u64>,
    writable_runs: Vec<WritableRun>,
    bytes_before: Vec<u8>,
    time_in_guest: Duration,
}

impl SingleStepMachine {
    /// Makes the process's one virtual machine and sets it up to run single instructions.
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "macos")]
        let machine = Host::machine(1)?;
        #[cfg(target_os = "linux")]
        let machine = Host::machine_with(1, crate::kvm::ScalableVectors::Shown)?;
        let memory = Memory::reserve(MEMORY_BYTES)?;
        let host = NonNull::new(memory.host(MEMORY_BASE)).expect("the reservation is not at zero");
        machine.map(host, MEMORY_BASE, MEMORY_BYTES)?;
        let pool = Pool::new()?;
        machine.map(pool.host(), POOL_BASE, POOL_BYTES)?;
        let cpu = machine.cpu()?;
        #[cfg(target_os = "macos")]
        cpu.leave_debugging_to_guest()?;
        let mut executor = SingleStepMachine {
            cpu,
            _machine: machine,
            memory,
            pool: Arc::new(Mutex::new(pool)),
            entry: 0,
            save: u64::MIN,
            entry_pstate: 0,
            next_free_page: ARENA.start,
            asid_count: 0,
            slot_count: 0,
            first_slot: None,
            last_slot: None,
            writable_runs: Vec::new(),
            bytes_before: Vec::new(),
            time_in_guest: Duration::ZERO,
        };
        executor.set_up()?;
        Ok(executor)
    }

    /// The time spent inside the guest so far, exits included.
    pub fn time_in_guest(&self) -> Duration {
        self.time_in_guest
    }

    /// Executes one instruction from each before state, as `execute_single_instruction` does for one, leaving the
    /// guest once for as many states as its memory holds.
    pub fn execute_each_single_instruction<'memory>(
        &mut self,
        befores: impl IntoIterator<Item = ExecutionState<'memory, Arm64>>,
    ) -> Vec<ExecutionState<'memory, Arm64>> {
        let mut afters = Vec::new();
        let mut waiting = Vec::new();
        self.start_batch();
        for before in befores {
            let loaded = match self.load(&before.processor, &before.memory_mutations) {
                Some(loaded) => loaded,
                None => {
                    self.finish_batch(&mut waiting, &mut afters);
                    self.start_batch();
                    self.load(&before.processor, &before.memory_mutations)
                        .expect("one state's pages fit in the guest's memory")
                }
            };
            waiting.push((before, loaded));
        }
        self.finish_batch(&mut waiting, &mut afters);
        afters
    }

    /// Writes the handler, its stack and translation tables, and the system registers that stay.
    fn set_up(&mut self) -> Result<()> {
        write_boot_map(BOOT_MAP, MEMORY_BYTES, |at, entry| {
            self.write_entry(at, entry)
        });
        let code = find_handler_code();
        let length = code.end as u64 - code.vectors as u64;
        assert!(length <= PAGE_SIZE, "the handler fits its page");
        // SAFETY: the handler's code lies between its first and last label in this program's own text.
        let bytes = unsafe { std::slice::from_raw_parts(code.vectors, length as usize) };
        self.write_bytes(HANDLER, length).copy_from_slice(bytes);
        self.entry = linear(HANDLER + (code.entry as u64 - code.vectors as u64));
        self.save = linear(HANDLER + (code.save as u64 - code.vectors as u64));
        for level in TOP_LEVEL + 1..=LEAF_LEVEL {
            self.fill_table(unmapped_table(level), to_unmapped_entry(level));
        }
        let memory_model_features = self.cpu.get(Register::MemoryModelFeatures)?;
        let BootRegisters {
            mair,
            tcr,
            sctlr,
            cpacr,
            ttbr1,
            pstate,
        } = boot_registers(
            BOOT_MAP,
            memory_model_features,
            <wcontainers_arm64::Arm64 as wcontainers_guest_cpu::Guest>::SP_ALIGNMENT_CHECKED,
        );
        self.entry_pstate = pstate;
        self.asid_count = match ID_AA64MMFR0_EL1::ASIDBits.read_as_enum(memory_model_features) {
            Some(ID_AA64MMFR0_EL1::ASIDBits::Value::Bits_16) => 1 << u16::BITS,
            Some(ID_AA64MMFR0_EL1::ASIDBits::Value::Bits_8) | None => 1 << u8::BITS,
        };
        // CTR_EL0 is readable at EL0, as Linux has it.
        let sctlr = sctlr | SCTLR_EL1::UCT::DontTrap.value;
        let has_sve = ID_AA64PFR0_EL1::SVE.read(self.cpu.get(Register::ProcessorFeatures)?) != 0;
        let cpacr = if has_sve {
            cpacr | CPACR_EL1::ZEN::TrapNothing.value
        } else {
            cpacr
        };
        // Hypervisor.framework hides the M5's SME here, so its EL0 gets SME's trap (0x76000000 for RDVL).
        let has_sme = self.cpu.get(Register::ProcessorFeatures1)? & SME_IMPLEMENTED != 0;
        let cpacr = if has_sme {
            cpacr | SME_TRAPS_NOTHING
        } else {
            cpacr
        };
        for (register, value) in [
            (Register::Mair, mair),
            (Register::Tcr, tcr),
            (Register::Cpacr, cpacr),
            (Register::Ttbr1, ttbr1),
            (Register::Sctlr, sctlr),
            (Register::Vectors, linear(HANDLER)),
            (Register::StackEl1, linear(HANDLER_STACK + PAGE_SIZE)),
        ] {
            self.cpu.set(register, value)?;
        }
        Ok(())
    }

    fn start_batch(&mut self) {
        self.next_free_page = ARENA.start;
        self.slot_count = 0;
        self.first_slot = None;
        self.last_slot = None;
        self.writable_runs.clear();
        self.bytes_before.clear();
    }

    /// Writes a state into a slot after the batch's others, or gives `None` when the batch has no room for it.
    fn load(
        &mut self,
        processor: &Processor,
        memory: &[MemoryDelta<'_, Arm64>],
    ) -> Option<LoadedSlot> {
        if self.slot_count == self.asid_count {
            return None;
        }
        let pages = to_page_permissions(memory);
        let mapped: Vec<(u64, DataAccess, InstructionFetch)> = pages
            .iter()
            .filter_map(|(&page, permission)| match *permission {
                PagePermission::Unmapped => None,
                PagePermission::Mapped {
                    data_access,
                    instruction_fetch,
                } => Some((page, data_access, instruction_fetch)),
            })
            .collect();
        // The slot, its top-level table, a frame and at most one table of each lower level per page.
        let most_pages = 2 + mapped.len() as u64 * (1 + u64::from(LEAF_LEVEL));
        if self.next_free_page + most_pages * PAGE_SIZE > ARENA.end {
            return None;
        }
        let slot = self.allocate();
        let top = self.allocate();
        self.fill_table(top, to_unmapped_entry(TOP_LEVEL));
        // Frames first, in address order, so that consecutive pages lie on consecutive frames.
        let frames: Vec<u64> = mapped.iter().map(|_| self.allocate()).collect();
        let writable_start = self.writable_runs.len();
        let mut bytes = vec![0; PAGE_SIZE as usize];
        for (&(page, data_access, _), &frame) in mapped.iter().zip(&frames) {
            compose_page(page, memory, &mut bytes);
            self.write_bytes(frame, PAGE_SIZE).copy_from_slice(&bytes);
            if data_access != DataAccess::ReadWrite {
                continue;
            }
            let continues = self.writable_runs[writable_start..]
                .last()
                .is_some_and(|run| run.start + run.before.len() as u64 == page);
            let before_start = self.bytes_before.len();
            self.bytes_before.extend_from_slice(&bytes);
            match (continues, self.writable_runs.last_mut()) {
                (true, Some(run)) => run.before.end = self.bytes_before.len(),
                _ => self.writable_runs.push(WritableRun {
                    start: page,
                    frame,
                    before: before_start..self.bytes_before.len(),
                }),
            }
        }
        for (&(page, data_access, instruction_fetch), &frame) in mapped.iter().zip(&frames) {
            let leaf = self.find_leaf_entry(top, page);
            self.write_entry(leaf, to_page_entry(frame, data_access, instruction_fetch));
        }
        let slot_contents = Slot {
            translation_table: top | TTBR0_EL1::ASID.val(self.slot_count).value,
            next: 0,
            ending: Ending::NotRun as u64,
            before: to_registers(processor, Stepping::On),
            after: to_registers(processor, Stepping::On),
        };
        self.write_slot(slot, slot_contents);
        if let Some(last) = self.last_slot {
            let mut previous = self.read_slot(last);
            previous.next = linear(slot);
            self.write_slot(last, previous);
        }
        self.slot_count += 1;
        self.first_slot.get_or_insert(slot);
        self.last_slot = Some(slot);
        Some(LoadedSlot {
            slot,
            writable: writable_start..self.writable_runs.len(),
        })
    }

    /// Runs every slot of the batch, then gives each state its after.
    fn finish_batch<'memory>(
        &mut self,
        waiting: &mut Vec<(ExecutionState<'memory, Arm64>, LoadedSlot)>,
        afters: &mut Vec<ExecutionState<'memory, Arm64>>,
    ) {
        self.run();
        for (mut state, loaded) in waiting.drain(..) {
            let mut mutations = Vec::new();
            self.read_back(&loaded, &mut state.processor, |mutation| {
                mutations.push(mutation.into_owned())
            });
            state.memory_mutations.append(&mut mutations);
            afters.push(state);
        }
    }

    /// Enters the handler at the batch's first slot and waits until it leaves after the last.
    fn run(&mut self) {
        let Some(first) = self.first_slot else {
            return;
        };
        self.enter(first, MDSCR_EL1::SS::SET.value);
        match self.run_until_exit() {
            Ended::Doorbell => {}
            Ended::Stopped => panic!("the single step machine was stopped from another thread"),
        }
    }

    /// Sets the virtual CPU to enter the handler at the slot, with the debug controls given.
    fn enter(&mut self, slot: u64, debug_control: u64) {
        for (register, value) in [
            (Register::InsideThread, linear(slot)),
            (Register::Pc, self.entry),
            (Register::Pstate, self.entry_pstate),
            (Register::DebugControl, debug_control),
        ] {
            self.cpu
                .set(register, value)
                .unwrap_or_else(|error| panic!("the hypervisor refused {register:?}: {error:?}"));
        }
    }

    /// Runs the virtual CPU until the handler's doorbell or a stop from another thread.
    fn run_until_exit(&mut self) -> Ended {
        let started = Instant::now();
        let ended = loop {
            match self.cpu.run() {
                Ok(Exit::Doorbell) => break Ended::Doorbell,
                Ok(Exit::Stopped) => break Ended::Stopped,
                // Hypervisor.framework traps the unlock of the OS lock and leaves it to us: the lock is already
                // open, so the handler goes on after it.
                Ok(Exit::Exception { syndrome, .. })
                    if ESR_EL2::EC.read(syndrome) == ESR_EL2::EC::Value::TrappedMsrMrs as u64
                        && self.cpu.get(Register::Pc).ok() == Some(self.entry) =>
                {
                    let after = self.entry + INSTRUCTION_BYTES as u64;
                    self.cpu
                        .set(Register::Pc, after)
                        .unwrap_or_else(|error| panic!("the hypervisor refused Pc: {error:?}"));
                }
                other => {
                    panic!("the guest stopped other than at the handler's doorbell: {other:?}")
                }
            }
        };
        self.time_in_guest += started.elapsed();
        ended
    }

    /// Returns the slot at `at` once the handler has ended it by an exception from EL0.
    fn read_ended_slot(&self, at: u64) -> Slot {
        let slot = self.read_slot(at);
        assert!(
            slot.ending == Ending::ExceptionFromEl0 as u64,
            "the handler ended the slot by {:?}: syndrome {:#x}, return address {:#x}, saved state {:#x}",
            [Ending::NotRun, Ending::ExceptionFromEl0, Ending::Unexpected]
                .into_iter()
                .find(|ending| *ending as u64 == slot.ending),
            slot.after.syndrome,
            slot.after.program_counter,
            slot.after.saved_program_status,
        );
        slot
    }

    /// Runs the guest on the space from the processor state until a trap the handler, or no handler, ends it.
    ///
    /// An interrupt is pending while `pending` is set; a stop of the virtual CPU without it is another execution's
    /// late interrupt, and the guest runs on.
    fn run_guest(
        &mut self,
        mut processor: Processor,
        memory: &GuestMemorySpace,
        handler: &mut impl TrapHandler,
        pending: &AtomicBool,
    ) -> Processor {
        assert!(
            memory.is_of(&self.pool),
            "the executor runs only spaces it made"
        );
        let translation_table = memory.find_root();
        // The last fault the machine answered itself: when it comes again at once, the handler answers it.
        let mut answered: Option<(u64, Access)> = None;
        // An interrupt that found the handler saving the slot, answered after the trap it saves.
        let mut interrupted = false;
        loop {
            if (std::mem::take(&mut interrupted) || pending.swap(false, Ordering::Acquire))
                && let InterruptResponse::Terminate = handler.handle_interrupt()
            {
                processor.exception = None;
                return processor;
            }
            let registers = to_registers(&processor, Stepping::Off);
            self.write_slot(
                EXECUTE_SLOT,
                Slot {
                    translation_table,
                    next: u64::MIN,
                    ending: Ending::NotRun as u64,
                    before: registers,
                    after: registers,
                },
            );
            self.enter(EXECUTE_SLOT, u64::MIN);
            while let Ended::Stopped = self.run_until_exit() {
                if !pending.swap(false, Ordering::Acquire) {
                    continue;
                }
                match self.locate_stop() {
                    Stop::Guest => {
                        if let InterruptResponse::Terminate = handler.handle_interrupt() {
                            return self.save_running_guest();
                        }
                    }
                    Stop::Entering => {
                        if let InterruptResponse::Terminate = handler.handle_interrupt() {
                            processor.exception = None;
                            return processor;
                        }
                    }
                    Stop::Saving => interrupted = true,
                }
            }
            let slot = self.read_ended_slot(EXECUTE_SLOT);
            let stopped = to_processor(&slot.after);
            processor = Processor {
                exception: None,
                ..stopped.clone()
            };
            match to_trap(slot.after.syndrome, &stopped) {
                Trap::SystemCall(request) => match handler.handle_system_call(request) {
                    SystemCallResponse::Return(value) => {
                        // The result goes in the first argument's register, x0.
                        if let Some(result) = processor.general_purpose_registers.first_mut() {
                            *result = value;
                        }
                        answered = None;
                    }
                    SystemCallResponse::Terminate => {
                        return Processor {
                            program_counter: stopped.program_counter - INSTRUCTION_BYTES as u64,
                            ..stopped
                        };
                    }
                },
                Trap::MemoryFault(address, access) => {
                    let fault = Some((address, access));
                    if answered != fault && memory.back_for(address, access) {
                        answered = fault;
                        continue;
                    }
                    answered = None;
                    match handler.handle_memory_fault(address, access) {
                        MemoryFaultResponse::Retry => {}
                        MemoryFaultResponse::Terminate => return stopped,
                    }
                }
                Trap::Ending => return stopped,
            }
        }
    }

    /// Says where a stop from another thread found the virtual CPU.
    fn locate_stop(&self) -> Stop {
        let get = |register| {
            self.cpu
                .get(register)
                .unwrap_or_else(|error| panic!("the hypervisor refused {register:?}: {error:?}"))
        };
        if SPSR_EL1::M.read(get(Register::Pstate)) == SPSR_EL1::M::Value::EL0t as u64 {
            return Stop::Guest;
        }
        if (self.entry..self.save).contains(&get(Register::Pc)) {
            Stop::Entering
        } else {
            Stop::Saving
        }
    }

    /// Returns the processor state of a guest stopped at EL0, which the handler saves as if a software step ended it.
    fn save_running_guest(&mut self) -> Processor {
        let get = |register| {
            self.cpu
                .get(register)
                .unwrap_or_else(|error| panic!("the hypervisor refused {register:?}: {error:?}"))
        };
        let stepped = ESR_EL1::EC::SoftwareStepLowerEL.value;
        for (register, value) in [
            (Register::ExceptionLink, get(Register::Pc)),
            (Register::SavedState, get(Register::Pstate)),
            (Register::Syndrome, stepped),
            (Register::Pc, self.save),
            (Register::Pstate, self.entry_pstate),
        ] {
            self.cpu
                .set(register, value)
                .unwrap_or_else(|error| panic!("the hypervisor refused {register:?}: {error:?}"));
        }
        while let Ended::Stopped = self.run_until_exit() {}
        to_processor(&self.read_ended_slot(EXECUTE_SLOT).after)
    }

    /// Gives the processor after a slot's run, and each run of bytes it changed as a slice of guest memory.
    fn read_back(
        &self,
        loaded: &LoadedSlot,
        processor: &mut Processor,
        mut on_memory_mutated: impl FnMut(MemoryDelta<'_, Arm64>),
    ) {
        let slot = self.read_ended_slot(loaded.slot);
        *processor = to_processor(&slot.after);
        for run in &self.writable_runs[loaded.writable.clone()] {
            let before = &self.bytes_before[run.before.clone()];
            let after = self.read_bytes(run.frame, before.len() as u64);
            report_changed_bytes(run.start, before, after, &mut on_memory_mutated);
        }
    }

    fn allocate(&mut self) -> u64 {
        let page = self.next_free_page;
        self.next_free_page += PAGE_SIZE;
        page
    }

    /// The address of the last-level entry of a virtual page, making the tables that lead to it.
    fn find_leaf_entry(&mut self, top: u64, page: u64) -> u64 {
        let mut table = top;
        for level in TOP_LEVEL..LEAF_LEVEL {
            let at = table + level_index(page, level) * ENTRY_BYTES;
            let entry = self.read_entry(at);
            table = if entry == to_unmapped_entry(level) {
                let own = self.allocate();
                self.fill_table(own, to_unmapped_entry(level + 1));
                self.write_entry(at, to_table_entry(own));
                own
            } else {
                entry & !to_table_entry(0)
            };
        }
        table + level_index(page, LEAF_LEVEL) * ENTRY_BYTES
    }

    fn fill_table(&mut self, table: u64, entry: u64) {
        for index in 0..ENTRIES {
            self.write_entry(table + index * ENTRY_BYTES, entry);
        }
    }

    fn read_entry(&self, at: u64) -> u64 {
        let mut entry = [u8::MIN; ENTRY_BYTES as usize];
        entry.copy_from_slice(self.read_bytes(at, ENTRY_BYTES));
        u64::from_le_bytes(entry)
    }

    fn write_entry(&mut self, at: u64, entry: u64) {
        self.write_bytes(at, ENTRY_BYTES)
            .copy_from_slice(&entry.to_le_bytes());
    }

    fn read_bytes(&self, at: u64, length: u64) -> &[u8] {
        self.memory.bytes(at, length)
    }

    fn write_bytes(&mut self, at: u64, length: u64) -> &mut [u8] {
        self.memory.bytes_mut(at, length)
    }

    fn read_slot(&self, at: u64) -> Slot {
        // SAFETY: the slot lies in guest memory and the guest is not running.
        unsafe { std::ptr::read(self.memory.host(at).cast::<Slot>()) }
    }

    fn write_slot(&mut self, at: u64, slot: Slot) {
        // SAFETY: the slot's page lies in guest memory, and the guest is not running.
        unsafe { std::ptr::write(self.memory.host(at).cast::<Slot>(), slot) };
    }
}

/// Interrupts one execution of the machine from any thread.
pub struct MachineInterrupter {
    /// Set until the execution answers the interrupt.
    pending: Arc<AtomicBool>,
    stopper: <<<Host as Hypervisor>::VirtualMachine as VirtualMachine>::Cpu as VirtualCpu>::Stopper,
}

impl ExecutionInterrupter for MachineInterrupter {
    /// Marks the interrupt pending and stops the virtual CPU, which answers it at once or at its next trap or entry.
    fn interrupt(&self) {
        self.pending.store(true, Ordering::Release);
        // The stopper refuses only once the virtual CPU is gone, when no execution is left to interrupt.
        self.stopper.stop().ok();
    }
}

/// What ended a run of the virtual CPU.
enum Ended {
    Doorbell,
    Stopped,
}

/// Where a stop from another thread found the virtual CPU.
enum Stop {
    /// Running the guest at EL0.
    Guest,
    /// In the handler, before the guest runs.
    Entering,
    /// In the handler, saving the slot after an exception from EL0; it rings the doorbell next.
    Saving,
}

/// What a guest's exception from EL0 asks of `execute`.
enum Trap {
    SystemCall(SystemCallRequest),
    /// A page that does not allow the access, at the address.
    MemoryFault(u64, Access),
    /// Every other exception, which ends the run with the fault in the processor state.
    Ending,
}

/// Returns what the exception the syndrome describes asks of `execute`, from the state it stopped at.
fn to_trap(syndrome: u64, stopped: &Processor) -> Trap {
    use ESR_EL1::EC::Value::{DataAbortLowerEL, InstrAbortLowerEL, SVC64};
    let class = ESR_EL1::EC.read(syndrome);
    if class == SVC64 as u64 {
        let registers = &stopped.general_purpose_registers;
        let mut arguments = [u64::MIN; SYSTEM_CALL_ARGUMENT_COUNT];
        arguments.copy_from_slice(&registers[..SYSTEM_CALL_ARGUMENT_COUNT]);
        return Trap::SystemCall(SystemCallRequest {
            number: SystemCallNumber(registers[SYSTEM_CALL_NUMBER_REGISTER]),
            arguments,
        });
    }
    let access = if class == InstrAbortLowerEL as u64 {
        Access::Execute
    } else if class != DataAbortLowerEL as u64 {
        return Trap::Ending;
    } else if syndrome & WRITE_NOT_READ != 0 && syndrome & CACHE_MAINTENANCE == 0 {
        Access::Write
    } else {
        Access::Read
    };
    let status = syndrome & FAULT_STATUS;
    let page_fault = PAGE_FAULTS.iter().any(|&fault| fault as u64 == status);
    match stopped.exception.and_then(|exception| exception.fault_address) {
        Some(address) if page_fault => Trap::MemoryFault(address, access),
        _ => Trap::Ending,
    }
}

impl Executor for SingleStepMachine {
    type Architecture = Arm64;
    type TranslatorConfig = ();
    type GuestMemory = GuestMemorySpace;
    type Interrupter = MachineInterrupter;
    type CreateGuestMemoryError = OutOfFrames;

    fn create_guest_memory(&self) -> std::result::Result<GuestMemorySpace, OutOfFrames> {
        GuestMemorySpace::new(&self.pool)
    }

    /// Runs only spaces it made, blocking the polling thread, and itself gives a frame to a page that allows the access.
    fn execute<'execution>(
        &'execution mut self,
        processor: Processor,
        memory: &'execution GuestMemorySpace,
        handler: &'execution mut impl TrapHandler,
    ) -> (
        impl Future<Output = Processor> + 'execution,
        MachineInterrupter,
    ) {
        let pending = Arc::new(AtomicBool::new(false));
        let interrupter = MachineInterrupter {
            pending: Arc::clone(&pending),
            stopper: self.cpu.stopper(),
        };
        let execution = async move { self.run_guest(processor, memory, handler, &pending) };
        (execution, interrupter)
    }

    /// Reports each run of bytes whose value changed: a store of the bytes already there is not reported.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor,
        memory: &[MemoryDelta<'_, Arm64>],
        on_memory_mutated: impl FnMut(MemoryDelta<'_, Arm64>),
    ) {
        self.start_batch();
        let loaded = self
            .load(processor, memory)
            .expect("one state's pages fit in the guest's memory");
        self.run();
        self.read_back(&loaded, processor, on_memory_mutated);
    }
}

fn unmapped_table(level: u32) -> u64 {
    UNMAPPED_TABLES + u64::from(level - TOP_LEVEL - 1) * PAGE_SIZE
}

/// What every entry of a table at the level holds where it maps nothing: the way to an invalid last-level entry.
pub(crate) fn to_unmapped_entry(level: u32) -> u64 {
    if level == LEAF_LEVEL {
        0
    } else {
        to_table_entry(unmapped_table(level + 1))
    }
}

pub(crate) fn to_table_entry(table: u64) -> u64 {
    table | (El1Attributes::VALID | El1Attributes::TABLE_OR_PAGE).bits() as u64
}

pub(crate) fn to_page_entry(frame: u64, data_access: DataAccess, instruction_fetch: InstructionFetch) -> u64 {
    let access = match data_access {
        DataAccess::Forbidden => El1Attributes::empty(),
        DataAccess::ReadOnly => El1Attributes::USER | El1Attributes::READ_ONLY,
        DataAccess::ReadWrite => El1Attributes::USER,
    };
    let fetch = match instruction_fetch {
        InstructionFetch::Forbidden => El1Attributes::UXN,
        InstructionFetch::Allowed => El1Attributes::empty(),
    };
    let normal = inside_attributes(ATTRIBUTE_NORMAL).difference(El1Attributes::UXN);
    // Not global, so that a slot's translations are tagged with its ASID.
    let attributes = normal
        | El1Attributes::TABLE_OR_PAGE
        | El1Attributes::PXN
        | El1Attributes::NON_GLOBAL
        | access
        | fetch;
    frame | attributes.bits() as u64
}

/// The permission of each page the deltas give one, by its first address; a later delta wins.
fn to_page_permissions(memory: &[MemoryDelta<'_, Arm64>]) -> BTreeMap<u64, PagePermission> {
    let mut pages = BTreeMap::new();
    for delta in memory {
        if let MemoryDelta::PermissionAssignment {
            addresses,
            permission,
        } = delta
        {
            assert!(
                addresses.start.is_multiple_of(PAGE_SIZE)
                    && addresses.end.is_multiple_of(PAGE_SIZE),
                "a chip gives permissions to whole pages, not {addresses:#x?}"
            );
            for page in addresses.clone().step_by(PAGE_SIZE as usize) {
                pages.insert(page, *permission);
            }
        }
    }
    pages
}

/// Writes the bytes the deltas leave in a page, which start as zeros.
fn compose_page(page: u64, memory: &[MemoryDelta<'_, Arm64>], bytes: &mut [u8]) {
    bytes.fill(0);
    let page_end = page + bytes.len() as u64;
    for delta in memory {
        if let MemoryDelta::ValueAssignment {
            start,
            bytes: values,
        } = delta
        {
            let end = start + values.len() as u64;
            let (from, to) = ((*start).max(page), end.min(page_end));
            if from < to {
                bytes[(from - page) as usize..(to - page) as usize]
                    .copy_from_slice(&values[(from - start) as usize..(to - start) as usize]);
            }
        }
    }
}

/// Whether software step ends a run after one instruction.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stepping {
    On,
    Off,
}

/// Returns the registers the handler loads to run the processor, one instruction or on.
fn to_registers(processor: &Processor, stepping: Stepping) -> Registers {
    let el0 = SPSR_EL1::M::EL0t
        + SPSR_EL1::SS.val(u64::from(stepping == Stepping::On))
        + SPSR_EL1::A::Masked
        + SPSR_EL1::I::Masked
        + SPSR_EL1::F::Masked;
    Registers {
        general_purpose: processor.general_purpose_registers,
        stack_pointer: processor.stack_pointer,
        program_counter: processor.program_counter,
        saved_program_status: u64::from(processor.condition_flags.bits()) | el0.value,
        software_thread_id: processor.software_thread_id,
        floating_point_control: processor.floating_point_control,
        floating_point_status: processor.floating_point_status,
        syndrome: 0,
        fault_address: 0,
        vector: processor.vector_registers,
    }
}

fn to_processor(registers: &Registers) -> Processor {
    let class = ESR_EL1::EC.read(registers.syndrome);
    // A software step ends an instruction that completed; anything else is the instruction's own exception.
    let exception = if class == ESR_EL1::EC::Value::SoftwareStepLowerEL as u64 {
        None
    } else {
        Some(Exception {
            syndrome: registers.syndrome,
            fault_address: to_fault_address(registers.syndrome, registers.fault_address),
        })
    };
    Processor {
        general_purpose_registers: registers.general_purpose,
        stack_pointer: registers.stack_pointer,
        program_counter: registers.program_counter,
        // NZCV sits at the same bits of SPSR_EL1; the rest of EL0's PSTATE is not part of the processor yet.
        condition_flags: ConditionFlags::from_bits_truncate(registers.saved_program_status as u32),
        vector_registers: registers.vector,
        floating_point_control: registers.floating_point_control,
        floating_point_status: registers.floating_point_status,
        software_thread_id: registers.software_thread_id,
        exception,
    }
}

/// FAR_EL1 where the syndrome makes it valid: an abort that does not say otherwise, a PC alignment fault or a
/// watchpoint.
fn to_fault_address(syndrome: u64, fault_address: u64) -> Option<u64> {
    use ESR_EL1::EC::Value::{
        DataAbortLowerEL, InstrAbortLowerEL, PCAlignmentFault, WatchpointLowerEL,
    };
    let class = ESR_EL1::EC.read(syndrome);
    let abort = [InstrAbortLowerEL, DataAbortLowerEL]
        .map(|value| value as u64)
        .contains(&class);
    let always = [PCAlignmentFault, WatchpointLowerEL]
        .map(|value| value as u64)
        .contains(&class);
    let valid = always || (abort && syndrome & FAULT_ADDRESS_NOT_VALID == 0);
    valid.then_some(fault_address)
}
