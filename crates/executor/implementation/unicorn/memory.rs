//! One address space of the stand-in: a unicorn engine whose guest memory is host memory lent to it, under the mapping
//! rule of the unicorn trait (`unicorn/interface.rs`).
//!
//! THE SPACE'S RULE, which the hostile-guest test below checks: unicorn maps each host memory once, allowing everything,
//! and the guest reaches a byte only through its TLB, which the TLB hook fills from the page table alone. The space's own
//! pages are one host memory at the physical addresses equal to theirs; each block of shared pages the space shows is a
//! host memory of its own, mapped past them at a physical address no other block of the space ever had. A page refusing
//! an access gets a physical page nothing maps, so the access faults at the address the guest used. Every change of the
//! page table or of the shared ranges flushes the TLB before the guest next runs, and our side reaches every byte only
//! through views of the host memories, so the guest and our side see the same bytes and nothing is copied between turns.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use wcontainers_abi::{
    Access, CpuError, Exit, PAGE_SIZE, Permissions, RegisterBlock, SYSTEM_CALL_ARGUMENT_COUNT,
    SystemCallNumber, SystemCallRequest,
};
use wcontainers_processor_architecture_arm64::encoding::IMM16;
use wcontainers_processor_architecture_arm64::{INSTRUCTION_SIZE, PREDICATE_REGISTERS};
use wcontainers_unicorn::{
    Arch, Arm64CpuModel, HookType, MemType, Mode, Prot, RegisterARM64, RegisterARM64CP, Requests,
    TlbEntry, TlbType, Unicorn, uc_error,
};
use wcontainers_unicorn_direct::DirectUnicorn;
use wcontainers_volatile_bytes::VolatileBytes;

use crate::standin::Unsupported;

/// The engine every space of the stand-in runs on.
pub(crate) type Emulator = DirectUnicorn<Guest>;

// QEMU's exception numbers, as unicorn's interrupt hook reports them.
/// [EXCP_UDEF](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_UNDEFINED: u32 = 1;
/// [EXCP_SWI](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_SYSCALL: u32 = 2;
/// [EXCP_PREFETCH_ABORT](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_PREFETCH_ABORT: u32 = 3;
/// [EXCP_DATA_ABORT](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_DATA_ABORT: u32 = 4;
/// [EXCP_BKPT](https://github.com/unicorn-engine/unicorn/blob/master/qemu/target/arm/cpu.h).
const EXCEPTION_BREAKPOINT: u32 = 7;

/// [PSTATE](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/SPSR-EL1--Saved-Program-Status-Register--EL1-)
/// of a program Linux runs: EL0 on SP_EL0 (M is 0b0000), interrupts unmasked, the condition flags loaded apart.
const USER_PSTATE: u64 = 0;
/// [SCTLR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/SCTLR-EL1--System-Control-Register--EL1-)
/// as op0, op1, CRn, CRm and op2.
const SYSTEM_CONTROL: RegisterARM64CP = RegisterARM64CP::new().op0(3).op1(0).crn(1).crm(0).op2(0);
/// The SCTLR_EL1 bits Linux sets for its programs that QEMU leaves clear: UCI (bit 26, cache maintenance at EL0), UCT
/// (bit 15, CTR_EL0 at EL0) and DZE (bit 14, DC ZVA at EL0).
const USER_CONTROLS_SET: u64 = 1 << 26 | 1 << 15 | 1 << 14;
/// The SCTLR_EL1 bit Linux clears that QEMU sets: nTWI (bit 16), so that EL1 traps WFI at EL0.
const USER_CONTROLS_CLEAR: u64 = 1 << 16;
/// [CPACR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CPACR-EL1--Architectural-Feature-Access-Control-Register)
/// with FPEN (bits 21:20) and ZEN (bits 17:16) at 0b11: neither floating point nor SVE traps at EL0.
const USER_FEATURE_ACCESS: u64 = 0b11 << 20 | 0b11 << 16;
/// [CPTR_EL3](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CPTR-EL3--Architectural-Feature-Trap-Register--EL3-)
/// as op0, op1, CRn, CRm and op2: unicorn's model has EL3, which traps SVE until EZ is set.
const MONITOR_FEATURE_TRAPS: RegisterARM64CP =
    RegisterARM64CP::new().op0(3).op1(6).crn(1).crm(1).op2(2);
/// CPTR_EL3 with EZ (bit 8) set and every trap bit clear.
const MONITOR_TRAPS_NONE: u64 = 1 << 8;
/// [TCR_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/TCR-EL1--Translation-Control-Register--EL1-)
/// as op0, op1, CRn, CRm and op2.
const TRANSLATION_CONTROL: RegisterARM64CP =
    RegisterARM64CP::new().op0(3).op1(0).crn(2).crm(0).op2(2);
/// TCR_EL1 as Linux sets it for a program's addresses: T0SZ (bits 5:0) at 16 for 48-bit addresses, which sets where a
/// pointer authentication code lies, and TBI0 (bit 37), so a data address's top byte is a tag the access ignores.
const USER_TRANSLATION_CONTROL: u64 = (64 - 48) | 1 << 37;
/// [CNTKCTL_EL1](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/CNTKCTL-EL1--Counter-timer-Kernel-Control-Register)
/// as op0, op1, CRn, CRm and op2.
const COUNTER_CONTROL: RegisterARM64CP = RegisterARM64CP::new().op0(3).op1(0).crn(14).crm(1).op2(0);
/// CNTKCTL_EL1 with EL0PCTEN and EL0VCTEN (bits 0 and 1) clear, and every other EL0 timer access too: a read of the
/// counter or its frequency at EL0 traps to EL1, so the kernel answers it from its own clock and a replay repeats it.
const USER_COUNTERS_TRAPPED: u64 = 0;

/// [SCR_EL3](https://developer.arm.com/documentation/ddi0601/latest/AArch64-Registers/SCR-EL3--Secure-Configuration-Register)
/// as op0, op1, CRn, CRm and op2.
const MONITOR_CONTROL: RegisterARM64CP = RegisterARM64CP::new().op0(3).op1(6).crn(1).crm(1).op2(0);
/// SCR_EL3's RW (bit 10): the levels below EL3 are AArch64, as under Linux. Unicorn starts with it clear, and QEMU then
/// translates EL0's addresses as an AArch32 regime's, which leaves TBI0 out (MEASURED: a load from a tagged address
/// faulted at that address until it was set).
const LOWER_LEVELS_AARCH64: u64 = 1 << 10;
/// Bit 55 of an address, which picks the upper half of the address space, where TBI1 is clear on Linux.
const UPPER_HALF: u64 = 1 << 55;
/// The bits of a lower-half address below its tag, the top byte TBI0 leaves out of translation.
pub(crate) const UNTAGGED: u64 = (1 << 56) - 1;

/// The count of instructions or of microseconds that unicorn takes as no limit.
const UNICORN_NO_LIMIT: usize = 0;
const UNICORN_NO_TIMEOUT: u64 = 0;
/// The slice that runs one instruction, at one instruction a nanosecond.
const ONE_INSTRUCTION_NANOSECONDS: u64 = ONE_INSTRUCTION_COUNT as u64;
/// The count of instructions unicorn runs that is one instruction.
const ONE_INSTRUCTION_COUNT: usize = 1;

/// A page's length as an index into host memory.
const PAGE_BYTES: usize = PAGE_SIZE as usize;
/// The pages one bit of a touched-page word stands for.
const PAGES_PER_WORD: u64 = u64::BITS as u64;
/// The most pages a copy between host memories moves at once.
const PAGES_PER_COPY: u64 = 16;

/// Where unicorn maps the first block of shared pages a space shows, past every address of a space.
const SHARED_BASE: u64 = 1 << 52;
/// Where the pages that stand for refusing guest pages start, past every block of shared pages.
const FAULT_PAGES: u64 = 1 << 60;

/// What the engine's hooks read and record: the space's page table, and what stopped the turn.
#[derive(Default)]
pub(crate) struct Guest {
    /// What each page allows, as the kernel last said.
    pages: Vec<Permissions>,
    /// The ranges that show shared pages, by their start, each with its end and the physical address of its start.
    physical: BTreeMap<u64, (u64, u64)>,
    /// The pages of addresses found refusing an access since the TLB was last flushed, each translated to a page of
    /// [`FAULT_PAGES`] by its place here, so the fault hook gives back the address the guest used.
    fault_pages: Vec<u64>,
    exception: Option<u32>,
    fault: Option<(u64, Access)>,
    /// Instructions of the blocks entered, for how long the turn took.
    instructions: u64,
    /// When a turn on the machine's clock ends, whatever it has run: on the clock at the
    /// resolution of the machine's tick, which is cheap to read at every block, as Linux looks at
    /// CPU time at its tick.
    deadline: Option<coarsetime::Instant>,
    /// Raised from another thread to end the turn at the next block.
    interrupt: Option<Arc<AtomicBool>>,
}

/// What stopped a run, as the hooks recorded it.
struct Stopped {
    exception: Option<u32>,
    fault: Option<(u64, Access)>,
    instructions: u64,
}

impl Guest {
    /// Forgets what stopped the last run, for one that ends at the deadline or the interrupt.
    fn start_run(
        &mut self,
        deadline: Option<coarsetime::Instant>,
        interrupt: Option<Arc<AtomicBool>>,
    ) {
        self.exception = None;
        self.fault = None;
        self.instructions = u64::MIN;
        self.deadline = deadline;
        self.interrupt = interrupt;
    }

    /// Returns what stopped the run.
    fn take_stopped(&mut self) -> Stopped {
        self.deadline = None;
        self.interrupt = None;
        Stopped {
            exception: self.exception.take(),
            fault: self.fault.take(),
            instructions: std::mem::take(&mut self.instructions),
        }
    }

    /// Returns the TLB entry of a page of addresses for the access: the physical page that holds it with what the page
    /// allows, or a physical page nothing maps when the page refuses the access.
    fn translate(&mut self, page: u64, needed: Prot) -> TlbEntry {
        let address = untag(page);
        let allowed = usize::try_from(address / PAGE_SIZE)
            .ok()
            .and_then(|index| self.pages.get(index))
            .map_or(Prot::NONE, |permissions| prot(*permissions));
        if allowed.0 & needed.0 != needed.0 {
            let place = self
                .fault_pages
                .iter()
                .position(|kept| *kept == page)
                .unwrap_or_else(|| {
                    self.fault_pages.push(page);
                    self.fault_pages.len() - 1
                });
            // Only the access asked for, so another kind of access to the page is translated again.
            return TlbEntry {
                paddr: FAULT_PAGES + place as u64 * PAGE_SIZE,
                perms: needed,
            };
        }
        let paddr = match self.physical.range(..=address).next_back() {
            Some((start, (end, base))) if address < *end => base + (address - start),
            _ => address,
        };
        TlbEntry {
            paddr,
            perms: allowed,
        }
    }

    /// Returns the address the guest used for the physical address a fault gave, which a page of [`FAULT_PAGES`] stands
    /// for.
    fn to_guest_address(&self, address: u64) -> u64 {
        let place = address
            .checked_sub(FAULT_PAGES)
            .map(|offset| offset / PAGE_SIZE);
        place
            .and_then(|place| self.fault_pages.get(usize::try_from(place).ok()?))
            .map_or(address, |page| page | (address % PAGE_SIZE))
    }
}

/// Returns a lower-half address without its tag, as TBI0 has it, and any other as it is, so an address outside the
/// space reaches no page and faults whole.
fn untag(address: u64) -> u64 {
    if address & UPPER_HALF == 0 {
        address & UNTAGGED
    } else {
        address
    }
}

/// Returns the permission an access needs.
fn to_needed(access: MemType) -> Prot {
    match access {
        MemType::WRITE => Prot::WRITE,
        MemType::FETCH => Prot::EXEC,
        _ => Prot::READ,
    }
}

/// Pages that may hold something other than zeros, one bit each.
#[derive(Clone)]
struct TouchedPages {
    words: Vec<u64>,
}

impl TouchedPages {
    fn new(pages: u64) -> Self {
        Self {
            words: vec![u64::MIN; to_index(pages.div_ceil(PAGES_PER_WORD))],
        }
    }

    fn touch(&mut self, pages: Range<u64>) {
        for page in pages {
            self.words[to_index(page / PAGES_PER_WORD)] |= 1 << (page % PAGES_PER_WORD);
        }
    }

    fn forget(&mut self, page: u64) {
        self.words[to_index(page / PAGES_PER_WORD)] &= !(1 << (page % PAGES_PER_WORD));
    }

    fn contains(&self, page: u64) -> bool {
        self.words[to_index(page / PAGES_PER_WORD)] & (1 << (page % PAGES_PER_WORD)) != 0
    }

    /// Returns the touched pages among the pages, in order, passing over words with none.
    fn list(&self, pages: Range<u64>) -> Vec<u64> {
        let mut listed = Vec::new();
        let mut page = pages.start;
        while page < pages.end {
            if self.words[to_index(page / PAGES_PER_WORD)] == u64::MIN {
                page = (page / PAGES_PER_WORD + 1) * PAGES_PER_WORD;
                continue;
            }
            if self.contains(page) {
                listed.push(page);
            }
            page += 1;
        }
        listed
    }

    /// Returns every touched page, in order, and forgets them all.
    fn take_all(&mut self) -> Vec<u64> {
        let mut taken = Vec::new();
        for (index, word) in self.words.iter_mut().enumerate() {
            let mut left = std::mem::take(word);
            while left != u64::MIN {
                let bit = u64::from(left.trailing_zeros());
                taken.push(index as u64 * PAGES_PER_WORD + bit);
                left &= left - 1;
            }
        }
        taken
    }
}

/// Returns the pages as runs of pages that follow each other, none longer than [`PAGES_PER_COPY`].
fn to_runs(pages: &[u64]) -> Vec<Range<u64>> {
    let mut runs: Vec<Range<u64>> = Vec::new();
    for page in pages {
        match runs.last_mut() {
            Some(run) if run.end == *page && run.end - run.start < PAGES_PER_COPY => run.end += 1,
            _ => runs.push(*page..*page + 1),
        }
    }
    runs
}

/// Copies the bytes at the offsets of one host memory to the same length from an offset of another.
fn copy_between<U: Unicorn<Guest>>(
    from: &U::HostMemory,
    offsets: Range<u64>,
    to: &U::HostMemory,
    at: u64,
) {
    let mut bytes = vec![u8::MIN; to_index(offsets.end - offsets.start)];
    U::host_memory_bytes(from, offsets)
        .expect("copied bytes lie inside their host memory")
        .read(&mut bytes);
    U::host_memory_bytes(to, at..at + bytes.len() as u64)
        .expect("copied bytes lie inside their host memory")
        .write(&bytes);
}

/// Pages that several spaces may show, in host memory that lives while any shows them.
pub(crate) struct SharedPages<U: Unicorn<Guest> = Emulator> {
    length: u64,
    memory: U::HostMemory,
    touched: RefCell<TouchedPages>,
    /// Counts the changes of the bytes no space's store hook sees: our side's writes and the turns of guests that may
    /// write them, so a space drops what it translated from them after one.
    changes: Cell<u64>,
}

impl<U: Unicorn<Guest>> SharedPages<U> {
    /// Returns new zeroed pages for the number of bytes.
    pub(crate) fn new(length: u64) -> Result<Self, CpuError> {
        // A host memory of no bytes is refused, so an empty block holds one page no one reaches.
        let pages = length.div_ceil(PAGE_SIZE).max(1);
        let memory = U::new_host_memory(pages * PAGE_SIZE).map_err(|_| CpuError::OutOfMemory)?;
        Ok(Self {
            length,
            memory,
            touched: RefCell::new(TouchedPages::new(pages)),
            changes: Cell::new(u64::MIN),
        })
    }

    /// Returns new pages holding these pages' bytes, which take host memory only for the pages that hold any.
    pub(crate) fn copy(&self) -> Result<Self, CpuError> {
        let copy = Self::new(self.length)?;
        let touched = self
            .touched
            .borrow()
            .list(0..self.length.div_ceil(PAGE_SIZE));
        for run in to_runs(&touched) {
            let offsets = run.start * PAGE_SIZE..run.end * PAGE_SIZE;
            copy_between::<U>(&self.memory, offsets.clone(), &copy.memory, offsets.start);
        }
        for page in touched {
            copy.touched.borrow_mut().touch(page..page + 1);
        }
        Ok(copy)
    }

    pub(crate) fn len(&self) -> u64 {
        self.length
    }

    /// Copies the bytes from the offset into the buffer, which must lie inside the pages.
    pub(crate) fn read(&self, offset: u64, into: &mut [u8]) {
        U::host_memory_bytes(&self.memory, offset..offset + into.len() as u64)
            .expect("the bytes read lie inside the pages")
            .read(into);
    }

    /// Returns the length of its host memory, whole pages.
    fn host_length(&self) -> u64 {
        self.length.div_ceil(PAGE_SIZE).max(1) * PAGE_SIZE
    }

    /// Copies the buffer to the offset, which must lie inside the pages.
    pub(crate) fn write(&self, offset: u64, from: &[u8]) {
        if from.is_empty() {
            return;
        }
        let end = offset + from.len() as u64;
        U::host_memory_bytes(&self.memory, offset..end)
            .expect("the bytes written lie inside the pages")
            .write(from);
        self.touched
            .borrow_mut()
            .touch(offset / PAGE_SIZE..end.div_ceil(PAGE_SIZE));
        self.changes.set(self.changes.get().wrapping_add(1));
    }

    fn is_touched(&self, page: u64) -> bool {
        self.touched.borrow().contains(page)
    }
}

/// A range of a space that shows shared pages.
pub(crate) struct Shown<U: Unicorn<Guest> = Emulator> {
    end: u64,
    pub(crate) pages: Rc<SharedPages<U>>,
    /// Where in the shared pages the range's start lies.
    offset: u64,
    /// Whether the range was mapped from elsewhere, so a copy shows the pages too instead of their bytes.
    pub(crate) mapped: bool,
}

impl<U: Unicorn<Guest>> Clone for Shown<U> {
    fn clone(&self) -> Self {
        Self {
            end: self.end,
            pages: Rc::clone(&self.pages),
            offset: self.offset,
            mapped: self.mapped,
        }
    }
}

/// A part of a space: its own pages, or shared pages it shows.
pub(crate) enum Part<U: Unicorn<Guest> = Emulator> {
    Own,
    Shown {
        pages: Rc<SharedPages<U>>,
        offset: u64,
        mapped: bool,
    },
}

/// A block of shared pages unicorn maps for a space, where, and how many of its changes the space's translations know of.
struct Base<U: Unicorn<Guest>> {
    pages: Rc<SharedPages<U>>,
    address: u64,
    /// The count of the block's changes unicorn's translations of it know of, none when unknown.
    seen: Option<u64>,
}

pub(crate) struct Memory<U: Unicorn<Guest> = Emulator> {
    unicorn: U,
    /// The CPU state as made, with the exclusive monitor clear, put back before each turn: a return to
    /// a program clears the monitor (`ClearExclusiveLocal` at exception return), and nothing of one
    /// thread's turn may reach another's.
    fresh: U::Context,
    /// The bytes of the space's own pages, all of its addresses, which unicorn maps at the same physical addresses.
    own: U::HostMemory,
    /// Ranges that show shared pages, by their start.
    shared: BTreeMap<u64, Shown<U>>,
    /// The blocks of shared pages unicorn maps, past the space's own pages.
    bases: Vec<Base<U>>,
    /// Where the next block is mapped.
    next_base: u64,
    /// Whether the page table or the shared ranges changed since unicorn's TLB and mappings last followed them.
    stale: bool,
    /// The blocks the guest may write, as the TLB last followed the pages.
    writable: Vec<Rc<SharedPages<U>>>,
    /// The addresses of each block the guest may run, as the TLB last followed the pages.
    executable: Vec<(Rc<SharedPages<U>>, Range<u64>)>,
    /// Own pages that may hold something other than zeros, so a copy copies only those.
    touched: TouchedPages,
    /// Whether its translated blocks count instructions. Unicorn adds its counting hook without
    /// translating again, so blocks made without it would never stop at a turn's end.
    counting: bool,
    /// Whether unicorn may hold translations of the space's code, which a write of our side then makes stale.
    translated: bool,
    /// Where its address space ends.
    end: u64,
}

impl<U: Unicorn<Guest>> Memory<U> {
    pub(crate) fn new(end: u64) -> Result<Self, CpuError> {
        let guest = Guest {
            pages: vec![Permissions::empty(); to_index(end / PAGE_SIZE)],
            ..Guest::default()
        };
        let mut unicorn =
            U::new(Arch::ARM64, Mode::ARM, guest).map_err(|_| CpuError::NotSupported)?;
        unicorn
            .ctl_set_cpu_model(Arm64CpuModel::MAX as i32)
            .expect("unicorn has the max ARM64 model");
        unicorn
            .add_intr_hook(|guest: &mut Guest, requests: &mut Requests, exception| {
                guest.exception = Some(exception);
                requests.stop = true;
            })
            .expect("adding the interrupt hook");
        unicorn
            .add_block_hook(
                1,
                0,
                |guest: &mut Guest, requests: &mut Requests, _, size| {
                    guest.instructions += u64::from(size) / INSTRUCTION_SIZE;
                    requests.stop = guest
                        .deadline
                        .is_some_and(|deadline| coarsetime::Instant::now() >= deadline)
                        || guest
                            .interrupt
                            .as_ref()
                            .is_some_and(|interrupt| interrupt.load(Ordering::Acquire));
                },
            )
            .expect("adding the block hook");
        let invalid = HookType(
            HookType::MEM_READ_UNMAPPED.0
                | HookType::MEM_WRITE_UNMAPPED.0
                | HookType::MEM_FETCH_UNMAPPED.0
                | HookType::MEM_READ_PROT.0
                | HookType::MEM_WRITE_PROT.0
                | HookType::MEM_FETCH_PROT.0,
        );
        unicorn
            .add_mem_hook(
                invalid,
                1,
                0,
                |guest: &mut Guest, _: &mut Requests, kind, address, _, _| {
                    let access = match kind {
                        MemType::WRITE_UNMAPPED | MemType::WRITE_PROT => Access::Write,
                        MemType::FETCH_UNMAPPED | MemType::FETCH_PROT => Access::Execute,
                        _ => Access::Read,
                    };
                    // QEMU stores an access that crosses pages one byte at a time and goes on past the first
                    // byte that faults, so the first is the instruction's fault.
                    let address = guest.to_guest_address(address);
                    guest.fault.get_or_insert((address, access));
                    false
                },
            )
            .expect("adding the fault hook");
        // Every address is translated by the page table: QEMU's own translation with the MMU off would reach every
        // byte unicorn maps, raise an address beyond its physical addresses as an abort with no address, and drop a
        // tag from a fault's address.
        unicorn
            .add_tlb_hook(1, 0, |guest: &mut Guest, _: &mut Requests, page, access| {
                Some(guest.translate(page, to_needed(access)))
            })
            .expect("adding the TLB hook");
        unicorn
            .ctl_set_tlb_type(TlbType::VIRTUAL)
            .expect("setting unicorn's TLB");
        enter_user_level(&mut unicorn);
        let fresh = unicorn
            .context_init()
            .expect("saving unicorn's fresh CPU state");
        let own = U::new_host_memory(end).map_err(|_| CpuError::OutOfMemory)?;
        unicorn
            .mem_map_host_memory(u64::MIN, end, Prot::ALL, &own, u64::MIN)
            .expect("mapping a space's own pages in unicorn");
        Ok(Self {
            unicorn,
            fresh,
            own,
            shared: BTreeMap::new(),
            bases: Vec::new(),
            next_base: SHARED_BASE,
            stale: true,
            writable: Vec::new(),
            executable: Vec::new(),
            touched: TouchedPages::new(end / PAGE_SIZE),
            counting: false,
            translated: false,
            end,
        })
    }

    pub(crate) fn end(&self) -> u64 {
        self.end
    }

    /// Makes the space all zeros allowing nothing again, with nothing translated, as a new space is.
    pub(crate) fn clear(&mut self) {
        let zeros = [u8::MIN; PAGE_BYTES];
        for page in self.touched.take_all() {
            self.own_bytes(page_bytes(page, self.end)).write(&zeros);
        }
        self.shared.clear();
        self.unicorn.get_data_mut().pages.fill(Permissions::empty());
        self.unicorn
            .ctl_flush_tb()
            .expect("flushing unicorn's translated blocks");
        self.translated = false;
        self.stale = true;
    }

    /// Returns what each page allows.
    fn pages(&self) -> &[Permissions] {
        &self.unicorn.get_data().pages
    }

    /// Returns a view of the space's own bytes at the addresses.
    fn own_bytes(&self, addresses: Range<u64>) -> U::Bytes {
        U::host_memory_bytes(&self.own, addresses)
            .expect("a space's addresses lie inside its own memory")
    }

    /// Returns what the page at the address allows, nothing past the end of the space.
    pub(crate) fn read_permissions(&self, address: u64) -> Permissions {
        usize::try_from(address / PAGE_SIZE)
            .ok()
            .and_then(|page| self.pages().get(page).copied())
            .unwrap_or_else(Permissions::empty)
    }

    /// Returns the addresses as host indices if they lie inside the space.
    pub(crate) fn range(&self, address: u64, length: u64) -> Result<Range<usize>, CpuError> {
        let end = address.checked_add(length).ok_or(CpuError::BadAddress)?;
        if end > self.end {
            return Err(CpuError::BadAddress);
        }
        Ok(to_index(address)..to_index(end))
    }

    /// Returns the shared range holding the address, with its start.
    fn find_shown(&self, address: u64) -> Option<(u64, &Shown<U>)> {
        self.shared
            .range(..=address)
            .next_back()
            .filter(|(_, shown)| address < shown.end)
            .map(|(start, shown)| (*start, shown))
    }

    /// Returns where the part of the space holding the address ends: its shared range's end, or the next one's start.
    fn part_end(&self, address: u64) -> u64 {
        match self.find_shown(address) {
            Some((_, shown)) => shown.end,
            None => self
                .shared
                .range(address..)
                .next()
                .map_or(self.end, |(start, _)| *start),
        }
    }

    /// Copies the bytes at the address into the buffer, whatever the pages allow.
    pub(crate) fn read(&self, address: u64, into: &mut [u8]) -> Result<(), CpuError> {
        self.range(address, into.len() as u64)?;
        let mut done = usize::MIN;
        while done < into.len() {
            let at = address + done as u64;
            let count = to_index((self.part_end(at) - at).min((into.len() - done) as u64));
            let piece = &mut into[done..done + count];
            match self.find_shown(at) {
                Some((start, shown)) => shown.pages.read(shown.offset + (at - start), piece),
                None => {
                    self.own_bytes(at..at + count as u64).read(piece);
                }
            }
            done += count;
        }
        Ok(())
    }

    /// Copies the buffer to the address, whatever the pages allow.
    pub(crate) fn write(&mut self, address: u64, from: &[u8]) -> Result<(), CpuError> {
        self.range(address, from.len() as u64)?;
        let mut done = usize::MIN;
        while done < from.len() {
            let at = address + done as u64;
            let count = to_index((self.part_end(at) - at).min((from.len() - done) as u64));
            let piece = &from[done..done + count];
            let end = at + count as u64;
            match self.find_shown(at) {
                Some((start, shown)) => shown.pages.write(shown.offset + (at - start), piece),
                None => {
                    self.own_bytes(at..end).write(piece);
                    self.touched.touch(at / PAGE_SIZE..end.div_ceil(PAGE_SIZE));
                    self.forget_code(at..end);
                }
            }
            done += count;
        }
        Ok(())
    }

    /// Sets the bytes over the addresses to zero and ends any sharing there.
    pub(crate) fn zero(&mut self, addresses: Range<u64>) -> Result<(), CpuError> {
        self.range(addresses.start, addresses.end - addresses.start)?;
        self.unshare(addresses.clone());
        let zeros = [u8::MIN; PAGE_BYTES];
        let mut page = addresses.start / PAGE_SIZE;
        while page * PAGE_SIZE < addresses.end {
            let bytes = page_bytes(page, self.end);
            let cut = bytes.start.max(addresses.start)..bytes.end.min(addresses.end);
            if cut == bytes {
                self.forget_page(page);
            } else {
                self.write(cut.start, &zeros[..to_index(cut.end - cut.start)])?;
            }
            page += 1;
        }
        Ok(())
    }

    /// Makes an own page read as zeros.
    fn forget_page(&mut self, page: u64) {
        if self.touched.contains(page) {
            let bytes = page_bytes(page, self.end);
            self.own_bytes(bytes.clone())
                .write(&[u8::MIN; PAGE_BYTES][..to_index(bytes.end - bytes.start)]);
            self.touched.forget(page);
            self.forget_code(bytes);
        }
    }

    /// Sets what the pages over the addresses allow, dropping first what unicorn translated from pages that no longer
    /// allow running, while its TLB still reaches them: the next translation of a page made executable again then reads
    /// the bytes it holds by then.
    pub(crate) fn set_permissions(&mut self, addresses: Range<u64>, allowed: Permissions) {
        if !allowed.contains(Permissions::EXECUTE) {
            self.forget_code(addresses.clone());
        }
        let pages = to_index(addresses.start / PAGE_SIZE)..to_index(addresses.end / PAGE_SIZE);
        self.unicorn.get_data_mut().pages[pages].fill(allowed);
        self.remap(addresses);
    }

    /// Returns what the pages over the addresses allow, one entry a page.
    pub(crate) fn list_permissions(&self, addresses: Range<u64>) -> Vec<Permissions> {
        self.pages()[to_index(addresses.start / PAGE_SIZE)..to_index(addresses.end / PAGE_SIZE)]
            .to_vec()
    }

    /// Sets what the pages from the address allow, one entry a page; the caller remaps them.
    pub(crate) fn copy_permissions(&mut self, start: u64, permissions: &[Permissions]) {
        let end = start + permissions.len() as u64 * PAGE_SIZE;
        if permissions
            .iter()
            .any(|allowed| !allowed.contains(Permissions::EXECUTE))
        {
            self.forget_code(start..end);
        }
        let first = to_index(start / PAGE_SIZE);
        self.unicorn.get_data_mut().pages[first..first + permissions.len()]
            .copy_from_slice(permissions);
    }

    /// Drops what unicorn translated from the executable pages over the addresses, after their code changed.
    pub(crate) fn forget_code(&mut self, addresses: Range<u64>) {
        if !self.translated {
            return;
        }
        let runs = self.list_executable(addresses);
        if runs.is_empty() {
            return;
        }
        self.sync();
        for run in runs {
            // Unicorn translates the run's first address and drops as many bytes from its physical page on, so a run
            // stops at the end of the part of the space it starts in.
            let mut at = run.start;
            while at < run.end {
                let end = self.part_end(at).min(run.end);
                self.unicorn
                    .ctl_remove_cache(at, end)
                    .expect("dropping unicorn's translations");
                at = end;
            }
        }
    }

    /// Returns the runs of executable pages over the addresses, each cut to them.
    fn list_executable(&self, addresses: Range<u64>) -> Vec<Range<u64>> {
        let mut runs: Vec<Range<u64>> = Vec::new();
        let first = addresses.start / PAGE_SIZE;
        let last = addresses.end.div_ceil(PAGE_SIZE);
        for (page, allowed) in (first..last).zip(&self.pages()[to_index(first)..to_index(last)]) {
            if !allowed.contains(Permissions::EXECUTE) {
                continue;
            }
            let start = (page * PAGE_SIZE).max(addresses.start);
            let end = ((page + 1) * PAGE_SIZE).min(addresses.end);
            match runs.last_mut() {
                Some(run) if run.end == start => run.end = end,
                _ => runs.push(start..end),
            }
        }
        runs
    }

    /// Returns the parts of the space over the addresses, each cut to them, with its start and end.
    pub(crate) fn list_parts(&self, addresses: Range<u64>) -> Vec<(Range<u64>, Part<U>)> {
        let mut parts = Vec::new();
        let mut at = addresses.start;
        while at < addresses.end {
            let end = self.part_end(at).min(addresses.end);
            match self.find_shown(at) {
                Some((start, shown)) => parts.push((
                    at..end,
                    Part::Shown {
                        pages: Rc::clone(&shown.pages),
                        offset: shown.offset + (at - start),
                        mapped: shown.mapped,
                    },
                )),
                None => parts.push((at..end, Part::Own)),
            }
            at = end;
        }
        parts
    }

    /// Returns whether the page may hold something other than zeros.
    pub(crate) fn is_touched(&self, page: u64) -> bool {
        match self.find_shown(page * PAGE_SIZE) {
            Some((start, shown)) => shown
                .pages
                .is_touched((shown.offset + (page * PAGE_SIZE - start)) / PAGE_SIZE),
            None => self.touched.contains(page),
        }
    }

    /// Returns whether the page, which shows no shared pages, may hold something other than zeros.
    pub(crate) fn is_own_touched(&self, page: u64) -> bool {
        self.touched.contains(page)
    }

    /// Takes the addresses out of the shared ranges, splitting those at its ends, and returns the parts taken.
    ///
    /// The addresses taken read as whatever the space's own pages last held there.
    pub(crate) fn unshare(&mut self, addresses: Range<u64>) -> Vec<(Range<u64>, Shown<U>)> {
        let overlapping: Vec<u64> = self
            .shared
            .range(..addresses.end)
            .filter(|(_, shown)| shown.end > addresses.start)
            .map(|(start, _)| *start)
            .collect();
        let mut taken = Vec::new();
        for range_start in overlapping {
            let shown = self.shared.remove(&range_start).expect("a shared range");
            if range_start < addresses.start {
                self.shared.insert(
                    range_start,
                    Shown {
                        end: addresses.start,
                        ..shown.clone()
                    },
                );
            }
            if shown.end > addresses.end {
                self.shared.insert(
                    addresses.end,
                    Shown {
                        offset: shown.offset + (addresses.end - range_start),
                        ..shown.clone()
                    },
                );
            }
            let from = range_start.max(addresses.start);
            let to = shown.end.min(addresses.end);
            let offset = shown.offset + (from - range_start);
            taken.push((
                from..to,
                Shown {
                    end: to,
                    offset,
                    ..shown
                },
            ));
        }
        if !taken.is_empty() {
            self.remap(addresses);
        }
        taken
    }

    /// Makes the addresses show the shared pages from the offset.
    pub(crate) fn show_unmapped(
        &mut self,
        addresses: Range<u64>,
        pages: Rc<SharedPages<U>>,
        offset: u64,
        mapped: bool,
    ) {
        self.unshare(addresses.clone());
        self.shared.insert(
            addresses.start,
            Shown {
                end: addresses.end,
                pages,
                offset,
                mapped,
            },
        );
        self.remap(addresses);
    }

    /// Makes the addresses show the shared pages from the offset.
    pub(crate) fn show(
        &mut self,
        addresses: Range<u64>,
        pages: Rc<SharedPages<U>>,
        offset: u64,
        mapped: bool,
    ) {
        self.show_unmapped(addresses, pages, offset, mapped);
    }

    /// Ends the sharing over the addresses, the space keeping as its own what the shared pages held.
    pub(crate) fn keep_shared_bytes(&mut self, addresses: Range<u64>) {
        for (taken, shown) in self.unshare(addresses) {
            let mut bytes = vec![u8::MIN; to_index(taken.end - taken.start)];
            shown.pages.read(shown.offset, &mut bytes);
            self.write(taken.start, &bytes)
                .expect("a shared range lies inside the space");
        }
    }

    /// Ends the sharing over the addresses where they were mapped from elsewhere, those parts reading as zeros.
    pub(crate) fn drop_mappings(&mut self, addresses: Range<u64>) {
        let mapped: Vec<Range<u64>> = self
            .list_parts(addresses)
            .into_iter()
            .filter_map(|(range, part)| match part {
                Part::Shown { mapped: true, .. } => Some(range),
                _ => None,
            })
            .collect();
        for range in mapped {
            self.unshare(range.clone());
            for page in range.start / PAGE_SIZE..range.end.div_ceil(PAGE_SIZE) {
                self.forget_page(page);
            }
        }
    }

    /// Makes the space's own pages over the addresses shared pages it shows, and returns every part as shared pages.
    pub(crate) fn share_out(
        &mut self,
        addresses: Range<u64>,
    ) -> Result<Vec<(Range<u64>, Rc<SharedPages<U>>, u64)>, CpuError> {
        let mut shared = Vec::new();
        for (range, part) in self.list_parts(addresses) {
            match part {
                Part::Shown { pages, offset, .. } => shared.push((range, pages, offset)),
                Part::Own => {
                    let pages = Rc::new(SharedPages::new(range.end - range.start)?);
                    let first = range.start / PAGE_SIZE;
                    let touched = self.touched.list(first..range.end / PAGE_SIZE);
                    for run in to_runs(&touched) {
                        let at = run.start * PAGE_SIZE;
                        copy_between::<U>(
                            &self.own,
                            at..run.end * PAGE_SIZE,
                            &pages.memory,
                            at - range.start,
                        );
                    }
                    for page in touched {
                        pages
                            .touched
                            .borrow_mut()
                            .touch(page - first..page - first + 1);
                    }
                    self.show_unmapped(range.clone(), Rc::clone(&pages), u64::MIN, false);
                    shared.push((range, pages, u64::MIN));
                }
            }
        }
        Ok(shared)
    }

    /// Points every shared range at the pages `to_copy` gives for its pages.
    pub(crate) fn retarget(
        &mut self,
        mut to_copy: impl FnMut(&Rc<SharedPages<U>>) -> Rc<SharedPages<U>>,
    ) {
        for shown in self.shared.values_mut() {
            shown.pages = to_copy(&shown.pages);
        }
        self.remap(0..self.end);
    }

    /// Returns whether the page lies in a range that shows shared pages.
    fn is_shared(&self, page: u64) -> bool {
        self.find_shown(page * PAGE_SIZE).is_some()
    }

    /// A copy with the same bytes, permissions and shared ranges, on a new unicorn engine.
    pub(crate) fn duplicate(&self) -> Result<Self, CpuError> {
        let mut copy = Self::new(self.end)?;
        let own: Vec<u64> = self
            .touched
            .list(0..self.end / PAGE_SIZE)
            .into_iter()
            .filter(|page| !self.is_shared(*page))
            .collect();
        for run in to_runs(&own) {
            let at = run.start * PAGE_SIZE;
            copy_between::<U>(&self.own, at..run.end * PAGE_SIZE, &copy.own, at);
        }
        copy.touched.clone_from(&self.touched);
        copy.unicorn
            .get_data_mut()
            .pages
            .clone_from(&self.unicorn.get_data().pages);
        copy.shared = self.shared.clone();
        Ok(copy)
    }

    /// Returns the pages that may hold something other than zeros, the shared ranges' pages among them, in order.
    #[cfg(feature = "harness")]
    pub(crate) fn list_held_pages(&self) -> Vec<u64> {
        let mut pages: std::collections::BTreeSet<u64> = self
            .touched
            .list(0..self.end / PAGE_SIZE)
            .into_iter()
            .collect();
        for (start, shown) in &self.shared {
            pages.extend(start / PAGE_SIZE..shown.end.div_ceil(PAGE_SIZE));
        }
        pages.into_iter().collect()
    }

    /// Makes unicorn follow the page table and the shared ranges over the addresses before the guest next runs, and
    /// records that the guest may write the pages there that allow it.
    pub(crate) fn remap(&mut self, addresses: Range<u64>) {
        self.stale = true;
        let pages =
            to_index(addresses.start / PAGE_SIZE)..to_index(addresses.end.div_ceil(PAGE_SIZE));
        if !self.pages()[pages.clone()]
            .iter()
            .any(|allowed| allowed.contains(Permissions::WRITE))
        {
            return;
        }
        for (range, part) in self.list_parts(addresses) {
            for page in range.start / PAGE_SIZE..range.end.div_ceil(PAGE_SIZE) {
                if !self.pages()[to_index(page)].contains(Permissions::WRITE) {
                    continue;
                }
                match &part {
                    Part::Own => self.touched.touch(page..page + 1),
                    Part::Shown { pages, offset, .. } => {
                        let at = (offset + (page * PAGE_SIZE - range.start)) / PAGE_SIZE;
                        pages.touched.borrow_mut().touch(at..at + 1);
                    }
                }
            }
        }
    }

    /// Makes unicorn's mappings and TLB follow the shared ranges and the page table, if either changed.
    fn sync(&mut self) {
        if !self.stale {
            return;
        }
        self.stale = false;
        let shown: Vec<Rc<SharedPages<U>>> = self
            .shared
            .values()
            .map(|shown| Rc::clone(&shown.pages))
            .collect();
        for pages in &shown {
            if self.bases.iter().any(|base| Rc::ptr_eq(&base.pages, pages)) {
                continue;
            }
            let address = self.next_base;
            let length = pages.host_length();
            self.unicorn
                .mem_map_host_memory(address, length, Prot::ALL, &pages.memory, u64::MIN)
                .expect("mapping a block of shared pages in unicorn");
            self.next_base = address + length + PAGE_SIZE;
            self.bases.push(Base {
                pages: Rc::clone(pages),
                address,
                seen: None,
            });
        }
        let unused: Vec<(u64, u64)> = self
            .bases
            .iter()
            .filter(|base| !shown.iter().any(|pages| Rc::ptr_eq(&base.pages, pages)))
            .map(|base| (base.address, base.pages.host_length()))
            .collect();
        for (address, length) in unused {
            self.unicorn
                .mem_unmap(address, length)
                .expect("unmapping a block of shared pages unicorn has");
            self.bases.retain(|base| base.address != address);
        }
        let physical = self
            .shared
            .iter()
            .map(|(start, shown)| {
                let base = self
                    .bases
                    .iter()
                    .find(|base| Rc::ptr_eq(&base.pages, &shown.pages))
                    .expect("a block shown is mapped");
                (*start, (shown.end, base.address + shown.offset))
            })
            .collect();
        self.writable.clear();
        self.executable.clear();
        for (start, shown) in &self.shared {
            let pages = &self.unicorn.get_data().pages
                [to_index(start / PAGE_SIZE)..to_index(shown.end.div_ceil(PAGE_SIZE))];
            if pages
                .iter()
                .any(|allowed| allowed.contains(Permissions::WRITE))
                && !self
                    .writable
                    .iter()
                    .any(|block| Rc::ptr_eq(block, &shown.pages))
            {
                self.writable.push(Rc::clone(&shown.pages));
            }
            for (page, allowed) in (*start / PAGE_SIZE..).zip(pages) {
                let at = page * PAGE_SIZE;
                if !allowed.contains(Permissions::EXECUTE) {
                    continue;
                }
                match self.executable.last_mut() {
                    Some((block, run)) if Rc::ptr_eq(block, &shown.pages) && run.end == at => {
                        run.end = at + PAGE_SIZE;
                    }
                    _ => self
                        .executable
                        .push((Rc::clone(&shown.pages), at..at + PAGE_SIZE)),
                }
            }
        }
        let guest = self.unicorn.get_data_mut();
        guest.physical = physical;
        guest.fault_pages.clear();
        self.unicorn
            .ctl_flush_tlb()
            .expect("flushing unicorn's TLB");
    }

    /// Drops what unicorn translated from every executable block whose pages changed since its translations were made.
    fn refresh_shared_code(&mut self) {
        let mut stale = Vec::new();
        for base in &mut self.bases {
            let changes = base.pages.changes.get();
            if base.seen == Some(changes) {
                continue;
            }
            base.seen = Some(changes);
            stale.extend(
                self.executable
                    .iter()
                    .filter(|(pages, _)| Rc::ptr_eq(pages, &base.pages))
                    .map(|(_, addresses)| addresses.clone()),
            );
        }
        for addresses in stale {
            self.forget_code(addresses);
        }
    }

    /// Counts a change of every block the guest could write in the turn just run, which this space's own translations
    /// of it already know of, since unicorn drops a translation itself at a guest's store to its physical page.
    fn count_guest_writes(&mut self) {
        for pages in &self.writable {
            pages.changes.set(pages.changes.get().wrapping_add(1));
        }
        for base in &mut self.bases {
            if self
                .writable
                .iter()
                .any(|pages| Rc::ptr_eq(pages, &base.pages))
            {
                base.seen = Some(base.pages.changes.get());
            }
        }
    }

    /// Writes the registers into unicorn over whatever else its CPU holds.
    fn load_registers(&mut self, registers: &RegisterBlock) -> Result<(), uc_error> {
        let unicorn = &mut self.unicorn;
        for (index, value) in registers.x.iter().enumerate() {
            unicorn.reg_write(x_register(index), *value)?;
        }
        unicorn.reg_write(RegisterARM64::SP as i32, registers.sp)?;
        unicorn.reg_write(RegisterARM64::PC as i32, registers.pc)?;
        unicorn.reg_write(RegisterARM64::NZCV as i32, registers.pstate)?;
        for (index, value) in registers.v.iter().enumerate() {
            let mut bytes = [0u8; 16];
            bytes[..8].copy_from_slice(&value[0].to_le_bytes());
            bytes[8..].copy_from_slice(&value[1].to_le_bytes());
            unicorn.reg_write_long(q_register(index), &bytes)?;
        }
        unicorn.reg_write(RegisterARM64::FPSR as i32, u64::from(registers.fpsr))?;
        unicorn.reg_write(RegisterARM64::FPCR as i32, u64::from(registers.fpcr))?;
        unicorn.reg_write(RegisterARM64::TPIDR_EL0 as i32, registers.tpidr_el0)?;
        Ok(())
    }

    fn save_registers(&self, registers: &mut RegisterBlock) -> Result<(), uc_error> {
        let unicorn = &self.unicorn;
        for (index, value) in registers.x.iter_mut().enumerate() {
            *value = unicorn.reg_read(x_register(index))?;
        }
        registers.sp = unicorn.reg_read(RegisterARM64::SP as i32)?;
        registers.pc = unicorn.reg_read(RegisterARM64::PC as i32)?;
        registers.pstate =
            unicorn.reg_read(RegisterARM64::NZCV as i32)? & wcontainers_abi::pstate::NZCV;
        for (index, value) in registers.v.iter_mut().enumerate() {
            let bytes = unicorn.reg_read_long(q_register(index))?;
            value[0] = u64::from_le_bytes(bytes[..8].try_into().expect("16 bytes"));
            value[1] = u64::from_le_bytes(bytes[8..16].try_into().expect("16 bytes"));
        }
        registers.fpsr = unicorn.reg_read(RegisterARM64::FPSR as i32)? as u32;
        registers.fpcr = unicorn.reg_read(RegisterARM64::FPCR as i32)? as u32;
        registers.tpidr_el0 = unicorn.reg_read(RegisterARM64::TPIDR_EL0 as i32)?;
        Ok(())
    }

    /// Runs the registers on this memory and returns why they came back with the number of instructions run.
    ///
    /// Instructions are counted in whole blocks. With a slice, it stops after that many instructions; with a
    /// deadline, also once the machine's clock passes it; with an interrupt, also once it is raised.
    pub(crate) fn run_turn(
        &mut self,
        registers: &mut RegisterBlock,
        slice_nanoseconds: Option<u64>,
        deadline: Option<coarsetime::Instant>,
        interrupt: Option<Arc<AtomicBool>>,
    ) -> (Result<Exit, Unsupported>, u64) {
        self.unicorn
            .context_restore(&self.fresh)
            .expect("restoring unicorn's fresh CPU state");
        self.run_loaded(registers, slice_nanoseconds, deadline, interrupt)
    }

    /// Runs as [`Memory::run_turn`] does, on what unicorn's CPU holds besides the registers.
    fn run_loaded(
        &mut self,
        registers: &mut RegisterBlock,
        slice_nanoseconds: Option<u64>,
        deadline: Option<coarsetime::Instant>,
        interrupt: Option<Arc<AtomicBool>>,
    ) -> (Result<Exit, Unsupported>, u64) {
        self.sync();
        self.refresh_shared_code();
        self.load_registers(registers)
            .expect("loading registers into unicorn");
        self.unicorn.get_data_mut().start_run(deadline, interrupt);
        let count = slice_nanoseconds.map_or(UNICORN_NO_LIMIT, |nanoseconds| {
            usize::try_from(nanoseconds.max(ONE_INSTRUCTION_NANOSECONDS)).unwrap_or(usize::MAX)
        });
        let result = self.start(registers.pc, count);
        let mut stopped = self.unicorn.get_data_mut().take_stopped();
        let pc = self
            .unicorn
            .reg_read(RegisterARM64::PC as i32)
            .expect("reading the program counter");
        // A branch to a misaligned upper-half address faults on the program counter's alignment before translation,
        // where QEMU raised the address's size.
        if stopped.exception == Some(EXCEPTION_PREFETCH_ABORT)
            && !pc.is_multiple_of(INSTRUCTION_SIZE)
        {
            stopped.exception = Some(EXCEPTION_UNDEFINED);
        }
        self.save_registers(registers)
            .expect("reading registers from unicorn");
        // A fetch that faults leaves the program counter at the address fetched, where unicorn leaves it at the
        // instruction before when that one branched there in the same run.
        if let Some((address, Access::Execute)) = stopped.fault {
            registers.pc = address;
        }
        self.count_guest_writes();
        (
            self.to_exit(result, &stopped, registers),
            stopped.instructions,
        )
    }

    /// Starts unicorn at the address for the count of instructions, none for no limit, with its TLB following the
    /// page table.
    fn start(&mut self, address: u64, count: usize) -> Result<(), uc_error> {
        self.sync();
        if count > UNICORN_NO_LIMIT && !self.counting {
            self.unicorn
                .ctl_flush_tb()
                .expect("flushing unicorn's translated blocks");
        }
        // Unicorn drops its blocks itself when counting stops.
        self.counting = count > UNICORN_NO_LIMIT;
        self.translated = true;
        self.unicorn
            .emu_start(address, u64::MAX, UNICORN_NO_TIMEOUT, count)
    }

    /// Runs one instruction of the registers with the predicates loaded before it and read back after it.
    ///
    /// Unicorn's interface reaches no predicate register, so code on the scratch page loads them from it and stores
    /// them to it: the page allows everything while that code runs and nothing while the instruction does.
    pub(crate) fn run_instruction(
        &mut self,
        registers: &mut RegisterBlock,
        predicates: &mut Predicates,
        scratch: u64,
    ) -> (Result<Exit, Unsupported>, u64) {
        let page = scratch..scratch + PAGE_SIZE;
        let data = scratch + PREDICATE_DATA;
        self.set_permissions(page.clone(), Permissions::all());
        self.write(scratch, &to_code(&PREDICATE_LOADER))
            .expect("the scratch page lies inside the space");
        self.write(scratch + PREDICATE_SAVER, &to_code(&PREDICATE_SAVER_CODE))
            .expect("the scratch page lies inside the space");
        self.write(data, &predicates.to_bytes())
            .expect("the scratch page lies inside the space");
        self.unicorn
            .context_restore(&self.fresh)
            .expect("restoring unicorn's fresh CPU state");
        self.run_helper(scratch, data, PREDICATE_LOADER.len());
        self.set_permissions(page.clone(), Permissions::empty());
        let ran = self.run_loaded(registers, Some(ONE_INSTRUCTION_NANOSECONDS), None, None);
        self.set_permissions(page.clone(), Permissions::all());
        self.run_helper(scratch + PREDICATE_SAVER, data, PREDICATE_SAVER_CODE.len());
        let mut bytes = [u8::MIN; PREDICATE_BYTES];
        self.read(data, &mut bytes)
            .expect("the scratch page lies inside the space");
        *predicates = Predicates::from_bytes(&bytes);
        self.set_permissions(page, Permissions::empty());
        ran
    }

    /// Runs the count of instructions of helper code at the address with X0 holding its data's address.
    fn run_helper(&mut self, code: u64, data: u64, count: usize) {
        self.unicorn
            .reg_write(x_register(PREDICATE_BASE), data)
            .expect("writing X0");
        self.unicorn.get_data_mut().start_run(None, None);
        let result = self.start(code, count);
        let stopped = self.unicorn.get_data_mut().take_stopped();
        assert!(
            result.is_ok() && stopped.exception.is_none() && stopped.fault.is_none(),
            "the predicate helper code ran: {result:?}, exception {:?}, fault {:?}",
            stopped.exception,
            stopped.fault
        );
    }

    /// Returns what the hooks recorded and unicorn's result as the exit they are.
    fn to_exit(
        &self,
        result: Result<(), uc_error>,
        stopped: &Stopped,
        registers: &RegisterBlock,
    ) -> Result<Exit, Unsupported> {
        // A jump to an address that is not a multiple of 4 faults on alignment before any fetch.
        if let Some((address, access)) = stopped.fault {
            if access == Access::Execute && !address.is_multiple_of(INSTRUCTION_SIZE) {
                return Ok(Exit::AlignmentFault { address });
            }
            return Ok(Exit::MemoryFault { address, access });
        }
        let pc = registers.pc;
        Ok(match (result, stopped.exception) {
            (_, Some(EXCEPTION_SYSCALL)) => {
                let x = &registers.x;
                Exit::Syscall(SystemCallRequest {
                    number: SystemCallNumber(x[wcontainers_abi::SYSTEM_CALL_NUMBER_REGISTER]),
                    arguments: x[..SYSTEM_CALL_ARGUMENT_COUNT]
                        .try_into()
                        .expect("a system call's arguments are registers"),
                })
            }
            (_, Some(EXCEPTION_BREAKPOINT)) => {
                let mut instruction = [u8::MIN; INSTRUCTION_SIZE as usize];
                self.read(pc, &mut instruction)
                    .expect("the breakpoint's instruction lies inside the space");
                let comment = u16::try_from(IMM16.read(u32::from_le_bytes(instruction)))
                    .expect("a breakpoint's comment is 16 bits");
                Exit::Breakpoint { comment }
            }
            // QEMU raises a fetch from a pc that is not a multiple of 4 as undefined.
            (_, Some(EXCEPTION_UNDEFINED)) if !pc.is_multiple_of(INSTRUCTION_SIZE) => {
                Exit::AlignmentFault { address: pc }
            }
            (_, Some(EXCEPTION_UNDEFINED)) | (Err(uc_error::INSN_INVALID), None) => {
                Exit::IllegalInstruction
            }
            // Permission and mapping faults come through the fault hook, so a data abort here is QEMU's
            // alignment check.
            (_, Some(EXCEPTION_DATA_ABORT)) => return Err(Unsupported::AlignmentFault { pc }),
            (Ok(()), None) => Exit::Stop,
            (result, exception) => {
                return Err(Unsupported::UnknownStop {
                    pc,
                    result,
                    exception,
                });
            }
        })
    }
}

fn prot(permissions: Permissions) -> Prot {
    let mut prot = Prot::NONE.0;
    if permissions.contains(Permissions::READ) {
        prot |= Prot::READ.0;
    }
    if permissions.contains(Permissions::WRITE) {
        prot |= Prot::WRITE.0;
    }
    if permissions.contains(Permissions::EXECUTE) {
        prot |= Prot::EXEC.0;
    }
    Prot(prot)
}

/// Unicorn's number of the general register `X<index>`; X0 to X28 are numbered in a run, X29 and X30 apart.
pub(crate) fn x_register(index: usize) -> i32 {
    match index {
        0..=28 => RegisterARM64::X0 as i32 + index as i32,
        29 => RegisterARM64::X29 as i32,
        30 => RegisterARM64::X30 as i32,
        _ => panic!("no general register X{index}"),
    }
}

/// Unicorn's number of the vector register `Q<index>`; Q0 to Q31 are numbered in a run.
pub(crate) fn q_register(index: usize) -> i32 {
    assert!(index < 32, "no vector register Q{index}");
    RegisterARM64::Q0 as i32 + index as i32
}

/// Returns the addresses of the page, cut at the end given.
fn page_bytes(page: u64, end: u64) -> Range<u64> {
    let start = page * PAGE_SIZE;
    start..(start + PAGE_SIZE).min(end.max(start))
}

/// Returns the number as an index into host memory.
fn to_index(number: u64) -> usize {
    usize::try_from(number).expect("an address of a space fits in host memory")
}

/// Puts unicorn's CPU at EL0 under the controls Linux gives its programs, from EL1h where unicorn starts it.
///
/// A write of a system register through `CP_REG` is what makes unicorn rebuild the flags its translations read, so the
/// level is changed before it.
fn enter_user_level<U: Unicorn<Guest>>(unicorn: &mut U) {
    let mut monitor = MONITOR_CONTROL;
    unicorn
        .reg_read_arm64_coproc(&mut monitor)
        .expect("reading SCR_EL3");
    unicorn
        .reg_write_arm64_coproc(&MONITOR_CONTROL.val(monitor.val | LOWER_LEVELS_AARCH64))
        .expect("writing SCR_EL3");
    unicorn
        .reg_write(RegisterARM64::CPACR_EL1 as i32, USER_FEATURE_ACCESS)
        .expect("writing CPACR_EL1");
    unicorn
        .reg_write_arm64_coproc(&MONITOR_FEATURE_TRAPS.val(MONITOR_TRAPS_NONE))
        .expect("writing CPTR_EL3");
    unicorn
        .reg_write_arm64_coproc(&TRANSLATION_CONTROL.val(USER_TRANSLATION_CONTROL))
        .expect("writing TCR_EL1");
    unicorn
        .reg_write_arm64_coproc(&COUNTER_CONTROL.val(USER_COUNTERS_TRAPPED))
        .expect("writing CNTKCTL_EL1");
    unicorn
        .reg_write(RegisterARM64::PSTATE as i32, USER_PSTATE)
        .expect("writing PSTATE");
    let mut control = SYSTEM_CONTROL;
    unicorn
        .reg_read_arm64_coproc(&mut control)
        .expect("reading SCTLR_EL1");
    let user = control.val & !USER_CONTROLS_CLEAR | USER_CONTROLS_SET;
    unicorn
        .reg_write_arm64_coproc(&SYSTEM_CONTROL.val(user))
        .expect("writing SCTLR_EL1");
}

/// The bytes of one predicate register at a 128-bit vector length, one bit for each byte of a vector.
const PREDICATE_REGISTER_BYTES: usize = size_of::<u16>();
/// The bytes the predicate registers and FFR take on the scratch page, FFR after P15.
const PREDICATE_BYTES: usize = (PREDICATE_REGISTERS + 1) * PREDICATE_REGISTER_BYTES;
/// Where on the scratch page the helper code that stores the predicates starts, after the code that loads them.
const PREDICATE_SAVER: u64 = 0x100;
/// Where on the scratch page the predicates are, after both helper codes.
const PREDICATE_DATA: u64 = 0x200;
/// The general register that holds the predicates' address while the helper code runs, X0.
const PREDICATE_BASE: usize = 0;
/// The slot of FFR on the scratch page, in predicate registers from the start.
const FIRST_FAULT_SLOT: u32 = PREDICATE_REGISTERS as u32;

/// [LDR (predicate)](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/LDR--predicate---Load-predicate-register-)
/// of Pt from X0 plus `slot` predicate registers.
const fn load_predicate(register: u32, slot: u32) -> u32 {
    0x8580_0000 | (slot >> 3) << 16 | (slot & 0b111) << 10 | register
}

/// [STR (predicate)](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/STR--predicate---Store-predicate-register-)
/// of Pt to X0 plus `slot` predicate registers.
const fn store_predicate(register: u32, slot: u32) -> u32 {
    0xE580_0000 | (slot >> 3) << 16 | (slot & 0b111) << 10 | register
}

/// [WRFFR](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/WRFFR--Write-the-first-fault-register-)
/// from P0.
const WRITE_FIRST_FAULT_FROM_P0: u32 = 0x2528_9000;
/// [RDFFR (unpredicated)](https://developer.arm.com/documentation/ddi0602/latest/SVE-Instructions/RDFFR--unpredicated---Read-the-first-fault-register-)
/// into P0.
const READ_FIRST_FAULT_INTO_P0: u32 = 0x2519_F000;

/// Loads FFR through P0, then P0 to P15.
const PREDICATE_LOADER: [u32; PREDICATE_REGISTERS + 2] = {
    let mut code = [WRITE_FIRST_FAULT_FROM_P0; PREDICATE_REGISTERS + 2];
    code[0] = load_predicate(0, FIRST_FAULT_SLOT);
    let mut register = 0;
    while register < PREDICATE_REGISTERS {
        code[register + 2] = load_predicate(register as u32, register as u32);
        register += 1;
    }
    code
};

/// Stores P0 to P15, then FFR through P0.
const PREDICATE_SAVER_CODE: [u32; PREDICATE_REGISTERS + 2] = {
    let mut code = [READ_FIRST_FAULT_INTO_P0; PREDICATE_REGISTERS + 2];
    let mut register = 0;
    while register < PREDICATE_REGISTERS {
        code[register] = store_predicate(register as u32, register as u32);
        register += 1;
    }
    code[PREDICATE_REGISTERS + 1] = store_predicate(0, FIRST_FAULT_SLOT);
    code
};

/// Returns the instruction words as the bytes of code.
fn to_code(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

/// SVE's predicate registers and FFR, each at a 128-bit vector length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Predicates {
    pub(crate) registers: [u16; PREDICATE_REGISTERS],
    pub(crate) first_fault: u16,
}

impl Predicates {
    fn to_bytes(self) -> [u8; PREDICATE_BYTES] {
        let mut bytes = [u8::MIN; PREDICATE_BYTES];
        let values = self.registers.into_iter().chain([self.first_fault]);
        for (slot, value) in bytes
            .as_chunks_mut::<PREDICATE_REGISTER_BYTES>()
            .0
            .iter_mut()
            .zip(values)
        {
            *slot = value.to_le_bytes();
        }
        bytes
    }

    fn from_bytes(bytes: &[u8; PREDICATE_BYTES]) -> Self {
        let mut values = bytes
            .as_chunks::<PREDICATE_REGISTER_BYTES>()
            .0
            .iter()
            .map(|slot| u16::from_le_bytes(*slot));
        let registers = std::array::from_fn(|_| values.next().expect("a predicate register"));
        Self {
            registers,
            first_fault: values.next().expect("FFR"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use wcontainers_abi::{Access, Exit, PAGE_SIZE, Permissions, RegisterBlock};
    use wcontainers_unicorn::{Prot, RegisterARM64, Unicorn};
    use wcontainers_unicorn_direct::DirectUnicorn;
    use wcontainers_unicorn_recording::{Call, RecordingUnicorn};

    use super::{
        FAULT_PAGES, Guest, Memory, SHARED_BASE, SharedPages, TouchedPages, q_register, to_runs,
        x_register,
    };

    /// Where the tests' spaces end: 16 pages.
    const END: u64 = 16 * PAGE_SIZE;
    /// Where the tests' code starts, the second page.
    const CODE: u64 = PAGE_SIZE;
    /// [SVC](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/SVC--Supervisor-call-) #0.
    const SYSTEM_CALL: u32 = 0xD400_0001;
    /// [STR (immediate)](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/STR--immediate---Store-register--immediate--)
    /// of X1 to the address in X0, unsigned offset 0.
    const STORE_X1_AT_X0: u32 = 0xF900_0001;
    /// The same of W1, the low 32 bits of X1.
    const STORE_W1_AT_X0: u32 = 0xB900_0001;
    /// The slice of the tests' turns, in instructions.
    const SLICE: u64 = 1000;
    /// A value the tests store.
    const WORD: u64 = 0x1122_3344_5566_7788;
    /// A tag in an address's top byte, which TBI0 leaves out of translation.
    const TAG: u64 = 0x5A << 56;
    /// Bit 55, which puts an address in the upper half.
    const UPPER: u64 = 1 << 55;

    type Direct = DirectUnicorn<Guest>;
    type Recording = RecordingUnicorn<DirectUnicorn<Guest>>;

    /// [MOVZ](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/MOVZ--Move-wide-with-zero-) of the
    /// 16-bit value into X1.
    const fn move_into_x1(value: u16) -> u32 {
        0xD280_0001 | (value as u32) << 5
    }

    fn read_write() -> Permissions {
        Permissions::READ | Permissions::WRITE
    }

    fn read_execute() -> Permissions {
        Permissions::READ | Permissions::EXECUTE
    }

    fn to_code(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    /// Returns a space with the code at [`CODE`], which allows reading and running it.
    fn with_code<U: Unicorn<Guest>>(code: &[u32]) -> Memory<U> {
        let mut memory = Memory::<U>::new(END).expect("a space");
        memory
            .write(CODE, &to_code(code))
            .expect("the code written");
        memory.set_permissions(CODE..CODE + PAGE_SIZE, read_execute());
        memory
    }

    /// Runs from the address with X0 and X1 holding the values, for a slice of the count of instructions, and returns
    /// how it ended and the registers it left.
    fn run<U: Unicorn<Guest>>(
        memory: &mut Memory<U>,
        from: u64,
        x0: u64,
        x1: u64,
        count: u64,
    ) -> (Exit, RegisterBlock) {
        let mut registers = RegisterBlock::ZERO;
        registers.pc = from;
        registers.x[0] = x0;
        registers.x[1] = x1;
        let exit = memory
            .run_turn(&mut registers, Some(count), None, None)
            .0
            .expect("a turn the stand-in supports");
        (exit, registers)
    }

    fn read_page<U: Unicorn<Guest>>(memory: &Memory<U>, address: u64) -> Vec<u8> {
        let mut bytes = vec![0; PAGE_SIZE as usize];
        memory
            .read(address, &mut bytes)
            .expect("a page of the space");
        bytes
    }

    fn read_word<U: Unicorn<Guest>>(memory: &Memory<U>, address: u64) -> u64 {
        let mut bytes = [0; 8];
        memory
            .read(address, &mut bytes)
            .expect("a word of the space");
        u64::from_le_bytes(bytes)
    }

    /// THE STAND-IN'S HOSTILE-GUEST TEST OF ITS RULE: a guest that stores to every address it can name, tagged, in the
    /// upper half, past the space's end and at the physical addresses unicorn maps its blocks and fault pages at, changes
    /// only bytes of pages the page table lets it write, through every view of them, and faults at the address it used.
    #[test]
    fn a_hostile_guest_writes_only_what_the_page_table_lets_it() {
        let mut memory = with_code::<Direct>(&[STORE_X1_AT_X0]);
        let mut other = with_code::<Direct>(&[STORE_X1_AT_X0]);
        let block: Rc<SharedPages<Direct>> = Rc::new(SharedPages::new(PAGE_SIZE).expect("a block"));
        let kept: Vec<u8> = (0..PAGE_SIZE).map(|index| (index % 251) as u8).collect();
        let page = |index: u64| index * PAGE_SIZE;
        memory.set_permissions(page(2)..page(3), read_write());
        memory
            .write(page(3), &kept)
            .expect("the read-only page's bytes");
        memory.set_permissions(page(3)..page(4), Permissions::READ);
        memory
            .write(page(4), &kept)
            .expect("the bytes of a page that allows nothing");
        memory.show(page(5)..page(6), Rc::clone(&block), 0, true);
        memory.set_permissions(page(5)..page(6), read_write());
        memory.show(page(6)..page(7), Rc::clone(&block), 0, true);
        memory.set_permissions(page(6)..page(7), Permissions::READ);
        other.show(page(5)..page(6), Rc::clone(&block), 0, true);
        other.set_permissions(page(5)..page(6), Permissions::READ);
        let code = read_page(&memory, CODE);
        let refused = [
            CODE,
            page(3),
            page(4) + 8,
            page(6),
            page(3) | TAG,
            page(2) | UPPER,
            END,
            END - 4,
            SHARED_BASE,
            FAULT_PAGES,
            u64::MAX - 7,
        ];
        for address in refused {
            assert_eq!(
                run(&mut memory, CODE, address, WORD, 1).0,
                Exit::MemoryFault {
                    address,
                    access: Access::Write,
                },
                "a store at {address:#x}"
            );
        }
        assert_eq!(
            run(&mut other, CODE, page(5), WORD, 1).0,
            Exit::MemoryFault {
                address: page(5),
                access: Access::Write,
            },
            "a store to the block through another space's read-only view"
        );
        assert_eq!(read_page(&memory, CODE), code);
        assert_eq!(read_page(&memory, page(3)), kept);
        assert_eq!(read_page(&memory, page(4)), kept);
        assert_eq!(read_page(&memory, page(6)), vec![0; PAGE_SIZE as usize]);
        // The stores the page table allows, a tagged one among them, reach the page and every view of the block.
        for address in [page(2), page(2) + 8 | TAG, page(5) + 16] {
            assert_eq!(
                run(&mut memory, CODE, address, WORD, 1).0,
                Exit::Stop,
                "a store at {address:#x}"
            );
        }
        assert_eq!(read_word(&memory, page(2)), WORD);
        assert_eq!(read_word(&memory, page(2) + 8), WORD);
        assert_eq!(read_word(&memory, page(6) + 16), WORD);
        assert_eq!(read_word(&other, page(5) + 16), WORD);
    }

    /// Code changed after unicorn translated it runs as changed: written by our side, written by a guest through
    /// another view of the same block in the same space or in another space, and rewritten while not executable.
    #[test]
    fn changed_code_runs_as_changed() {
        let page = |index: u64| index * PAGE_SIZE;
        let ran = |memory: &mut Memory<Direct>, from: u64| {
            let (exit, registers) = run(memory, from, 0, 0, SLICE);
            assert!(
                matches!(exit, Exit::Syscall(_)),
                "the code ran to its system call: {exit:?}"
            );
            registers.x[1]
        };
        let mut storing = with_code::<Direct>(&[STORE_W1_AT_X0, SYSTEM_CALL]);
        let mut running = Memory::<Direct>::new(END).expect("a space");
        let block: Rc<SharedPages<Direct>> = Rc::new(SharedPages::new(PAGE_SIZE).expect("a block"));
        block.write(0, &to_code(&[move_into_x1(1), SYSTEM_CALL]));
        storing.show(page(5)..page(6), Rc::clone(&block), 0, true);
        storing.set_permissions(page(5)..page(6), read_write());
        storing.show(page(6)..page(7), Rc::clone(&block), 0, true);
        storing.set_permissions(page(6)..page(7), read_execute());
        running.show(page(8)..page(9), Rc::clone(&block), 0, true);
        running.set_permissions(page(8)..page(9), read_execute());
        assert_eq!(ran(&mut running, page(8)), 1);
        assert_eq!(ran(&mut storing, page(6)), 1);
        // Our side writes the block.
        block.write(0, &move_into_x1(2).to_le_bytes());
        assert_eq!(ran(&mut running, page(8)), 2);
        assert_eq!(ran(&mut storing, page(6)), 2);
        // The storing space's guest writes it through its writable view.
        let (exit, _) = run(&mut storing, CODE, page(5), u64::from(move_into_x1(3)), 1);
        assert_eq!(exit, Exit::Stop);
        assert_eq!(ran(&mut storing, page(6)), 3);
        assert_eq!(ran(&mut running, page(8)), 3);
        // Our side rewrites the running space's own code while it may not run, then lets it run again.
        let mut own = with_code::<Direct>(&[move_into_x1(4), SYSTEM_CALL]);
        assert_eq!(ran(&mut own, CODE), 4);
        own.set_permissions(CODE..CODE + PAGE_SIZE, read_write());
        own.write(CODE, &move_into_x1(5).to_le_bytes())
            .expect("the code rewritten");
        own.set_permissions(CODE..CODE + PAGE_SIZE, read_execute());
        assert_eq!(ran(&mut own, CODE), 5);
        // And while it may run.
        own.write(CODE, &move_into_x1(6).to_le_bytes())
            .expect("the code rewritten");
        assert_eq!(ran(&mut own, CODE), 6);
    }

    /// Unicorn maps the space's own memory once, and a block once from the first turn after a space shows it until the
    /// first after none of its ranges does; a change of permissions maps nothing, and code is dropped only where it ran.
    #[test]
    fn mappings_are_made_once_and_code_is_dropped_where_it_changed() {
        let page = |index: u64| index * PAGE_SIZE;
        let mut memory = Memory::<Recording>::new(END).expect("a space");
        let all = Prot::ALL.0;
        assert_eq!(
            memory.unicorn.take_calls(),
            vec![Call::MemMapHostMemory {
                address: 0,
                size: END,
                perms: all,
                offset: 0,
            }]
        );
        memory.set_permissions(CODE..CODE + PAGE_SIZE, read_execute());
        memory.set_permissions(page(2)..page(4), read_write());
        memory.set_permissions(page(3)..page(4), Permissions::READ);
        memory
            .write(CODE, &SYSTEM_CALL.to_le_bytes())
            .expect("the code written");
        assert_eq!(memory.unicorn.take_calls(), Vec::new());
        assert!(matches!(
            run(&mut memory, CODE, 0, 0, SLICE).0,
            Exit::Syscall(_)
        ));
        let start = Call::EmuStart {
            begin: CODE,
            count: SLICE as usize,
        };
        assert_eq!(
            memory.unicorn.take_calls(),
            vec![Call::CtlFlushTb, start.clone()]
        );
        memory
            .write(CODE, &SYSTEM_CALL.to_le_bytes())
            .expect("the code written again");
        memory.write(page(2), &[1]).expect("a byte of data written");
        memory.set_permissions(CODE..CODE + PAGE_SIZE, read_write());
        memory.set_permissions(CODE..CODE + PAGE_SIZE, read_execute());
        let block: Rc<SharedPages<Recording>> =
            Rc::new(SharedPages::new(PAGE_SIZE).expect("a block"));
        memory.show(page(5)..page(6), Rc::clone(&block), 0, true);
        memory.set_permissions(page(5)..page(6), read_write());
        assert!(matches!(
            run(&mut memory, CODE, 0, 0, SLICE).0,
            Exit::Syscall(_)
        ));
        assert_eq!(
            memory.unicorn.take_calls(),
            vec![
                Call::CtlRemoveCache {
                    address: CODE,
                    end: CODE + 4,
                },
                Call::CtlRemoveCache {
                    address: CODE,
                    end: CODE + PAGE_SIZE,
                },
                Call::MemMapHostMemory {
                    address: SHARED_BASE,
                    size: PAGE_SIZE,
                    perms: all,
                    offset: 0,
                },
                start.clone(),
            ]
        );
        memory.drop_mappings(page(5)..page(6));
        assert!(matches!(
            run(&mut memory, CODE, 0, 0, SLICE).0,
            Exit::Syscall(_)
        ));
        assert_eq!(
            memory.unicorn.take_calls(),
            vec![
                Call::MemUnmap {
                    address: SHARED_BASE,
                    size: PAGE_SIZE,
                },
                start,
            ]
        );
    }

    /// A branch to an address the guest may not run faults on the fetch there, with the program counter at that
    /// address, as an instruction abort leaves it; one to an address that is no multiple of 4 is an alignment fault.
    #[test]
    fn a_branch_to_code_the_guest_may_not_run_stops_at_its_target() {
        /// [BR](https://developer.arm.com/documentation/ddi0602/latest/Base-Instructions/BR--Branch-to-register-) X0.
        const BRANCH_TO_X0: u32 = 0xD61F_0000;
        let mut memory = with_code::<Direct>(&[BRANCH_TO_X0]);
        let target = 8 * PAGE_SIZE;
        let (exit, registers) = run(&mut memory, CODE, target, 0, SLICE);
        assert_eq!(
            exit,
            Exit::MemoryFault {
                address: target,
                access: Access::Execute,
            }
        );
        assert_eq!(registers.pc, target);
        let (exit, registers) = run(&mut memory, CODE, target + 1, 0, SLICE);
        assert_eq!(
            exit,
            Exit::AlignmentFault {
                address: target + 1
            }
        );
        assert_eq!(registers.pc, target + 1);
        // An upper-half target, aligned or not, faults as a lower-half one does.
        for (target, exit) in [
            (
                UPPER | target,
                Exit::MemoryFault {
                    address: UPPER | target,
                    access: Access::Execute,
                },
            ),
            (
                UPPER | target | 2,
                Exit::AlignmentFault {
                    address: UPPER | target | 2,
                },
            ),
        ] {
            let mut memory = with_code::<Direct>(&[BRANCH_TO_X0]);
            for count in [1, SLICE] {
                let (stopped, registers) = run(&mut memory, CODE, target, 0, count);
                assert_eq!(stopped, exit, "a branch to {target:#x}, a slice of {count}");
                assert_eq!(
                    registers.pc, target,
                    "a branch to {target:#x}, a slice of {count}"
                );
            }
        }
    }

    #[test]
    fn touched_pages_are_listed_in_order_and_runs_are_cut() {
        let mut touched = TouchedPages::new(300);
        touched.touch(3..5);
        touched.touch(130..131);
        touched.touch(299..300);
        assert_eq!(touched.list(0..300), vec![3, 4, 130, 299]);
        assert_eq!(touched.list(4..299), vec![4, 130]);
        let pages: Vec<u64> = (10..30).chain([40, 41]).collect();
        assert_eq!(to_runs(&pages), vec![10..26, 26..30, 40..42]);
    }

    #[test]
    fn register_numbers_end_where_unicorn_numbers_them() {
        assert_eq!(x_register(28), RegisterARM64::X28 as i32);
        assert_eq!(x_register(29), RegisterARM64::X29 as i32);
        assert_eq!(x_register(30), RegisterARM64::X30 as i32);
        assert_eq!(q_register(31), RegisterARM64::Q31 as i32);
    }

    /// Before, this index became a value of unicorn's register enum that names no register, which
    /// is undefined behavior from a safe function.
    #[test]
    #[should_panic(expected = "no general register X31")]
    fn a_general_register_past_x30_is_refused() {
        x_register(31);
    }

    /// As for the general registers.
    #[test]
    #[should_panic(expected = "no vector register Q32")]
    fn a_vector_register_past_q31_is_refused() {
        q_register(32);
    }
}
