//! Translated code in a stub (plan 3.2 to 3.4, ARM64): pieces the native output makes for the process way, loaded into
//! a memory's code area with their entry slots filled and their exits linked, and a thread run to its next exit, every
//! stop resolved to the guest's state through the stop crate.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;

use wcontainers_abi::{
    Access, CpuError, Exit, MemoryId, PAGE_SIZE, Permissions, RegisterBlock, Resume,
    SystemCallNumber, SystemCallRequest, ThreadId,
};
use wcontainers_arm64::carry::{LinkWords, link_words};
use wcontainers_arm64::mirror::{
    LAST_TARGET_BRANCH, LAST_TARGET_CALL, LAST_TARGET_TEST, SLOT_MOVE_BITS, SLOT_MOVES,
};
use wcontainers_arm64::thread_state::{FPCR_BITS, RegisterPage};
use wcontainers_arm64::{Arm64, Way};
use wcontainers_executor_arm64_stop::{
    self as stop, FloatStatus, GENERAL, GuestState, Interrupted, Piece, Resolved, SYSCALL_NUMBER,
    Stop, Taken,
};
use wcontainers_guest_cpu::HostCode;
use wcontainers_guest_cpu::position::NativeReason;
use wcontainers_guest_cpu::reason::CODE_CHANGED_AFTER_IT_RAN;
use wcontainers_translator::{LookAhead, translate_native_for};

use super::{FaultOwner, HostProcessExecutor, Then};
use crate::code::CodeArea;
use crate::layout::{
    CODE_AREA, CODE_AREA_SIZE, CODE_FILE_SIZE, OUR_AREA, REGISTER_PAGES, SLOT_TABLE,
    SLOT_TABLE_SIZE, THREAD_PAGE_SIZE,
};
use crate::pages::Command;
use crate::probe::{CTR_EL0_IMINLINE_SHIFT, CTR_EL0_LINE_MASK};
use crate::stub::TurnEnd;

/// The way every piece here is translated for.
const WAY: Way = Way::Process;

/// The bytes of one instruction.
const INSTRUCTION: u64 = size_of::<u32>() as u64;

/// The slow paths of running translated code, as counted and warned.
pub mod reason {
    /// Guest code was translated, at its first run in a memory.
    pub const TRANSLATED: &str = "native code translated";
    /// The code area was full, so every piece of its memory was dropped.
    pub const CODE_AREA_EMPTIED: &str = "code area emptied";
    /// Every page of the slot table a memory can hold is in use, so its process is ended.
    pub const SLOT_PAGES_EXHAUSTED: &str = "slot pages exhausted";
    /// A turn limited to a count ended past it: a host process cannot stop a thread at an exact count.
    pub const TURN_LIMIT_PASSED: &str = "turn limit passed";
    /// A stub reported an exit from translated code that nothing we loaded makes, so its process is ended.
    pub const EXIT_NOT_UNDERSTOOD: &str = "stub exit not understood";
    /// A fault's frame held no syndrome, so whether it wrote is taken from the page's permissions.
    pub const FAULT_WITHOUT_SYNDROME: &str = "fault without syndrome";
}

/// An immediate field of a word: the word with the field clear, and the field's bits.
#[derive(Clone, Copy)]
struct Field {
    clear: u32,
    mask: u32,
}

impl Field {
    fn new((clear, mask): (u32, u32)) -> Self {
        Self { clear, mask }
    }

    /// The word with the low bits of `value` in the field.
    fn with(self, value: u64) -> u32 {
        self.clear | ((value as u32) << self.mask.trailing_zeros()) & self.mask
    }

    /// The signed value in the field of `word`.
    fn value(self, word: u32) -> i64 {
        let bits = self.mask.count_ones();
        let raw = (word & self.mask) >> self.mask.trailing_zeros();
        let shift = u32::BITS - bits;
        i64::from(((raw << shift) as i32) >> shift)
    }

    /// Whether the signed `value` fits the field.
    fn fits(self, value: i64) -> bool {
        let half = 1i64 << (self.mask.count_ones() - 1);
        (-half..half).contains(&value)
    }
}

/// A word that leaves a piece for a target, and how it is linked.
#[derive(Clone, Copy, Debug)]
struct Site {
    /// Its host address.
    at: u64,
    /// The word as translated.
    word: u32,
    /// The bits of its own offset where the word itself is pointed at the target's piece.
    retarget: Option<u32>,
}

/// One loaded piece.
struct Loaded {
    base: u64,
    /// Its code as translated, before any linking.
    code: Arc<HostCode>,
    /// The guest code it was made from, as far as it covers.
    source: Vec<u8>,
    /// Its exits: their targets and host addresses.
    exits: Vec<(u64, u64)>,
}

/// One memory's translated code: its code area, its slot pages and its pieces.
pub(super) struct Code {
    area: CodeArea,
    /// The next free byte of the code area.
    next: u64,
    /// Each slot page in use: its address in the stub and its bytes' offset in the memfd.
    slot_pages: HashMap<u64, usize>,
    /// The next free offset for a slot page.
    slot_next: usize,
    /// The pieces by guest address.
    pieces: BTreeMap<u64, Loaded>,
    /// The guest address of the piece at each host address.
    hosts: BTreeMap<u64, u64>,
    /// The sites linked, or to be linked, to each guest address.
    exits_to: HashMap<u64, Vec<Site>>,
}

impl Code {
    /// The piece holding host address `pc`, or an empty one at `pc` in the slot table; none for any other address.
    fn piece_at(&self, pc: u64) -> Option<Piece<'_>> {
        let found = self
            .hosts
            .range(..=pc)
            .next_back()
            .and_then(|(&base, guest)| {
                let loaded = &self.pieces[guest];
                (pc - base < loaded.code.bytes.len() as u64).then_some(Piece {
                    base,
                    length: loaded.code.bytes.len(),
                    spans: &loaded.code.spans,
                })
            });
        found.or_else(|| {
            (SLOT_TABLE as u64..(SLOT_TABLE + SLOT_TABLE_SIZE) as u64)
                .contains(&pc)
                .then_some(Piece {
                    base: pc,
                    length: 0,
                    spans: &[],
                })
        })
    }
}

/// A guest thread's state between its turns: the guest's own, as resolution gives it.
pub(super) struct Guest {
    state: GuestState,
    float: FloatStatus,
    page: RegisterPage,
    vectors: [[u64; 2]; 32],
    tpidr: u64,
}

impl Guest {
    pub(super) fn new() -> Self {
        Self {
            state: GuestState {
                x: [0; GENERAL],
                sp: 0,
                pc: 0,
                flags: 0,
                count: 0,
                exclusive: Default::default(),
            },
            float: FloatStatus::default(),
            page: RegisterPage::default(),
            vectors: [[0; 2]; 32],
            tpidr: 0,
        }
    }

    pub(super) fn registers(&self, into: &mut RegisterBlock) {
        into.x = self.state.x;
        into.sp = self.state.sp;
        into.pc = self.state.pc;
        into.pstate = self.state.flags;
        into.v = self.vectors;
        into.fpsr = self.float.fpsr;
        into.fpcr = self.float.fpcr;
        into.tpidr_el0 = self.tpidr;
    }

    pub(super) fn set_registers(&mut self, from: &RegisterBlock) {
        self.state.x = from.x;
        self.state.sp = from.sp;
        self.state.pc = from.pc;
        self.state.flags = from.pstate;
        self.vectors = from.v;
        self.float.fpsr = from.fpsr;
        self.float.fpcr = from.fpcr & FPCR_BITS;
        self.tpidr = from.tpidr_el0;
    }

    pub(super) fn count(&self) -> u64 {
        self.state.count
    }
}

/// Translations already made, by guest address and the code they were made from: the process way's code depends on
/// nothing else, so a fork child or another memory loads them again as they are.
#[derive(Default)]
pub(super) struct Translations {
    kept: HashMap<(u64, Vec<u8>), Option<Arc<HostCode>>>,
}

/// How a run of translated code came back.
enum Ended {
    /// The turn is over: a stop, or its limit reached.
    TurnOver,
    /// The guest announced that the code at this address changed.
    CodeChanged(u64),
    /// A jump's first miss: its last target is to be made, the code going on where it was carried.
    LastTarget {
        call: u64,
        target: u64,
    },
    Stopped(Stop),
    /// The process can only be killed.
    Killed,
}

/// The link words, made once.
fn words() -> &'static LinkWords {
    static WORDS: std::sync::OnceLock<LinkWords> = std::sync::OnceLock::new();
    WORDS.get_or_init(link_words)
}

impl HostProcessExecutor {
    /// Counts a slow path, to be warned at the next [`Self::take_warnings`].
    pub(super) fn slow(&mut self, reason: &'static str, times: u64, detail: u64) {
        let entry = self.slow.entry(reason).or_default();
        entry.0 += times;
        entry.1 = detail;
        self.unsent.insert(reason, *entry);
    }

    /// Every slow path counted since this was last called, with its count so far and its last detail.
    pub(super) fn take_warnings(&mut self) -> Vec<(&'static str, u64, u64)> {
        core::mem::take(&mut self.unsent)
            .into_iter()
            .map(|(reason, (count, detail))| (reason, count, detail))
            .collect()
    }

    /// The memory's code, made with its code area mapped at the next turn on first use.
    fn code(&mut self, memory: MemoryId) -> Result<&mut Code, CpuError> {
        let index = memory.0 as usize;
        if self.codes.len() <= index {
            self.codes.resize_with(index + 1, || None);
        }
        if self.codes[index].is_none() {
            let area = CodeArea::new().map_err(super::host_error)?;
            let file = area.shared_file();
            self.memory_mut(memory).push_code(Command::MapCode, file);
            self.codes[index] = Some(Code {
                area,
                next: CODE_AREA as u64,
                slot_pages: HashMap::new(),
                slot_next: CODE_AREA_SIZE,
                pieces: BTreeMap::new(),
                hosts: BTreeMap::new(),
                exits_to: HashMap::new(),
            });
        }
        Ok(self.codes[index].as_mut().expect("made above"))
    }

    /// Drops a memory's code with the memory.
    pub(super) fn drop_code(&mut self, memory: MemoryId) {
        if let Some(code) = self.codes.get_mut(memory.0 as usize) {
            *code = None;
        }
    }

    /// The guest translation of the code at `pc`, made or kept.
    fn translate(&mut self, pc: u64, code: Vec<u8>) -> Option<Arc<HostCode>> {
        let key = (pc, code);
        if let Some(kept) = self.translations.kept.get(&key) {
            return kept.clone();
        }
        let made = translate_native_for::<Arm64>(&key.1, pc, LookAhead::Full, true, &WAY, &())
            .map(Arc::new);
        self.slow(reason::TRANSLATED, 1, pc);
        self.translations.kept.insert(key, made.clone());
        made
    }

    /// Makes sure a piece starting at guest `pc` is loaded in `memory`, or gives the exit that running from there is.
    fn load(&mut self, memory: MemoryId, pc: u64) -> Result<(), Exit> {
        if self
            .code(memory)
            .map_err(|_| Exit::MemoryFaultWithoutRegisters)?
            .pieces
            .contains_key(&pc)
        {
            return Ok(());
        }
        if !pc.is_multiple_of(INSTRUCTION) {
            return Err(Exit::AlignmentFault { address: pc });
        }
        let refused = Exit::MemoryFault {
            address: pc,
            access: Access::Execute,
        };
        let page = pc & !(PAGE_SIZE - 1);
        let executable = pc < OUR_AREA as u64
            && self
                .memory(memory)
                .permissions
                .get(&page)
                .is_some_and(|granted| granted.contains(Permissions::EXECUTE));
        if !executable {
            return Err(refused);
        }
        // The rest of the page: a piece never runs on into another page, whose permissions may differ.
        let mut source = vec![0; (page + PAGE_SIZE - pc) as usize];
        self.read_guest(memory, pc, &mut source)
            .map_err(|_| refused)?;
        let piece = self
            .translate(pc, source.clone())
            .ok_or(Exit::IllegalInstruction)?;
        let base = match self.place(memory, &piece.bytes) {
            Some(base) => base,
            None => {
                self.empty_code_area(memory);
                self.place(memory, &piece.bytes)
                    .expect("one piece fits an empty code area")
            }
        };
        for reason in NativeReason::ALL {
            let sites = piece.sites[reason.index()];
            if sites != 0 {
                self.slow(reason.name(), sites, pc);
            }
        }
        // Code placed out of line comes after the run's last instruction, so the run ends at its highest guest.
        let covered = piece
            .spans
            .iter()
            .map(|span| span.guest + INSTRUCTION - pc)
            .max()
            .unwrap_or(0);
        source.truncate(covered as usize);
        let site = |start: u32, retarget: Option<u32>| {
            let from = start as usize;
            let word = &piece.bytes[from..from + size_of::<u32>()];
            Site {
                at: base + u64::from(start),
                word: u32::from_le_bytes(word.try_into().expect("a word")),
                retarget,
            }
        };
        let mut sites: Vec<(u64, Site)> = piece
            .exits
            .iter()
            .map(|exit| (exit.target, site(exit.start, exit.retarget)))
            .collect();
        // Each branch of a known jump's table is an exit to its target; one no piece can be at stays on the slot.
        for table in &piece.tables {
            for k in 0..table.entries {
                let target = table.target(k);
                if target >> WAY.reach_bits() == 0 && target.is_multiple_of(INSTRUCTION) {
                    let start = table.start + k * size_of::<u32>() as u32;
                    sites.push((target, site(start, Some(table.retarget))));
                }
            }
        }
        let code = self.code(memory).expect("made above");
        code.pieces.insert(
            pc,
            Loaded {
                base,
                code: Arc::clone(&piece),
                source,
                exits: sites
                    .iter()
                    .map(|(target, site)| (*target, site.at))
                    .collect(),
            },
        );
        code.hosts.insert(base, pc);
        for &(target, site) in &sites {
            code.exits_to.entry(target).or_default().push(site);
        }
        if self.fill_slot(memory, pc, base).is_none() {
            self.slow(reason::SLOT_PAGES_EXHAUSTED, 1, pc);
            return Err(Exit::MemoryFaultWithoutRegisters);
        }
        let code = &self.codes[memory.0 as usize].as_ref().expect("made above");
        let into = code
            .exits_to
            .get(&pc)
            .into_iter()
            .flatten()
            .map(|site| (*site, base));
        let out = sites
            .iter()
            .filter(|(target, _)| *target != pc)
            .filter_map(|(target, site)| Some((*site, code.pieces.get(target)?.base)));
        let links: Vec<(u64, u32)> = into
            .chain(out)
            .filter_map(|(site, to)| Some((site.at, self.link_word(&site, to)?)))
            .collect();
        for (at, word) in links {
            self.write_code(memory, at, &[word]);
        }
        Ok(())
    }

    /// Writes host words into a memory's code area.
    fn write_code(&mut self, memory: MemoryId, at: u64, words: &[u32]) {
        let probe = *self.host.probe();
        let code = self.codes[memory.0 as usize]
            .as_mut()
            .expect("a memory with code");
        code.area.write(&probe, at as usize, words);
    }

    /// Writes host code into the next free bytes of the code area, giving its address; none when it does not fit.
    fn place(&mut self, memory: MemoryId, bytes: &[u8]) -> Option<u64> {
        let code = self.codes[memory.0 as usize].as_mut()?;
        let start = code.next;
        let end = start + bytes.len() as u64;
        if end > (CODE_AREA + CODE_AREA_SIZE) as u64 {
            return None;
        }
        code.next = end;
        let words: Vec<u32> = bytes
            .as_chunks::<{ size_of::<u32>() }>()
            .0
            .iter()
            .map(|word| u32::from_le_bytes(*word))
            .collect();
        self.write_code(memory, start, &words);
        Some(start)
    }

    /// Drops every piece of a memory and starts its code area again; its slot pages stay its own, every slot empty.
    fn empty_code_area(&mut self, memory: MemoryId) {
        let probe = *self.host.probe();
        let empty = words().empty;
        let code = self.codes[memory.0 as usize]
            .as_mut()
            .expect("a memory with code");
        let dropped = code.pieces.len() as u64;
        code.pieces.clear();
        code.hosts.clear();
        code.exits_to.clear();
        code.next = CODE_AREA as u64;
        let page = vec![empty; THREAD_PAGE_SIZE / size_of::<u32>()];
        let offsets: Vec<usize> = code.slot_pages.values().copied().collect();
        for offset in offsets {
            code.area.write_slots(&probe, offset, &page);
        }
        self.slow(reason::CODE_AREA_EMPTIED, 1, dropped);
    }

    /// The word that links `site` to the piece at `to`, when it reaches.
    fn link_word(&self, site: &Site, to: u64) -> Option<u32> {
        let offset = (to.wrapping_sub(site.at) as i64) / INSTRUCTION as i64;
        let branch = match site.retarget {
            Some(mask) => Field {
                clear: site.word & !mask,
                mask,
            },
            None => Field::new(words().branch),
        };
        branch.fits(offset).then(|| branch.with(offset as u64))
    }

    /// Fills the entry slot of guest `guest` with a jump to the piece at `base`, its page mapped in the stub at its
    /// first use; none when no slot page is free.
    fn fill_slot(&mut self, memory: MemoryId, guest: u64, base: u64) -> Option<()> {
        let probe = *self.host.probe();
        let words_of = words();
        let at = WAY.slot(guest);
        let page = at & !(THREAD_PAGE_SIZE as u64 - 1);
        let code = self.codes[memory.0 as usize].as_mut()?;
        let offset = match code.slot_pages.get(&page) {
            Some(&offset) => offset,
            None => {
                let offset = code.slot_next;
                if offset + THREAD_PAGE_SIZE > CODE_FILE_SIZE {
                    return None;
                }
                code.slot_next += THREAD_PAGE_SIZE;
                code.slot_pages.insert(page, offset);
                let file = code.area.shared_file();
                self.memory_mut(memory).push_code(
                    Command::MapSlots {
                        address: page as usize,
                        offset,
                    },
                    file,
                );
                offset
            }
        };
        let mut words = [words_of.jump; SLOT_MOVES + 1];
        for (k, (word, &field)) in words.iter_mut().zip(&words_of.moves).enumerate() {
            *word = Field::new(field).with(base >> (k as u32 * SLOT_MOVE_BITS));
        }
        let code = self.codes[memory.0 as usize].as_mut()?;
        code.area
            .write_slots(&probe, offset + (at - page) as usize, &words);
        Some(())
    }

    /// The host address of the word in line that leads to the out-of-line call whose `blr` is at `call`: the word
    /// before the one its closing `b` goes back to (`str x30; mov x30, entry; blr x30; ldr x30; b back`, plan 3.2).
    fn call_in_line(&self, memory: MemoryId, call: u64) -> Option<u64> {
        let code = self.codes[memory.0 as usize].as_ref()?;
        let (&base, guest) = code.hosts.range(..=call).next_back()?;
        let bytes = &code.pieces[guest].code.bytes;
        let back_at = call + 2 * INSTRUCTION;
        let from = usize::try_from(back_at - base).ok()?;
        let word = u32::from_le_bytes(bytes.get(from..from + size_of::<u32>())?.try_into().ok()?);
        let back = back_at
            .wrapping_add_signed(Field::new(words().branch).value(word) * INSTRUCTION as i64);
        Some(back - INSTRUCTION)
    }

    /// Makes guest `target` the last target of the jump whose call into the floor has its `blr` at host address
    /// `call`. A target no piece can be at is not made one, so the call comes again at the next miss.
    fn make_last_target(&mut self, memory: MemoryId, call: u64, target: u64) {
        if target >> WAY.reach_bits() != 0 || !target.is_multiple_of(INSTRUCTION) {
            return;
        }
        let Some(call) = self.call_in_line(memory, call) else {
            return;
        };
        let words_of = words();
        let word = |index: usize| call - ((LAST_TARGET_CALL - index) as u64 * INSTRUCTION);
        let site = Site {
            at: word(LAST_TARGET_BRANCH),
            word: words_of.last_target_branch,
            retarget: None,
        };
        let code = self.codes[memory.0 as usize]
            .as_ref()
            .expect("a memory with code");
        let Some(&holder) = code
            .hosts
            .range(..=call)
            .next_back()
            .map(|(_, guest)| guest)
        else {
            return;
        };
        let link = code
            .pieces
            .get(&target)
            .and_then(|piece| self.link_word(&site, piece.base));
        let mut moves = [0; SLOT_MOVES];
        let fields = core::iter::once(&words_of.last_target_first).chain(&words_of.moves[1..]);
        for (k, (move_word, &field)) in moves.iter_mut().zip(fields).enumerate() {
            *move_word = Field::new(field).with(target >> (k as u32 * SLOT_MOVE_BITS));
        }
        self.write_code(memory, word(0), &moves);
        self.write_code(memory, word(LAST_TARGET_TEST), &[words_of.last_target_test]);
        if let Some(linked) = link {
            self.write_code(memory, site.at, &[linked]);
        }
        let code = self.codes[memory.0 as usize]
            .as_mut()
            .expect("a memory with code");
        if let Some(piece) = code.pieces.get_mut(&holder) {
            piece.exits.push((target, site.at));
        }
        code.exits_to.entry(target).or_default().push(site);
    }

    /// Drops the pieces of `memory` made from guest code overlapping `range`; whether one was dropped, and whether the
    /// code of one is no longer what it was made from.
    fn forget_code(&mut self, memory: MemoryId, range: std::ops::Range<u64>) -> (bool, bool) {
        let Some(code) = self.codes.get(memory.0 as usize).and_then(Option::as_ref) else {
            return (false, false);
        };
        let overlapping: Vec<(u64, Vec<u8>)> = code
            .pieces
            .iter()
            .filter(|(at, loaded)| {
                **at < range.end && range.start < **at + loaded.source.len() as u64
            })
            .map(|(&at, loaded)| (at, loaded.source.clone()))
            .collect();
        let mut changed = false;
        for (at, source) in &overlapping {
            let mut now = vec![0; source.len()];
            changed |= self.read_guest(memory, *at, &mut now).is_err() || now != *source;
        }
        let empty = words().empty;
        for (at, _) in &overlapping {
            self.drop_piece(memory, *at, empty);
        }
        (!overlapping.is_empty(), changed)
    }

    /// Drops the pieces of a range the kernel changed, counting "code changed after it ran" where one's code changed.
    pub(super) fn forget_changed_code(&mut self, memory: MemoryId, address: u64, length: u64) {
        if self
            .forget_code(memory, address..address.saturating_add(length))
            .1
        {
            self.slow(CODE_CHANGED_AFTER_IT_RAN, 1, address);
        }
    }

    /// Drops the piece at guest `at`: its slot emptied, the exits linked to it unlinked, its own exits forgotten.
    fn drop_piece(&mut self, memory: MemoryId, at: u64, empty: u32) {
        let probe = *self.host.probe();
        let code = self.codes[memory.0 as usize]
            .as_mut()
            .expect("a memory with code");
        let Some(loaded) = code.pieces.remove(&at) else {
            return;
        };
        code.hosts.remove(&loaded.base);
        let slot = WAY.slot(at);
        let page = slot & !(THREAD_PAGE_SIZE as u64 - 1);
        if let Some(&offset) = code.slot_pages.get(&page) {
            code.area
                .write_slots(&probe, offset + (slot - page) as usize, &[empty]);
        }
        let unlink: Vec<Site> = code.exits_to.get(&at).cloned().unwrap_or_default();
        for site in unlink {
            code.area.write(&probe, site.at as usize, &[site.word]);
        }
        for (target, site) in loaded.exits {
            if let Some(sites) = code.exits_to.get_mut(&target) {
                sites.retain(|s| s.at != site);
                if sites.is_empty() {
                    code.exits_to.remove(&target);
                }
            }
        }
    }

    /// What a fault in translated code was, from its syndrome where the frame gave one.
    fn taken(
        &mut self,
        memory: MemoryId,
        address: u64,
        signal: u32,
        code: u32,
        syndrome: u64,
    ) -> Taken {
        /// The syndrome's exception classes (Arm ARM, ESR_EL1.EC).
        const CLASS_SHIFT: u32 = 26;
        const INSTRUCTION_ABORT: u64 = 0x20;
        const PC_ALIGNMENT: u64 = 0x22;
        const DATA_ABORT: u64 = 0x24;
        const SP_ALIGNMENT: u64 = 0x26;
        /// A data abort's write-not-read bit and its status code of an alignment fault (ISS.WnR, ISS.DFSC).
        const WRITE_NOT_READ: u64 = 1 << 6;
        const STATUS: u64 = 0x3f;
        const ALIGNMENT_STATUS: u64 = 0x21;
        if syndrome != 0 {
            return match syndrome >> CLASS_SHIFT {
                DATA_ABORT => Taken::Access {
                    alignment: syndrome & STATUS == ALIGNMENT_STATUS,
                    fault: address,
                    write: syndrome & WRITE_NOT_READ != 0,
                },
                SP_ALIGNMENT => Taken::StackAlignment,
                PC_ALIGNMENT => Taken::JumpAlignment,
                INSTRUCTION_ABORT => Taken::Other,
                _ => Taken::Other,
            };
        }
        match signal as i32 {
            libc::SIGBUS if code as i32 == libc::BUS_ADRALN => Taken::Access {
                alignment: true,
                fault: address,
                write: false,
            },
            libc::SIGSEGV | libc::SIGBUS => {
                self.slow(reason::FAULT_WITHOUT_SYNDROME, 1, address);
                let page = address & !(PAGE_SIZE - 1);
                Taken::Access {
                    alignment: false,
                    fault: address,
                    write: self
                        .memory(memory)
                        .permissions
                        .get(&page)
                        .is_some_and(|granted| granted.contains(Permissions::WRITE)),
                }
            }
            _ => Taken::Other,
        }
    }

    /// Runs `thread` from `resume` to its next exit, or, from the first exit at or after the count `until`, to
    /// [`Exit::Stop`]; `slice` bounds the time of each entry into translated code. `clock` reads the monotonic clock
    /// for the guest's counter reads.
    pub(super) fn run_translated(
        &mut self,
        thread: ThreadId,
        resume: Resume,
        until: Option<u64>,
        slice: Option<Duration>,
        clock: &mut dyn FnMut() -> u64,
    ) -> Exit {
        let super::Thread { memory, index, .. } = self.thread(thread);
        let mut t = self.guests[thread.0 as usize]
            .take()
            .expect("a thread the kernel holds");
        if let Resume::Return(value) = resume {
            t.state.x[0] = value;
        }
        let exit = self.run_pieces(thread, memory, index, &mut t, until, slice, clock);
        self.guests[thread.0 as usize] = Some(t);
        exit
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "one thread's run, its parts named"
    )]
    fn run_pieces(
        &mut self,
        thread: ThreadId,
        memory: MemoryId,
        index: usize,
        t: &mut Guest,
        until: Option<u64>,
        slice: Option<Duration>,
        clock: &mut dyn FnMut() -> u64,
    ) -> Exit {
        let reached = |state: &GuestState| until.is_some_and(|until| state.count >= until);
        let register_page = (REGISTER_PAGES + index * THREAD_PAGE_SIZE) as u64;
        let mut carried: Option<(Interrupted, RegisterPage)> = None;
        loop {
            let resumed = carried.take();
            if resumed.is_none()
                && let Err(exit) = self.load(memory, t.state.pc)
            {
                return exit;
            }
            let (mut hardware, mut page) = resumed.unwrap_or_else(|| {
                let (mut hardware, page) =
                    stop::rejoin(&t.state, t.page.clone(), &t.float, register_page);
                hardware.pc = self.codes[memory.0 as usize]
                    .as_ref()
                    .expect("loaded above")
                    .pieces[&t.state.pc]
                    .base;
                (hardware, page)
            });
            let ended = loop {
                let block = RegisterBlock {
                    x: hardware.x[..31].try_into().expect("x0 to x30"),
                    sp: hardware.sp,
                    pc: hardware.pc,
                    pstate: hardware.state,
                    v: t.vectors,
                    fpsr: hardware.fpsr,
                    fpcr: hardware.fpcr,
                    tpidr_el0: t.tpidr,
                };
                let stub = &self.memory(memory).stub;
                stub.set_registers(index, &block);
                stub.set_translated_page(index, &page);
                let end = match self.turn_with(thread, Then::Enter(hardware.pc as usize), slice) {
                    Ok(end) => end,
                    Err(_) => break Ended::Killed,
                };
                let stub = &self.memory(memory).stub;
                let block = stub.registers(index);
                let held = stub.translated_page(index);
                let mut h = Interrupted {
                    x: block.x[..GENERAL]
                        .try_into()
                        .expect("the general registers"),
                    sp: block.sp,
                    pc: block.pc,
                    state: block.pstate,
                    fpsr: block.fpsr,
                    fpcr: block.fpcr,
                };
                let taken = match end {
                    TurnEnd::Called { entry } => {
                        h.pc = block.pc.wrapping_sub(INSTRUCTION);
                        Taken::Call(entry as u16)
                    }
                    TurnEnd::Fault {
                        address,
                        signal,
                        code,
                    } => {
                        let syndrome = self.memory(memory).stub.fault_syndrome(index);
                        self.taken(memory, address, signal, code, syndrome)
                    }
                    TurnEnd::Interrupted { at } => {
                        h.pc = at;
                        t.vectors = block.v;
                        t.tpidr = block.tpidr_el0;
                        let state = self.resolve_interrupt(memory, &h, &held, &mut t.float);
                        match state {
                            Some(state) => {
                                t.state = state;
                                t.page = held;
                                break Ended::TurnOver;
                            }
                            None => break self.not_understood(memory, h.pc),
                        }
                    }
                    // A stop before the code ran: the block was not touched.
                    TurnEnd::Stopped { .. } => break Ended::TurnOver,
                    _ => break self.not_understood(memory, h.pc),
                };
                t.vectors = block.v;
                t.tpidr = block.tpidr_el0;
                let resolved = {
                    let code = self.codes[memory.0 as usize]
                        .as_ref()
                        .expect("loaded above");
                    let this = &*self;
                    let mut memory_of = |at: u64, into: &mut [u8]| {
                        let _ = this.read_guest(memory, at, into);
                    };
                    code.piece_at(h.pc).and_then(|piece| {
                        stop::resolve(WAY, &piece, taken, &h, &held, &mut t.float, &mut memory_of)
                    })
                };
                match resolved {
                    Some(Resolved::Again(next, next_page)) => {
                        hardware = next;
                        page = next_page;
                    }
                    Some(Resolved::FloatStatusNotKept(next, next_page)) => {
                        hardware = next;
                        page = next_page;
                        self.slow(
                            wcontainers_guest_cpu::reason::EXACT_FLOATING_POINT_REQUESTED,
                            1,
                            t.state.pc,
                        );
                    }
                    Some(Resolved::Clock {
                        into,
                        then,
                        page: then_page,
                    }) => {
                        hardware = then;
                        page = then_page;
                        if let Some(register) = into {
                            stop::set_register(register, clock(), &mut hardware, &mut page);
                        }
                    }
                    Some(Resolved::CodeChanged {
                        address,
                        at,
                        then,
                        page: then_page,
                    }) => {
                        if !self.readable(memory, address) {
                            t.state = *at;
                            t.page = held;
                            break Ended::Stopped(Stop::MemoryFault {
                                address,
                                write: false,
                            });
                        }
                        match self.resolve_interrupt(memory, &then, &then_page, &mut t.float) {
                            Some(state) => {
                                t.state = state;
                                t.page = then_page;
                                break Ended::CodeChanged(address);
                            }
                            None => break self.not_understood(memory, then.pc),
                        }
                    }
                    Some(Resolved::LastTarget {
                        call,
                        target,
                        then,
                        page: then_page,
                    }) => {
                        carried = Some((then, then_page));
                        break Ended::LastTarget { call, target };
                    }
                    Some(Resolved::Stop(stop, state)) => {
                        t.state = state;
                        t.page = held;
                        break Ended::Stopped(stop);
                    }
                    None => break self.not_understood(memory, h.pc),
                }
            };
            let stop = match ended {
                Ended::TurnOver => return Exit::Stop,
                Ended::Killed => return Exit::MemoryFaultWithoutRegisters,
                Ended::CodeChanged(address) => {
                    self.code_line_changed(memory, address);
                    if reached(&t.state) {
                        return Exit::Stop;
                    }
                    continue;
                }
                Ended::LastTarget { call, target } => {
                    self.make_last_target(memory, call, target);
                    continue;
                }
                Ended::Stopped(stop) => stop,
            };
            match stop {
                Stop::Landed(_) if reached(&t.state) => {
                    self.slow(
                        reason::TURN_LIMIT_PASSED,
                        1,
                        t.state.count - until.unwrap_or(0),
                    );
                    return Exit::Stop;
                }
                Stop::Landed(_) => {}
                Stop::Syscall => {
                    let mut call = SystemCallRequest {
                        number: SystemCallNumber(t.state.x[SYSCALL_NUMBER]),
                        arguments: Default::default(),
                    };
                    let arguments = call.arguments.len();
                    call.arguments.copy_from_slice(&t.state.x[..arguments]);
                    return Exit::Syscall(call);
                }
                Stop::Breakpoint(comment) => return Exit::Breakpoint { comment },
                Stop::Illegal => return Exit::IllegalInstruction,
                Stop::Alignment(address) => return Exit::AlignmentFault { address },
                Stop::MemoryFault { address, write } => {
                    let access = if write { Access::Write } else { Access::Read };
                    match self.resolve_fault(memory, address, access) {
                        // The same instruction runs again, over the page as the kernel allows it.
                        Ok(FaultOwner::Executor) => {}
                        Ok(FaultOwner::Guest) => {
                            self.slow(wcontainers_guest_cpu::reason::MEMORY_FAULT, 1, address);
                            return Exit::MemoryFault { address, access };
                        }
                        Err(_) => return Exit::MemoryFaultWithoutRegisters,
                    }
                }
            }
        }
    }

    /// The guest's state at an interrupt at `h`'s pc, none where that is in no piece we loaded.
    fn resolve_interrupt(
        &self,
        memory: MemoryId,
        h: &Interrupted,
        page: &RegisterPage,
        float: &mut FloatStatus,
    ) -> Option<GuestState> {
        let code = self.codes[memory.0 as usize].as_ref()?;
        let mut memory_of = |at: u64, into: &mut [u8]| {
            let _ = self.read_guest(memory, at, into);
        };
        let piece = code.piece_at(h.pc)?;
        stop::interrupted(WAY, &piece, h, page, float, &mut memory_of)
    }

    /// A stub reported what no code we loaded does: its process is ended.
    fn not_understood(&mut self, memory: MemoryId, pc: u64) -> Ended {
        self.slow(reason::EXIT_NOT_UNDERSTOOD, 1, pc);
        let _ = self.host.kill(&self.memory(memory).stub);
        Ended::Killed
    }

    /// Whether the guest may read the byte at `address`.
    fn readable(&self, memory: MemoryId, address: u64) -> bool {
        self.memory(memory)
            .permissions
            .get(&(address & !(PAGE_SIZE - 1)))
            .is_some_and(|granted| granted.intersects(Permissions::READ | Permissions::WRITE))
    }

    /// The guest said the code at `address` changed: drops what was translated from its line, in every memory that
    /// shows the same bytes there.
    fn code_line_changed(&mut self, memory: MemoryId, address: u64) {
        let line = address & !(self.code_line() - 1);
        let length = self.code_line();
        let mut lines = vec![(memory, line)];
        if let Some(&(block, delta)) = self.memory(memory).shared.get(&line) {
            let offset = line.wrapping_add(delta);
            for (other, m) in self.memories.iter().enumerate() {
                let Some(m) = m else { continue };
                lines.extend(
                    m.shared
                        .iter()
                        .filter(|(_, (b, _))| *b == block)
                        .map(|(range, (_, d))| (range.clone(), offset.wrapping_sub(*d)))
                        .filter(|(range, at)| range.contains(at))
                        .map(|(_, at)| (MemoryId(other as u32), at)),
                );
            }
        }
        lines.sort_unstable();
        lines.dedup();
        for (other, line) in lines {
            if self.forget_code(other, line..line + length).0 {
                self.slow(CODE_CHANGED_AFTER_IT_RAN, 1, line);
            }
        }
    }

    /// The bytes one announcement of changed code covers: the instruction cache line of the CPU we present.
    fn code_line(&self) -> u64 {
        INSTRUCTION
            << ((wcontainers_arm64::CACHE_TYPE >> CTR_EL0_IMINLINE_SHIFT) & CTR_EL0_LINE_MASK)
    }
}
