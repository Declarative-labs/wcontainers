//! A behavior source that really runs one ARM64 instruction: at EL0 of a KVM virtual machine with software step on,
//! under a handler at EL1 that saves the registers when the step, or the instruction's own exception, ends it.
//!
//! Each state is a slot in guest memory: its registers before and after, and translation tables and pages of its own.
//! Guest memory is reached only through `VolatileBytes`.

#![cfg(all(target_arch = "aarch64", target_os = "linux"))]

use std::collections::BTreeMap;
use std::mem::{self, offset_of};
use std::ops::Range;

use aarch64_cpu::registers::{
    CPACR_EL1, ID_AA64MMFR0_EL1, ID_AA64PFR0_EL1, MDSCR_EL1, SCTLR_EL1, SPSR_EL1, TTBR0_EL1,
};
use aarch64_paging::descriptor::El1Attributes;
use wcontainers_abi::PAGE_SIZE;
use wcontainers_behavior_source::BehaviorSource;
use wcontainers_processor_architecture::{
    MemoryDelta, MemorySink, ProcessorArchitecture, diff_memory,
};
use wcontainers_processor_architecture_arm64::memory::report_changed_bytes;
use wcontainers_processor_architecture_arm64::{
    Arm64, ConditionFlags, DataAccess, InstructionFetch, PagePermission, Processor, User,
};
use wcontainers_volatile_bytes::VolatileBytes;
use zerocopy::{FromZeros, IntoBytes};

use crate::boot::{
    BOOT_MAP_BYTES, BootRegisters, to_boot_registers, to_handler_attributes, write_boot_map,
};
use crate::guest_layout::{
    ATTRIBUTE_NORMAL, ENTRIES, ENTRY_BYTES, LEAF_LEVEL, LEVEL_BITS, MEMORY_BASE, TOP_LEVEL,
    VECTOR_BYTES, VECTOR_TABLE_BYTES, VECTORS, level_index, linear,
};
use crate::machine::{GuestMemory, Machine, MachineError, Register, create_machine};
use crate::slot::{Ending, Registers, Slot};

mod boot;
mod guest_layout;
mod machine;
mod slot;
mod syndrome;

include!(concat!(env!("OUT_DIR"), "/handler_offsets.rs"));

/// The handler's machine code, from its exception vectors to its end, as the build script assembled it.
const HANDLER_CODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/handler.bin"));

/// Bytes of guest-physical memory: one block of the boot map.
const MEMORY_BYTES: u64 = 1 << 30;

// Where each part lies in guest-physical memory.
const BOOT_MAP: u64 = MEMORY_BASE;
const HANDLER: u64 = BOOT_MAP + BOOT_MAP_BYTES;
const HANDLER_STACK: u64 = HANDLER + PAGE_SIZE;
/// One table for each level below the top whose entries all lead to an invalid entry at the last level.
const UNMAPPED_TABLES: u64 = HANDLER_STACK + PAGE_SIZE;
/// What the slots of a full load take their pages from.
const ARENA: Range<u64> = UNMAPPED_TABLES + LEAF_LEVEL as u64 * PAGE_SIZE..RESIDENT.start;
/// The pages the guest keeps between states when loading incrementally: its slot, its top-level table, then lower
/// tables and frames as pages first need them.
const RESIDENT: Range<u64> = MEMORY_BASE + MEMORY_BYTES - (64 << 20)..MEMORY_BASE + MEMORY_BYTES;
const RESIDENT_SLOT: u64 = RESIDENT.start;
const RESIDENT_TOP: u64 = RESIDENT.start + PAGE_SIZE;

// VBAR_EL1 holds the vectors aligned to their size, and the entry follows the last vector.
const _: () = assert!(HANDLER.is_multiple_of(VECTOR_TABLE_BYTES));
const _: () = assert!(HANDLER_ENTRY_OFFSET == VECTORS * VECTOR_BYTES);

/// A page of zeros, which a fresh frame is given.
const ZERO_PAGE: [u8; PAGE_SIZE as usize] = [0; PAGE_SIZE as usize];

/// The switch that chooses how memory is loaded, and the one that chooses how runs are entered.
const LOADING_SWITCH: &str = "SINGLE_STEP_LOADING";
const ENTRY_SWITCH: &str = "SINGLE_STEP_ENTRY";

/// How the machine brings the virtual CPU to the handler's entry before each run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EntryPath {
    /// Writes TPIDR_EL1, PC and PSTATE through KVM, and the handler opens the OS lock, every run.
    Plain,
    /// Leaves the CPU where the last run's doorbell left it, parked at the entry past the OS lock, when that run had
    /// one slot; then writes only TPIDR_EL1, and only when the slot differs.
    Incremental,
}

/// How the machine makes the guest hold a state's memory before each instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemoryLoading {
    /// Writes a new slot, tables and pages for every state.
    Full,
    /// Writes only where the memory the guest holds differs from the state's.
    Incremental,
    /// As incremental, then reads the whole of the guest's memory back and panics where it differs from the state's.
    Checked,
}

/// [SME](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/ID-AA64PFR1-EL1--AArch64-Processor-Feature-Register-1#:~:text=SME%2C%20bits%20%5B27%3A24%5D), not zero when the chip has SME.
const SME_IMPLEMENTED: u64 = 0b1111 << 24;

/// [SMEN](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CPACR-EL1--Architectural-Feature-Access-Control-Register#:~:text=SMEN%2C%20bits%20%5B25%3A24%5D) at 0b11: EL0 may use SME, as Linux lets a program once it has used it.
const SME_TRAPS_NOTHING: u64 = 0b11 << 24;

/// A slot loaded for the next run, and the pages of it that EL0 may write.
struct LoadedSlot {
    slot: u64,
    writable: Range<usize>,
}

/// Consecutive pages EL0 may write.
struct WritableRun {
    /// Its first virtual address.
    start: u64,
    /// The first frame when the pages lie on consecutive frames, which a full load always gives them; otherwise
    /// `None`, and each page's frame is the resident one.
    frame: Option<u64>,
    /// Where the bytes before the run lie in [`KvmSingleStepMachine::bytes_before`].
    before: Range<usize>,
}

/// One KVM virtual machine with one virtual CPU, bound to the thread that made it, that runs one instruction a state.
pub struct KvmSingleStepMachine {
    machine: Machine,
    /// Guest-physical memory from [`MEMORY_BASE`] on.
    memory: GuestMemory,
    /// The handler's entry, at its linear address.
    entry: u64,
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
    /// The bytes a run of writable pages holds after the instruction, read back from guest memory.
    bytes_after: Vec<u8>,
    loading: MemoryLoading,
    entry_path: EntryPath,
    /// The slot TPIDR_EL1 names while the CPU waits at the doorbell of a run that ended there, which goes on to the
    /// entry with the OS lock open, PSTATE that of the handler and every other system register as the setup left it;
    /// `None` when the CPU may be anywhere else.
    parked_at: Option<u64>,
    resident: Resident,
}

/// What the guest holds between states when loading incrementally.
struct Resident {
    /// The memory the guest holds, as far as it is known.
    memory: Held,
    /// The frame of each virtual page the guest has held bytes or a permission on; any other page holds zeros.
    frames: BTreeMap<u64, u64>,
    next_free_page: u64,
}

/// Whether the machine knows what the guest's resident frames and tables hold.
enum Held {
    /// The changes of the state loaded last, then those its instruction made, set only once its read-back is done.
    Known(Vec<MemoryDelta<'static, Arm64<User>>>),
    /// From the first write of a load until its read-back is done, and before the first load: the next load resets
    /// every resident frame and table before it writes.
    Unknown,
}

impl KvmSingleStepMachine {
    /// Makes a virtual machine, loading memory as `SINGLE_STEP_LOADING` says (`full`, the default, `incremental` or
    /// `checked`) and entering as `SINGLE_STEP_ENTRY` says (`plain`, the default, or `incremental`).
    ///
    /// TODO: temporary placeholder until the behavior source interface says how a source is made.
    pub fn new() -> Result<Self, MachineError> {
        let loading = match std::env::var(LOADING_SWITCH).as_deref() {
            Ok("incremental") => MemoryLoading::Incremental,
            Ok("checked") => MemoryLoading::Checked,
            Ok("full") | Err(_) => MemoryLoading::Full,
            Ok(other) => {
                panic!("{LOADING_SWITCH} must be full, incremental or checked, not {other}")
            }
        };
        let entry_path = match std::env::var(ENTRY_SWITCH).as_deref() {
            Ok("incremental") => EntryPath::Incremental,
            Ok("plain") | Err(_) => EntryPath::Plain,
            Ok(other) => panic!("{ENTRY_SWITCH} must be plain or incremental, not {other}"),
        };
        let mut source = Self::with_loading(loading)?;
        source.entry_path = entry_path;
        Ok(source)
    }

    /// Makes a virtual machine and sets it up to run single instructions, loading memory so.
    fn with_loading(loading: MemoryLoading) -> Result<Self, MachineError> {
        let (machine, memory) = create_machine(MEMORY_BASE, MEMORY_BYTES as usize)?;
        let mut source = KvmSingleStepMachine {
            machine,
            memory,
            entry: 0,
            entry_pstate: 0,
            next_free_page: ARENA.start,
            asid_count: 0,
            slot_count: 0,
            first_slot: None,
            last_slot: None,
            writable_runs: Vec::new(),
            bytes_before: Vec::new(),
            bytes_after: Vec::new(),
            loading,
            entry_path: EntryPath::Plain,
            parked_at: None,
            resident: Resident {
                memory: Held::Unknown,
                frames: BTreeMap::new(),
                next_free_page: RESIDENT_TOP + PAGE_SIZE,
            },
        };
        source.set_up()?;
        Ok(source)
    }

    /// Writes the handler, its stack and translation tables, and the system registers that stay.
    fn set_up(&mut self) -> Result<(), MachineError> {
        write_boot_map(BOOT_MAP, MEMORY_BYTES, |at, entry| {
            self.write_entry(at, entry)
        });
        assert!(
            HANDLER_CODE.len() as u64 <= PAGE_SIZE,
            "the handler fits its page"
        );
        self.write_bytes(HANDLER, HANDLER_CODE);
        self.entry = linear(HANDLER + HANDLER_ENTRY_OFFSET);
        for level in TOP_LEVEL + 1..=LEAF_LEVEL {
            self.fill_table(unmapped_table(level), to_unmapped_entry(level));
        }
        let memory_model_features = self.machine.get(Register::MemoryModelFeatures)?;
        let BootRegisters {
            mair,
            tcr,
            sctlr,
            cpacr,
            ttbr1,
            pstate,
        } = to_boot_registers(BOOT_MAP, memory_model_features);
        self.entry_pstate = pstate;
        self.asid_count = match ID_AA64MMFR0_EL1::ASIDBits.read_as_enum(memory_model_features) {
            Some(ID_AA64MMFR0_EL1::ASIDBits::Value::Bits_16) => 1 << u16::BITS,
            Some(ID_AA64MMFR0_EL1::ASIDBits::Value::Bits_8) | None => 1 << u8::BITS,
        };
        // CTR_EL0 is readable at EL0, as Linux has it.
        let sctlr = sctlr | SCTLR_EL1::UCT::DontTrap.value;
        let has_sve =
            ID_AA64PFR0_EL1::SVE.read(self.machine.get(Register::ProcessorFeatures)?) != 0;
        let cpacr = if has_sve {
            cpacr | CPACR_EL1::ZEN::TrapNothing.value
        } else {
            cpacr
        };
        let has_sme = self.machine.get(Register::ProcessorFeatures1)? & SME_IMPLEMENTED != 0;
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
            (Register::DebugControl, MDSCR_EL1::SS::SET.value),
        ] {
            self.machine.set(register, value)?;
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
        processor: &Processor<()>,
        memory: &[MemoryDelta<'_, Arm64<User>>],
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
        let slot = allocate(&mut self.next_free_page);
        let top = allocate(&mut self.next_free_page);
        self.fill_table(top, to_unmapped_entry(TOP_LEVEL));
        // Frames first, in address order, so that consecutive pages lie on consecutive frames.
        let frames: Vec<u64> = mapped
            .iter()
            .map(|_| allocate(&mut self.next_free_page))
            .collect();
        let writable_start = self.writable_runs.len();
        let mut bytes = vec![0; PAGE_SIZE as usize];
        for (&(page, data_access, _), &frame) in mapped.iter().zip(&frames) {
            compose_page(page, memory, &mut bytes);
            self.write_bytes(frame, &bytes);
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
                    frame: Some(frame),
                    before: before_start..self.bytes_before.len(),
                }),
            }
        }
        let mut next_free_page = self.next_free_page;
        for (&(page, data_access, instruction_fetch), &frame) in mapped.iter().zip(&frames) {
            let leaf = self.find_leaf_entry(&mut next_free_page, top, page);
            self.write_entry(leaf, to_page_entry(frame, data_access, instruction_fetch));
        }
        self.next_free_page = next_free_page;
        let slot_contents = Slot {
            translation_table: top | TTBR0_EL1::ASID.val(self.slot_count).value,
            next: 0,
            ending: Ending::NotRun as u64,
            alignment: 0,
            before: to_registers(processor),
            after: to_registers(processor),
        };
        self.write_slot(slot, &slot_contents);
        if let Some(last) = self.last_slot {
            self.write_entry(last + offset_of!(Slot, next) as u64, linear(slot));
        }
        self.slot_count += 1;
        self.first_slot.get_or_insert(slot);
        self.last_slot = Some(slot);
        Some(LoadedSlot {
            slot,
            writable: writable_start..self.writable_runs.len(),
        })
    }

    /// Makes the guest hold the state, writing memory only where it differs from what the guest holds, and the
    /// state's registers into the resident slot. Gives the state's memory, which the caller stores as what the guest
    /// holds once the run's read-back is done; until then what the guest holds is unknown.
    fn prepare(
        &mut self,
        processor: &Processor<()>,
        memory: &[MemoryDelta<'_, Arm64<User>>],
        resident_pages: u64,
    ) -> (LoadedSlot, Vec<MemoryDelta<'static, Arm64<User>>>) {
        // Unknown from here on: a panic before the caller stores the next record makes the next load reset.
        let mut held = match mem::replace(&mut self.resident.memory, Held::Unknown) {
            // The pages the state needs from empty resident pages bound the frames and tables it adds to these.
            Held::Known(held)
                if self.resident.next_free_page + resident_pages * PAGE_SIZE <= RESIDENT.end =>
            {
                held
            }
            Held::Known(_) | Held::Unknown => {
                self.reset_resident();
                Vec::new()
            }
        };
        diff_memory(&held, memory, &mut |mutation: MemoryDelta<
            '_,
            Arm64<User>,
        >| match mutation {
            MemoryDelta::ValueAssignment { start, bytes } => {
                self.write_resident_bytes(start, &bytes)
            }
            MemoryDelta::PermissionAssignment {
                addresses,
                permission,
            } => self.write_resident_permission(&addresses, permission),
        });
        // What EL0 may write, and the bytes it holds before, to find what the instruction changes.
        let writable: Vec<u64> = to_page_permissions(memory)
            .into_iter()
            .filter_map(|(page, permission)| match permission {
                PagePermission::Mapped {
                    data_access: DataAccess::ReadWrite,
                    ..
                } => Some(page),
                _ => None,
            })
            .collect();
        for page in writable {
            let frame = self.resident.frames[&page];
            let before_start = self.bytes_before.len();
            self.bytes_before
                .resize(before_start + PAGE_SIZE as usize, 0);
            compose_page(page, memory, &mut self.bytes_before[before_start..]);
            match self.writable_runs.last_mut() {
                Some(run) if run.start + run.before.len() as u64 == page => {
                    if run
                        .frame
                        .is_some_and(|first| first + run.before.len() as u64 != frame)
                    {
                        run.frame = None;
                    }
                    run.before.end = self.bytes_before.len();
                }
                _ => self.writable_runs.push(WritableRun {
                    start: page,
                    frame: Some(frame),
                    before: before_start..self.bytes_before.len(),
                }),
            }
        }
        held.clear();
        held.extend(memory.iter().cloned().map(MemoryDelta::into_owned));
        let slot_contents = Slot {
            translation_table: RESIDENT_TOP | TTBR0_EL1::ASID.val(0).value,
            next: 0,
            ending: Ending::NotRun as u64,
            alignment: 0,
            before: to_registers(processor),
            after: to_registers(processor),
        };
        self.write_slot(RESIDENT_SLOT, &slot_contents);
        self.slot_count = 1;
        self.first_slot = Some(RESIDENT_SLOT);
        self.last_slot = Some(RESIDENT_SLOT);
        let loaded = LoadedSlot {
            slot: RESIDENT_SLOT,
            writable: 0..self.writable_runs.len(),
        };
        (loaded, held)
    }

    /// Makes the resident pages hold nothing, as an empty list of changes leaves memory: the top table unmapped, and no
    /// frames, so that each page gets a zeroed frame and each lower table is filled anew.
    fn reset_resident(&mut self) {
        self.fill_table(RESIDENT_TOP, to_unmapped_entry(TOP_LEVEL));
        self.resident.frames.clear();
        self.resident.next_free_page = RESIDENT_TOP + PAGE_SIZE;
    }

    /// Writes bytes into the frames of the pages they lie on.
    fn write_resident_bytes(&mut self, start: u64, bytes: &[u8]) {
        let mut done = 0;
        while done < bytes.len() {
            let address = start + done as u64;
            let page = address & !(PAGE_SIZE - 1);
            let length = ((page + PAGE_SIZE - address) as usize).min(bytes.len() - done);
            let frame = self.find_frame(page);
            self.write_bytes(frame + (address - page), &bytes[done..done + length]);
            done += length;
        }
    }

    /// Gives pages the permission.
    fn write_resident_permission(&mut self, addresses: &Range<u64>, permission: PagePermission) {
        for page in to_pages(addresses) {
            let entry = match permission {
                PagePermission::Mapped {
                    data_access,
                    instruction_fetch,
                } => to_page_entry(self.find_frame(page), data_access, instruction_fetch),
                PagePermission::Unmapped => to_unmapped_entry(LEAF_LEVEL),
            };
            let mut next_free_page = self.resident.next_free_page;
            let leaf = self.find_leaf_entry(&mut next_free_page, RESIDENT_TOP, page);
            assert!(next_free_page <= RESIDENT.end, "the resident pages fit");
            self.resident.next_free_page = next_free_page;
            self.write_entry(leaf, entry);
        }
    }

    /// Returns the frame of a virtual page, given a zeroed one the first time.
    fn find_frame(&mut self, page: u64) -> u64 {
        if let Some(&frame) = self.resident.frames.get(&page) {
            return frame;
        }
        let frame = allocate(&mut self.resident.next_free_page);
        assert!(frame + PAGE_SIZE <= RESIDENT.end, "the resident pages fit");
        self.write_bytes(frame, &ZERO_PAGE);
        self.resident.frames.insert(page, frame);
        frame
    }

    /// Reads the guest's whole resident memory back, its tables and every frame, and gives whether it holds exactly
    /// the state's bytes and permissions.
    fn check_guest_holds(&self, memory: &[MemoryDelta<'_, Arm64<User>>]) -> bool {
        let mut same = true;
        let mut bytes = vec![0; PAGE_SIZE as usize];
        let mut held = vec![0; PAGE_SIZE as usize];
        // Every page the state writes or the guest has a frame for: the frame's bytes, or zeros without one.
        let mut pages: Vec<u64> = memory
            .iter()
            .filter_map(|delta| match delta {
                MemoryDelta::ValueAssignment { start, bytes } if !bytes.is_empty() => Some(
                    to_pages(&((*start & !(PAGE_SIZE - 1))..*start + bytes.len() as u64)),
                ),
                _ => None,
            })
            .flatten()
            .chain(self.resident.frames.keys().copied())
            .collect();
        pages.sort_unstable();
        pages.dedup();
        for page in pages {
            compose_page(page, memory, &mut bytes);
            match self.resident.frames.get(&page) {
                Some(&frame) => self.read_bytes(frame, &mut held),
                None => held.fill(0),
            }
            if held != bytes {
                eprintln!("checked loading: the guest holds other bytes at page {page:#x}");
                same = false;
            }
        }
        // Every valid last-level entry the guest's tables hold, against the one the state's permissions give.
        let mut leaves = BTreeMap::new();
        self.collect_leaves(RESIDENT_TOP, TOP_LEVEL, 0, &mut leaves);
        let wanted: BTreeMap<u64, u64> = to_page_permissions(memory)
            .into_iter()
            .filter_map(|(page, permission)| match permission {
                PagePermission::Unmapped => None,
                PagePermission::Mapped {
                    data_access,
                    instruction_fetch,
                } => Some((
                    page,
                    to_page_entry(
                        *self.resident.frames.get(&page)?,
                        data_access,
                        instruction_fetch,
                    ),
                )),
            })
            .collect();
        let mapped = to_page_permissions(memory)
            .values()
            .filter(|permission| **permission != PagePermission::Unmapped)
            .count();
        if leaves != wanted || wanted.len() != mapped {
            eprintln!(
                "checked loading: the guest's tables map {leaves:#x?}, the state {wanted:#x?}"
            );
            same = false;
        }
        same
    }

    /// Collects every valid last-level entry under a table, by the virtual page it maps.
    fn collect_leaves(&self, table: u64, level: u32, base: u64, leaves: &mut BTreeMap<u64, u64>) {
        for index in 0..ENTRIES {
            let entry = self.read_entry(table + index * ENTRY_BYTES);
            if entry == to_unmapped_entry(level) {
                continue;
            }
            let address =
                base | index << (PAGE_SIZE.trailing_zeros() + (LEAF_LEVEL - level) * LEVEL_BITS);
            if level == LEAF_LEVEL {
                leaves.insert(address, entry);
            } else {
                self.collect_leaves(entry & !to_table_entry(0), level + 1, address, leaves);
            }
        }
    }

    /// Enters the handler at the batch's first slot and waits until it leaves after the last.
    fn run(&mut self) {
        let Some(first) = self.first_slot else {
            return;
        };
        let all = [
            (Register::HandlerThread, linear(first)),
            (Register::Pc, self.entry),
            (Register::Pstate, self.entry_pstate),
        ];
        let writes = match (self.entry_path, self.parked_at) {
            (EntryPath::Incremental, Some(parked)) if parked == linear(first) => &all[..0],
            (EntryPath::Incremental, Some(_)) => &all[..1],
            (EntryPath::Plain, _) | (EntryPath::Incremental, None) => &all[..],
        };
        // Until the doorbell, the CPU is wherever the run took it.
        self.parked_at = None;
        for &(register, value) in writes {
            if let Err(error) = self.machine.set(register, value) {
                panic!("KVM refused {register:?}: {error:?}");
            }
        }
        if let Err(error) = self.machine.run() {
            panic!("the guest stopped other than at the handler's doorbell: {error:?}");
        }
        // With one slot the handler never moved TPIDR_EL1 on (a batch leaves it at a slot after the first).
        if self.first_slot == self.last_slot {
            self.parked_at = Some(linear(first));
        }
    }

    /// Gives the processor after a slot's run, and each run of bytes it changed, read back from guest memory.
    fn read_back(
        &mut self,
        loaded: &LoadedSlot,
        processor: &mut Processor<()>,
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) {
        let slot = self.read_slot(loaded.slot);
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
        *processor = to_processor(&slot.after);
        let mut after = mem::take(&mut self.bytes_after);
        for run in &self.writable_runs[loaded.writable.clone()] {
            let before = &self.bytes_before[run.before.clone()];
            after.clear();
            after.resize(before.len(), 0);
            match run.frame {
                Some(frame) => self.read_bytes(frame, &mut after),
                None => {
                    for (page, held) in to_pages(&(run.start..run.start + before.len() as u64))
                        .zip(after.chunks_mut(PAGE_SIZE as usize))
                    {
                        self.read_bytes(self.resident.frames[&page], held);
                    }
                }
            }
            report_changed_bytes(run.start, before, &after, mutation_hook);
        }
        self.bytes_after = after;
    }

    /// Returns the address of the last-level entry of a virtual page, making the tables that lead to it from the given
    /// pages.
    fn find_leaf_entry(&mut self, next_free_page: &mut u64, top: u64, page: u64) -> u64 {
        let mut table = top;
        for level in TOP_LEVEL..LEAF_LEVEL {
            let at = table + level_index(page, level) * ENTRY_BYTES;
            let entry = self.read_entry(at);
            table = if entry == to_unmapped_entry(level) {
                let own = allocate(next_free_page);
                self.fill_table(own, to_unmapped_entry(level + 1));
                self.write_entry(at, to_table_entry(own));
                own
            } else {
                entry & !to_table_entry(0)
            };
        }
        table + level_index(page, LEAF_LEVEL) * ENTRY_BYTES
    }

    fn fill_table(&self, table: u64, entry: u64) {
        let entries = [entry; ENTRIES as usize];
        self.write_bytes(table, entries.as_bytes());
    }

    /// Returns the view of guest memory at the guest-physical address, for the length.
    fn view(&self, at: u64, length: u64) -> GuestMemory {
        let offset = at
            .checked_sub(MEMORY_BASE)
            .unwrap_or_else(|| panic!("{at:#x} lies below guest memory"));
        match self.memory.part(offset..offset + length) {
            Some(view) => view,
            None => panic!("{at:#x} and {length:#x} bytes on lie past guest memory"),
        }
    }

    fn read_entry(&self, at: u64) -> u64 {
        let mut entry = [0; size_of::<u64>()];
        self.view(at, ENTRY_BYTES).read(&mut entry);
        u64::from_le_bytes(entry)
    }

    fn write_entry(&self, at: u64, entry: u64) {
        self.view(at, ENTRY_BYTES).write(&entry.to_le_bytes());
    }

    fn read_bytes(&self, at: u64, into: &mut [u8]) {
        self.view(at, into.len() as u64).read(into);
    }

    fn write_bytes(&self, at: u64, bytes: &[u8]) {
        self.view(at, bytes.len() as u64).write(bytes);
    }

    fn read_slot(&self, slot: u64) -> Slot {
        let mut contents = Slot::new_zeroed();
        self.read_bytes(slot, contents.as_mut_bytes());
        contents
    }

    /// Writes a slot's contents into its page.
    fn write_slot(&self, slot: u64, contents: &Slot) {
        assert!(
            (ARENA.start..RESIDENT.end).contains(&slot) && slot.is_multiple_of(PAGE_SIZE),
            "a slot is a page of the arena or the resident pages"
        );
        self.write_bytes(slot, contents.as_bytes());
    }
}

impl BehaviorSource for KvmSingleStepMachine {
    type Architecture = Arm64<User>;

    /// Reports each run of bytes whose value changed: a store of the bytes already there is not reported.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor<()>,
        memory: &[MemoryDelta<'_, Arm64<User>>],
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) {
        self.start_batch();
        let resident_pages = match self.loading {
            MemoryLoading::Full => None,
            MemoryLoading::Incremental | MemoryLoading::Checked => count_resident_pages(memory),
        };
        let Some(resident_pages) = resident_pages else {
            if self.loading != MemoryLoading::Full {
                self.resident.memory = Held::Unknown;
            }
            let Some(loaded) = self.load(processor, memory) else {
                panic!("one state's pages fit in the guest's memory");
            };
            self.run();
            self.read_back(&loaded, processor, mutation_hook);
            return;
        };
        let (loaded, mut held) = self.prepare(processor, memory, resident_pages);
        if self.loading == MemoryLoading::Checked {
            assert!(
                self.check_guest_holds(memory),
                "checked loading: the guest holds memory other than the state's"
            );
        }
        self.run();
        // What the instruction changed is part of what the guest holds, which is known again once all is read back.
        self.read_back(&loaded, processor, &mut |mutation: MemoryDelta<
            '_,
            Arm64<User>,
        >| {
            held.push(mutation.clone().into_owned());
            mutation_hook.on_memory_mutated(mutation);
        });
        self.resident.memory = Held::Known(held);
    }
}

/// Returns the most resident pages the state needs, frames and lower tables, from empty resident pages; `None` when
/// they cannot hold it, or when it reaches addresses the guest's tables do not translate.
fn count_resident_pages(memory: &[MemoryDelta<'_, Arm64<User>>]) -> Option<u64> {
    let mut pages: u64 = 0;
    for delta in memory {
        let (addresses, needed_per_page) = match delta {
            MemoryDelta::ValueAssignment { start, bytes } => {
                (*start..start.checked_add(bytes.len() as u64)?, 1)
            }
            MemoryDelta::PermissionAssignment {
                addresses,
                permission: PagePermission::Mapped { .. },
            } => (addresses.clone(), 1 + u64::from(LEAF_LEVEL - TOP_LEVEL)),
            MemoryDelta::PermissionAssignment { .. } => continue,
        };
        if addresses.is_empty() {
            continue;
        }
        if addresses.end > Arm64::<User>::ADDRESSABLE.end {
            return None;
        }
        pages = pages.saturating_add(
            (addresses.end.div_ceil(PAGE_SIZE) - addresses.start / PAGE_SIZE) * needed_per_page,
        );
    }
    (pages <= (RESIDENT.end - RESIDENT_TOP - PAGE_SIZE) / PAGE_SIZE).then_some(pages)
}

/// Takes the next page of the given pages.
fn allocate(next_free_page: &mut u64) -> u64 {
    let page = *next_free_page;
    *next_free_page += PAGE_SIZE;
    page
}

/// Returns the first address of each page in the addresses, which start on a page.
fn to_pages(addresses: &Range<u64>) -> impl Iterator<Item = u64> + use<> {
    assert!(
        addresses.start.is_multiple_of(PAGE_SIZE),
        "a chip gives permissions to whole pages, not {addresses:#x?}"
    );
    (addresses.start..addresses.end).step_by(PAGE_SIZE as usize)
}

fn unmapped_table(level: u32) -> u64 {
    UNMAPPED_TABLES + u64::from(level - TOP_LEVEL - 1) * PAGE_SIZE
}

/// Returns what every entry of a table at the level holds where it maps nothing: the way to an invalid last-level
/// entry.
fn to_unmapped_entry(level: u32) -> u64 {
    if level == LEAF_LEVEL {
        0
    } else {
        to_table_entry(unmapped_table(level + 1))
    }
}

fn to_table_entry(table: u64) -> u64 {
    table | (El1Attributes::VALID | El1Attributes::TABLE_OR_PAGE).bits() as u64
}

fn to_page_entry(frame: u64, data_access: DataAccess, instruction_fetch: InstructionFetch) -> u64 {
    let access = match data_access {
        DataAccess::Forbidden => El1Attributes::empty(),
        DataAccess::ReadOnly => El1Attributes::USER | El1Attributes::READ_ONLY,
        DataAccess::ReadWrite => El1Attributes::USER,
    };
    let fetch = match instruction_fetch {
        InstructionFetch::Forbidden => El1Attributes::UXN,
        InstructionFetch::Allowed => El1Attributes::empty(),
    };
    let normal = to_handler_attributes(ATTRIBUTE_NORMAL).difference(El1Attributes::UXN);
    // Not global, so that a slot's translations are tagged with its ASID.
    let attributes = normal
        | El1Attributes::TABLE_OR_PAGE
        | El1Attributes::PXN
        | El1Attributes::NON_GLOBAL
        | access
        | fetch;
    frame | attributes.bits() as u64
}

// A page no delta gives a permission to is left as fresh tables leave it, which must be the architecture's default.
const _: () = assert!(matches!(
    Arm64::<User>::DEFAULT_PERMISSION,
    PagePermission::Unmapped
));

/// Returns the permission of each page the deltas give one, by its first address; a later delta wins.
fn to_page_permissions(memory: &[MemoryDelta<'_, Arm64<User>>]) -> BTreeMap<u64, PagePermission> {
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
fn compose_page(page: u64, memory: &[MemoryDelta<'_, Arm64<User>>], bytes: &mut [u8]) {
    bytes.fill(0);
    for delta in memory {
        if let MemoryDelta::ValueAssignment {
            start,
            bytes: values,
        } = delta
        {
            // Addresses wrap after the last one, as the memory diff has them.
            let (into_values, into_page) = (page.wrapping_sub(*start), start.wrapping_sub(page));
            if into_values < values.len() as u64 {
                let length = (values.len() - into_values as usize).min(bytes.len());
                bytes[..length].copy_from_slice(&values[into_values as usize..][..length]);
            } else if into_page < bytes.len() as u64 {
                let length = values.len().min(bytes.len() - into_page as usize);
                bytes[into_page as usize..][..length].copy_from_slice(&values[..length]);
            }
        }
    }
}

fn to_registers(processor: &Processor<()>) -> Registers {
    let el0 = SPSR_EL1::M::EL0t
        + SPSR_EL1::SS.val(1)
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
        alignment: 0,
        vector: processor.vector_registers,
    }
}

fn to_processor(registers: &Registers) -> Processor<()> {
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
        exception: syndrome::to_exception(registers.syndrome, registers.fault_address),
        level_specific: (),
    }
}
