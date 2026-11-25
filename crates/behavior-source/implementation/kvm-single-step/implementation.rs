//! A behavior source that really runs one ARM64 instruction: at EL0 of a KVM virtual machine with software step on,
//! under a handler at EL1 that saves the registers when the step, or the instruction's own exception, ends it.
//!
//! Each state is a slot in guest memory: its registers before and after, and translation tables and pages of its own.
//! Guest memory is reached only through `VolatileBytes`.

#![cfg(all(target_arch = "aarch64", target_os = "linux"))]

use std::borrow::Cow;
use std::collections::{BTreeMap, BinaryHeap};
use std::mem::{self, offset_of};
use std::ops::Range;

use aarch64_cpu::registers::{
    CPACR_EL1, ID_AA64MMFR0_EL1, ID_AA64MMFR2_EL1, ID_AA64PFR0_EL1, MDSCR_EL1, SCTLR_EL1, SPSR_EL1,
    TTBR0_EL1,
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
    VECTOR_BYTES, VECTOR_TABLE_BYTES, VECTORS, level_index, level_span, linear,
};
use crate::machine::{GuestMemory, Machine, MachineError, Register, create_machine};
use crate::slot::{Ending, Registers, Slot};

mod boot;
mod guest_layout;
mod machine;
mod slot;
mod syndrome;
#[cfg(test)]
mod tests;

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
/// The first page after the set-up's own.
const ARENA_START: u64 = UNMAPPED_TABLES + LEAF_LEVEL as u64 * PAGE_SIZE;
/// Bytes of the arena: room for a state that maps 256 MiB with its tables, yet few enough that every frame the guest
/// ever touches, these and the resident pages, stays under half of the 1 GiB a process holding the source may be given;
/// the rest of guest memory is never touched.
const ARENA_BYTES: u64 = 384 << 20;
/// What the slots of a full load take their pages from.
const ARENA: Range<u64> = ARENA_START..ARENA_START + ARENA_BYTES;
/// The pages the guest keeps between states when loading incrementally: its slot, its top-level table, then lower
/// tables and frames as pages first need them.
const RESIDENT: Range<u64> = MEMORY_BASE + MEMORY_BYTES - (64 << 20)..MEMORY_BASE + MEMORY_BYTES;

// The arena lies below the resident pages, and every page the guest can touch is under half of its memory.
const _: () = assert!(ARENA.end <= RESIDENT.start);
const _: () =
    assert!(ARENA.end - MEMORY_BASE + (RESIDENT.end - RESIDENT.start) <= MEMORY_BYTES / 2);
const RESIDENT_SLOT: u64 = RESIDENT.start;
const RESIDENT_TOP: u64 = RESIDENT.start + PAGE_SIZE;

/// Pages a slot takes besides those its state maps: the slot itself and its top-level table.
const SLOT_PAGES: u64 = 2;

/// Pages the arena holds, and so the most one state takes when loaded in full.
const ARENA_PAGES: u64 = (ARENA.end - ARENA.start) / PAGE_SIZE;

/// Pages the resident frames and lower tables can take, after the resident slot and top-level table.
const RESIDENT_ROOM: u64 = (RESIDENT.end - RESIDENT_TOP - PAGE_SIZE) / PAGE_SIZE;

/// Bytes of a writable run compared at once after the instruction, so that the host holds a window of a large run and
/// never the whole of it.
const WINDOW_BYTES: u64 = 16 * PAGE_SIZE;

// VBAR_EL1 holds the vectors aligned to their size, and the entry follows the last vector.
const _: () = assert!(HANDLER.is_multiple_of(VECTOR_TABLE_BYTES));
const _: () = assert!(HANDLER_ENTRY_OFFSET == VECTORS * VECTOR_BYTES);

/// The most changes the memory the guest holds and the next state's may have together for `diff_memory` to pair them,
/// whose time grows with the product of the two lengths; longer ones are written whole into emptied resident pages.
const MOST_CHANGES_PAIRED: usize = 64;

/// Entries each of the resolving's buffers keeps between states, so that one state with very many changes does not keep
/// its room for every state after it.
const KEPT_ENTRIES: usize = 1024;

/// One past the last address, where a value assignment goes on from zero.
const ADDRESSES_END: u128 = 1 << u64::BITS;

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

/// [E0PD1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/TCR-EL1--Translation-Control-Register--EL1-#:~:text=E0PD1%2C%20bit%20%5B56%5D), which makes every EL0 access to the handler's half fault at level 0, as Linux sets it on a chip with FEAT_E0PD.
const E0PD1: u64 = 1 << 56;

/// A slot loaded for the next run, and the pages of it that EL0 may write.
struct LoadedSlot {
    slot: u64,
    writable: Range<usize>,
}

/// Consecutive pages EL0 may write.
struct WritableRun {
    addresses: Range<u64>,
    /// The first frame when the pages lie on consecutive frames, which a full load always gives them; otherwise
    /// `None`, and each page's frame is the resident one.
    frame: Option<u64>,
}

/// Pages a program can use that hold one permission other than the default, which leaves a page unmapped.
#[derive(Clone, Debug)]
struct MappedRange {
    /// Whole pages, below the end of the addressable ones.
    addresses: Range<u64>,
    data_access: DataAccess,
    instruction_fetch: InstructionFetch,
}

/// The addresses a program can use that one change of a state reaches, and where the change lies in the state's list.
#[derive(Clone, Debug)]
struct Reach {
    addresses: Range<u64>,
    change: usize,
}

/// What a state's changes leave in the memory a program can use, worked out in time that grows with the number of
/// changes and never with the pages they cover.
#[derive(Default)]
struct Resolved {
    /// The pages that hold a permission other than the default, in address order and apart.
    mapped: Vec<MappedRange>,
    /// The runs of addresses whose bytes a value assignment gives, in address order and apart, each by the latest
    /// assignment that reaches it; every other byte is zero.
    values: Vec<Reach>,
    // Room the resolving reuses from state to state.
    reaches: Vec<Reach>,
    latest: Vec<Reach>,
    bounds: Vec<u64>,
    open: BinaryHeap<(usize, u64)>,
}

/// Why this source did not run a state, found before the processor or the hook is written.
///
/// The first two are found before the guest is written; after any other, the next state runs on a new machine.
#[derive(Debug)]
pub enum Refusal {
    /// A permission starts or ends inside a page a program can use, which no chip can give.
    PermissionWithinAPage(Range<u64>),
    /// The pages the state maps, with the tables that lead to them and its slot, are more than the guest has.
    MorePagesThanTheGuestHas { pages: u64, room: u64 },
    /// A call to KVM that runs the state failed, in KVM's words.
    KvmFailed(String),
    /// An exception other than one from EL0 reached the handler, with ESR_EL1, ELR_EL1 and SPSR_EL1 as it saved them.
    UnexpectedException {
        syndrome: u64,
        return_address: u64,
        saved_program_status: u64,
    },
    /// The guest rang the doorbell without the handler ending the state's slot.
    SlotNotRun,
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
    /// The bytes a window of a writable run holds before the instruction, as the state gives them.
    bytes_before: Vec<u8>,
    /// The bytes a window of a writable run holds after the instruction, read back from guest memory.
    bytes_after: Vec<u8>,
    /// What the state being run leaves in memory, kept to reuse its room.
    resolved: Resolved,
    loading: MemoryLoading,
    entry_path: EntryPath,
    /// The slot TPIDR_EL1 names while the CPU waits at the doorbell of a run that ended there, which goes on to the
    /// entry with the OS lock open, PSTATE that of the handler and every other system register as the setup left it;
    /// `None` when the CPU may be anywhere else.
    parked_at: Option<u64>,
    resident: Resident,
    /// Set during a run and after a failed one, when the CPU may be anywhere: the next state makes a new machine.
    renew_machine: bool,
}

/// What the guest holds between states when loading incrementally.
struct Resident {
    /// The memory the guest holds, as far as it is known.
    memory: Held,
    /// The frame of each virtual page the guest has held bytes or a permission on; any other page holds zeros.
    frames: BTreeMap<u64, u64>,
    /// The address of the valid last-level entry of each virtual page the resident tables map; every other page is
    /// unmapped, so unmapping a range of any size visits only these.
    mapped: BTreeMap<u64, u64>,
    next_free_page: u64,
}

/// Whether the machine knows what the guest's resident frames and tables hold.
enum Held {
    /// The changes of the state loaded last, then those its instruction made, set only once its read-back is done.
    Known(Vec<MemoryDelta<'static, Arm64<User>>>),
    /// From the first write of a load until its read-back is done, before the first load, and after a state with more
    /// changes than are paired: the next load resets every resident frame and table before it writes.
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
            resolved: Resolved::default(),
            loading,
            entry_path: EntryPath::Plain,
            parked_at: None,
            resident: Resident {
                memory: Held::Unknown,
                frames: BTreeMap::new(),
                mapped: BTreeMap::new(),
                next_free_page: RESIDENT_TOP + PAGE_SIZE,
            },
            renew_machine: false,
        };
        source.set_up()?;
        Ok(source)
    }

    /// Replaces the machine with a new one that loads and enters as this one does.
    fn renew(&mut self) -> Result<(), Refusal> {
        let mut renewed = Self::with_loading(self.loading)
            .map_err(|MachineError(error)| Refusal::KvmFailed(error))?;
        renewed.entry_path = self.entry_path;
        *self = renewed;
        Ok(())
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
        let has_e0pd =
            ID_AA64MMFR2_EL1::E0PD.read(self.machine.get(Register::MemoryModelFeatures2)?) != 0;
        let tcr = if has_e0pd { tcr | E0PD1 } else { tcr };
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
    }

    /// Writes a state, whose memory is resolved, into a slot after the batch's others, or gives `None`, having written
    /// nothing, when the batch has no room for it.
    fn load(
        &mut self,
        processor: &Processor<()>,
        memory: &[MemoryDelta<'_, Arm64<User>>],
        resolved: &Resolved,
    ) -> Option<LoadedSlot> {
        if self.slot_count == self.asid_count {
            return None;
        }
        let mapped = &resolved.mapped;
        let most_pages = SLOT_PAGES + count_guest_pages(mapped);
        if most_pages > (ARENA.end - self.next_free_page) / PAGE_SIZE {
            return None;
        }
        let slot = allocate(&mut self.next_free_page);
        let top = allocate(&mut self.next_free_page);
        self.fill_table(top, to_unmapped_entry(TOP_LEVEL));
        // Frames first, in address order, so that consecutive pages lie on consecutive frames.
        let first_frame = self.next_free_page;
        let writable_start = self.writable_runs.len();
        let mut bytes = vec![0; PAGE_SIZE as usize];
        for range in mapped {
            let range_frame = self.next_free_page;
            for page in to_pages(&range.addresses) {
                compose_bytes(page, &resolved.values, memory, &mut bytes);
                let frame = allocate(&mut self.next_free_page);
                self.write_bytes(frame, &bytes);
            }
            if range.data_access != DataAccess::ReadWrite {
                continue;
            }
            match self.writable_runs[writable_start..].last_mut() {
                Some(run) if run.addresses.end == range.addresses.start => {
                    run.addresses.end = range.addresses.end;
                }
                _ => self.writable_runs.push(WritableRun {
                    addresses: range.addresses.clone(),
                    frame: Some(range_frame),
                }),
            }
        }
        let mut next_free_page = self.next_free_page;
        let mut frame = first_frame;
        for range in mapped {
            for page in to_pages(&range.addresses) {
                let leaf = self.find_leaf_entry(&mut next_free_page, top, page);
                self.write_entry(
                    leaf,
                    to_page_entry(frame, range.data_access, range.instruction_fetch),
                );
                frame += PAGE_SIZE;
            }
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
    /// holds once the run's read-back is done, or `None` when it has more changes than are paired; until then what the
    /// guest holds is unknown.
    fn prepare(
        &mut self,
        processor: &Processor<()>,
        memory: &[MemoryDelta<'_, Arm64<User>>],
        resolved: &Resolved,
        resident_pages: u64,
    ) -> (LoadedSlot, Option<Vec<MemoryDelta<'static, Arm64<User>>>>) {
        // The pages the state needs from empty resident pages bound the frames and tables it adds to these.
        let has_room = self.resident.next_free_page + resident_pages * PAGE_SIZE <= RESIDENT.end;
        // Unknown from here on: an error or a panic before the caller stores the next record makes the next load reset.
        let mut held = match mem::replace(&mut self.resident.memory, Held::Unknown) {
            Held::Known(held) if has_room && held.len() + memory.len() <= MOST_CHANGES_PAIRED => {
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
                held
            }
            Held::Known(_) | Held::Unknown => {
                self.reset_resident();
                self.write_resident_state(memory, resolved);
                Vec::new()
            }
        };
        // What EL0 may write, to find what the instruction changes.
        for range in &resolved.mapped {
            if range.data_access != DataAccess::ReadWrite {
                continue;
            }
            for page in to_pages(&range.addresses) {
                let frame = self.resident.frames[&page];
                match self.writable_runs.last_mut() {
                    Some(run) if run.addresses.end == page => {
                        if run
                            .frame
                            .is_some_and(|first| first + (page - run.addresses.start) != frame)
                        {
                            run.frame = None;
                        }
                        run.addresses.end = page + PAGE_SIZE;
                    }
                    _ => self.writable_runs.push(WritableRun {
                        addresses: page..page + PAGE_SIZE,
                        frame: Some(frame),
                    }),
                }
            }
        }
        let held = (memory.len() <= MOST_CHANGES_PAIRED).then(|| {
            held.clear();
            held.extend(memory.iter().cloned().map(MemoryDelta::into_owned));
            held
        });
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
        self.resident.mapped.clear();
        self.resident.next_free_page = RESIDENT_TOP + PAGE_SIZE;
    }

    /// Writes a state into resident pages that hold nothing: each run of its bytes once, then each page it maps.
    fn write_resident_state(
        &mut self,
        memory: &[MemoryDelta<'_, Arm64<User>>],
        resolved: &Resolved,
    ) {
        for run in &resolved.values {
            if let Some(bytes) = to_run_bytes(run, memory) {
                self.write_resident_bytes(run.addresses.start, bytes);
            }
        }
        for range in &resolved.mapped {
            let permission = PagePermission::Mapped {
                data_access: range.data_access,
                instruction_fetch: range.instruction_fetch,
            };
            self.write_resident_permission(&range.addresses, permission);
        }
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

    /// Gives the pages a program can use among the addresses, which come from a state the source runs, the permission.
    fn write_resident_permission(&mut self, addresses: &Range<u64>, permission: PagePermission) {
        let addresses = to_addressable(addresses);
        if addresses.is_empty() {
            return;
        }
        match permission {
            PagePermission::Mapped {
                data_access,
                instruction_fetch,
            } => {
                for page in to_pages(&addresses) {
                    let entry =
                        to_page_entry(self.find_frame(page), data_access, instruction_fetch);
                    let leaf = match self.resident.mapped.get(&page) {
                        Some(&leaf) => leaf,
                        None => {
                            let mut next_free_page = self.resident.next_free_page;
                            let leaf =
                                self.find_leaf_entry(&mut next_free_page, RESIDENT_TOP, page);
                            assert!(next_free_page <= RESIDENT.end, "the resident pages fit");
                            self.resident.next_free_page = next_free_page;
                            self.resident.mapped.insert(page, leaf);
                            leaf
                        }
                    };
                    self.write_entry(leaf, entry);
                }
            }
            // Only the pages mapped now change, however many the addresses cover.
            PagePermission::Unmapped => {
                let mut mapped = mem::take(&mut self.resident.mapped);
                for (_, leaf) in mapped.extract_if(addresses, |_, _| true) {
                    self.write_entry(leaf, to_unmapped_entry(LEAF_LEVEL));
                }
                self.resident.mapped = mapped;
            }
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
    fn check_guest_holds(
        &self,
        memory: &[MemoryDelta<'_, Arm64<User>>],
        resolved: &Resolved,
    ) -> bool {
        let mut same = true;
        let mut bytes = vec![0; PAGE_SIZE as usize];
        let mut held = vec![0; PAGE_SIZE as usize];
        // Every page the state writes or the guest has a frame for: the frame's bytes, or zeros without one.
        let mut pages: Vec<u64> = resolved
            .values
            .iter()
            .flat_map(|run| {
                to_pages(&((run.addresses.start & !(PAGE_SIZE - 1))..run.addresses.end))
            })
            .chain(self.resident.frames.keys().copied())
            .collect();
        pages.sort_unstable();
        pages.dedup();
        for page in pages {
            compose_bytes(page, &resolved.values, memory, &mut bytes);
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
        let mut wanted = BTreeMap::new();
        let mut frameless = false;
        for range in &resolved.mapped {
            for page in to_pages(&range.addresses) {
                match self.resident.frames.get(&page) {
                    Some(&frame) => {
                        let entry =
                            to_page_entry(frame, range.data_access, range.instruction_fetch);
                        wanted.insert(page, entry);
                    }
                    None => frameless = true,
                }
            }
        }
        if leaves != wanted || frameless {
            eprintln!(
                "checked loading: the guest's tables map {leaves:#x?}, the state {wanted:#x?}"
            );
            same = false;
        }
        // Every page the tables map is one the machine knows to unmap.
        if !leaves.keys().eq(self.resident.mapped.keys()) {
            eprintln!(
                "checked loading: the guest's tables map {:#x?}, the machine knows of {:#x?}",
                leaves.keys(),
                self.resident.mapped.keys()
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
    fn run(&mut self) -> Result<(), Refusal> {
        let Some(first) = self.first_slot else {
            return Ok(());
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
        self.renew_machine = true;
        for &(register, value) in writes {
            self.machine
                .set(register, value)
                .map_err(|MachineError(error)| Refusal::KvmFailed(error))?;
        }
        self.machine
            .run()
            .map_err(|MachineError(error)| Refusal::KvmFailed(error))?;
        self.renew_machine = false;
        // With one slot the handler never moved TPIDR_EL1 on (a batch leaves it at a slot after the first).
        if self.first_slot == self.last_slot {
            self.parked_at = Some(linear(first));
        }
        Ok(())
    }

    /// Gives the processor after a slot's run, and each run of bytes it changed from the state's memory, read back from
    /// guest memory.
    fn read_back(
        &mut self,
        loaded: &LoadedSlot,
        memory: &[MemoryDelta<'_, Arm64<User>>],
        values: &[Reach],
        processor: &mut Processor<()>,
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<(), Refusal> {
        let slot = self.read_slot(loaded.slot);
        if slot.ending != Ending::ExceptionFromEl0 as u64 {
            self.renew_machine = true;
            return Err(if slot.ending == Ending::Unexpected as u64 {
                Refusal::UnexpectedException {
                    syndrome: slot.after.syndrome,
                    return_address: slot.after.program_counter,
                    saved_program_status: slot.after.saved_program_status,
                }
            } else {
                Refusal::SlotNotRun
            });
        }
        *processor = to_processor(&slot.after);
        let mut before = mem::take(&mut self.bytes_before);
        let mut after = mem::take(&mut self.bytes_after);
        for run in &self.writable_runs[loaded.writable.clone()] {
            // A change that reaches the end of its window, held until the next window shows where it ends.
            let mut pending = None;
            let mut window_start = run.addresses.start;
            while window_start < run.addresses.end {
                let window = window_start..run.addresses.end.min(window_start + WINDOW_BYTES);
                window_start = window.end;
                let length = (window.end - window.start) as usize;
                before.resize(length, 0);
                compose_bytes(window.start, values, memory, &mut before);
                after.resize(length, 0);
                self.read_writable(run, &window, &mut after);
                if window == run.addresses {
                    report_changed_bytes(window.start, &before, &after, mutation_hook);
                } else {
                    report_changed_bytes(
                        window.start,
                        &before,
                        &after,
                        &mut |mutation: MemoryDelta<'_, Arm64<User>>| {
                            join_across_windows(&mut pending, mutation, window.end, mutation_hook);
                        },
                    );
                }
            }
            report_pending(pending, mutation_hook);
        }
        self.bytes_before = before;
        self.bytes_after = after;
        Ok(())
    }

    /// Reads the bytes a writable run's frames hold at the addresses, which lie in it and start on a page.
    fn read_writable(&self, run: &WritableRun, addresses: &Range<u64>, into: &mut [u8]) {
        match run.frame {
            Some(frame) => self.read_bytes(frame + (addresses.start - run.addresses.start), into),
            None => {
                for (page, held) in to_pages(addresses).zip(into.chunks_mut(PAGE_SIZE as usize)) {
                    self.read_bytes(self.resident.frames[&page], held);
                }
            }
        }
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

    /// Runs one state with room for its resolved memory, writing the processor and the hook only once it has run.
    fn execute_resolved(
        &mut self,
        processor: &mut Processor<()>,
        memory: &[MemoryDelta<'_, Arm64<User>>],
        resolved: &mut Resolved,
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<(), Refusal> {
        resolve_memory(memory, resolved)?;
        self.start_batch();
        let resident_pages = match self.loading {
            MemoryLoading::Full => None,
            MemoryLoading::Incremental | MemoryLoading::Checked => {
                count_resident_pages(memory, &resolved.mapped)
            }
        };
        match resident_pages {
            None => {
                // An empty batch has room for every state the guest can hold.
                let Some(loaded) = self.load(processor, memory, resolved) else {
                    return Err(Refusal::MorePagesThanTheGuestHas {
                        pages: SLOT_PAGES + count_guest_pages(&resolved.mapped),
                        room: ARENA_PAGES,
                    });
                };
                if self.loading != MemoryLoading::Full {
                    self.resident.memory = Held::Unknown;
                }
                self.run()?;
                self.read_back(&loaded, memory, &resolved.values, processor, mutation_hook)
            }
            Some(resident_pages) => {
                let (loaded, mut held) = self.prepare(processor, memory, resolved, resident_pages);
                if self.loading == MemoryLoading::Checked {
                    assert!(
                        self.check_guest_holds(memory, resolved),
                        "checked loading: the guest holds memory other than the state's"
                    );
                }
                self.run()?;
                // What the instruction changed is part of what the guest holds, which is known again once all is read
                // back.
                self.read_back(
                    &loaded,
                    memory,
                    &resolved.values,
                    processor,
                    &mut |mutation: MemoryDelta<'_, Arm64<User>>| {
                        if let Some(held) = &mut held {
                            held.push(mutation.clone().into_owned());
                        }
                        mutation_hook.on_memory_mutated(mutation);
                    },
                )?;
                self.resident.memory = held.map_or(Held::Unknown, Held::Known);
                Ok(())
            }
        }
    }
}

impl BehaviorSource for KvmSingleStepMachine {
    type Architecture = Arm64<User>;
    type Error = Refusal;

    /// Reports each run of bytes whose value changed: a store of the bytes already there is not reported.
    fn execute_single_instruction_in_place(
        &mut self,
        processor: &mut Processor<()>,
        memory: &[MemoryDelta<'_, Arm64<User>>],
        mutation_hook: &mut impl MemorySink<Arm64<User>>,
    ) -> Result<(), Refusal> {
        if self.renew_machine {
            self.renew()?;
        }
        let mut resolved = mem::take(&mut self.resolved);
        let executed = self.execute_resolved(processor, memory, &mut resolved, mutation_hook);
        release_excess_room(&mut resolved);
        self.resolved = resolved;
        executed
    }
}

/// Works out what the state's changes leave in the memory a program can use, a later change winning where two meet: an
/// empty range assigns nothing, `Unmapped` only cuts what came before, and nothing past the addressable addresses is
/// kept, since every access there faults at level 0 whatever its page holds.
fn resolve_memory(
    memory: &[MemoryDelta<'_, Arm64<User>>],
    resolved: &mut Resolved,
) -> Result<(), Refusal> {
    let Resolved {
        mapped,
        values,
        reaches,
        latest,
        bounds,
        open,
    } = resolved;
    reaches.clear();
    for (change, delta) in memory.iter().enumerate() {
        let MemoryDelta::PermissionAssignment { addresses, .. } = delta else {
            continue;
        };
        let addressable = to_addressable(addresses);
        if addressable.is_empty() {
            continue;
        }
        if !(addressable.start.is_multiple_of(PAGE_SIZE)
            && addressable.end.is_multiple_of(PAGE_SIZE))
        {
            return Err(Refusal::PermissionWithinAPage(addresses.clone()));
        }
        reaches.push(Reach {
            addresses: addressable,
            change,
        });
    }
    find_latest(reaches, bounds, open, latest);
    mapped.clear();
    mapped.extend(
        latest
            .iter()
            .filter_map(|reach| match memory.get(reach.change) {
                Some(MemoryDelta::PermissionAssignment {
                    permission:
                        PagePermission::Mapped {
                            data_access,
                            instruction_fetch,
                        },
                    ..
                }) => Some(MappedRange {
                    addresses: reach.addresses.clone(),
                    data_access: *data_access,
                    instruction_fetch: *instruction_fetch,
                }),
                _ => None,
            }),
    );
    reaches.clear();
    let end = u128::from(Arm64::<User>::ADDRESSABLE.end);
    for (change, delta) in memory.iter().enumerate() {
        let MemoryDelta::ValueAssignment { start, bytes } = delta else {
            continue;
        };
        let value_end = u128::from(*start) + bytes.len() as u128;
        // A value that runs past the last address goes on from zero.
        for piece in [
            u128::from(*start)..value_end,
            0..value_end.saturating_sub(ADDRESSES_END),
        ] {
            let addresses = piece.start.min(end) as u64..piece.end.min(end) as u64;
            if !addresses.is_empty() {
                reaches.push(Reach { addresses, change });
            }
        }
    }
    find_latest(reaches, bounds, open, values);
    Ok(())
}

/// Writes into `latest` the addresses the reaches cover, in address order and apart, each run with the latest change
/// that reaches it, in time that grows with the number of reaches and never with the addresses they cover.
fn find_latest(
    reaches: &mut Vec<Reach>,
    bounds: &mut Vec<u64>,
    open: &mut BinaryHeap<(usize, u64)>,
    latest: &mut Vec<Reach>,
) {
    latest.clear();
    reaches.sort_unstable_by_key(|reach| reach.addresses.start);
    // Reaches apart, as a state's mostly are, each hold their own addresses: they are the answer, and the room `latest`
    // had becomes the reaches' for the next resolving.
    if reaches
        .iter()
        .zip(reaches.iter().skip(1))
        .all(|(first, second)| first.addresses.end <= second.addresses.start)
    {
        mem::swap(reaches, latest);
        return;
    }
    bounds.clear();
    bounds.extend(
        reaches
            .iter()
            .flat_map(|reach| [reach.addresses.start, reach.addresses.end]),
    );
    bounds.sort_unstable();
    bounds.dedup();
    open.clear();
    let mut starting = reaches.iter().peekable();
    // Between two neighboring bounds the same reaches are open, and the latest of them holds the addresses.
    for (&at, &end) in bounds.iter().zip(bounds.iter().skip(1)) {
        while let Some(reach) = starting.next_if(|reach| reach.addresses.start == at) {
            open.push((reach.change, reach.addresses.end));
        }
        while open.peek().is_some_and(|&(_, open_end)| open_end <= at) {
            open.pop();
        }
        let Some(&(change, _)) = open.peek() else {
            continue;
        };
        match latest.last_mut() {
            Some(last) if last.change == change && last.addresses.end == at => {
                last.addresses.end = end;
            }
            _ => latest.push(Reach {
                addresses: at..end,
                change,
            }),
        }
    }
}

/// Gives back the room of the resolving's buffers beyond what a usual state needs.
fn release_excess_room(resolved: &mut Resolved) {
    resolved.mapped.shrink_to(KEPT_ENTRIES);
    resolved.values.shrink_to(KEPT_ENTRIES);
    resolved.reaches.shrink_to(KEPT_ENTRIES);
    resolved.latest.shrink_to(KEPT_ENTRIES);
    resolved.bounds.shrink_to(KEPT_ENTRIES);
    resolved.open.shrink_to(KEPT_ENTRIES);
}

/// Returns the bytes a resolved run of values holds, from the value assignment it names.
fn to_run_bytes<'memory>(
    run: &Reach,
    memory: &'memory [MemoryDelta<'_, Arm64<User>>],
) -> Option<&'memory [u8]> {
    let Some(MemoryDelta::ValueAssignment { start, bytes }) = memory.get(run.change) else {
        return None;
    };
    // Offsets are taken modulo 2^64, as a value that runs past the last address goes on from zero.
    let from = run.addresses.start.wrapping_sub(*start) as usize;
    bytes.get(from..from + (run.addresses.end - run.addresses.start) as usize)
}

/// Returns the pages mapped ranges take in a guest whose tables map nothing else: a frame for each page, and the tables
/// below the top level that lead to them.
fn count_guest_pages(mapped: &[MappedRange]) -> u64 {
    let frames: u64 = mapped
        .iter()
        .map(|range| (range.addresses.end - range.addresses.start) / PAGE_SIZE)
        .sum();
    let tables: u64 = (TOP_LEVEL + 1..=LEAF_LEVEL)
        .map(|level| count_tables(mapped, level))
        .sum();
    frames + tables
}

/// Returns how many tables of the level the pages of the ranges, which lie in address order, lie under.
fn count_tables(mapped: &[MappedRange], level: u32) -> u64 {
    // One table of the level covers what one entry of the level above does.
    let table_bits = level_span(level - 1).trailing_zeros();
    let mut tables = 0;
    let mut last_table = None;
    for range in mapped {
        let first = range.addresses.start >> table_bits;
        let last = (range.addresses.end - 1) >> table_bits;
        tables += last - first + 1 - u64::from(last_table == Some(first));
        last_table = Some(last);
    }
    tables
}

/// Returns the most resident pages the state needs, frames and lower tables, from empty resident pages; `None` when
/// they cannot hold it, or when its values reach past the addressable addresses.
fn count_resident_pages(
    memory: &[MemoryDelta<'_, Arm64<User>>],
    mapped: &[MappedRange],
) -> Option<u64> {
    let mut pages = count_guest_pages(mapped);
    for delta in memory {
        if let MemoryDelta::ValueAssignment { start, bytes } = delta
            && !bytes.is_empty()
        {
            let end = start.checked_add(bytes.len() as u64)?;
            if end > Arm64::<User>::ADDRESSABLE.end {
                return None;
            }
            pages += end.div_ceil(PAGE_SIZE) - start / PAGE_SIZE;
        }
    }
    (pages <= RESIDENT_ROOM).then_some(pages)
}

/// Passes a change of a window on to the hook, joined to the one before it where that one ran to the end of the last
/// window; a change that runs to the end of this window is held, so that each run of changed bytes is one change.
fn join_across_windows(
    pending: &mut Option<(u64, Vec<u8>)>,
    mutation: MemoryDelta<'_, Arm64<User>>,
    window_end: u64,
    mutation_hook: &mut impl MemorySink<Arm64<User>>,
) {
    let MemoryDelta::ValueAssignment { start, bytes } = mutation else {
        mutation_hook.on_memory_mutated(mutation);
        return;
    };
    match pending {
        Some((pending_start, pending_bytes))
            if *pending_start + pending_bytes.len() as u64 == start =>
        {
            pending_bytes.extend_from_slice(&bytes);
        }
        _ => {
            report_pending(pending.take(), mutation_hook);
            *pending = Some((start, bytes.into_owned()));
        }
    }
    let whole = pending.take_if(|(pending_start, pending_bytes)| {
        *pending_start + pending_bytes.len() as u64 != window_end
    });
    report_pending(whole, mutation_hook);
}

/// Passes a held change, if there is one, on to the hook.
fn report_pending(
    pending: Option<(u64, Vec<u8>)>,
    mutation_hook: &mut impl MemorySink<Arm64<User>>,
) {
    if let Some((start, bytes)) = pending {
        mutation_hook.on_memory_mutated(MemoryDelta::ValueAssignment {
            start,
            bytes: Cow::Owned(bytes),
        });
    }
}

/// Takes the next page of the given pages.
fn allocate(next_free_page: &mut u64) -> u64 {
    let page = *next_free_page;
    *next_free_page += PAGE_SIZE;
    page
}

/// Returns the first address of each page in the addresses, which start on a page.
fn to_pages(addresses: &Range<u64>) -> impl Iterator<Item = u64> + use<> {
    (addresses.start..addresses.end).step_by(PAGE_SIZE as usize)
}

/// Returns the addresses among the given ones that a program can use.
fn to_addressable(addresses: &Range<u64>) -> Range<u64> {
    let end = Arm64::<User>::ADDRESSABLE.end;
    addresses.start.min(end)..addresses.end.min(end)
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

// Every address from zero to the end of the addressable ones is one a program can use.
const _: () = assert!(Arm64::<User>::ADDRESSABLE.start == 0);

/// Writes the bytes the state leaves from the address on, given its resolved runs of values: zeros where none reaches.
fn compose_bytes(
    address: u64,
    values: &[Reach],
    memory: &[MemoryDelta<'_, Arm64<User>>],
    bytes: &mut [u8],
) {
    bytes.fill(0);
    let end = address.saturating_add(bytes.len() as u64);
    let first = values.partition_point(|run| run.addresses.end <= address);
    for run in values[first..]
        .iter()
        .take_while(|run| run.addresses.start < end)
    {
        let Some(run_bytes) = to_run_bytes(run, memory) else {
            continue;
        };
        let reached = run.addresses.start.max(address)..run.addresses.end.min(end);
        let length = (reached.end - reached.start) as usize;
        bytes[(reached.start - address) as usize..][..length].copy_from_slice(
            &run_bytes[(reached.start - run.addresses.start) as usize..][..length],
        );
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
        // Only NZCV: any other bit would put EL0 in another state, which the handler does not expect.
        saved_program_status: u64::from(
            ConditionFlags::from_bits_truncate(processor.condition_flags.bits()).bits(),
        ) | el0.value,
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
