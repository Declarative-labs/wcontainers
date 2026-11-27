//! Translated code in a stub (plan 3.2 to 3.4, ARM64): pieces the native output makes for the process way, loaded into
//! a memory's code area with their entry slots filled and their exits linked, and a thread run to its next exit, every
//! stop resolved to the guest's state through the stop crate.

use std::cell::OnceCell;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

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
use wcontainers_guest_cpu::position::{AddressPlace, NativeReason};
use wcontainers_guest_cpu::reason::CODE_CHANGED_AFTER_IT_RAN;
use wcontainers_guest_cpu::{HostCode, Native, Translation};
use wcontainers_translator::{LookAhead, translate_native_for};

use super::{FaultOwner, HostProcessExecutor, Then};
use crate::code::CodeArea;
use crate::layout::{
    CODE_AREA, CODE_AREA_SIZE, OUR_AREA, REGISTER_PAGES, SLOT_TABLE, SLOT_TABLE_SIZE,
    THREAD_PAGE_SIZE,
};
use crate::pages::Command;
use crate::probe::{CTR_EL0_IMINLINE_SHIFT, CTR_EL0_LINE_MASK};
use crate::stub::{StopWhen, TurnEnd};

/// The way every piece here is translated for.
const WAY: Way = Way::Process;

/// The bytes of one instruction.
const INSTRUCTION: u64 = size_of::<u32>() as u64;

/// The link register, which a call into the floor keeps in the register page.
const LINK: u8 = 30;

/// The top byte of an address of the lower half, which EL0's accesses ignore and a fault's address keeps.
pub const TAG: u64 = 0xFF << 56;

/// The slow paths of running translated code, as counted and warned.
pub mod reason {
    /// Guest code was translated, at its first run in a memory.
    pub const TRANSLATED: &str = "native code translated";
    /// The code area was full, so every piece of its memory was dropped.
    pub const CODE_AREA_EMPTIED: &str = "code area emptied";
    /// The host refused to give back a page of the slot table that a jump reached and no slot was written in.
    pub const SLOT_PAGE_KEPT: &str = "slot page kept";
    /// A turn limited to a count ended past it: linked pieces cannot stop a thread at an exact count.
    pub const TURN_LIMIT_PASSED: &str = "turn limit passed";
    /// A turn limited to a count ran one instruction at a time, so that it ends exactly at the count.
    pub const TURN_RUN_IN_STEPS: &str = "turn with a count limit run in steps";
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
    /// Its translation as kept, made `by` bytes of guest address below where it is loaded.
    translation: Arc<Translation>,
    by: u64,
    /// Its bytes as loaded, before any linking, where they differ from the kept translation's.
    moved_bytes: Option<Vec<u8>>,
    /// The kept translation moved here whole, made when a stop or a check first needs it.
    moved: OnceCell<Arc<Translation>>,
    /// The guest code it was made from, as far as it covers.
    source: Vec<u8>,
    /// Its exits: their targets and host addresses.
    exits: Vec<(u64, u64)>,
    /// Where its translation is kept.
    kept: KeptAt,
}

impl Loaded {
    /// Its code as translated for where it is loaded, before any linking.
    fn code(&self) -> &Arc<Translation> {
        if self.by == 0 {
            return &self.translation;
        }
        self.moved.get_or_init(|| {
            let mut moved = Translation::clone(&self.translation);
            moved.moved::<Arm64>(self.by);
            Arc::new(moved)
        })
    }

    /// Its host bytes, before any linking.
    fn bytes(&self) -> &[u8] {
        self.moved_bytes
            .as_deref()
            .unwrap_or(&self.translation.bytes)
    }
}

/// The host bytes of `translation` moved `by` bytes of guest address, as [`Translation::moved`] writes them.
fn moved_bytes(translation: &Translation, by: u64) -> Vec<u8> {
    let mut bytes = translation.bytes.clone();
    for place in &translation.places {
        let moved = AddressPlace {
            address: place.address.wrapping_add(by),
            ..*place
        };
        Arm64::write_place(&mut bytes, &moved);
    }
    bytes
}

/// One memory's translated code: its code area, its slot table and its pieces.
pub(super) struct Code {
    area: CodeArea,
    /// The next free byte of the code area.
    next: u64,
    /// The slot table's pages ever written, by their address in the stub; every other page reads as empty slots.
    slot_pages: HashSet<u64>,
    /// The pieces by guest address.
    pieces: BTreeMap<u64, Loaded>,
    /// The guest address of the piece at each host address.
    hosts: BTreeMap<u64, u64>,
    /// The sites linked, or to be linked, to each guest address, by that address and the site's host address.
    exits_to: BTreeMap<(u64, u64), Site>,
    /// The kept translations of the pieces loaded first and last, and the guest address of the last, after which the
    /// next is recorded.
    loaded_last: Option<(KeptAt, KeptAt, u64)>,
}

/// Decision 16's stand-in (plan section 12): a fork child runs its parent's translated code, see
/// [`HostProcessExecutor::share_code`].
const FORK_SHARES_CODE: bool = true;

/// Each memory's translated code, one shared by memories whose executable pages are the same.
#[derive(Default)]
pub(super) struct Codes {
    /// The index of each memory's code, by memory.
    of: Vec<Option<usize>>,
    /// Each code and the memories using it.
    codes: Vec<Option<(Code, Vec<MemoryId>)>>,
}

impl Codes {
    fn index(&self, memory: MemoryId) -> Option<usize> {
        self.of.get(memory.0 as usize).copied().flatten()
    }

    fn get(&self, memory: MemoryId) -> Option<&Code> {
        let (code, _) = self.codes[self.index(memory)?].as_ref()?;
        Some(code)
    }

    fn get_mut(&mut self, memory: MemoryId) -> Option<&mut Code> {
        let index = self.index(memory)?;
        let (code, _) = self.codes[index].as_mut()?;
        Some(code)
    }

    /// The memories using a memory's code, itself among them; none where it has none.
    fn users(&self, memory: MemoryId) -> &[MemoryId] {
        self.index(memory)
            .and_then(|index| self.codes[index].as_ref())
            .map_or(&[], |(_, users)| users)
    }

    fn set(&mut self, memory: MemoryId, index: Option<usize>) {
        let at = memory.0 as usize;
        if self.of.len() <= at {
            self.of.resize(at + 1, None);
        }
        self.of[at] = index;
    }

    fn make(&mut self, memory: MemoryId, code: Code) {
        let index = super::place(&mut self.codes, (code, vec![memory]));
        self.set(memory, Some(index));
    }

    fn share(&mut self, source: MemoryId, child: MemoryId) {
        let Some(index) = self.index(source) else {
            return;
        };
        if let Some((_, users)) = self.codes[index].as_mut() {
            users.push(child);
            self.set(child, Some(index));
        }
    }

    /// The memory stops using its code; the code, where no other memory uses it.
    fn release(&mut self, memory: MemoryId) -> Option<Code> {
        let index = self.index(memory)?;
        self.set(memory, None);
        let (_, users) = self.codes[index].as_mut()?;
        users.retain(|&user| user != memory);
        if users.is_empty() {
            self.codes[index].take().map(|(code, _)| code)
        } else {
            None
        }
    }
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
                let length = loaded.bytes().len();
                (pc - base < length as u64).then(|| Piece {
                    base,
                    length,
                    spans: &loaded.code().spans,
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
    /// The top byte the chip reported in the address of the guest's last memory fault, which [`Exit::MemoryFault`]
    /// leaves out.
    fault_tag: u64,
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
            fault_tag: 0,
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
    kept: Vec<Kept>,
    /// Where the translations of each key are kept.
    by_key: HashMap<Key, Vec<KeptAt>>,
    /// What a memory that loaded the first piece loaded after the second, the last time one did.
    after: HashMap<(KeptAt, KeptAt), After>,
}

/// What a memory did after loading a piece, all of it relative to the piece's guest address and its base.
#[derive(Default)]
struct After {
    /// How far from the piece the next piece it had to load was; none where it loaded no other.
    next: Option<u64>,
    /// The last targets it made for the piece's jumps: each jump's call into the floor and its target.
    targets: Vec<(u64, u64)>,
}

/// The bytes of the code a translation is found by; the rest of the page is compared only with what was found.
const KEY_BYTES: usize = 64;

/// What a translation is found by: its offset in the page, which gives the length of the rest of the page, and that
/// rest's first [`KEY_BYTES`], zero past its end.
#[derive(Clone, Hash, PartialEq, Eq)]
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
    /// The bytes of code its run covers from its start, wherever it is moved.
    covered: u64,
    /// The lowest and highest guest address it names, where it is kept.
    named: Option<(u64, u64)>,
}

/// A kept translation and how many bytes of guest address it moves where it is loaded; none where the code has none.
type Found = Option<(Arc<Translation>, u64)>;

/// Where a kept translation is among all those kept.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct KeptAt(usize);

/// The bytes of guest code from `pc` that a translation made at `pc` covers: code placed out of line comes after the
/// run's last instruction, so the run ends at its highest guest address.
fn covered(translation: &Translation, pc: u64) -> u64 {
    translation
        .spans
        .iter()
        .map(|span| span.guest.wrapping_add(INSTRUCTION).wrapping_sub(pc))
        .max()
        .unwrap_or(0)
}

/// The most pieces loaded ahead after one a thread had to stop for, each the next one an earlier memory stopped for.
const AHEAD: usize = 4096;

/// The lowest and highest guest address a translation names (its address places, its exits' targets, its tables' bases
/// and targets); none where it names none.
fn named(translation: &Translation) -> Option<(u64, u64)> {
    let tables = translation.tables.iter().flat_map(|table| {
        core::iter::once(table.base).chain((0..table.entries).map(|k| table.target(k)))
    });
    translation
        .places
        .iter()
        .map(|place| place.address)
        .chain(translation.exits.iter().map(|exit| exit.target))
        .chain(tables)
        .fold(None, |named, address| match named {
            None => Some((address, address)),
            Some((low, high)) => Some((low.min(address), high.max(address))),
        })
}

/// Whether a translation that names the guest addresses `named` gives, made at page `from` and moved to page `to`, what
/// translating it there gives: both pages are among those every translation moves between, or, on pages no direct
/// branch from which wraps below zero or leaves the slots' reach, every guest address it names is in that reach both
/// before and after the move.
fn moves(named: Option<(u64, u64)>, from: u64, to: u64) -> bool {
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
    // Every address between the two stays where both do and the move does not wrap between them.
    named.is_none_or(|(low, high)| {
        stays(low) && stays(high) && low.wrapping_add(by) <= high.wrapping_add(by)
    })
}

/// How a guest's reads of its counter are answered.
pub enum CounterReads<'clock> {
    /// From this clock, the guest going on after the read.
    Answered(&'clock mut dyn FnMut() -> u64),
    /// By the system: the run stops at the read, which has not run.
    Trapped,
}

/// How a run of translated code ended for its caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ran {
    Exit(Exit),
    /// A read of the counter, stopped at its instruction for the system to answer.
    CounterRead,
}

/// How a run of translated code came back.
enum Ended {
    /// The turn is over: a stop, or its limit reached.
    TurnOver,
    /// The guest announced that the code at this address changed.
    CodeChanged(u64),
    /// A counter read, stopped at it.
    CounterRead,
    /// A jump's first miss: its last target is to be made, the code going on where it was carried.
    LastTarget {
        call: u64,
        target: u64,
    },
    Stopped(Stop),
    /// The process can only be killed.
    Killed,
}

/// The word that links `site` to the piece at `to`, when it reaches.
fn link_word(site: &Site, to: u64) -> Option<u32> {
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

    /// The memory's code, made with its code area and slot table mapped at the next turn on first use.
    fn code(&mut self, memory: MemoryId) -> Result<&mut Code, CpuError> {
        if self.codes.get(memory).is_none() {
            let area = CodeArea::new().map_err(super::host_error)?;
            let stub = self.memory_mut(memory);
            stub.push_code(Command::MapCode, area.shared_file());
            stub.push_code(Command::MapSlots, area.shared_file());
            self.codes.make(
                memory,
                Code {
                    area,
                    next: CODE_AREA as u64,
                    slot_pages: HashSet::new(),
                    pieces: BTreeMap::new(),
                    hosts: BTreeMap::new(),
                    exits_to: BTreeMap::new(),
                    loaded_last: None,
                },
            );
        }
        Ok(self.codes.get_mut(memory).expect("made above"))
    }

    /// A fork child runs its parent's code area, slot table and pieces, where the parent has code and no page both
    /// writable and executable; until either changes its executable pages, theirs are the same.
    pub(super) fn share_code(&mut self, source: MemoryId, child: MemoryId) {
        if !FORK_SHARES_CODE
            || self
                .memory(source)
                .permissions
                .iter()
                .any(|(_, granted)| granted.contains(Permissions::WRITE | Permissions::EXECUTE))
        {
            return;
        }
        let Some(code) = self.codes.get(source) else {
            return;
        };
        let file = code.area.shared_file();
        self.codes.share(source, child);
        let pages = self.memory(source).code_pages.clone();
        let stub = self.memory_mut(child);
        stub.code_pages = pages;
        stub.push_code(Command::MapCode, Arc::clone(&file));
        stub.push_code(Command::MapSlots, file);
    }

    /// A memory about to change its executable pages stops sharing its code: it makes its own at its next load.
    pub(super) fn unshare_code(&mut self, memory: MemoryId) {
        if self.codes.users(memory).len() > 1 {
            let _ = self.codes.release(memory);
            self.counters.codes_unshared += 1;
        }
    }

    /// Drops a memory's code with the memory, where no other memory uses it; the piece it loaded last had none loaded
    /// after it.
    pub(super) fn drop_code(&mut self, memory: MemoryId) {
        if let Some(code) = self.codes.release(memory)
            && let Some((first, last, _)) = code.loaded_last
            && let Some(after) = self.translations.after.get_mut(&(first, last))
        {
            after.next = None;
        }
    }

    /// The guest translation of the code at `pc`, or none where the code has none, kept, kept at another page it moves
    /// from, or, where `make` is set, made; with how far it moves (zero where it was made for this page), and where it
    /// is kept.
    fn translate(&mut self, pc: u64, code: &[u8], make: bool) -> Option<(Found, KeptAt)> {
        let page = pc & !(PAGE_SIZE - 1);
        let key = Key::new(pc - page, code);
        if let Some(ats) = self.translations.by_key.get(&key) {
            let mut movable = None;
            for &at in ats {
                let kept = &self.translations.kept[at.0];
                if kept.code != code {
                    continue;
                }
                if kept.page == page {
                    let same = kept.translation.clone().map(|translation| (translation, 0));
                    return Some((same, at));
                }
                if movable.is_none()
                    && let Some(translation) = &kept.translation
                    && moves(kept.named, kept.page, page)
                {
                    movable = Some((Arc::clone(translation), page.wrapping_sub(kept.page), at));
                }
            }
            if let Some((translation, by, at)) = movable {
                return Some((Some((translation, by)), at));
            }
        }
        if !make {
            return None;
        }
        let look_ahead = if self.lone {
            LookAhead::Bounded(1)
        } else {
            LookAhead::Full
        };
        let made = translate_native_for::<Arm64>(code, pc, look_ahead, true, &WAY, &())
            .map(|made| Arc::new(HostCode::translation(made)));
        self.slow(reason::TRANSLATED, 1, pc);
        let at = KeptAt(self.translations.kept.len());
        self.translations.kept.push(Kept {
            covered: made.as_deref().map_or(0, |made| covered(made, pc)),
            named: made.as_deref().and_then(named),
            translation: made.clone(),
            code: code.to_vec(),
            page,
        });
        self.translations.by_key.entry(key).or_default().push(at);
        Some((made.map(|made| (made, 0)), at))
    }

    /// Makes sure a piece starting at guest `pc` is loaded in `memory`, or gives the exit that running from there is.
    /// A piece it loads is recorded as the next one after the piece loaded last, and the pieces an earlier memory that
    /// loaded the same piece first loaded next after it are loaded too, as long as each is kept and its code the same.
    fn load(&mut self, memory: MemoryId, pc: u64) -> Result<(), Exit> {
        if self
            .code(memory)
            .map_err(|_| Exit::MemoryFaultWithoutRegisters)?
            .pieces
            .contains_key(&pc)
        {
            return Ok(());
        }
        let mut read = HashMap::new();
        let Some(mut at) = self.load_piece(memory, pc, true, &mut read)? else {
            unreachable!("a piece is made where none is kept")
        };
        if self.lone {
            return Ok(());
        }
        let code = self.codes.get_mut(memory).expect("made above");
        let first = match code.loaded_last.take() {
            Some((first, last, from)) => {
                self.translations
                    .after
                    .entry((first, last))
                    .or_default()
                    .next = Some(pc.wrapping_sub(from));
                first
            }
            None => at,
        };
        let mut at_pc = pc;
        for _ in 0..AHEAD {
            let Some(after) = self.translations.after.get(&(first, at)) else {
                break;
            };
            let (next, targets) = (after.next, after.targets.clone());
            if !targets.is_empty() {
                let base = self.codes.get(memory).expect("made above").pieces[&at_pc].base;
                for (call, target) in targets {
                    self.make_last_target(memory, base + call, at_pc.wrapping_add(target));
                    self.counters.last_targets_made_ahead += 1;
                }
            }
            let Some(next) = next else {
                break;
            };
            let next = at_pc.wrapping_add(next);
            let code = self.codes.get(memory).expect("made above");
            if code.pieces.contains_key(&next) {
                break;
            }
            match self.load_piece(memory, next, false, &mut read) {
                Ok(Some(found)) => {
                    self.counters.pieces_loaded_ahead += 1;
                    (at, at_pc) = (found, next);
                }
                _ => break,
            }
        }
        self.codes.get_mut(memory).expect("made above").loaded_last = Some((first, at, at_pc));
        Ok(())
    }

    /// Loads a piece at guest `pc` and gives where its translation is kept; where `make` is clear, only one kept, and
    /// none where none is kept, the code has none or the code area is full. `read` holds the guest pages read so far
    /// by one load, by address, so each is read once.
    fn load_piece(
        &mut self,
        memory: MemoryId,
        pc: u64,
        make: bool,
        read: &mut HashMap<u64, Vec<u8>>,
    ) -> Result<Option<KeptAt>, Exit> {
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
        let guest_page = match read.entry(page) {
            Entry::Occupied(bytes) => bytes.into_mut(),
            Entry::Vacant(entry) => {
                let mut bytes = vec![0; PAGE_SIZE as usize];
                self.read_guest(memory, page, &mut bytes)
                    .map_err(|_| refused)?;
                entry.insert(bytes)
            }
        };
        let rest = &guest_page[(pc - page) as usize..];
        let Some((piece, at)) = self.translate(pc, rest, make) else {
            return Ok(None);
        };
        let Some((piece, by)) = piece else {
            return if make {
                Err(Exit::IllegalInstruction)
            } else {
                Ok(None)
            };
        };
        let moved = (by != 0).then(|| moved_bytes(&piece, by));
        let bytes = moved.as_deref().unwrap_or(&piece.bytes);
        let base = match self.place(memory, bytes) {
            Some(base) => base,
            None if !make => return Ok(None),
            None => {
                self.empty_code_area(memory);
                self.place(memory, bytes)
                    .expect("one piece fits an empty code area")
            }
        };
        for reason in NativeReason::ALL {
            let sites = piece.sites[reason.index()];
            if sites != 0 {
                self.slow(reason.name(), sites, pc);
            }
        }
        let covered = self.translations.kept[at.0].covered;
        let source = rest[..rest.len().min(covered as usize)].to_vec();
        let site = |start: u32, retarget: Option<u32>| {
            let from = start as usize;
            let word = &bytes[from..from + size_of::<u32>()];
            Site {
                at: base + u64::from(start),
                word: u32::from_le_bytes(word.try_into().expect("a word")),
                retarget,
            }
        };
        let mut sites: Vec<(u64, Site)> = piece
            .exits
            .iter()
            .map(|exit| {
                (
                    exit.target.wrapping_add(by),
                    site(exit.start, exit.retarget),
                )
            })
            .collect();
        // Each branch of a known jump's table is an exit to its target; one no piece can be at stays on the slot.
        for table in &piece.tables {
            for k in 0..table.entries {
                let target = table.target(k).wrapping_add(by);
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
                translation: Arc::clone(&piece),
                by,
                moved_bytes: moved,
                moved: OnceCell::new(),
                source,
                exits: sites
                    .iter()
                    .map(|(target, site)| (*target, site.at))
                    .collect(),
                kept: at,
            },
        );
        code.hosts.insert(base, pc);
        for &(target, site) in &sites {
            code.exits_to.insert((target, site.at), site);
        }
        // Every way out of a lone instruction, even back to itself, lands on an empty slot.
        if self.lone {
            return Ok(Some(at));
        }
        self.fill_slot(memory, pc, base);
        let code = self.codes.get_mut(memory).expect("made above");
        let into = code
            .exits_to
            .range((pc, 0)..=(pc, u64::MAX))
            .map(|(_, site)| (*site, base));
        let out = sites
            .iter()
            .filter(|(target, _)| *target != pc)
            .filter_map(|(target, site)| Some((*site, code.pieces.get(target)?.base)));
        for (site, to) in into.chain(out) {
            if let Some(word) = link_word(&site, to) {
                code.area.stage(site.at as usize, &[word]);
            }
        }
        Ok(Some(at))
    }

    /// Stages host words for a memory's code area, written before its next turn.
    fn write_code(&mut self, memory: MemoryId, at: u64, words: &[u32]) {
        let code = self.codes.get_mut(memory).expect("a memory with code");
        code.area.stage(at as usize, words);
    }

    /// Writes a memory's staged code, before one of its threads takes a turn.
    pub(super) fn flush_code(&mut self, memory: MemoryId) {
        let probe = *self.host.probe();
        if let Some(code) = self.codes.get_mut(memory) {
            let (given, kept) = code.area.flush(&probe);
            self.counters.slot_pages_given_back += given as u64;
            if kept > 0 {
                self.slow(reason::SLOT_PAGE_KEPT, kept as u64, memory.0.into());
            }
        }
    }

    /// Writes host code into the next free bytes of the code area, giving its address; none when it does not fit.
    fn place(&mut self, memory: MemoryId, bytes: &[u8]) -> Option<u64> {
        let code = self.codes.get_mut(memory)?;
        let start = code.next;
        let end = start + bytes.len() as u64;
        if end > (CODE_AREA + CODE_AREA_SIZE) as u64 {
            return None;
        }
        code.next = end;
        code.area.stage_code(start as usize, bytes);
        Some(start)
    }

    /// Drops every piece of a memory and starts its code area again; its slot pages stay its own, every slot empty.
    fn empty_code_area(&mut self, memory: MemoryId) {
        let empty = words().empty;
        let code = self.codes.get_mut(memory).expect("a memory with code");
        let dropped = code.pieces.len() as u64;
        code.pieces.clear();
        code.hosts.clear();
        code.exits_to.clear();
        code.next = CODE_AREA as u64;
        let words = vec![empty; THREAD_PAGE_SIZE / size_of::<u32>()];
        for &page in &code.slot_pages {
            code.area.stage_slots(page as usize, &words);
        }
        self.slow(reason::CODE_AREA_EMPTIED, 1, dropped);
    }

    /// Fills the entry slot of guest `guest` with a jump to the piece at `base`.
    fn fill_slot(&mut self, memory: MemoryId, guest: u64, base: u64) {
        let words_of = words();
        let at = WAY.slot(guest);
        let code = self.codes.get_mut(memory).expect("a memory with code");
        code.slot_pages.insert(at & !(THREAD_PAGE_SIZE as u64 - 1));
        let mut words = [words_of.jump; SLOT_MOVES + 1];
        for (k, (word, &field)) in words.iter_mut().zip(&words_of.moves).enumerate() {
            *word = Field::new(field).with(base >> (k as u32 * SLOT_MOVE_BITS));
        }
        code.area.stage_slots(at as usize, &words);
    }

    /// The host address of the word in line that leads to the out-of-line call whose `blr` is at `call`: the word
    /// before the one its closing `b` goes back to (`str x30; mov x30, entry; blr x30; ldr x30; b back`, plan 3.2).
    fn call_in_line(&self, memory: MemoryId, call: u64) -> Option<u64> {
        let code = self.codes.get(memory)?;
        let (&base, guest) = code.hosts.range(..=call).next_back()?;
        let bytes = code.pieces[guest].bytes();
        let back_at = call + 2 * INSTRUCTION;
        let from = usize::try_from(back_at - base).ok()?;
        let word = u32::from_le_bytes(bytes.get(from..from + size_of::<u32>())?.try_into().ok()?);
        let back = back_at
            .wrapping_add_signed(Field::new(words().branch).value(word) * INSTRUCTION as i64);
        Some(back - INSTRUCTION)
    }

    /// The word of loaded code at host address `pc`.
    fn host_word(&self, memory: MemoryId, pc: u64) -> Option<u32> {
        let code = self.codes.get(memory)?;
        let (&base, guest) = code.hosts.range(..=pc).next_back()?;
        let from = usize::try_from(pc - base).ok()?;
        let bytes = code.pieces[guest]
            .bytes()
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
        let code = self.codes.get(memory).expect("a memory with code");
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
            .filter(|_| !self.lone)
            .and_then(|piece| link_word(&site, piece.base));
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
        let code = self.codes.get_mut(memory).expect("a memory with code");
        if let Some(piece) = code.pieces.get_mut(&holder) {
            piece.exits.push((target, site.at));
        }
        code.exits_to.insert((target, site.at), site);
    }

    /// Records a last target made in `memory` for the memory loading the same pieces next.
    fn record_last_target(&mut self, memory: MemoryId, call: u64, target: u64) {
        let Some(code) = self.codes.get(memory) else {
            return;
        };
        let (Some((first, _, _)), Some((&base, &start))) =
            (&code.loaded_last, code.hosts.range(..=call).next_back())
        else {
            return;
        };
        let after = self
            .translations
            .after
            .entry((*first, code.pieces[&start].kept))
            .or_default();
        let (call, target) = (call - base, target.wrapping_sub(start));
        match after.targets.iter_mut().find(|(at, _)| *at == call) {
            Some(made) => made.1 = target,
            None => after.targets.push((call, target)),
        }
    }

    /// Drops the pieces of `memory` made from guest code overlapping `range`; whether one was dropped, and whether the
    /// code of one is no longer what it was made from.
    fn forget_code(&mut self, memory: MemoryId, range: std::ops::Range<u64>) -> (bool, bool) {
        let Some(code) = self.codes.get(memory) else {
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

    /// Makes the guest page `page` one translated code was made from, in every memory using the code: its view loses
    /// write at the next turn, unless the guest wrote it while it held code before.
    fn protect_code_page(&mut self, memory: MemoryId, page: u64) {
        let settled = |stub: &super::Memory| {
            stub.code_pages.contains(&page) || stub.written_code_pages.contains(&page)
        };
        if self
            .codes
            .users(memory)
            .iter()
            .all(|&user| settled(self.memory(user)))
        {
            return;
        }
        for memory in self.codes.users(memory).to_vec() {
            if settled(self.memory(memory)) {
                continue;
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
    }

    /// The guest writes a page translated code was made from: its pieces are dropped and the page is left writable,
    /// which the fault's settling then gives what the kernel allows. Protecting it again at its next piece would fault
    /// a store into the page its own code is on for ever.
    pub(super) fn code_page_written(&mut self, memory: MemoryId, page: u64) {
        self.unshare_code(memory);
        self.forget_code(memory, page..page + PAGE_SIZE);
        let stub = self.memory_mut(memory);
        stub.code_pages.remove(page..page + PAGE_SIZE);
        stub.written_code_pages.insert(page..page + PAGE_SIZE);
        self.counters.code_pages_left_writable += 1;
    }

    /// Drops the pieces made from `range`, which the kernel is about to change or stop letting run; the range's pages
    /// are ordinary ones again.
    pub(super) fn drop_code_of(&mut self, memory: MemoryId, range: std::ops::Range<u64>) {
        if self
            .memory(memory)
            .permissions
            .overlapping(range.clone())
            .any(|(_, granted)| granted.contains(Permissions::EXECUTE))
        {
            self.unshare_code(memory);
        }
        let stub = self.memory(memory);
        if !stub.code_pages.overlaps(&range) && !stub.written_code_pages.overlaps(&range) {
            return;
        }
        let pages = range.start & !(PAGE_SIZE - 1)..range.end.next_multiple_of(PAGE_SIZE);
        if self.forget_code(memory, pages.clone()).1 {
            self.slow(CODE_CHANGED_AFTER_IT_RAN, 1, range.start);
        }
        let stub = self.memory_mut(memory);
        stub.code_pages.remove(pages.clone());
        stub.written_code_pages.remove(pages);
    }

    /// Drops the pieces of a range the kernel changed, counting "code changed after it ran" where one's code changed.
    pub(super) fn forget_changed_code(&mut self, memory: MemoryId, address: u64, length: u64) {
        self.unshare_code(memory);
        if self
            .forget_code(memory, address..address.saturating_add(length))
            .1
        {
            self.slow(CODE_CHANGED_AFTER_IT_RAN, 1, address);
        }
    }

    /// Drops the piece at guest `at`: its slot emptied, the exits linked to it unlinked, its own exits forgotten.
    fn drop_piece(&mut self, memory: MemoryId, at: u64, empty: u32) {
        let code = self.codes.get_mut(memory).expect("a memory with code");
        let Some(loaded) = code.pieces.remove(&at) else {
            return;
        };
        code.hosts.remove(&loaded.base);
        let slot = WAY.slot(at);
        if code
            .slot_pages
            .contains(&(slot & !(THREAD_PAGE_SIZE as u64 - 1)))
        {
            code.area.stage_slots(slot as usize, &[empty]);
        }
        for (_, site) in code.exits_to.range((at, 0)..=(at, u64::MAX)) {
            code.area.stage(site.at as usize, &[site.word]);
        }
        for exit in loaded.exits {
            code.exits_to.remove(&exit);
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

    /// Runs `thread` from `resume` to its next exit, or to [`Exit::Stop`] from the first exit at or after the count
    /// `until` (with lone pieces, the first instruction's end at or after it); `when` says when translated code is
    /// stopped, its slice counted from the first entry. `clock` answers the guest's counter reads.
    pub fn run_translated(
        &mut self,
        thread: ThreadId,
        resume: Resume,
        until: Option<u64>,
        when: StopWhen<'_>,
        clock: &mut dyn FnMut() -> u64,
    ) -> Exit {
        match self.run_translated_with(
            thread,
            resume,
            until,
            when,
            &mut CounterReads::Answered(clock),
        ) {
            Ran::Exit(exit) => exit,
            Ran::CounterRead => unreachable!("a counter read answered never stops the run"),
        }
    }

    /// The top byte the chip reported in the address of the thread's last memory fault, which [`Exit::MemoryFault`]
    /// leaves out, as the guest's pages are found without it.
    pub fn fault_tag(&self, thread: ThreadId) -> u64 {
        self.guests[thread.0 as usize]
            .as_ref()
            .expect("a thread the kernel holds")
            .fault_tag
    }

    /// [`Self::run_translated`] with every counter read stopped at, for the system to answer.
    pub fn run_translated_trapping_counter(
        &mut self,
        thread: ThreadId,
        resume: Resume,
        until: Option<u64>,
        when: StopWhen<'_>,
    ) -> Ran {
        self.run_translated_with(thread, resume, until, when, &mut CounterReads::Trapped)
    }

    fn run_translated_with(
        &mut self,
        thread: ThreadId,
        resume: Resume,
        until: Option<u64>,
        when: StopWhen<'_>,
        counter: &mut CounterReads<'_>,
    ) -> Ran {
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
                && self
                    .codes
                    .get(memory)
                    .and_then(|code| code.pieces.get(&at.start))
                    .is_some_and(|loaded| {
                        loaded.base == at.base && Arc::ptr_eq(loaded.code(), &at.code)
                    })
        });
        let ran = self.run_pieces(
            thread,
            memory,
            index,
            &mut t,
            interrupted.map(|at| (at.hardware, at.page)),
            until,
            when,
            counter,
        );
        self.positions.0 += 1;
        if ran != Ran::Exit(Exit::MemoryFaultWithoutRegisters) {
            use core::hash::Hasher;
            self.positions.1.write_u64(t.state.count);
            self.positions.1.write_u64(t.state.pc);
        }
        self.guests[thread.0 as usize] = Some(t);
        ran
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
        when: StopWhen<'_>,
        counter: &mut CounterReads<'_>,
    ) -> Ran {
        let reached = |state: &GuestState| until.is_some_and(|until| state.count >= until);
        let register_page = (REGISTER_PAGES + index * THREAD_PAGE_SIZE) as u64;
        // The slice counts from the first entry into translated code: translating is not the guest's time.
        let mut slice_end: Option<Instant> = None;
        loop {
            let resumed = carried.take();
            // The slice ended in an earlier turn: the thread stops where it landed.
            if resumed.is_none() && slice_end.is_some_and(|end| Instant::now() >= end) {
                return Ran::Exit(Exit::Stop);
            }
            if resumed.is_none()
                && let Err(exit) = self.load(memory, t.state.pc)
            {
                return Ran::Exit(exit);
            }
            let (mut hardware, mut page) = resumed.unwrap_or_else(|| {
                let (mut hardware, page) =
                    stop::rejoin(&t.state, t.page.clone(), &t.float, register_page);
                hardware.pc =
                    self.codes.get(memory).expect("loaded above").pieces[&t.state.pc].base;
                (hardware, page)
            });
            // The top byte of the address of the turn's fault, which the chip reports and ignores (TCR_EL1.TBI0).
            let mut fault_tag: u64;
            let ended = loop {
                fault_tag = 0;
                let stub = &self.memory(memory).stub;
                if t.vectors_held {
                    stub.set_scalars(index, &hardware, t.tpidr);
                } else {
                    stub.set_registers(
                        index,
                        &RegisterBlock {
                            x: hardware.x,
                            sp: hardware.sp,
                            pc: hardware.pc,
                            pstate: hardware.state,
                            v: t.vectors,
                            fpsr: hardware.fpsr,
                            fpcr: hardware.fpcr,
                            tpidr_el0: t.tpidr,
                        },
                    );
                    t.vectors_held = true;
                }
                stub.set_translated_page(index, &page);
                let turn_when = StopWhen {
                    slice: when.slice.map(|slice| {
                        slice_end
                            .get_or_insert_with(|| Instant::now() + slice)
                            .saturating_duration_since(Instant::now())
                    }),
                    wanted: when.wanted,
                };
                let end = match self.turn_with(thread, Then::Enter(hardware.pc as usize), turn_when)
                {
                    // A stop before the code ran: the thread is where it was to enter.
                    Ok(TurnEnd::Stopped { .. }) => TurnEnd::Interrupted { at: hardware.pc },
                    Ok(end) => end,
                    Err(_) => break Ended::Killed,
                };
                let stub = &self.memory(memory).stub;
                let (mut h, tpidr) = stub.scalars(index);
                let held = stub.translated_page(index);
                let taken = match end {
                    TurnEnd::Called { entry } => {
                        h.pc = h.pc.wrapping_sub(INSTRUCTION);
                        Taken::Call(entry as u16)
                    }
                    TurnEnd::Fault {
                        address,
                        signal,
                        code,
                    } => {
                        fault_tag = address & TAG;
                        let syndrome = self.memory(memory).stub.fault_syndrome(index);
                        self.taken(memory, h.pc, address & !TAG, signal, code, syndrome)
                    }
                    TurnEnd::Interrupted { at } => {
                        h.pc = at;
                        t.tpidr = tpidr;
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
                    _ => break self.not_understood(memory, h.pc),
                };
                t.tpidr = tpidr;
                let resolved = {
                    let code = self.codes.get(memory).expect("loaded above");
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
                        at,
                        then,
                        page: then_page,
                    }) => match counter {
                        CounterReads::Answered(clock) => {
                            hardware = then;
                            page = then_page;
                            if let Some(register) = into {
                                stop::set_register(register, clock(), &mut hardware, &mut page);
                            }
                        }
                        CounterReads::Trapped => {
                            t.state = *at;
                            t.page = held;
                            break Ended::CounterRead;
                        }
                    },
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
                        if let Stop::Landed(_) = stop
                            && let Some(code) = self.codes.get_mut(memory)
                        {
                            code.area.reached_slot(h.pc as usize);
                        }
                        let stop = self.store_faults_as_write(memory, stop, state.pc);
                        t.state = state;
                        t.page = held;
                        break Ended::Stopped(stop);
                    }
                    None => break self.not_understood(memory, h.pc),
                }
            };
            let stop = match ended {
                Ended::TurnOver => return Ran::Exit(Exit::Stop),
                Ended::Killed => return Ran::Exit(Exit::MemoryFaultWithoutRegisters),
                Ended::CounterRead => return Ran::CounterRead,
                Ended::CodeChanged(address) => {
                    self.code_line_changed(memory, address);
                    if reached(&t.state) {
                        return Ran::Exit(Exit::Stop);
                    }
                    continue;
                }
                Ended::LastTarget { call, target } => {
                    self.make_last_target(memory, call, target);
                    self.record_last_target(memory, call, target);
                    continue;
                }
                Ended::Stopped(stop) => stop,
            };
            // A jump to a misaligned target has ended at it: the fault is the target's, which the next turn's load
            // gives, as the hardware takes it at the target's fetch.
            let ended = match stop {
                Stop::Landed(_) => true,
                Stop::Alignment(address) => address == t.state.pc,
                _ => false,
            };
            match stop {
                // Lone instructions land at every instruction's end, so a limit is reached exactly there.
                _ if ended && reached(&t.state) => {
                    if !self.lone {
                        self.slow(
                            reason::TURN_LIMIT_PASSED,
                            1,
                            t.state.count - until.unwrap_or(0),
                        );
                    }
                    return Ran::Exit(Exit::Stop);
                }
                Stop::Landed(_) => {}
                Stop::Syscall => {
                    let mut call = SystemCallRequest {
                        number: SystemCallNumber(t.state.x[SYSCALL_NUMBER]),
                        arguments: Default::default(),
                    };
                    let arguments = call.arguments.len();
                    call.arguments.copy_from_slice(&t.state.x[..arguments]);
                    return Ran::Exit(Exit::Syscall(call));
                }
                Stop::Breakpoint(comment) => return Ran::Exit(Exit::Breakpoint { comment }),
                Stop::Illegal => return Ran::Exit(Exit::IllegalInstruction),
                Stop::Alignment(address) => return Ran::Exit(Exit::AlignmentFault { address }),
                Stop::MemoryFault { address, write } => {
                    let access = if write { Access::Write } else { Access::Read };
                    match self.resolve_fault(memory, address, access) {
                        // The same instruction runs again, over the page as the kernel allows it.
                        Ok(FaultOwner::Executor) => {}
                        Ok(FaultOwner::Guest) => {
                            self.slow(wcontainers_guest_cpu::reason::MEMORY_FAULT, 1, address);
                            t.fault_tag = fault_tag;
                            return Ran::Exit(Exit::MemoryFault { address, access });
                        }
                        Err(_) => return Ran::Exit(Exit::MemoryFaultWithoutRegisters),
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
        let code = self.codes.get(memory)?;
        let mut memory_of = |at: u64, into: &mut [u8]| {
            let _ = self.read_guest(memory, at, into);
        };
        let piece = code.piece_at(h.pc)?;
        stop::interrupted(WAY, &piece, h, page, float, &mut memory_of)
    }

    /// The loaded piece holding host address `pc`: its guest start, its base and its code.
    fn piece_holding(&self, memory: MemoryId, pc: u64) -> Option<(u64, u64, Arc<Translation>)> {
        let code = self.codes.get(memory)?;
        let (&base, &start) = code.hosts.range(..=pc).next_back()?;
        let loaded = &code.pieces[&start];
        (pc - base < loaded.bytes().len() as u64).then(|| (start, base, Arc::clone(loaded.code())))
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

    use super::{INSTRUCTION, WAY, covered, moved_bytes, moves, named};

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

    /// [`moves`] asked of every address the translation names, one at a time.
    fn moves_by_each(translation: &Translation, from: u64, to: u64) -> bool {
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
            && translation.tables.iter().all(|table| {
                stays(table.base) && (0..table.entries).all(|k| stays(table.target(k)))
            })
    }

    /// Wherever `moves` lets a translation move, the moved one is what translating there gives, on pages under the
    /// movable ones (where a position-independent program lies when the kernel places mappings low) as well as on them;
    /// and asking it of the lowest and highest address named answers as asking it of each.
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
            let by_each = moves_by_each(&made, from, to);
            assert_eq!(
                moves(named(&made), from, to),
                by_each,
                "case {case} of seed {seed}: moving by the lowest and highest address named, {from:#x} to {to:#x}"
            );
            if !by_each {
                refused += 1;
                continue;
            }
            moved_count += 1;
            places += made.places.len();
            let loaded_bytes = moved_bytes(&made, to.wrapping_sub(from));
            let covered_made = covered(&made, from + offset);
            let mut moved = made;
            moved.moved::<Arm64>(to.wrapping_sub(from));
            assert_eq!(
                loaded_bytes, moved.bytes,
                "case {case} of seed {seed}: the bytes a load writes, {from:#x} to {to:#x}"
            );
            assert_eq!(
                covered(&moved, to + offset),
                covered_made,
                "case {case} of seed {seed}: the code covered, {from:#x} to {to:#x}"
            );
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
