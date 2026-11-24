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
use wcontainers_arm64::carry::{LinkWords, link_words, movable};
use wcontainers_arm64::encode::branch_reach;
use wcontainers_arm64::mirror::{
    LAST_TARGET_BRANCH, LAST_TARGET_CALL, LAST_TARGET_TEST, SLOT_MOVE_BITS, SLOT_MOVES,
};
use wcontainers_arm64::thread_state::{FPCR_BITS, RegisterPage};
use wcontainers_arm64::{Arm64, Instruction, Way};
use wcontainers_executor_arm64_stop::{
    self as stop, FloatStatus, GENERAL, GuestState, Interrupted, Piece, Resolved, SYSCALL_NUMBER,
    Stop, Taken,
};
use wcontainers_guest_cpu::position::NativeReason;
use wcontainers_guest_cpu::reason::CODE_CHANGED_AFTER_IT_RAN;
use wcontainers_guest_cpu::{HostCode, Translation};
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

/// The link register, which a call into the floor keeps in the register page.
const LINK: u8 = 30;

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
    code: Arc<Translation>,
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

/// Where a thread a stop interrupted in translated code is resumed: the hardware's state there, and the piece it ran in
/// as loaded then.
struct Where {
    hardware: Interrupted,
    page: RegisterPage,
    start: u64,
    base: u64,
    code: Arc<Translation>,
}

/// A guest thread's state between its turns: the guest's own, as resolution gives it.
pub(super) struct Guest {
    state: GuestState,
    float: FloatStatus,
    page: RegisterPage,
    vectors: [[u64; 2]; 32],
    /// The vectors are the register page's, newer than `vectors`: they stay there between turns and are read only when
    /// the registers are.
    vectors_held: bool,
    tpidr: u64,
    /// Where a stop left it in translated code, kept until its registers are replaced.
    interrupted: Option<Where>,
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
            vectors_held: false,
            tpidr: 0,
            interrupted: None,
        }
    }

    /// The guest's registers; its vectors are those of [`Self::take_vectors`] when the register page holds them.
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
        self.interrupted = None;
        self.state.x = from.x;
        self.state.sp = from.sp;
        self.state.pc = from.pc;
        self.state.flags = from.pstate;
        self.vectors = from.v;
        self.vectors_held = false;
        self.float.fpsr = from.fpsr;
        self.float.fpcr = from.fpcr & FPCR_BITS;
        self.tpidr = from.tpidr_el0;
    }

    /// Whether the register page holds the vectors, which must then be given to [`Self::take_vectors`].
    pub(super) fn vectors_held(&self) -> bool {
        self.vectors_held
    }

    /// Takes the vectors the register page holds.
    pub(super) fn take_vectors(&mut self, vectors: [[u64; 2]; 32]) {
        self.vectors = vectors;
        self.vectors_held = false;
    }

    pub(super) fn count(&self) -> u64 {
        self.state.count
    }
}

/// Translations already made, by the code they were made from and its offset in its page, each with the page it was
/// made at: the process way's code depends on nothing else, so a fork child, another memory or a program placed at
/// another base loads them again, moved there where moving gives what translating there would.
#[derive(Default)]
pub(super) struct Translations {
    kept: HashMap<Key, Vec<Kept>>,
}

/// The bytes of the code a translation is found by; the rest of the page is compared only with what was found.
const KEY_BYTES: usize = 64;

/// What a translation is found by: its offset in the page, which gives the length of the rest of the page, and that
/// rest's first [`KEY_BYTES`], zero past its end.
#[derive(Hash, PartialEq, Eq)]
struct Key {
    offset: u64,
    head: [u8; KEY_BYTES],
}

impl Key {
    fn new(offset: u64, code: &[u8]) -> Self {
        let mut head = [0; KEY_BYTES];
        let length = code.len().min(KEY_BYTES);
        head[..length].copy_from_slice(&code[..length]);
        Self { offset, head }
    }
}

/// A translation, or none where the code has none, the code it was made from (the rest of its page) and that page.
struct Kept {
    translation: Option<Arc<Translation>>,
    code: Vec<u8>,
    page: u64,
}

/// Whether `translation`, made at page `from`, moved to page `to` is what translating it there gives: both pages are
/// among those every translation moves between, or, on pages no direct branch from which wraps below zero or leaves
/// the slots' reach, every guest address it names is in that reach both before and after the move.
fn moves(translation: &Translation, from: u64, to: u64) -> bool {
    let movable = movable(WAY);
    if movable.contains(&from) && movable.contains(&to) {
        return true;
    }
    let unwrapped = branch_reach() + PAGE_SIZE..movable.end;
    if !unwrapped.contains(&from) || !unwrapped.contains(&to) {
        return false;
    }
    let by = to.wrapping_sub(from);
    let reach = 1u64 << WAY.reach_bits();
    let stays = |address: u64| address < reach && address.wrapping_add(by) < reach;
    translation.places.iter().all(|place| stays(place.address))
        && translation.exits.iter().all(|exit| stays(exit.target))
        && translation
            .tables
            .iter()
            .all(|table| stays(table.base) && (0..table.entries).all(|k| stays(table.target(k))))
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
        if self.unsent.is_empty() {
            return Vec::new();
        }
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

    /// The guest translation of the code at `pc`, made, kept, or kept and moved from another page.
    fn translate(&mut self, pc: u64, code: &[u8]) -> Option<Arc<Translation>> {
        let page = pc & !(PAGE_SIZE - 1);
        let key = Key::new(pc - page, code);
        if let Some(kept) = self.translations.kept.get(&key) {
            let mut same_code = kept.iter().filter(|kept| kept.code == code);
            if let Some(same) = same_code.clone().find(|kept| kept.page == page) {
                return same.translation.clone();
            }
            let movable = same_code.find_map(|kept| {
                let translation = kept.translation.as_ref()?;
                moves(translation, kept.page, page).then_some((translation, kept.page))
            });
            if let Some((translation, from)) = movable {
                let mut moved = Translation::clone(translation);
                moved.moved::<Arm64>(page.wrapping_sub(from));
                return Some(Arc::new(moved));
            }
        }
        let look_ahead = if self.single {
            LookAhead::Bounded(1)
        } else {
            LookAhead::Full
        };
        let made = translate_native_for::<Arm64>(code, pc, look_ahead, true, &WAY, &())
            .map(|made| Arc::new(HostCode::translation(made)));
        self.slow(reason::TRANSLATED, 1, pc);
        self.translations.kept.entry(key).or_default().push(Kept {
            translation: made.clone(),
            code: code.to_vec(),
            page,
        });
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
            .translate(pc, &source)
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
        self.protect_code_page(memory, page);
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
        // Every way out of a lone instruction, even back to itself, lands on an empty slot.
        if self.single {
            return Ok(());
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

    /// The word of loaded code at host address `pc`.
    fn host_word(&self, memory: MemoryId, pc: u64) -> Option<u32> {
        let code = self.codes[memory.0 as usize].as_ref()?;
        let (&base, guest) = code.hosts.range(..=pc).next_back()?;
        let from = usize::try_from(pc - base).ok()?;
        let bytes = code.pieces[guest]
            .code
            .bytes
            .get(from..from + size_of::<u32>())?;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
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
            .filter(|_| !self.single)
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

    /// Makes the guest page `page` one translated code was made from: its view loses write at the next turn.
    fn protect_code_page(&mut self, memory: MemoryId, page: u64) {
        if self.memory(memory).code_pages.contains(&page) {
            return;
        }
        let pages = page..page + PAGE_SIZE;
        self.memory_mut(memory).code_pages.insert(pages.clone());
        if let Some(&granted) = self.memory(memory).permissions.get(&page)
            && granted.contains(Permissions::WRITE)
        {
            let view = self.view_bound(memory, pages.clone(), granted);
            self.memory_mut(memory)
                .push(super::protect(pages, view), None);
        }
    }

    /// The guest writes a page translated code was made from: its pieces are dropped and the page is an ordinary one
    /// again, which the fault's settling then gives what the kernel allows.
    pub(super) fn code_page_written(&mut self, memory: MemoryId, page: u64) {
        self.forget_code(memory, page..page + PAGE_SIZE);
        self.memory_mut(memory)
            .code_pages
            .remove(page..page + PAGE_SIZE);
    }

    /// Drops the pieces made from `range`, which the kernel is about to change or stop letting run; the range's pages
    /// are ordinary ones again.
    pub(super) fn drop_code_of(&mut self, memory: MemoryId, range: std::ops::Range<u64>) {
        if !self.memory(memory).code_pages.overlaps(&range) {
            return;
        }
        let pages = range.start & !(PAGE_SIZE - 1)..range.end.next_multiple_of(PAGE_SIZE);
        if self.forget_code(memory, pages.clone()).1 {
            self.slow(CODE_CHANGED_AFTER_IT_RAN, 1, range.start);
        }
        self.memory_mut(memory).code_pages.remove(pages);
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

    /// What a fault in translated code at host `pc` was, from its syndrome where the frame gave one.
    fn taken(
        &mut self,
        memory: MemoryId,
        pc: u64,
        address: u64,
        signal: u32,
        code: u32,
        syndrome: u64,
    ) -> Taken {
        // Linux records no syndrome for a `brk` (a frame may hold an earlier fault's): its call is the word at pc.
        if signal as i32 == libc::SIGTRAP {
            return match self.host_word(memory, pc).map(wcontainers_arm64::decode) {
                Some(Instruction::Breakpoint { comment }) => Taken::Call(comment),
                _ => Taken::Other,
            };
        }
        /// The syndrome's exception classes (Arm ARM, ESR_EL1.EC).
        const CLASS_SHIFT: u32 = 26;
        const INSTRUCTION_ABORT: u64 = 0x20;
        const PC_ALIGNMENT: u64 = 0x22;
        const DATA_ABORT: u64 = 0x24;
        const SP_ALIGNMENT: u64 = 0x26;
        /// A data abort's write-not-read and cache-maintenance bits and its status code of an alignment fault (ISS.WnR,
        /// ISS.CM, ISS.DFSC); a cache maintenance fault sets WnR but reads.
        const WRITE_NOT_READ: u64 = 1 << 6;
        const CACHE_MAINTENANCE: u64 = 1 << 8;
        const STATUS: u64 = 0x3f;
        const ALIGNMENT_STATUS: u64 = 0x21;
        if syndrome != 0 {
            return match syndrome >> CLASS_SHIFT {
                DATA_ABORT => Taken::Access {
                    alignment: syndrome & STATUS == ALIGNMENT_STATUS,
                    fault: address,
                    write: syndrome & (WRITE_NOT_READ | CACHE_MAINTENANCE) == WRITE_NOT_READ,
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
        // Nothing changed since the stop: the thread goes on in the piece it was interrupted in, if that is still
        // loaded, so a stop never makes a translation from the pc it landed at.
        let interrupted = t.interrupted.take().filter(|at| {
            resume == Resume::Continue
                && self.codes[memory.0 as usize]
                    .as_ref()
                    .and_then(|code| code.pieces.get(&at.start))
                    .is_some_and(|loaded| {
                        loaded.base == at.base && Arc::ptr_eq(&loaded.code, &at.code)
                    })
        });
        let exit = self.run_pieces(
            thread,
            memory,
            index,
            &mut t,
            interrupted.map(|at| (at.hardware, at.page)),
            until,
            slice,
            clock,
        );
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
        mut carried: Option<(Interrupted, RegisterPage)>,
        until: Option<u64>,
        slice: Option<Duration>,
        clock: &mut dyn FnMut() -> u64,
    ) -> Exit {
        let reached = |state: &GuestState| until.is_some_and(|until| state.count >= until);
        let register_page = (REGISTER_PAGES + index * THREAD_PAGE_SIZE) as u64;
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
                if t.vectors_held {
                    stub.set_registers_but_vectors(index, &block);
                } else {
                    stub.set_registers(index, &block);
                    t.vectors_held = true;
                }
                stub.set_translated_page(index, &page);
                let end = match self.turn_with(thread, Then::Enter(hardware.pc as usize), slice) {
                    Ok(end) => end,
                    Err(_) => break Ended::Killed,
                };
                let stub = &self.memory(memory).stub;
                let block = stub.registers_but_vectors(index);
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
                        self.taken(memory, h.pc, address, signal, code, syndrome)
                    }
                    TurnEnd::Interrupted { at } => {
                        h.pc = at;
                        t.tpidr = block.tpidr_el0;
                        let state = self.resolve_interrupt(memory, &h, &held, &mut t.float);
                        match state {
                            Some(state) => {
                                t.interrupted =
                                    self.piece_holding(memory, h.pc).map(|(start, base, code)| {
                                        Where {
                                            hardware: h,
                                            page: held.clone(),
                                            start,
                                            base,
                                            code,
                                        }
                                    });
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
                        // The guest goes on after the announcing instruction in line, where the call's own last two
                        // words would take it: x30 back from the page, then the branch back.
                        let Some(in_line) = self.call_in_line(memory, h.pc) else {
                            break self.not_understood(memory, h.pc);
                        };
                        let mut then = then;
                        then.x[usize::from(LINK)] = then_page.link;
                        then.pc = in_line + INSTRUCTION;
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
                        let stop = self.store_faults_as_write(memory, stop, state.pc);
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
                // A lone instruction has run once anything lands: its piece is never linked.
                Stop::Landed(_) if self.single => return Exit::Stop,
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

    /// The loaded piece holding host address `pc`: its guest start, its base and its code.
    fn piece_holding(&self, memory: MemoryId, pc: u64) -> Option<(u64, u64, Arc<Translation>)> {
        let code = self.codes[memory.0 as usize].as_ref()?;
        let (&base, &start) = code.hosts.range(..=pc).next_back()?;
        let loaded = &code.pieces[&start];
        (pc - base < loaded.code.bytes.len() as u64)
            .then(|| (start, base, Arc::clone(&loaded.code)))
    }

    /// A stub reported what no code we loaded does: its process is ended.
    fn not_understood(&mut self, memory: MemoryId, pc: u64) -> Ended {
        self.slow(reason::EXIT_NOT_UNDERSTOOD, 1, pc);
        let _ = self.host.kill(&self.memory(memory).stub);
        Ended::Killed
    }

    /// A fault of a plain store, as Linux reports it: a write, also where the probe in front of an access that may
    /// cross a page took it as an atomic that reads first (the syndrome's WnR clear where a read would fault too).
    fn store_faults_as_write(&self, memory: MemoryId, stop: Stop, pc: u64) -> Stop {
        let Stop::MemoryFault {
            address,
            write: false,
        } = stop
        else {
            return stop;
        };
        let mut word = [0; INSTRUCTION as usize];
        if self.read_guest(memory, pc, &mut word).is_err() {
            return stop;
        }
        let write = matches!(
            wcontainers_arm64::decode(u32::from_le_bytes(word)),
            Instruction::Store { .. }
                | Instruction::StorePair { .. }
                | Instruction::StoreVector { .. }
                | Instruction::StoreVectorPair { .. }
        );
        Stop::MemoryFault { address, write }
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

#[cfg(test)]
mod tests {
    use wcontainers_abi::PAGE_SIZE;
    use wcontainers_arm64::Arm64;
    use wcontainers_arm64::carry::movable;
    use wcontainers_arm64::encode::branch_reach;
    use wcontainers_guest_cpu::{HostCode, Translation};
    use wcontainers_translator::{LookAhead, translate_native_for};

    use super::{INSTRUCTION, WAY, moves};

    /// The moving test's knobs: how many cases, and the seed.
    fn knob(name: &str, default: u64) -> u64 {
        std::env::var(name).map_or(default, |value| {
            value
                .trim()
                .parse()
                .unwrap_or_else(|_| panic!("{name} is a number"))
        })
    }

    /// A word of guest code: one of the forms that name an address (`adr`, `adrp`, `b`, `bl`, `b.cond`, a literal
    /// load, `blr`, `ret`) with random fields, or any word.
    fn word(rng: &mut fastrand::Rng) -> u32 {
        let forms = [
            (0x1000_0001u32, 0x60ff_ffffu32), // adr x1 and up
            (0x9000_0000, 0x60ff_ffff),       // adrp
            (0x1400_0000, 0x03ff_ffff),       // b
            (0x9400_0000, 0x03ff_ffff),       // bl
            (0x5400_0000, 0x00ff_ffef),       // b.cond
            (0x5800_0001, 0x00ff_ffff),       // ldr x1 and up, literal
            (0xd63f_0000, 0x0000_03e0),       // blr
            (0xd65f_03c0, 0),                 // ret
            (0xd100_0400, 0x0000_001f),       // sub x, x0, #1
            (0xb5ff_ffe0, 0x00ff_ffff),       // cbnz x
            (0, u32::MAX),                    // any word
        ];
        let (fixed, free) = forms[rng.usize(..forms.len())];
        fixed | rng.u32(..) & free
    }

    fn translated(bytes: &[u8], at: u64, look_ahead: LookAhead) -> Option<Translation> {
        translate_native_for::<Arm64>(bytes, at, look_ahead, true, &WAY, &())
            .map(HostCode::translation)
    }

    /// Wherever `moves` lets a translation move, the moved one is what translating there gives, on pages under the
    /// movable ones (where a position-independent program lies when the kernel places mappings low) as well as on them.
    #[test]
    fn a_translation_moves_only_where_moving_gives_what_translating_there_would() {
        let cases = knob("WCHP_MOVED_CASES", 2000);
        let seed = knob("WCHP_SEED", 1);
        let mut rng = fastrand::Rng::with_seed(seed);
        let movable = movable(WAY);
        let page = |rng: &mut fastrand::Rng| {
            let (low, high) = match rng.u8(..3) {
                0 => (PAGE_SIZE, branch_reach() + PAGE_SIZE),
                1 => (branch_reach() + PAGE_SIZE, movable.start),
                _ => (movable.start, movable.end),
            };
            low + rng.u64(..(high - low) / PAGE_SIZE) * PAGE_SIZE
        };
        let (mut moved_count, mut refused, mut places) = (0, 0, 0);
        for case in 0..cases {
            let offset = rng.u64(..PAGE_SIZE / INSTRUCTION) * INSTRUCTION;
            let words = rng.u64(1..=(PAGE_SIZE - offset) / INSTRUCTION);
            let bytes: Vec<u8> = (0..words)
                .flat_map(|_| word(&mut rng).to_le_bytes())
                .collect();
            let look_ahead = [LookAhead::Bounded(rng.u32(1..64)), LookAhead::Full][rng.usize(..2)];
            let (from, to) = (page(&mut rng), page(&mut rng));
            let Some(made) = translated(&bytes, from + offset, look_ahead) else {
                continue;
            };
            if !moves(&made, from, to) {
                refused += 1;
                continue;
            }
            moved_count += 1;
            places += made.places.len();
            let mut moved = made;
            moved.moved::<Arm64>(to.wrapping_sub(from));
            let there = translated(&bytes, to + offset, look_ahead);
            assert_eq!(
                Some(moved),
                there,
                "case {case} of seed {seed}: {from:#x} to {to:#x}, {look_ahead:?}"
            );
        }
        println!(
            "moving: {cases} cases of seed {seed}, {moved_count} moved with {places} address places, {refused} refused"
        );
        assert!(
            moved_count > cases / 4,
            "the check lets few move: {moved_count} of {cases}"
        );
    }
}
