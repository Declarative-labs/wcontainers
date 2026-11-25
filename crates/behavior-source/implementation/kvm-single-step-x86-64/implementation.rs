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
//! A state the source cannot hold is refused, with a panic that names why, before anything of it is written, so
//! that the machine runs every later state as a fresh one would: memory outside the program's addresses (where the
//! source keeps its own pages), RFLAGS bits a program cannot set, and the instructions it cannot step (see
//! [`check_steppable`]).

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::collections::BTreeMap;
use std::io;
use std::mem::offset_of;
use std::ops::Range;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use iced_x86::{Code, Decoder, DecoderOptions, Mnemonic, OpKind, Register};

use kvm_bindings::{
    KVM_GUESTDBG_ENABLE, KVM_GUESTDBG_SINGLESTEP, KVM_MAX_CPUID_ENTRIES, kvm_fpu, kvm_guest_debug, kvm_regs,
    kvm_segment, kvm_sregs,
};
use kvm_ioctls::{Kvm as KvmSystem, VcpuExit, VcpuFd, VmFd};
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_kvm::Kvm;
use wcontainers_kvm_direct::DirectKvm;
use wcontainers_processor_architecture::{
    MemoryDelta, MemorySink, ProcessorArchitecture, diff_memory,
};
use wcontainers_processor_architecture_x86_64::memory::{read_bytes, report_changed_bytes};
use wcontainers_processor_architecture_x86_64::{
    Access, DataAccess, Exception, Flags, GENERAL_PURPOSE_REGISTERS, InstructionFetch, MXCSR_DEFAULT,
    PagePermission, Processor, VECTOR_REGISTERS, X86_64,
};
use wcontainers_volatile_bytes::VolatileBytes;
use zerocopy::{FromZeros, IntoBytes};

use crate::handler::{Handler, VECTORS};
use crate::layout::{
    ACCESSED, ADDRESS_MASK, ARENA, DESCRIPTOR_ENTRIES, DIRTY, ENTRIES, ENTRY_BYTES, FAULT_STACK_TOP,
    FX_CONTROL_WORD, FX_MXCSR, FX_VECTOR_REGISTERS, FX_X87_REGISTERS, FxImage, GLOBAL, GLOBAL_DESCRIPTOR_TABLE,
    HANDLER, HANDLER_BYTES, INTERRUPT_DESCRIPTOR_TABLE, KERNEL_BASE, KERNEL_CODE, KERNEL_DATA, KERNEL_DIRECTORY,
    KERNEL_DIRECTORY_POINTERS, KERNEL_ENTRY, KERNEL_PAGES, KERNEL_TABLE, LEVELS, MEMORY_BYTES, NO_EXECUTE,
    LARGE_PAGE, NOT_RUN, PAGE_BYTES, PRESENT, RESIDENT, Registers, SLOT, STACK_TOP, Slot, TASK_STATE,
    TASK_STATE_SEGMENT, TaskStateSegment, UPPER_HALF_DIRECTORY_POINTERS, USER, USER_CODE, USER_DATA, WRITABLE,
    X87_CONTROL_DEFAULT, list_descriptors, to_gate, to_gate_privilege, to_index, to_interrupt_stack, to_kernel,
};

mod handler;
mod layout;

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

/// What steps the instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stepping {
    /// The trap flag, set in the state's RFLAGS: the chip raises #DB after the instruction into the guest's handler.
    TrapFlag,
    /// KVM's own single step: the host enters ring 3 with KVM_SET_SREGS, KVM_SET_REGS and KVM_SET_FPU, and KVM exits
    /// on the #DB after the instruction; an exception of the instruction's own still goes to the handler.
    Hypervisor,
}

/// The executor's choices; the first value of each is the reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub loading: MemoryLoading,
    pub entry: EntryPath,
    pub stepping: Stepping,
}

impl Options {
    pub const PLAIN: Self = Self {
        loading: MemoryLoading::Full,
        entry: EntryPath::Plain,
        stepping: Stepping::TrapFlag,
    };

    /// The options `X86_SINGLE_STEP_LOADING` (`full`, `incremental`, `checked`), `X86_SINGLE_STEP_ENTRY` (`plain`,
    /// `parked`) and `X86_SINGLE_STEP_STEPPING` (`trap-flag`, `hypervisor`) name, the reference where unset.
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
        let stepping = match read("X86_SINGLE_STEP_STEPPING").as_deref() {
            None | Some("trap-flag") => Stepping::TrapFlag,
            Some("hypervisor") => Stepping::Hypervisor,
            Some(other) => panic!("X86_SINGLE_STEP_STEPPING is trap-flag or hypervisor, not {other}"),
        };
        Self {
            loading,
            entry,
            stepping,
        }
    }
}

/// What went wrong while making the machine.
#[derive(Debug)]
pub struct SetUpError(pub String);

/// A page of zeros, which clears guest memory.
static ZERO_PAGE: [u8; PAGE_BYTES as usize] = [0; PAGE_BYTES as usize];

// RFLAGS bits the source sets or checks, besides the flags a program changes.
const RESERVED_ONE: u64 = 1 << 1;
const TRAP: u64 = 1 << 8;
const INTERRUPT_ENABLE: u64 = 1 << 9;
const IO_PRIVILEGE: u64 = 0b11 << 12;
const RESUME: u64 = 1 << 16;
const VIRTUAL_8086: u64 = 1 << 17;
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

// Control registers: CR0's PE, MP, ET, NE, WP, AM and PG; CR4's PAE, PGE, OSFXSR, OSXMMEXCPT, UMIP and FSGSBASE;
// EFER's SCE, LME, LMA and NXE.
const CONTROL_0: u64 = 1 | 1 << 1 | 1 << 4 | 1 << 5 | 1 << 16 | 1 << 18 | 1 << 31;
const CONTROL_4: u64 = 1 << 5 | 1 << 7 | 1 << 9 | 1 << 10 | 1 << 11 | 1 << 16;
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
    /// Whether the CPU is at ring 0 in the handler, as it is after every halt; KVM's single step leaves it at ring 3.
    in_handler: bool,
    /// HYPERVISOR STEPPING: whether KVM's single step is on.
    hypervisor_stepping: bool,
    /// HYPERVISOR STEPPING: whether the last entry at ring 3 set CR3's write-through bit, which alternates.
    write_through: bool,
    next_free_page: u64,
    writable: Vec<WritablePage>,
    bytes_before: Vec<u8>,
    resident: Resident,
    /// The special registers of the handler.
    kernel_sregs: kvm_sregs,
    /// The special registers every entry at ring 3 starts from.
    user_sregs: kvm_sregs,
    pub time_loading: Duration,
    pub time_setting_registers: Duration,
    pub time_in_guest: Duration,
    pub time_reading_back: Duration,
    pub runs: u64,
    /// INCREMENTAL MEMORY: the states whose prepared memory differed from the state's, in checked loading.
    pub check_mismatches: u64,
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
        let mut executor = Self {
            cpu,
            _machine: machine,
            memory,
            page: vec![0; PAGE_BYTES as usize],
            handler: handler::assemble(),
            options,
            parked: false,
            in_handler: true,
            hypervisor_stepping: false,
            write_through: false,
            next_free_page: ARENA.start,
            writable: Vec::new(),
            bytes_before: Vec::new(),
            resident: Resident {
                memory: Held::Unknown,
                frames: BTreeMap::new(),
                next_free_page: RESIDENT.start + PAGE_BYTES,
            },
            kernel_sregs: kvm_sregs::default(),
            user_sregs: kvm_sregs::default(),
            time_loading: Duration::ZERO,
            time_setting_registers: Duration::ZERO,
            time_in_guest: Duration::ZERO,
            time_reading_back: Duration::ZERO,
            runs: 0,
            check_mismatches: 0,
        };
        executor.set_up()?;
        Ok(executor)
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
        self.kernel_sregs = sregs;
        self.user_sregs = kvm_sregs {
            cs: segment(USER_CODE, 0xB, 1, 3),
            ss: segment(USER_DATA, 0x3, 0, 3),
            ..sregs
        };
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
        let slot = to_slot(processor, self.read_slot().page_table, self.options.stepping);
        self.write_bytes(SLOT, slot.as_bytes());
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
        diff_memory(&held, memory, &mut |mutation: MemoryDelta<'_, X86_64>| match mutation {
            MemoryDelta::ValueAssignment { start, bytes } => self.write_resident_bytes(start, &bytes),
            MemoryDelta::PermissionAssignment { addresses, permission } => {
                self.write_resident_permission(&addresses, permission)
            }
        });
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
    fn run(&mut self, processor: &Processor) {
        let setting = Instant::now();
        match self.options.stepping {
            Stepping::TrapFlag => {
                self.set_hypervisor_stepping(false);
                if !self.in_handler {
                    self.cpu
                        .set_sregs(&self.kernel_sregs)
                        .unwrap_or_else(|error| panic!("KVM refused the handler's segments: {error}"));
                }
                if !(self.options.entry == EntryPath::Parked && self.parked) {
                    self.enter_handler();
                }
            }
            Stepping::Hypervisor => {
                let page_table = self.read_slot().page_table;
                self.enter_ring_3(processor, page_table);
                // KVM sets the trap flag only while RIP is where it was when its single step was turned on.
                self.hypervisor_stepping = false;
                self.set_hypervisor_stepping(true);
            }
        }
        self.parked = false;
        self.time_setting_registers += setting.elapsed();
        let started = Instant::now();
        self.runs += 1;
        loop {
            let exit = match self.cpu.run() {
                Ok(VcpuExit::Hlt) => Exit::Halted,
                Ok(VcpuExit::Debug(debug)) if self.hypervisor_stepping => {
                    if debug.pc >= KERNEL_BASE {
                        Exit::SteppedIntoHandler
                    } else {
                        Exit::Stepped
                    }
                }
                Ok(other) => panic!("the guest stopped other than at the handler's halt: {other:?}"),
                // A signal ends a run too: then the CPU runs on.
                Err(error) if io::Error::from_raw_os_error(error.errno()).kind() == io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(error) => panic!("KVM_RUN failed: {error}"),
            };
            match exit {
                Exit::Halted => {
                    self.parked = true;
                    self.in_handler = true;
                }
                Exit::Stepped => {
                    self.in_handler = false;
                    self.save_stepped();
                }
                // The instruction's own exception took the CPU into the handler, which runs to its halt unstepped.
                Exit::SteppedIntoHandler => {
                    self.set_hypervisor_stepping(false);
                    continue;
                }
            }
            break;
        }
        self.time_in_guest += started.elapsed();
    }

    /// PROBE-COST TIMING: runs a bare halt at ring 0 `count` times, one KVM_RUN and exit each, and gives their time.
    pub fn time_bare_runs(&mut self, count: u32) -> Duration {
        self.set_hypervisor_stepping(false);
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
        self.in_handler = true;
        elapsed
    }

    fn set_hypervisor_stepping(&mut self, on: bool) {
        if self.hypervisor_stepping == on {
            return;
        }
        let control = if on { KVM_GUESTDBG_ENABLE | KVM_GUESTDBG_SINGLESTEP } else { 0 };
        let debug = kvm_guest_debug {
            control,
            ..kvm_guest_debug::default()
        };
        self.cpu
            .set_guest_debug(&debug)
            .unwrap_or_else(|error| panic!("KVM refused its single step: {error}"));
        self.hypervisor_stepping = on;
    }

    fn enter_handler(&mut self) {
        let regs = kvm_regs {
            rip: self.handler.entry,
            rsp: to_kernel(STACK_TOP),
            rflags: RESERVED_ONE,
            ..kvm_regs::default()
        };
        self.cpu
            .set_regs(&regs)
            .unwrap_or_else(|error| panic!("KVM refused the handler's entry: {error}"));
    }

    /// HYPERVISOR STEPPING: puts the state's registers into the CPU at ring 3.
    fn enter_ring_3(&mut self, processor: &Processor, page_table: u64) {
        // KVM flushes the guest's translations on KVM_SET_SREGS only when CR3 changes, and the tables change in
        // place, so CR3 alternates between two values that name the same table.
        const WRITE_THROUGH: u64 = 1 << 3;
        self.write_through = !self.write_through;
        let mut sregs = self.user_sregs;
        sregs.cr3 = page_table | if self.write_through { WRITE_THROUGH } else { 0 };
        sregs.fs.base = processor.fs_base;
        sregs.gs.base = processor.gs_base;
        // The kernel's segments of the handler, which the next exception loads, stay unusable as KVM gives them.
        let general = processor.general_purpose_registers;
        let regs = kvm_regs {
            rax: general[0],
            rcx: general[1],
            rdx: general[2],
            rbx: general[3],
            rsp: general[4],
            rbp: general[5],
            rsi: general[6],
            rdi: general[7],
            r8: general[8],
            r9: general[9],
            r10: general[10],
            r11: general[11],
            r12: general[12],
            r13: general[13],
            r14: general[14],
            r15: general[15],
            rip: processor.instruction_pointer,
            rflags: processor.flags.bits() | RESERVED_ONE | INTERRUPT_ENABLE,
        };
        let mut fpu = kvm_fpu {
            fcw: X87_CONTROL_DEFAULT,
            mxcsr: processor.mxcsr,
            ..kvm_fpu::default()
        };
        for (to, from) in fpu.xmm.iter_mut().zip(processor.vector_registers) {
            *to = from.to_le_bytes();
        }
        for result in [self.cpu.set_sregs(&sregs), self.cpu.set_regs(&regs), self.cpu.set_fpu(&fpu)] {
            result.unwrap_or_else(|error| panic!("KVM refused the state's registers: {error}"));
        }
    }

    /// HYPERVISOR STEPPING: writes the registers KVM's step left into the slot, as the handler would.
    fn save_stepped(&mut self) {
        let regs = take_registers(self.cpu.get_regs());
        let sregs = take_registers(self.cpu.get_sregs());
        // KVM_GET_FPU gives MXCSR as 0 here, so the legacy area of the XSAVE image is read instead.
        let xsave = take_registers(self.cpu.get_xsave());
        let mut slot = self.read_slot();
        slot.vector = DEBUG_VECTOR;
        slot.error_code = 0;
        slot.fault_address = sregs.cr2;
        slot.debug_status = DEBUG_STATUS_RESET | SINGLE_STEP;
        slot.after = Registers {
            general: [
                regs.rax, regs.rcx, regs.rdx, regs.rbx, regs.rsp, regs.rbp, regs.rsi, regs.rdi, regs.r8, regs.r9,
                regs.r10, regs.r11, regs.r12, regs.r13, regs.r14, regs.r15,
            ],
            instruction_pointer: regs.rip,
            flags: regs.rflags,
            code_segment: u64::from(sregs.cs.selector),
            stack_segment: u64::from(sregs.ss.selector),
            fs_base: sregs.fs.base,
            gs_base: sregs.gs.base,
        };
        let mut image = slot.before_fx;
        let legacy = &xsave.region.as_bytes()[..size_of::<FxImage>()];
        image.bytes[FX_MXCSR..FX_MXCSR + 4].copy_from_slice(&legacy[FX_MXCSR..FX_MXCSR + 4]);
        image.bytes[FX_VECTOR_REGISTERS..FX_VECTOR_REGISTERS + VECTOR_REGISTERS * 16]
            .copy_from_slice(&legacy[FX_VECTOR_REGISTERS..FX_VECTOR_REGISTERS + VECTOR_REGISTERS * 16]);
        slot.after_fx = image;
        self.write_bytes(SLOT, slot.as_bytes());
    }

    /// Gives the processor after the run, and each run of bytes it changed.
    fn read_back(&mut self, processor: &mut Processor, mut on_memory_mutated: impl FnMut(MemoryDelta<'_, X86_64>)) {
        let started = Instant::now();
        let slot = self.read_slot();
        *processor = to_processor(&slot);
        let mut after = std::mem::take(&mut self.page);
        for writable in &self.writable {
            let before = &self.bytes_before[writable.before.clone()];
            self.read_into(writable.frame, &mut after);
            report_changed_bytes(writable.page, before, &after, &mut on_memory_mutated);
        }
        self.page = after;
        self.time_reading_back += started.elapsed();
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

    fn read_slot(&self) -> Slot {
        let mut slot = Slot::new_zeroed();
        self.read_into(SLOT, slot.as_mut_bytes());
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

/// How a run came back.
enum Exit {
    /// The handler saved the slot and halted.
    Halted,
    /// HYPERVISOR STEPPING: KVM stepped the instruction and exited with the state still in the CPU.
    Stepped,
    /// HYPERVISOR STEPPING: KVM stepped into the handler, which an exception of the instruction's own entered.
    SteppedIntoHandler,
}

impl BehaviorSource for SingleStepMachine {
    type Architecture = X86_64;

    /// Reports each run of bytes whose value changed: a store of the bytes already there is not reported.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor,
        memory: &[MemoryDelta<'_, X86_64>],
        mutation_hook: &mut impl MemorySink<X86_64>,
    ) {
        check_holdable(processor, memory);
        check_steppable(processor, memory);
        let held = self.load(processor, memory);
        let before = processor.clone();
        let Some(mut held) = held else {
            self.run(&before);
            self.read_back(processor, |mutation| mutation_hook.on_memory_mutated(mutation));
            return;
        };
        if self.options.loading == MemoryLoading::Checked && !self.check_guest_holds(memory) {
            self.check_mismatches += 1;
        }
        self.run(&before);
        self.read_back(processor, |mutation| {
            held.push(mutation.clone().into_owned());
            mutation_hook.on_memory_mutated(mutation);
        });
        self.resident.memory = Held::Known(held);
    }
}

/// The slot that loads the processor under the page table.
fn to_slot(processor: &Processor, page_table: u64, stepping: Stepping) -> Slot {
    let trap = match stepping {
        Stepping::TrapFlag => TRAP,
        Stepping::Hypervisor => 0,
    };
    let registers = Registers {
        general: processor.general_purpose_registers,
        instruction_pointer: processor.instruction_pointer,
        flags: processor.flags.bits() | RESERVED_ONE | INTERRUPT_ENABLE | trap,
        code_segment: u64::from(USER_CODE),
        stack_segment: u64::from(USER_DATA),
        fs_base: processor.fs_base,
        gs_base: processor.gs_base,
    };
    let mut image = FxImage { bytes: [0; 512] };
    image.bytes[FX_CONTROL_WORD..FX_CONTROL_WORD + 2].copy_from_slice(&X87_CONTROL_DEFAULT.to_le_bytes());
    image.bytes[FX_MXCSR..FX_MXCSR + 4].copy_from_slice(&processor.mxcsr.to_le_bytes());
    for (number, value) in processor.vector_registers.iter().enumerate() {
        let at = FX_VECTOR_REGISTERS + number * 16;
        image.bytes[at..at + 16].copy_from_slice(&value.to_le_bytes());
    }
    Slot {
        before_fx: image,
        after_fx: image,
        page_table,
        vector: NOT_RUN,
        error_code: 0,
        fault_address: 0,
        debug_status: 0,
        before: registers,
        after: registers,
        reserved: 0,
    }
}

/// Refuses, before anything is written, a state the architecture does not have or the source cannot hold.
///
/// # Panics
///
/// When the state gives a permission or bytes outside the program's addresses, where the source keeps its own
/// pages, or a permission to part of a page; when its RFLAGS sets a bit a program at ring 3 cannot set (TF, RF, a
/// reserved bit); when its instruction pointer or a segment base is not canonical or its MXCSR sets a reserved bit;
/// or when its pages do not fit the arena.
fn check_holdable(processor: &Processor, memory: &[MemoryDelta<'_, X86_64>]) {
    for delta in memory {
        let addresses = match delta {
            MemoryDelta::PermissionAssignment { addresses, .. } => {
                assert!(
                    addresses.start.is_multiple_of(PAGE_BYTES) && addresses.end.is_multiple_of(PAGE_BYTES),
                    "a chip gives permissions to whole pages, not {addresses:#x?}"
                );
                addresses.clone()
            }
            MemoryDelta::ValueAssignment { start, bytes } => {
                *start..start.saturating_add(bytes.len() as u64)
            }
        };
        let addressable = X86_64::ADDRESSABLE;
        assert!(
            addresses.is_empty() || (addressable.start <= addresses.start && addresses.end <= addressable.end),
            "a state's memory lies in the program's addresses {addressable:#x?}, not at {addresses:#x?}"
        );
    }
    assert!(
        Flags::all().contains(processor.flags),
        "a state's RFLAGS {:#x} sets only bits a program at ring 3 can set, {:#x}",
        processor.flags.bits(),
        Flags::all().bits()
    );
    assert!(is_canonical(processor.instruction_pointer), "a state's instruction pointer is canonical");
    assert!(
        is_canonical(processor.fs_base) && is_canonical(processor.gs_base),
        "a state's segment bases are canonical"
    );
    assert!(processor.mxcsr <= u32::from(u16::MAX), "a state's MXCSR has no reserved bit set");
    let mapped = to_page_permissions(memory)
        .values()
        .filter(|permission| **permission != PagePermission::Unmapped)
        .count() as u64;
    // The top-level table, and per page a frame and at most one table of each lower level.
    assert!(
        mapped * u64::from(LEVELS) < (ARENA.end - ARENA.start) / PAGE_BYTES,
        "a state's {mapped} mapped pages fit the arena"
    );
}

/// Refuses, before anything is written, the instructions the source cannot step from ring 3 with the trap flag.
///
/// # Panics
///
/// For MOV SS (it steps two instructions), a REP string form (it steps one iteration), POPF and IRET (they can
/// change TF), PUSHF (it pushes the source's TF), SYSCALL and SYSENTER (they enter ring 0 at MSRs the source does
/// not set as Linux does), a far CALL, JMP or RET (it changes CS), any other write of a segment register (the
/// selectors are not modeled), and VMCALL and VMMCALL (KVM answers them as hypercalls, which the chip alone does
/// not).
fn check_steppable(processor: &Processor, memory: &[MemoryDelta<'_, X86_64>]) {
    let address = processor.instruction_pointer;
    let bytes: [u8; 15] = read_bytes(memory, address);
    let decoded = Decoder::with_ip(64, &bytes, address, DecoderOptions::NONE).decode();
    if decoded.is_invalid() {
        return;
    }
    let is_string = (0..decoded.op_count()).any(|operand| {
        matches!(
            decoded.op_kind(operand),
            OpKind::MemorySegSI
                | OpKind::MemorySegESI
                | OpKind::MemorySegRSI
                | OpKind::MemoryESDI
                | OpKind::MemoryESEDI
                | OpKind::MemoryESRDI
        )
    });
    let writes_segment = decoded.op_count() > 0
        && decoded.op0_kind() == OpKind::Register
        && decoded.op0_register().is_segment_register();
    let far = matches!(
        decoded.code(),
        Code::Call_m1616 | Code::Call_m1632 | Code::Call_m1664 | Code::Jmp_m1616 | Code::Jmp_m1632 | Code::Jmp_m1664
    );
    let reason = match decoded.mnemonic() {
        Mnemonic::Mov if writes_segment && decoded.op0_register() == Register::SS => {
            Some("it steps two instructions")
        }
        _ if is_string && (decoded.has_rep_prefix() || decoded.has_repne_prefix()) => {
            Some("it steps one iteration")
        }
        Mnemonic::Popf | Mnemonic::Popfq | Mnemonic::Iret | Mnemonic::Iretd | Mnemonic::Iretq => {
            Some("it can change TF, which steps it")
        }
        Mnemonic::Pushf | Mnemonic::Pushfq => Some("it pushes the TF that steps it"),
        Mnemonic::Syscall => Some("it enters ring 0 at an LSTAR the source does not set"),
        // Linux sets IA32_SYSENTER_CS, so a 64-bit SYSENTER enters its kernel; the source leaves it 0, where it faults.
        Mnemonic::Sysenter => Some("it enters ring 0 at SYSENTER MSRs the source does not set as Linux does"),
        Mnemonic::Retf => Some("it changes CS"),
        _ if far => Some("it changes CS"),
        Mnemonic::Lss | Mnemonic::Lfs | Mnemonic::Lgs => Some("it writes a selector, which is not modeled"),
        _ if writes_segment => Some("it writes a selector, which is not modeled"),
        Mnemonic::Vmcall | Mnemonic::Vmmcall => Some("KVM answers it as a hypercall"),
        _ => None,
    };
    if let Some(reason) = reason {
        panic!("the chip source refuses {:?} at {address:#x}: {reason}", decoded.code());
    }
}

fn take_registers<T>(result: Result<T, kvm_ioctls::Error>) -> T {
    result.unwrap_or_else(|error| panic!("KVM refused to give the registers: {error}"))
}

/// The processor a run left in the slot.
///
/// # Panics
///
/// When the run changed state the processor does not model: the x87 state, the selectors, IF, IOPL or VM.
fn to_processor(slot: &Slot) -> Processor {
    let after = &slot.after;
    assert!(slot.vector != NOT_RUN, "the handler saved the slot");
    assert!(
        after.code_segment == u64::from(USER_CODE) && after.stack_segment == u64::from(USER_DATA),
        "the exception came from ring 3: vector {}, error code {:#x}, from {:#x} at {:#x}",
        slot.vector,
        slot.error_code,
        after.code_segment,
        after.instruction_pointer,
    );
    assert!(
        after.flags & (INTERRUPT_ENABLE | IO_PRIVILEGE | VIRTUAL_8086) == INTERRUPT_ENABLE,
        "RFLAGS {:#x} keeps IF set and IOPL and VM clear",
        after.flags
    );
    let (before_x87, after_x87) = (&slot.before_fx.bytes, &slot.after_fx.bytes);
    assert!(
        before_x87[..FX_MXCSR] == after_x87[..FX_MXCSR]
            && before_x87[FX_X87_REGISTERS..FX_VECTOR_REGISTERS] == after_x87[FX_X87_REGISTERS..FX_VECTOR_REGISTERS],
        "the instruction changed the x87 state, which is not modeled yet"
    );
    let mxcsr = u32::from_le_bytes(after_x87[FX_MXCSR..FX_MXCSR + 4].try_into().expect("four bytes"));
    let vector_registers: [u128; VECTOR_REGISTERS] = std::array::from_fn(|number| {
        let at = FX_VECTOR_REGISTERS + number * 16;
        u128::from_le_bytes(after_x87[at..at + 16].try_into().expect("sixteen bytes"))
    });
    let completed = slot.vector == DEBUG_VECTOR && slot.debug_status & SINGLE_STEP != 0;
    let exception = if completed { None } else { Some(to_exception(slot)) };
    let general_purpose_registers: [u64; GENERAL_PURPOSE_REGISTERS] = after.general;
    Processor {
        general_purpose_registers,
        instruction_pointer: after.instruction_pointer,
        flags: Flags::from_bits_truncate(after.flags & !(TRAP | RESUME)),
        fs_base: after.fs_base,
        gs_base: after.gs_base,
        mxcsr,
        vector_registers,
        exception,
    }
}

fn to_exception(slot: &Slot) -> Exception {
    let error_code = slot.error_code as u32;
    match slot.vector {
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
            assert!(
                error_code & FAULT_USER != 0 && error_code & !known == 0,
                "a page fault at ring 3 with no reserved bit, key or shadow stack: {error_code:#x}"
            );
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
        vector => panic!("vector {vector} does not end an instruction at ring 3"),
    }
}

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
