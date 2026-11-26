//! A behavior source that really runs one x86-64 instruction: at ring 3 of a KVM virtual machine in long mode, with
//! the trap flag set, under a ring 0 handler that saves the registers when the trap after the instruction, or the
//! instruction's own exception, ends it.
//!
//! Each state is a slot in guest memory: its registers before and after, and page tables built from its memory and
//! permissions. The handler loads the slot, returns to ring 3 with IRETQ, and halts once it has saved the slot.
//!
//! Guest memory comes from `wcontainers-kvm`, which keeps it mapped until the process ends; a machine that is dropped
//! leaves its memory to the next machine made, zeroed first, so that making machines does not grow the process. Every
//! read and write of it here goes through `VolatileBytes`, never a slice.
//!
//! A repeated string form steps one iteration, as the architecture's model of it does.
//!
//! A state the source cannot hold is refused, with a [`Refusal`] that names why, before anything of it is written:
//! memory outside the program's addresses (where the source keeps its own pages), RFLAGS bits a program cannot set,
//! and the instructions it cannot step (see [`check_steppable`]). A run that ends otherwise than the model has it is
//! refused too, and the next state runs on a new machine.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::collections::BTreeMap;
use std::io;
use std::mem::offset_of;
use std::ops::Range;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use iced_x86::{Code, Decoder, DecoderOptions, Mnemonic, OpKind, Register};

use kvm_bindings::{
    KVM_CAP_DISABLE_QUIRKS2, KVM_MAX_CPUID_ENTRIES, KVM_X86_QUIRK_FIX_HYPERCALL_INSN, Msrs, kvm_enable_cap, kvm_msr_entry,
    kvm_regs, kvm_segment, kvm_sregs, kvm_xcr, kvm_xcrs,
};
use kvm_ioctls::{Kvm as KvmSystem, VcpuExit, VcpuFd, VmFd};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_kvm::Kvm;
use wcontainers_kvm_direct::DirectKvm;
use wcontainers_processor_architecture::{MemoryDelta, MemorySink, MemorySnapshot, ProcessorArchitecture};
use wcontainers_processor_architecture_x86_64::memory::{read_bytes, read_permission, report_changed_bytes};
use wcontainers_processor_architecture_x86_64::{
    Access, DataAccess, Exception, Flags, GENERAL_PURPOSE_REGISTERS, InstructionFetch, MXCSR_DEFAULT,
    CODE_SEGMENT, PagePermission, Processor, STACK_POINTER, VECTOR_REGISTERS, X86_64,
};
use wcontainers_volatile_bytes::VolatileBytes;
use zerocopy::{FromZeros, IntoBytes};

use crate::handler::{Handler, KERNEL_CODE_SELECTOR, SYSTEM_CALL_VECTOR, VECTORS};
use crate::layout::{
    ACCESSED, ADDRESS_MASK, ARENA, AVX_COMPONENT, DESCRIPTOR_ENTRIES, DIRTY, ENTRIES, ENTRY_BYTES,
    EXTENDED_CONTROL_0, FAULT_STACK_TOP, FX_MXCSR, FX_VECTOR_REGISTERS, GLOBAL,
    GLOBAL_DESCRIPTOR_TABLE, HIGH_16_ZMM_COMPONENT, OPMASK_COMPONENT, SAVED_COMPONENTS, SSE_COMPONENT, XSAVE_AVX,
    XSAVE_HEADER, XSAVE_HIGH_16_ZMM, XSAVE_IMAGE_BYTES, XSAVE_OPMASK, XSAVE_PKRU, XSAVE_ZMM_HIGH_256, XsaveImage, PKRU_COMPONENT,
    ZMM_HIGH_256_COMPONENT,
    HANDLER, HANDLER_BYTES, INTERRUPT_DESCRIPTOR_TABLE, KERNEL_CODE, KERNEL_DATA, KERNEL_DIRECTORY,
    KERNEL_DIRECTORY_POINTERS, KERNEL_ENTRY, KERNEL_PAGES, KERNEL_TABLE, LEVELS, MEMORY_BYTES, NO_EXECUTE,
    LARGE_PAGE, NOT_RUN, PAGE_BYTES, PRESENT, RESIDENT, Registers, SLOT, STACK_TOP, Slot, TASK_STATE,
    TASK_STATE_SEGMENT, TaskStateSegment, UPPER_HALF_DIRECTORY_POINTERS, USER, USER_CODE, USER_DATA, WRITABLE,
    list_descriptors, to_gate, to_gate_privilege, to_index, to_interrupt_stack, to_kernel,
};

mod handler;
mod layout;
mod x87_image;

/// The guest memory KVM gives the machine.
type GuestMemory = <DirectKvm as Kvm>::GuestMemory;

/// The memory of machines that are dropped, which the next machine made takes before KVM is asked for more.
static UNUSED_MEMORY: Mutex<Vec<GuestMemory>> = Mutex::new(Vec::new());

/// Guest memory that goes to [`UNUSED_MEMORY`] when it is dropped, after the CPU and the machine before it.
struct PooledMemory(GuestMemory);

impl Drop for PooledMemory {
    fn drop(&mut self) {
        if let Some(whole) = self.0.part(0..self.0.len()) {
            UNUSED_MEMORY.lock().unwrap_or_else(PoisonError::into_inner).push(whole);
        }
    }
}

/// Gives the machine its memory: that of a machine dropped before, zeroed, or else fresh memory from KVM.
fn give_memory(machine: &VmFd) -> Result<GuestMemory, SetUpError> {
    let unused = UNUSED_MEMORY.lock().unwrap_or_else(PoisonError::into_inner).pop();
    let Some(memory) = unused else {
        return DirectKvm::set_user_memory_region(machine, 0, 0, MEMORY_BYTES as usize)
            .map_err(|error| refused("giving KVM the memory", error));
    };
    let zeros = vec![0; ZEROED_AT_ONCE];
    for start in (0..memory.len()).step_by(ZEROED_AT_ONCE) {
        let view = memory.part(start..start + ZEROED_AT_ONCE as u64).expect("inside guest memory");
        view.write(&zeros);
    }
    DirectKvm::set_user_memory_region_again(machine, 0, 0, &memory)
        .map_err(|error| refused("giving KVM the memory again", error))?;
    Ok(memory)
}

/// Bytes of guest memory zeroed by one copy when a machine takes the memory of one dropped before.
const ZEROED_AT_ONCE: usize = 1 << 20;
const _: () = assert!(MEMORY_BYTES.is_multiple_of(ZEROED_AT_ONCE as u64));

/// INCREMENTAL MEMORY: how the executor makes the guest hold a state's memory before each instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryLoading {
    /// Writes new tables and frames for every state.
    Full,
    /// Writes only where the memory the guest holds differs from the state's.
    Incremental,
    /// As incremental, then reads the whole of the guest's memory back and compares it with the state's.
    Checked,
}

/// ENTRY PATH: how the host starts the handler for the next state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryPath {
    /// Writes the handler's entry into RIP, RSP and RFLAGS before every run.
    Plain,
    /// Leaves the CPU halted at the end of the save, where it resumes into the entry.
    Parked,
}

/// The source's choices; the first value of each is the reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub loading: MemoryLoading,
    pub entry: EntryPath,
}

impl Options {
    pub const PLAIN: Self = Self {
        loading: MemoryLoading::Full,
        entry: EntryPath::Plain,
    };

    /// The options `X86_SINGLE_STEP_LOADING` (`full`, `incremental`, `checked`) and `X86_SINGLE_STEP_ENTRY` (`plain`,
    /// `parked`) name, the reference where unset.
    pub fn from_environment() -> Self {
        let read = |name: &str| std::env::var(name).ok();
        let loading = match read("X86_SINGLE_STEP_LOADING").as_deref() {
            None | Some("full") => MemoryLoading::Full,
            Some("incremental") => MemoryLoading::Incremental,
            Some("checked") => MemoryLoading::Checked,
            Some(other) => panic!("X86_SINGLE_STEP_LOADING is full, incremental or checked, not {other}"),
        };
        let entry = match read("X86_SINGLE_STEP_ENTRY").as_deref() {
            None | Some("plain") => EntryPath::Plain,
            Some("parked") => EntryPath::Parked,
            Some(other) => panic!("X86_SINGLE_STEP_ENTRY is plain or parked, not {other}"),
        };
        Self {
            loading,
            entry,
        }
    }
}

/// What went wrong while making the machine.
#[derive(Debug)]
pub struct SetUpError(pub String);

/// Why this source did not run a state.
///
/// Those up to `NotSteppable` are found before anything is written; after any other, the next state runs on a new
/// machine.
#[derive(Debug)]
pub enum Refusal {
    /// A permission starts or ends inside a page, which no chip can give.
    PermissionWithinAPage(Range<u64>),
    /// Memory outside the program's addresses, where the source keeps its own pages.
    OutsideTheProgramsAddresses(Range<u64>),
    /// RFLAGS sets a bit a program at ring 3 cannot set: TF, RF or a reserved bit.
    FlagsAProgramCannotSet(u64),
    /// The instruction pointer or a segment base is not canonical.
    NotCanonical(u64),
    /// MXCSR sets a reserved bit.
    MxcsrReservedBits(u32),
    /// The pages the state maps, each with its tables, are more than the arena has.
    MorePagesThanTheArenaHas { pages: u64, room: u64 },
    /// The instruction the source cannot step from ring 3, or whose result under Linux differs from the guest's.
    NotSteppable {
        instruction: String,
        address: u64,
        reason: &'static str,
    },
    /// A call to KVM failed, in KVM's words.
    KvmFailed(String),
    /// The guest stopped other than at the handler's halt.
    UnexpectedExit(String),
    /// The handler halted without saving the slot.
    SlotNotRun,
    /// The exception that ended the run did not come from ring 3, as the handler saved it.
    NotFromRing3 {
        vector: u64,
        error_code: u64,
        code_segment: u64,
        instruction_pointer: u64,
    },
    /// The state holds what the architecture does not (an x87 register wider than 80 bits), or the run changed state the
    /// processor does not model: IF, IOPL or VM.
    NotModeled(&'static str),
    /// A vector that does not end an instruction at ring 3.
    UnexpectedVector(u64),
    /// A page fault with a reserved bit, a key or a shadow stack in its error code.
    UnexpectedPageFault(u32),
}

/// A page of zeros, which clears guest memory.
static ZERO_PAGE: [u8; PAGE_BYTES as usize] = [0; PAGE_BYTES as usize];

// RFLAGS bits the source sets or checks, besides the flags a program changes.
const RESERVED_ONE: u64 = 1 << 1;
const TRAP: u64 = 1 << 8;
const INTERRUPT_ENABLE: u64 = 1 << 9;
const IO_PRIVILEGE: u64 = 0b11 << 12;
const RESUME: u64 = 1 << 16;
const VIRTUAL_8086: u64 = 1 << 17;
/// IA32_STAR, IA32_LSTAR and IA32_FMASK, and the RFLAGS bits SYSCALL clears.
const SYSTEM_CALL_TARGET: u32 = 0xC000_0081;
const LONG_MODE_SYSTEM_CALL_TARGET: u32 = 0xC000_0082;
const SYSTEM_CALL_FLAG_MASK: u32 = 0xC000_0084;
const SYSTEM_CALL_MASKED_FLAGS: u64 = TRAP | INTERRUPT_ENABLE | 1 << 10 | IO_PRIVILEGE | 1 << 14 | RESUME | 1 << 18;
/// R11, where SYSCALL saves RFLAGS.
const SAVED_FLAGS: usize = 11;
/// DR6.BS: the #DB is the trap after a single step.
const SINGLE_STEP: u64 = 1 << 14;
/// DR6 as reset.
const DEBUG_STATUS_RESET: u64 = 0xFFFF_0FF0;
const DEBUG_VECTOR: u64 = 1;
/// The page fault's error code bits.
const FAULT_PRESENT: u32 = 1 << 0;
const FAULT_WRITE: u32 = 1 << 1;
const FAULT_USER: u32 = 1 << 2;
const FAULT_INSTRUCTION_FETCH: u32 = 1 << 4;
const PAGE_FAULT_VECTOR: u64 = 14;

// Control registers: CR0's PE, MP, ET, NE, WP, AM and PG; CR4's PAE, PGE, OSFXSR, OSXMMEXCPT, UMIP, FSGSBASE, OSXSAVE
// and PKE; EFER's SCE, LME, LMA and NXE.
const CONTROL_0: u64 = 1 | 1 << 1 | 1 << 4 | 1 << 5 | 1 << 16 | 1 << 18 | 1 << 31;
const CONTROL_4: u64 = 1 << 5 | 1 << 7 | 1 << 9 | 1 << 10 | 1 << 11 | 1 << 16 | 1 << 18 | 1 << 22;
const EXTENDED_FEATURES: u64 = 1 | 1 << 8 | 1 << 10 | 1 << 11;

/// A writable page of the state: where the guest keeps it, and its bytes before the instruction.
struct WritablePage {
    page: u64,
    frame: u64,
    before: Range<usize>,
}

/// The source: one virtual machine with one virtual CPU.
///
/// The fields drop in order: the CPU, then the machine, then the memory, which goes to the next machine made.
pub struct SingleStepMachine {
    cpu: VcpuFd,
    _machine: VmFd,
    memory: PooledMemory,
    /// A page the read-back copies a writable page into.
    page: Vec<u8>,
    handler: Handler,
    options: Options,
    /// ENTRY PATH: whether the CPU is halted at the end of the save.
    parked: bool,
    next_free_page: u64,
    writable: Vec<WritablePage>,
    bytes_before: Vec<u8>,
    resident: Resident,
    /// The special registers of the handler.
    kernel_sregs: kvm_sregs,
    pub time_loading: Duration,
    pub time_setting_registers: Duration,
    pub time_in_guest: Duration,
    pub time_reading_back: Duration,
    pub runs: u64,
    /// INCREMENTAL MEMORY: the states whose prepared memory differed from the state's, in checked loading.
    pub check_mismatches: u64,
    /// Set during a run and after a failed one, when the CPU may be anywhere: the next state makes a new machine.
    renew_machine: bool,
}

/// INCREMENTAL MEMORY: what the guest holds between states when loading incrementally.
struct Resident {
    memory: Held,
    /// The frame of each virtual page the guest has held bytes or a permission on; any other page holds zeros.
    frames: BTreeMap<u64, u64>,
    next_free_page: u64,
}

/// INCREMENTAL MEMORY: whether the executor knows what the guest's resident frames and tables hold.
enum Held {
    Known(Vec<MemoryDelta<'static, X86_64>>),
    /// From the first write of a load until its read-back is done, and before the first load.
    Unknown,
}

fn refused(what: &str, error: impl std::fmt::Display) -> SetUpError {
    SetUpError(format!("{what}: {error}"))
}

impl SingleStepMachine {
    /// Makes the machine with the options the environment names.
    pub fn new() -> Result<Self, SetUpError> {
        Self::with_options(Options::from_environment())
    }

    pub fn with_options(options: Options) -> Result<Self, SetUpError> {
        let kvm = KvmSystem::new().map_err(|error| refused("opening KVM", error))?;
        let machine = kvm.create_vm().map_err(|error| refused("making the machine", error))?;
        machine
            .set_tss_address(0xFFFB_D000)
            .map_err(|error| refused("placing KVM's real mode task state", error))?;
        // KVM would rewrite a guest's VMMCALL into VMCALL in the guest's own code; without the quirk it raises #UD, as
        // an Intel chip does.
        let quirks = kvm_enable_cap { cap: KVM_CAP_DISABLE_QUIRKS2, args: [u64::from(KVM_X86_QUIRK_FIX_HYPERCALL_INSN), 0, 0, 0], ..kvm_enable_cap::default() };
        machine.enable_cap(&quirks).map_err(|error| refused("disabling KVM's hypercall rewriting", error))?;
        let memory = PooledMemory(give_memory(&machine)?);
        let cpu = machine.create_vcpu(0).map_err(|error| refused("making a virtual CPU", error))?;
        let mut cpuid = kvm
            .get_supported_cpuid(KVM_MAX_CPUID_ENTRIES)
            .map_err(|error| refused("reading the supported CPUID", error))?;
        // KVM gives the APIC ID of the host CPU that answered; the guest's one CPU has APIC ID 0.
        for entry in cpuid.as_mut_slice() {
            match entry.function {
                1 => entry.ebx &= 0x00FF_FFFF,
                0xB | 0x1F => entry.edx = 0,
                _ => {}
            }
        }
        cpu.set_cpuid2(&cpuid).map_err(|error| refused("setting CPUID", error))?;
        let handler = handler::assemble();
        // SYSCALL enters the handler's own entry on the kernel's code segment, with TF, IF, DF, IOPL, NT, RF and AC
        // masked so that nothing steps or interrupts it.
        let entries: Vec<kvm_msr_entry> = [
            (SYSTEM_CALL_TARGET, KERNEL_CODE_SELECTOR << 32),
            (LONG_MODE_SYSTEM_CALL_TARGET, handler.system_call),
            (SYSTEM_CALL_FLAG_MASK, SYSTEM_CALL_MASKED_FLAGS),
        ]
        .map(|(index, data)| kvm_msr_entry { index, data, ..kvm_msr_entry::default() })
        .to_vec();
        let msrs = Msrs::from_entries(&entries).map_err(|error| refused("listing SYSCALL's MSRs", format!("{error:?}")))?;
        cpu.set_msrs(&msrs).map_err(|error| refused("setting SYSCALL's MSRs", error))?;
        let mut executor = Self {
            cpu,
            _machine: machine,
            memory,
            page: vec![0; PAGE_BYTES as usize],
            handler,
            options,
            parked: false,
            next_free_page: ARENA.start,
            writable: Vec::new(),
            bytes_before: Vec::new(),
            resident: Resident {
                memory: Held::Unknown,
                frames: BTreeMap::new(),
                next_free_page: RESIDENT.start + PAGE_BYTES,
            },
            kernel_sregs: kvm_sregs::default(),
            time_loading: Duration::ZERO,
            time_setting_registers: Duration::ZERO,
            time_in_guest: Duration::ZERO,
            time_reading_back: Duration::ZERO,
            runs: 0,
            check_mismatches: 0,
            renew_machine: false,
        };
        executor.set_up()?;
        Ok(executor)
    }

    /// Replaces the machine with a new one of the same options.
    fn renew(&mut self) -> Result<(), Refusal> {
        *self = Self::with_options(self.options).map_err(|SetUpError(error)| Refusal::KvmFailed(error))?;
        Ok(())
    }

    /// PROBE-COST TIMING: sets every time and count kept so far to zero.
    pub fn reset_times(&mut self) {
        self.time_loading = Duration::ZERO;
        self.time_setting_registers = Duration::ZERO;
        self.time_in_guest = Duration::ZERO;
        self.time_reading_back = Duration::ZERO;
        self.runs = 0;
    }

    /// Changes how the next states are run; what the guest holds stays known.
    pub fn set_options(&mut self, options: Options) {
        self.options = options;
    }

    pub fn options(&self) -> Options {
        self.options
    }

    /// Writes the descriptor tables, the task state segment, the handler and the kernel's page tables, and puts
    /// the CPU at ring 0 in long mode.
    fn set_up(&mut self) -> Result<(), SetUpError> {
        let descriptors = list_descriptors();
        self.write_bytes(GLOBAL_DESCRIPTOR_TABLE, descriptors.as_bytes());
        let task_state = TaskStateSegment {
            privilege_stacks: [to_kernel(STACK_TOP), 0, 0],
            interrupt_stacks: [to_kernel(FAULT_STACK_TOP), 0, 0, 0, 0, 0, 0],
            io_map_base: size_of::<TaskStateSegment>() as u16,
            ..TaskStateSegment::default()
        };
        self.write_bytes(TASK_STATE_SEGMENT, task_state.as_bytes());
        for vector in 0..VECTORS as u64 {
            let gate = to_gate(
                self.handler.stubs[vector as usize],
                to_gate_privilege(vector),
                to_interrupt_stack(vector),
            );
            self.write_bytes(INTERRUPT_DESCRIPTOR_TABLE + vector * 16, gate.as_bytes());
        }
        let code = self.handler.bytes.clone();
        self.write_bytes(HANDLER, &code);
        // The kernel's pages, supervisor only and global; only the handler's may be executed.
        for page in 0..KERNEL_PAGES {
            let physical = page * PAGE_BYTES;
            let executable = (HANDLER..HANDLER + HANDLER_BYTES).contains(&physical);
            let fetch = if executable { 0 } else { NO_EXECUTE };
            let entry = physical | PRESENT | WRITABLE | ACCESSED | DIRTY | GLOBAL | fetch;
            self.write_entry(KERNEL_TABLE + to_index(to_kernel(physical), LEVELS - 1) * ENTRY_BYTES, entry);
        }
        let table = PRESENT | WRITABLE | ACCESSED;
        self.write_entry(KERNEL_DIRECTORY + (ENTRIES - 1) * ENTRY_BYTES, KERNEL_TABLE | table);
        self.write_entry(KERNEL_DIRECTORY_POINTERS + (ENTRIES - 1) * ENTRY_BYTES, KERNEL_DIRECTORY | table);
        // Every other address of the upper half is a present, supervisor-only, read-only page, as the architecture
        // has the kernel's half: 1 GiB, 2 MiB and 4 KiB pages of frame 0, which ring 3 can never reach.
        let filler = PRESENT | ACCESSED | NO_EXECUTE;
        for index in 0..ENTRIES - 1 {
            self.write_entry(UPPER_HALF_DIRECTORY_POINTERS + index * ENTRY_BYTES, filler | LARGE_PAGE);
            self.write_entry(KERNEL_DIRECTORY_POINTERS + index * ENTRY_BYTES, filler | LARGE_PAGE);
            self.write_entry(KERNEL_DIRECTORY + index * ENTRY_BYTES, filler | LARGE_PAGE);
        }
        self.write_entry(UPPER_HALF_DIRECTORY_POINTERS + (ENTRIES - 1) * ENTRY_BYTES, filler | LARGE_PAGE);
        for index in KERNEL_PAGES..ENTRIES {
            self.write_entry(KERNEL_TABLE + index * ENTRY_BYTES, filler);
        }
        self.reset_resident();
        let empty = self.make_top_table(ARENA.start);
        self.write_slot_page_table(empty);

        let mut sregs = self
            .cpu
            .get_sregs()
            .map_err(|error| refused("reading the special registers", error))?;
        let segment = |selector: u16, type_: u8, long: u8, privilege: u8| kvm_segment {
            base: 0,
            limit: 0xFFFF_FFFF,
            selector,
            type_,
            present: 1,
            dpl: privilege,
            db: 1 - long,
            s: 1,
            l: long,
            g: 1,
            avl: 0,
            unusable: 0,
            padding: 0,
        };
        let unused = kvm_segment {
            unusable: 1,
            ..kvm_segment::default()
        };
        sregs.cs = segment(KERNEL_CODE, 0xB, 1, 0);
        sregs.ss = segment(KERNEL_DATA, 0x3, 0, 0);
        (sregs.ds, sregs.es, sregs.fs, sregs.gs) = (unused, unused, unused, unused);
        sregs.tr = kvm_segment {
            base: to_kernel(TASK_STATE_SEGMENT),
            limit: size_of::<TaskStateSegment>() as u32 - 1,
            selector: TASK_STATE,
            type_: 0xB,
            present: 1,
            ..kvm_segment::default()
        };
        sregs.ldt = unused;
        sregs.gdt.base = to_kernel(GLOBAL_DESCRIPTOR_TABLE);
        sregs.gdt.limit = (DESCRIPTOR_ENTRIES * 8 - 1) as u16;
        sregs.idt.base = to_kernel(INTERRUPT_DESCRIPTOR_TABLE);
        sregs.idt.limit = (VECTORS * 16 - 1) as u16;
        sregs.cr0 = CONTROL_0;
        sregs.cr3 = empty;
        sregs.cr4 = CONTROL_4;
        sregs.efer = EXTENDED_FEATURES;
        self.cpu
            .set_sregs(&sregs)
            .map_err(|error| refused("entering long mode", error))?;
        let mut extended = kvm_xcrs {
            nr_xcrs: 1,
            ..kvm_xcrs::default()
        };
        extended.xcrs[0] = kvm_xcr {
            xcr: 0,
            value: EXTENDED_CONTROL_0,
            ..kvm_xcr::default()
        };
        self.cpu
            .set_xcrs(&extended)
            .map_err(|error| refused("setting XCR0", error))?;
        self.kernel_sregs = sregs;
        Ok(())
    }

    /// Writes a top-level table at `at` that maps the kernel's half and nothing else.
    fn make_top_table(&mut self, at: u64) -> u64 {
        let table = PRESENT | WRITABLE | ACCESSED;
        let mut entries = [0; ENTRIES as usize];
        entries[(ENTRIES / 2) as usize..KERNEL_ENTRY as usize].fill(UPPER_HALF_DIRECTORY_POINTERS | table);
        entries[KERNEL_ENTRY as usize] = KERNEL_DIRECTORY_POINTERS | table;
        // One copy of the whole table: guest memory is reached only by copies, each with a cost of its own.
        self.write_bytes(at, entries.as_bytes());
        at
    }

    fn write_slot_page_table(&mut self, top: u64) {
        let at = SLOT + offset_of!(Slot, page_table) as u64;
        self.write_bytes(at, top.as_bytes());
    }

    /// Writes the state's memory and registers into the guest, fully or incrementally.
    fn load(&mut self, processor: &Processor, memory: &[MemoryDelta<'_, X86_64>]) -> Option<Vec<MemoryDelta<'static, X86_64>>> {
        let started = Instant::now();
        self.writable.clear();
        self.bytes_before.clear();
        let held = match self.options.loading {
            MemoryLoading::Full => {
                self.load_fully(memory);
                None
            }
            MemoryLoading::Incremental | MemoryLoading::Checked => Some(self.prepare(memory)),
        };
        let slot = to_slot(processor, self.read_page_table());
        // The handler reads the image before (up to its last component loaded) and the fields up to the registers
        // before; it writes the rest.
        let end = to_image_end(to_loaded_components(processor));
        self.write_bytes(SLOT, &slot.before_xsave.as_bytes()[..end]);
        let inputs = offset_of!(Slot, page_table)..offset_of!(Slot, after);
        self.write_bytes(SLOT + inputs.start as u64, &slot.as_bytes()[inputs]);
        self.time_loading += started.elapsed();
        held
    }

    fn load_fully(&mut self, memory: &[MemoryDelta<'_, X86_64>]) {
        self.next_free_page = ARENA.start;
        let top = self.allocate();
        self.make_top_table(top);
        for (page, permission) in to_page_permissions(memory) {
            let PagePermission::Mapped {
                data_access,
                instruction_fetch,
            } = permission
            else {
                continue;
            };
            let frame = self.allocate();
            let mut bytes = vec![0; PAGE_BYTES as usize];
            compose_page(page, memory, &mut bytes);
            self.write_bytes(frame, &bytes);
            if data_access == DataAccess::ReadWrite {
                self.remember_writable(page, frame, &bytes);
            }
            let mut next_free_page = self.next_free_page;
            let leaf = self.find_leaf_entry(&mut next_free_page, top, page);
            self.next_free_page = next_free_page;
            assert!(self.next_free_page <= ARENA.end, "one state's pages fit the arena");
            self.write_entry(leaf, to_page_entry(frame, data_access, instruction_fetch));
        }
        self.write_slot_page_table(top);
    }

    fn remember_writable(&mut self, page: u64, frame: u64, bytes: &[u8]) {
        let start = self.bytes_before.len();
        self.bytes_before.extend_from_slice(bytes);
        self.writable.push(WritablePage {
            page,
            frame,
            before: start..self.bytes_before.len(),
        });
    }

    /// INCREMENTAL MEMORY: makes the guest hold the state, writing memory only where it differs from what the guest
    /// holds. Gives the state's memory, which the caller stores as what the guest holds once the read-back is done.
    fn prepare(&mut self, memory: &[MemoryDelta<'_, X86_64>]) -> Vec<MemoryDelta<'static, X86_64>> {
        let mut held = match std::mem::replace(&mut self.resident.memory, Held::Unknown) {
            Held::Known(held) => held,
            Held::Unknown => {
                self.reset_resident();
                Vec::new()
            }
        };
        MemorySnapshot { mutations: &held }.diff(&MemorySnapshot { mutations: memory }, &mut ResidentWriter(self));
        for (page, permission) in to_page_permissions(memory) {
            if let PagePermission::Mapped {
                data_access: DataAccess::ReadWrite,
                ..
            } = permission
            {
                let frame = self.resident.frames[&page];
                let bytes = self.read_bytes(frame, PAGE_BYTES);
                self.remember_writable(page, frame, &bytes);
            }
        }
        held.clear();
        held.extend(memory.iter().cloned().map(MemoryDelta::into_owned));
        self.write_slot_page_table(RESIDENT.start);
        held
    }

    /// INCREMENTAL MEMORY: makes the resident pages hold nothing, as an empty list of changes leaves memory.
    fn reset_resident(&mut self) {
        self.make_top_table(RESIDENT.start);
        self.resident.frames.clear();
        self.resident.next_free_page = RESIDENT.start + PAGE_BYTES;
    }

    fn write_resident_bytes(&mut self, start: u64, bytes: &[u8]) {
        let mut done = 0;
        while done < bytes.len() {
            let address = start + done as u64;
            let page = address & !(PAGE_BYTES - 1);
            let length = ((page + PAGE_BYTES - address) as usize).min(bytes.len() - done);
            let frame = self.find_frame(page);
            self.write_bytes(frame + (address - page), &bytes[done..done + length]);
            done += length;
        }
    }

    fn write_resident_permission(&mut self, addresses: &Range<u64>, permission: PagePermission) {
        for page in to_pages(addresses) {
            let entry = match permission {
                PagePermission::Mapped {
                    data_access,
                    instruction_fetch,
                } => to_page_entry(self.find_frame(page), data_access, instruction_fetch),
                PagePermission::Unmapped => 0,
            };
            let mut next_free_page = self.resident.next_free_page;
            let leaf = self.find_leaf_entry(&mut next_free_page, RESIDENT.start, page);
            assert!(next_free_page <= RESIDENT.end, "the resident pages fit");
            self.resident.next_free_page = next_free_page;
            self.write_entry(leaf, entry);
        }
    }

    fn find_frame(&mut self, page: u64) -> u64 {
        if let Some(&frame) = self.resident.frames.get(&page) {
            return frame;
        }
        let frame = self.resident.next_free_page;
        self.resident.next_free_page += PAGE_BYTES;
        assert!(frame + PAGE_BYTES <= RESIDENT.end, "the resident pages fit");
        self.fill_zeros(frame, PAGE_BYTES);
        self.resident.frames.insert(page, frame);
        frame
    }

    /// INCREMENTAL MEMORY: reads the guest's whole resident memory back, its tables and every frame, and gives whether
    /// it holds exactly the state's bytes and permissions.
    fn check_guest_holds(&self, memory: &[MemoryDelta<'_, X86_64>]) -> bool {
        let mut same = true;
        let mut bytes = vec![0; PAGE_BYTES as usize];
        let mut pages: Vec<u64> = memory
            .iter()
            .filter_map(|delta| match delta {
                MemoryDelta::ValueAssignment { start, bytes } if !bytes.is_empty() => {
                    Some(to_pages(&((*start & !(PAGE_BYTES - 1))..*start + bytes.len() as u64)).collect::<Vec<_>>())
                }
                _ => None,
            })
            .flatten()
            .chain(self.resident.frames.keys().copied())
            .collect();
        pages.sort_unstable();
        pages.dedup();
        for page in pages {
            compose_page(page, memory, &mut bytes);
            let held = match self.resident.frames.get(&page) {
                Some(&frame) => self.read_bytes(frame, PAGE_BYTES),
                None => ZERO_PAGE.to_vec(),
            };
            if held != bytes {
                same = false;
            }
        }
        let mut leaves = BTreeMap::new();
        self.collect_leaves(RESIDENT.start, 0, 0, &mut leaves);
        let permissions = to_page_permissions(memory);
        let wanted: BTreeMap<u64, u64> = permissions
            .iter()
            .filter_map(|(&page, permission)| match *permission {
                PagePermission::Unmapped => None,
                PagePermission::Mapped {
                    data_access,
                    instruction_fetch,
                } => Some((
                    page,
                    to_page_entry(*self.resident.frames.get(&page)?, data_access, instruction_fetch),
                )),
            })
            .collect();
        let mapped = permissions
            .values()
            .filter(|permission| **permission != PagePermission::Unmapped)
            .count();
        same && leaves == wanted && wanted.len() == mapped
    }

    /// INCREMENTAL MEMORY: every present last-level entry of the lower half under a table, by the page it maps.
    fn collect_leaves(&self, table: u64, level: u32, base: u64, leaves: &mut BTreeMap<u64, u64>) {
        let entries = if level == 0 { ENTRIES / 2 } else { ENTRIES };
        for index in 0..entries {
            let entry = self.read_entry(table + index * ENTRY_BYTES);
            if entry & PRESENT == 0 {
                continue;
            }
            let address = base | index << (PAGE_BYTES.trailing_zeros() + (LEVELS - 1 - level) * 9);
            if level == LEVELS - 1 {
                leaves.insert(address, entry);
            } else {
                self.collect_leaves(entry & ADDRESS_MASK, level + 1, address, leaves);
            }
        }
    }

    /// Runs the loaded slot to the handler's halt.
    fn run(&mut self) -> Result<(), Refusal> {
        let setting = Instant::now();
        // Until the halt, the CPU is wherever the run took it.
        self.renew_machine = true;
        if !(self.options.entry == EntryPath::Parked && self.parked) {
            self.enter_handler()?;
        }
        self.parked = false;
        self.time_setting_registers += setting.elapsed();
        let started = Instant::now();
        self.runs += 1;
        loop {
            match self.cpu.run() {
                Ok(VcpuExit::Hlt) => self.parked = true,
                Ok(other) => return Err(Refusal::UnexpectedExit(format!("{other:?}"))),
                // A signal ends a run too: then the CPU runs on.
                Err(error) if io::Error::from_raw_os_error(error.errno()).kind() == io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(error) => return Err(Refusal::KvmFailed(format!("KVM_RUN: {error}"))),
            }
            break;
        }
        self.renew_machine = false;
        self.time_in_guest += started.elapsed();
        Ok(())
    }

    /// PROBE-COST TIMING: runs a bare halt at ring 0 `count` times, one KVM_RUN and exit each, and gives their time.
    pub fn time_bare_runs(&mut self, count: u32) -> Duration {
        self.cpu
            .set_sregs(&self.kernel_sregs)
            .unwrap_or_else(|error| panic!("KVM refused the handler's segments: {error}"));
        let regs = kvm_regs {
            rip: self.handler.floor,
            rsp: to_kernel(STACK_TOP),
            rflags: RESERVED_ONE,
            ..kvm_regs::default()
        };
        self.cpu
            .set_regs(&regs)
            .unwrap_or_else(|error| panic!("KVM refused the bare halt: {error}"));
        let started = Instant::now();
        for _ in 0..count {
            match self.cpu.run() {
                Ok(VcpuExit::Hlt) => {}
                other => panic!("the bare halt stopped otherwise: {other:?}"),
            }
        }
        let elapsed = started.elapsed();
        self.parked = false;
        elapsed
    }

    fn enter_handler(&mut self) -> Result<(), Refusal> {
        let regs = kvm_regs {
            rip: self.handler.entry,
            rsp: to_kernel(STACK_TOP),
            rflags: RESERVED_ONE,
            ..kvm_regs::default()
        };
        self.cpu
            .set_regs(&regs)
            .map_err(|error| Refusal::KvmFailed(format!("the handler's entry: {error}")))
    }

    /// Gives the processor after the run, and each run of bytes it changed; writes neither when the run is refused.
    fn read_back(
        &mut self,
        processor: &mut Processor,
        mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>),
    ) -> Result<(), Refusal> {
        let started = Instant::now();
        let slot = self.read_slot_outputs();
        *processor = to_processor(&slot).inspect_err(|_| self.renew_machine = true)?;
        let mut after = std::mem::take(&mut self.page);
        for writable in &self.writable {
            let before = &self.bytes_before[writable.before.clone()];
            self.read_into(writable.frame, &mut after);
            report_changed_bytes(writable.page, before, &after, &mut on_memory_mutated);
        }
        self.page = after;
        self.time_reading_back += started.elapsed();
        Ok(())
    }

    /// Clears TF in the image a completed PUSHF wrote: the source sets TF to step, where a program's PUSHF pushes it
    /// clear.
    fn hide_pushed_trap_flag(&mut self, processor: &Processor, memory: &[MemoryDelta<'_, X86_64>]) {
        let bytes: [u8; 15] = read_bytes(memory, processor.instruction_pointer);
        let decoded = Decoder::with_ip(64, &bytes, processor.instruction_pointer, DecoderOptions::NONE).decode();
        let size = match decoded.code() {
            Code::Pushfw => 2,
            Code::Pushfq => 8,
            _ => return,
        };
        let vector = self.read_entry(SLOT + offset_of!(Slot, vector) as u64);
        let status = self.read_entry(SLOT + offset_of!(Slot, debug_status) as u64);
        if !(vector == DEBUG_VECTOR && status & SINGLE_STEP != 0) {
            return;
        }
        // TF is bit 8 of the image, bit 0 of its second byte.
        let address = processor.general_purpose_registers[STACK_POINTER].wrapping_sub(size).wrapping_add(1);
        let page = address & !(PAGE_BYTES - 1);
        if let Some(frame) = self.writable.iter().find(|writable| writable.page == page).map(|writable| writable.frame) {
            let at = frame + (address - page);
            let byte = self.read_bytes(at, 1)[0];
            self.write_bytes(at, &[byte & !1]);
        }
    }

    /// The address of the last-level entry of a page under a top-level table, making the tables that lead to it.
    fn find_leaf_entry(&mut self, next_free_page: &mut u64, top: u64, page: u64) -> u64 {
        let mut table = top;
        for level in 0..LEVELS - 1 {
            let at = table + to_index(page, level) * ENTRY_BYTES;
            let entry = self.read_entry(at);
            table = if entry & PRESENT == 0 {
                let own = *next_free_page;
                *next_free_page += PAGE_BYTES;
                self.fill_zeros(own, PAGE_BYTES);
                self.write_entry(at, own | PRESENT | WRITABLE | USER | ACCESSED);
                own
            } else {
                entry & ADDRESS_MASK
            };
        }
        table + to_index(page, LEVELS - 1) * ENTRY_BYTES
    }

    fn allocate(&mut self) -> u64 {
        let page = self.next_free_page;
        self.next_free_page += PAGE_BYTES;
        page
    }

    fn read_page_table(&self) -> u64 {
        self.read_entry(SLOT + offset_of!(Slot, page_table) as u64)
    }

    /// The slot as a run leaves it, as far as [`to_processor`] reads it: the image before's legacy area up to the
    /// vector registers, the image after, and the fields from the vector on.
    fn read_slot_outputs(&self) -> Slot {
        let mut slot = Slot::new_zeroed();
        let after = offset_of!(Slot, after_xsave);
        let bytes = slot.as_mut_bytes();
        for range in [0..FX_VECTOR_REGISTERS, after..after + to_image_end(0), offset_of!(Slot, vector)..size_of::<Slot>()] {
            self.read_into(SLOT + range.start as u64, &mut bytes[range]);
        }
        // The AVX-512 areas only where the header says XSAVE wrote them.
        let header = u64::from_le_bytes(bytes[after + XSAVE_HEADER..after + XSAVE_HEADER + 8].try_into().expect("eight bytes"));
        let end = to_image_end(header);
        if end > to_image_end(0) {
            let range = after + to_image_end(0)..after + end;
            self.read_into(SLOT + range.start as u64, &mut bytes[range]);
        }
        slot
    }

    fn read_entry(&self, at: u64) -> u64 {
        let mut entry = [0; ENTRY_BYTES as usize];
        self.read_into(at, &mut entry);
        u64::from_le_bytes(entry)
    }

    fn write_entry(&mut self, at: u64, entry: u64) {
        self.write_bytes(at, entry.as_bytes());
    }

    /// Copies the guest's bytes from `at` into the buffer, which it fills.
    fn read_into(&self, at: u64, into: &mut [u8]) {
        let view = self.memory.0.part(at..at + into.len() as u64).expect("inside guest memory");
        view.read(into);
    }

    fn read_bytes(&self, at: u64, length: u64) -> Vec<u8> {
        let mut bytes = vec![0; length as usize];
        self.read_into(at, &mut bytes);
        bytes
    }

    fn write_bytes(&mut self, at: u64, bytes: &[u8]) {
        let view = self.memory.0.part(at..at + bytes.len() as u64).expect("inside guest memory");
        view.write(bytes);
    }

    fn fill_zeros(&mut self, at: u64, length: u64) {
        self.write_bytes(at, &ZERO_PAGE[..length as usize]);
    }
}

/// The hook that writes each change it receives into the resident pages, which then hold the memory it leads to.
struct ResidentWriter<'machine>(&'machine mut SingleStepMachine);

impl MemorySink<X86_64> for ResidentWriter<'_> {
    fn on_memory_mutated(&mut self, mutation: MemoryDelta<'_, X86_64>) {
        match mutation {
            MemoryDelta::ValueAssignment { start, bytes } => self.0.write_resident_bytes(start, &bytes),
            MemoryDelta::PermissionAssignment { addresses, permission } => {
                self.0.write_resident_permission(&addresses, permission)
            }
        }
    }
}

impl BehaviorSource for SingleStepMachine {
    type Architecture = X86_64;
    type Error = Refusal;

    /// Reports each run of bytes whose value changed: a store of the bytes already there is not reported.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor,
        memory: MemorySnapshot<'_, '_, X86_64>,
        mutation_hook: &mut impl MemorySink<X86_64>,
    ) -> Result<(), Refusal> {
        let memory = memory.mutations;
        if self.renew_machine {
            self.renew()?;
        }
        check_holdable(processor, memory)?;
        check_steppable(processor, memory)?;
        let held = self.load(processor, memory);
        let Some(mut held) = held else {
            self.run()?;
            self.hide_pushed_trap_flag(processor, memory);
            return self.read_back(processor, |mutation| mutation_hook.on_memory_mutated(mutation));
        };
        if self.options.loading == MemoryLoading::Checked && !self.check_guest_holds(memory) {
            self.check_mismatches += 1;
        }
        self.run()?;
        self.hide_pushed_trap_flag(processor, memory);
        self.read_back(processor, |mutation| {
            held.push(mutation.clone().into_owned());
            mutation_hook.on_memory_mutated(mutation);
        })?;
        self.resident.memory = Held::Known(held);
        Ok(())
    }
}

/// The slot that loads the processor under the page table.
fn to_slot(processor: &Processor, page_table: u64) -> Slot {
    let registers = Registers {
        general: processor.general_purpose_registers,
        instruction_pointer: processor.instruction_pointer,
        flags: processor.flags.bits() | RESERVED_ONE | INTERRUPT_ENABLE | TRAP,
        code_segment: u64::from(USER_CODE),
        stack_segment: u64::from(USER_DATA),
        fs_base: processor.fs_base,
        gs_base: processor.gs_base,
        data_segments: processor.data_segments.map(u64::from),
    };
    let mut image = XsaveImage { bytes: [0; XSAVE_IMAGE_BYTES] };
    x87_image::write_x87(&mut image.bytes, &processor.x87);
    image.bytes[FX_MXCSR..FX_MXCSR + 4].copy_from_slice(&processor.mxcsr.to_le_bytes());
    for (number, value) in processor.vector_registers.iter().enumerate() {
        let at = FX_VECTOR_REGISTERS + number * VECTOR_HALF_BYTES;
        image.bytes[at..at + VECTOR_HALF_BYTES].copy_from_slice(&value.to_le_bytes());
    }
    for (number, value) in processor.upper_vector_halves.iter().enumerate() {
        let at = XSAVE_AVX + number * VECTOR_HALF_BYTES;
        image.bytes[at..at + VECTOR_HALF_BYTES].copy_from_slice(&value.to_le_bytes());
    }
    for (number, value) in processor.opmask_registers.iter().enumerate() {
        let at = XSAVE_OPMASK + number * size_of::<u64>();
        image.bytes[at..at + size_of::<u64>()].copy_from_slice(&value.to_le_bytes());
    }
    let parts = processor.zmm_upper_halves.iter().flatten().map(|part| (XSAVE_ZMM_HIGH_256, part));
    let high = processor.high_vector_registers.iter().flatten().map(|part| (XSAVE_HIGH_16_ZMM, part));
    for (index, (area, value)) in parts.enumerate().chain(high.enumerate()) {
        let at = area + index * VECTOR_HALF_BYTES;
        image.bytes[at..at + VECTOR_HALF_BYTES].copy_from_slice(&value.to_le_bytes());
    }
    // Every saved component is loaded from the image rather than put in its initial state, but an AVX-512 component
    // that holds zeros, whose initial state is the same.
    image.bytes[XSAVE_PKRU..XSAVE_PKRU + size_of::<u32>()].copy_from_slice(&processor.pkru.to_le_bytes());
    image.bytes[XSAVE_HEADER..XSAVE_HEADER + size_of::<u64>()].copy_from_slice(&to_loaded_components(processor).to_le_bytes());
    Slot {
        before_xsave: image,
        after_xsave: image,
        page_table,
        vector: NOT_RUN,
        error_code: 0,
        fault_address: 0,
        debug_status: 0,
        before: registers,
        after: registers,
        reserved: [0; 7],
    }
}

/// The components the handler loads from the image: every saved one but each AVX-512 component that holds zeros.
fn to_loaded_components(processor: &Processor) -> u64 {
    let zero = |held: bool, component: u64| if held { 0 } else { component };
    SAVED_COMPONENTS
        & !zero(processor.opmask_registers.iter().any(|value| *value != 0), OPMASK_COMPONENT)
        & !zero(processor.zmm_upper_halves.iter().flatten().any(|part| *part != 0), ZMM_HIGH_256_COMPONENT)
        & !zero(processor.high_vector_registers.iter().flatten().any(|part| *part != 0), HIGH_16_ZMM_COMPONENT)
}

/// The bytes of the image up to the end of the last component the header names, AVX's at least.
fn to_image_end(components: u64) -> usize {
    match components {
        _ if components & PKRU_COMPONENT != 0 => XSAVE_PKRU + size_of::<u64>(),
        _ if components & HIGH_16_ZMM_COMPONENT != 0 => XSAVE_PKRU,
        _ if components & ZMM_HIGH_256_COMPONENT != 0 => XSAVE_HIGH_16_ZMM,
        _ if components & OPMASK_COMPONENT != 0 => XSAVE_ZMM_HIGH_256,
        _ => XSAVE_AVX + VECTOR_REGISTERS * VECTOR_HALF_BYTES,
    }
}

/// Refuses, before anything is written, a state the architecture does not have or the source cannot hold.
///
/// # Errors
///
/// When the state gives a permission or bytes outside the program's addresses, where the source keeps its own
/// pages, or a permission to part of a page; when its RFLAGS sets a bit a program at ring 3 cannot set (TF, RF, a
/// reserved bit); when its instruction pointer or a segment base is not canonical or its MXCSR sets a reserved bit;
/// or when its pages do not fit the arena.
fn check_holdable(processor: &Processor, memory: &[MemoryDelta<'_, X86_64>]) -> Result<(), Refusal> {
    for delta in memory {
        let addresses = match delta {
            MemoryDelta::PermissionAssignment { addresses, .. } => {
                if !(addresses.start.is_multiple_of(PAGE_BYTES) && addresses.end.is_multiple_of(PAGE_BYTES)) {
                    return Err(Refusal::PermissionWithinAPage(addresses.clone()));
                }
                addresses.clone()
            }
            MemoryDelta::ValueAssignment { start, bytes } => {
                *start..start.saturating_add(bytes.len() as u64)
            }
        };
        let addressable = X86_64::ADDRESSABLE;
        if !(addresses.is_empty() || (addressable.start <= addresses.start && addresses.end <= addressable.end)) {
            return Err(Refusal::OutsideTheProgramsAddresses(addresses));
        }
    }
    if !Flags::all().contains(processor.flags) {
        return Err(Refusal::FlagsAProgramCannotSet(processor.flags.bits()));
    }
    for address in [processor.instruction_pointer, processor.fs_base, processor.gs_base] {
        if !is_canonical(address) {
            return Err(Refusal::NotCanonical(address));
        }
    }
    if processor.mxcsr > u32::from(u16::MAX) {
        return Err(Refusal::MxcsrReservedBits(processor.mxcsr));
    }
    if processor.code_segment != CODE_SEGMENT {
        return Err(Refusal::NotModeled("CS other than Linux's 64-bit code segment, which every state holds"));
    }
    if !processor.data_segments.iter().all(|selector| is_loadable(*selector)) {
        return Err(Refusal::NotModeled("a data segment selector no program can load under Linux"));
    }
    if !x87_image::is_holdable(&processor.x87) {
        return Err(Refusal::NotModeled(
            "an x87 register wider than 80 bits, an opcode wider than 11 bits, a reserved bit of FCW, or FSW's ES or B not an unmasked flag",
        ));
    }
    let mapped = to_page_permissions(memory)
        .values()
        .filter(|permission| **permission != PagePermission::Unmapped)
        .count() as u64;
    // The top-level table, and per page a frame and at most one table of each lower level.
    let room = (ARENA.end - ARENA.start) / PAGE_BYTES;
    if mapped * u64::from(LEVELS) >= room {
        return Err(Refusal::MorePagesThanTheArenaHas {
            pages: mapped * u64::from(LEVELS),
            room,
        });
    }
    Ok(())
}

/// Refuses, before anything is written, the instructions the source cannot step from ring 3 with the trap flag.
///
/// # Errors
///
/// For a MOV SS that loads SS (it steps two instructions) and SYSENTER (it enters ring 0 at MSRs the source does not
/// set as Linux does). VMCALL runs, which KVM answers as a hypercall, and VMMCALL, which raises #UD once KVM's quirk
/// that rewrites it is off. Then for each instruction whose result under Linux differs from the source's guest, the table of settings
/// on the page says why: UMWAIT and TPAUSE, XGETBV, the
/// XSAVE family, INT 0x80, and AMX's. LAR, LSL, VERR and VERW run: the guest's descriptor table is Linux's. SGDT, SIDT and SMSW run: the chip's #GP under
/// UMIP is the program's outcome at this level, as every other exception is, whatever Linux then does with it.
fn check_steppable(processor: &Processor, memory: &[MemoryDelta<'_, X86_64>]) -> Result<(), Refusal> {
    let address = processor.instruction_pointer;
    let bytes: [u8; 15] = read_bytes(memory, address);
    let decoded = Decoder::with_ip(64, &bytes, address, DecoderOptions::NONE).decode();
    // An instruction whose bytes cannot all be fetched faults before it runs, whatever it is.
    let fetchable = (0..decoded.len() as u64).all(|offset| {
        let at = address.wrapping_add(offset);
        is_canonical(at)
            && matches!(
                read_permission(memory, at),
                PagePermission::Mapped { instruction_fetch: InstructionFetch::Allowed, .. }
            )
    });
    if decoded.is_invalid() || !fetchable {
        return Ok(());
    }
    // MOV SS and POP SS hold off the trap until after the next instruction once they load SS, which only Linux's own
    // 0x2B does at ring 3.
    let loads_stack_segment = matches!(decoded.code(), Code::Mov_Sreg_r32m16 | Code::Mov_Sreg_r64m16)
        && decoded.op0_register() == Register::SS
        && read_source_selector(processor, memory, &decoded) == Some(STACK_SEGMENT_SELECTOR);
    let reason = match decoded.mnemonic() {
        _ if loads_stack_segment => Some("it loads SS, which holds off the trap until after the next instruction"),
        // Linux sets IA32_SYSENTER_CS, so a 64-bit SYSENTER enters its kernel; the source leaves it 0, where it faults.
        Mnemonic::Sysenter => Some("it enters ring 0 at SYSENTER MSRs the source does not set as Linux does"),
        // RDTSC, RDTSCP, RDPID, RDRAND and RDSEED run: their behaviors do not compare the registers the time,
        // IA32_TSC_AUX and the random source decide.
        Mnemonic::Umwait | Mnemonic::Tpause => Some("its result depends on the time, which is not in the state"),
        Mnemonic::Xgetbv => Some("XCR0 under Linux also enables AMX's tiles, and XINUSE is not in the state"),
        Mnemonic::Xsave
        | Mnemonic::Xsave64
        | Mnemonic::Xsaveopt
        | Mnemonic::Xsaveopt64
        | Mnemonic::Xsavec
        | Mnemonic::Xsavec64
        | Mnemonic::Xrstor
        | Mnemonic::Xrstor64 => Some("it reaches components the state does not hold, under an XCR0 that is not Linux's"),
        Mnemonic::Int if decoded.immediate8() == LINUX_SYSTEM_CALL_VECTOR => {
            Some("Linux takes it as a 32-bit system call")
        }
        _ if decoded.cpuid_features().iter().any(|feature| format!("{feature:?}").starts_with("AMX")) => {
            Some("Linux traps its first use of the tiles with #NM, where the source's XCR0 gives #UD")
        }
        _ => None,
    };
    match reason {
        Some(reason) => Err(Refusal::NotSteppable {
            instruction: format!("{:?}", decoded.code()),
            address,
            reason,
        }),
        None => Ok(()),
    }
}


/// The processor a run left in the slot.
///
/// # Errors
///
/// When the handler did not save the slot, the exception did not come from ring 3, or the run changed state the
/// processor does not model: the selectors, IF, IOPL or VM.
fn to_processor(slot: &Slot) -> Result<Processor, Refusal> {
    let after = &slot.after;
    if slot.vector == NOT_RUN {
        return Err(Refusal::SlotNotRun);
    }
    // A far transfer may leave Linux's 32-bit code segment in CS, at ring 3 too.
    let at_ring_3 = after.code_segment == u64::from(USER_CODE) || after.code_segment == USER_32_BIT_CODE;
    if !(at_ring_3 && after.stack_segment == u64::from(USER_DATA)) {
        return Err(Refusal::NotFromRing3 {
            vector: slot.vector,
            error_code: slot.error_code,
            code_segment: after.code_segment,
            instruction_pointer: after.instruction_pointer,
        });
    }
    if after.flags & (INTERRUPT_ENABLE | IO_PRIVILEGE | VIRTUAL_8086) != INTERRUPT_ENABLE {
        return Err(Refusal::NotModeled("RFLAGS changed IF, IOPL or VM"));
    }
    let after_x87 = &slot.after_xsave.bytes;
    let mxcsr = u32::from_le_bytes(after_x87[FX_MXCSR..FX_MXCSR + 4].try_into().expect("four bytes"));
    let saved = u64::from_le_bytes(after_x87[XSAVE_HEADER..XSAVE_HEADER + 8].try_into().expect("eight bytes"));
    // A component XSAVE marks as in its initial state holds zeros, whatever its area of the image holds.
    let read_halves = |component: u64, area: usize| -> [u128; VECTOR_REGISTERS] {
        std::array::from_fn(|number| {
            let at = area + number * VECTOR_HALF_BYTES;
            let half = u128::from_le_bytes(after_x87[at..at + VECTOR_HALF_BYTES].try_into().expect("sixteen bytes"));
            if saved & component == 0 { 0 } else { half }
        })
    };
    let vector_registers = read_halves(SSE_COMPONENT, FX_VECTOR_REGISTERS);
    let upper_vector_halves = read_halves(AVX_COMPONENT, XSAVE_AVX);
    let read_part = |component: u64, at: usize| -> u128 {
        let part = u128::from_le_bytes(after_x87[at..at + VECTOR_HALF_BYTES].try_into().expect("sixteen bytes"));
        if saved & component == 0 { 0 } else { part }
    };
    let zmm_upper_halves = std::array::from_fn(|number| {
        std::array::from_fn(|part| read_part(ZMM_HIGH_256_COMPONENT, XSAVE_ZMM_HIGH_256 + (2 * number + part) * VECTOR_HALF_BYTES))
    });
    let high_vector_registers = std::array::from_fn(|number| {
        std::array::from_fn(|part| read_part(HIGH_16_ZMM_COMPONENT, XSAVE_HIGH_16_ZMM + (4 * number + part) * VECTOR_HALF_BYTES))
    });
    let opmask_registers = std::array::from_fn(|number| {
        let at = XSAVE_OPMASK + number * size_of::<u64>();
        let value = u64::from_le_bytes(after_x87[at..at + size_of::<u64>()].try_into().expect("eight bytes"));
        if saved & OPMASK_COMPONENT == 0 { 0 } else { value }
    });
    let completed = slot.vector == DEBUG_VECTOR && slot.debug_status & SINGLE_STEP != 0;
    let exception = if completed { None } else { Some(to_exception(slot)?) };
    let mut general_purpose_registers: [u64; GENERAL_PURPOSE_REGISTERS] = after.general;
    // R11 holds the RFLAGS SYSCALL saw, with the TF the source steps by.
    if slot.vector == SYSTEM_CALL_VECTOR {
        general_purpose_registers[SAVED_FLAGS] &= !TRAP;
    }
    Ok(Processor {
        general_purpose_registers,
        instruction_pointer: after.instruction_pointer,
        flags: Flags::from_bits_truncate(after.flags & !(TRAP | RESUME)),
        fs_base: after.fs_base,
        gs_base: after.gs_base,
        mxcsr,
        vector_registers,
        upper_vector_halves,
        x87: x87_image::read_x87(after_x87, saved),
        zmm_upper_halves,
        high_vector_registers,
        opmask_registers,
        exception,
        data_segments: after.data_segments.map(|selector| selector as u16),
        pkru: if saved & PKRU_COMPONENT == 0 {
            0
        } else {
            u32::from_le_bytes(after_x87[XSAVE_PKRU..XSAVE_PKRU + size_of::<u32>()].try_into().expect("four bytes"))
        },
        code_segment: after.code_segment as u16,
    })
}

/// Bytes of a half of a YMM register, as the image holds each.
const VECTOR_HALF_BYTES: usize = size_of::<u128>();

fn to_exception(slot: &Slot) -> Result<Exception, Refusal> {
    let error_code = slot.error_code as u32;
    Ok(match slot.vector {
        0 => Exception::DivideError,
        DEBUG_VECTOR => Exception::Debug {
            status: slot.debug_status & !DEBUG_STATUS_RESET,
        },
        3 => Exception::Breakpoint,
        4 => Exception::Overflow,
        5 => Exception::BoundRange,
        6 => Exception::InvalidOpcode,
        7 => Exception::DeviceNotAvailable,
        10 => Exception::InvalidTss { error_code },
        11 => Exception::SegmentNotPresent { error_code },
        12 => Exception::StackSegmentFault { error_code },
        13 => Exception::GeneralProtection { error_code },
        PAGE_FAULT_VECTOR => {
            let known = FAULT_PRESENT | FAULT_WRITE | FAULT_USER | FAULT_INSTRUCTION_FETCH;
            if !(error_code & FAULT_USER != 0 && error_code & !known == 0) {
                return Err(Refusal::UnexpectedPageFault(error_code));
            }
            let access = if error_code & FAULT_INSTRUCTION_FETCH != 0 {
                Access::InstructionFetch
            } else if error_code & FAULT_WRITE != 0 {
                Access::Write
            } else {
                Access::Read
            };
            Exception::PageFault {
                access,
                page_present: error_code & FAULT_PRESENT != 0,
                address: slot.fault_address,
            }
        }
        16 => Exception::X87FloatingPoint,
        17 => Exception::AlignmentCheck,
        18 => Exception::MachineCheck,
        19 => Exception::SimdFloatingPoint,
        20 => Exception::Virtualization,
        21 => Exception::ControlProtection { error_code },
        vector @ 32..=255 => Exception::SoftwareInterrupt { vector: vector as u8 },
        SYSTEM_CALL_VECTOR => Exception::SystemCall,
        vector => return Err(Refusal::UnexpectedVector(vector)),
    })
}

/// The only selector ring 3 can load into SS: Linux's 0x2B.
const STACK_SEGMENT_SELECTOR: u16 = 0x2B;

/// The selector MOV to a segment register reads: its register's low 16 bits, or two bytes of the state's memory.
fn read_source_selector(processor: &Processor, memory: &[MemoryDelta<'_, X86_64>], decoded: &iced_x86::Instruction) -> Option<u16> {
    if decoded.op1_kind() == OpKind::Register {
        let number = decoded.op1_register().full_register().number();
        return Some(processor.general_purpose_registers[number] as u16);
    }
    let address = decoded.virtual_address(1, 0, |register, _, _| {
        let value = match register {
            Register::FS => processor.fs_base,
            Register::GS => processor.gs_base,
            Register::ES | Register::CS | Register::SS | Register::DS => 0,
            register if register.is_gpr64() => processor.general_purpose_registers[register.number()],
            register if register.is_gpr32() => processor.general_purpose_registers[register.full_register().number()] & u64::from(u32::MAX),
            Register::RIP => processor.instruction_pointer,
            _ => return None,
        };
        Some(value)
    })?;
    let bytes: [u8; 2] = read_bytes(memory, address);
    Some(u16::from_le_bytes(bytes))
}

/// Linux's 32-bit code segment, which a far transfer may leave in CS.
const USER_32_BIT_CODE: u64 = 0x23;

/// Whether a program can hold the selector in a data segment register under Linux: null, or naming one of the
/// segments of Linux's descriptor table that ring 3 reaches.
fn is_loadable(selector: u16) -> bool {
    const TABLE_INDICATOR: u16 = 1 << 2;
    selector & !3 == 0 || (selector & TABLE_INDICATOR == 0 && matches!(selector >> 3, 4 | 5 | 6 | 15))
}

/// The vector Linux gives ring 3 for 32-bit system calls.
const LINUX_SYSTEM_CALL_VECTOR: u8 = 0x80;

const fn is_canonical(address: u64) -> bool {
    ((address << 16) as i64 >> 16) as u64 == address
}

/// The first address of each page in the addresses, which start on a page.
fn to_pages(addresses: &Range<u64>) -> impl Iterator<Item = u64> + use<> {
    assert!(
        addresses.start.is_multiple_of(PAGE_BYTES),
        "a chip gives permissions to whole pages, not {addresses:#x?}"
    );
    (addresses.start..addresses.end).step_by(PAGE_BYTES as usize)
}

fn to_page_entry(frame: u64, data_access: DataAccess, instruction_fetch: InstructionFetch) -> u64 {
    let write = match data_access {
        DataAccess::ReadOnly => 0,
        DataAccess::ReadWrite => WRITABLE,
    };
    let fetch = match instruction_fetch {
        InstructionFetch::Forbidden => NO_EXECUTE,
        InstructionFetch::Allowed => 0,
    };
    // Accessed and dirty already, so that the chip never writes the entry.
    frame | PRESENT | USER | ACCESSED | DIRTY | write | fetch
}

/// The permission of each page the deltas give one, by its first address; a later delta wins.
fn to_page_permissions(memory: &[MemoryDelta<'_, X86_64>]) -> BTreeMap<u64, PagePermission> {
    let mut pages = BTreeMap::new();
    for delta in memory {
        if let MemoryDelta::PermissionAssignment { addresses, permission } = delta {
            assert!(
                addresses.end.is_multiple_of(PAGE_BYTES),
                "a chip gives permissions to whole pages, not {addresses:#x?}"
            );
            for page in to_pages(addresses) {
                pages.insert(page, *permission);
            }
        }
    }
    pages
}

/// Writes the bytes the deltas leave in a page, which start as zeros.
fn compose_page(page: u64, memory: &[MemoryDelta<'_, X86_64>], bytes: &mut [u8]) {
    bytes.fill(0);
    let page_end = page + bytes.len() as u64;
    for delta in memory {
        if let MemoryDelta::ValueAssignment { start, bytes: values } = delta {
            let end = start + values.len() as u64;
            let (from, to) = ((*start).max(page), end.min(page_end));
            if from < to {
                bytes[(from - page) as usize..(to - page) as usize]
                    .copy_from_slice(&values[(from - start) as usize..(to - start) as usize]);
            }
        }
    }
}

// The default MXCSR a state starts from fits what the handler loads.
const _: () = assert!(MXCSR_DEFAULT <= u16::MAX as u32);
