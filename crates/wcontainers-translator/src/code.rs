//! Code modules: the pieces of one translation, and the emitter for look-ahead none.

use crate::contract::*;
use crate::layout::Layout;
use crate::state::{Slot, load_flag, store_flags};
use crate::thread::{memarg, piece_ref, piece_type, state_memory};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, CustomSection, ElementSection, Elements, Encode, EntityType,
    FunctionSection, GlobalSection, GlobalType, ImportSection, InstructionSink, MemoryType, Module,
    TableType, TagKind, TagType, TypeSection, ValType,
};
use wcontainers_abi::PAGE_SIZE;
use wcontainers_guest_cpu::{
    Access, Effects, Emitter, GUEST_REASONS, Guest, ReturnAddress, Target, TrapDetail, TrapKind,
    TrapPc,
};

/// The shift from an address to its page number.
const PAGE_SHIFT: i64 = PAGE_SIZE.trailing_zeros() as i64;

/// How much code the translator sees at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LookAhead {
    /// One instruction per piece. The permanent reference. Every instruction the starts reach is translated.
    None,
    /// One piece per start: the stretch of at most this many instructions the code reaches from it, with all
    /// that full look-ahead does inside it. A stretch of one instruction is that instruction's piece of
    /// look-ahead none, and nothing else.
    Bounded(u32),
    /// The whole blob: one piece per function found in it.
    Full,
}

impl LookAhead {
    /// The look-ahead code reached at run time with no piece is translated at.
    pub const AT_RUN_TIME: LookAhead = LookAhead::Bounded(BOUND);
}

/// What a translation is made for: the one setting the translator has. Both give the same results.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Optimise {
    /// Nothing is given up for size.
    #[default]
    Speed,
    /// Run speed may be given up for smaller output.
    Size,
}

/// Tuning number: the default N of bounded look-ahead.
pub const BOUND: u32 = 256;

/// Checks a translation can leave out, only so that what each costs can be measured. Output without any of
/// them is not correct.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Omit {
    /// The stop check at jumps back and at piece entries (not the one after a system call, which is how the
    /// kernel stops a thread).
    pub stop: bool,
    /// The page permission check, and with it `Guest::data_address`, which it makes.
    pub permissions: bool,
    /// The check that a return goes where the caller expects.
    pub returns: bool,
    /// The check at a piece's entry that the engine's stack is not nearly used up.
    pub depth: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A start address is not a multiple of the instruction alignment inside the code range.
    BadStart(u64),
    /// The code range is not inside the blob, or does not start at a multiple of the instruction alignment.
    BadCodeRange,
    /// A bounded look-ahead of no instructions.
    NoBound,
}

/// One translation: its code modules, which together hold every piece. There is more than one only where one
/// module would pass an engine's limits; code in one module reaches another's through the lookup.
pub struct Translation {
    pub modules: Vec<CodeModule>,
    /// The blob's bytes the translation was made from, as sorted ranges of blob offsets that neither
    /// overlap nor touch: every instruction it decoded. Its code is right only while these bytes stay as
    /// they were.
    pub read: Vec<Range<u64>>,
    /// How many of the pieces in the lookup table each means found, by [`Means`] number.
    pub found: [u64; Means::ALL.len()],
}

/// What made an address the start of a piece in the lookup table. Where several did, the first listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Means {
    /// A start the caller gave.
    Given,
    /// The target of a direct call.
    Call,
    /// An address the code builds that points into the code.
    Built,
    /// Where the code goes from another piece, at look-ahead none.
    Followed,
    /// An entry that keeps a function's control flow reducible.
    Reducible,
    /// A cut in a function too large for the translator's own limit or an engine's.
    Cut,
}

impl Means {
    pub const ALL: [Means; 6] = [
        Means::Given,
        Means::Call,
        Means::Built,
        Means::Followed,
        Means::Reducible,
        Means::Cut,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Means::Given => "given",
            Means::Call => "call targets",
            Means::Built => "built addresses",
            Means::Followed => "followed",
            Means::Reducible => "kept reducible",
            Means::Cut => "cuts",
        }
    }
}

/// One code module and where its pieces start.
pub struct CodeModule {
    pub wasm: Vec<u8>,
    /// How many places in its code may take each slow path, by [`Reason`] number: what translating alone can
    /// count. Slow paths decided by a hook (floating point) are not counted here.
    pub sites: Sites,
    /// Blob offsets of the pieces the module puts in the lookup table, in order from its slot base.
    pub pieces: Vec<u64>,
}

impl Translation {
    /// Blob offsets of every piece the translation puts in the lookup table, in order.
    pub fn pieces(&self) -> impl Iterator<Item = u64> + '_ {
        self.modules.iter().flat_map(|m| m.pieces.iter().copied())
    }
}

/// Engine limits, V8's (wasmtime's validator has the same): a module's bytes and functions, and a function
/// body's bytes and locals.
const ENGINE_MODULE_BYTES: usize = 1 << 30;
const ENGINE_FUNCTIONS: usize = 1_000_000;
const ENGINE_BODY_BYTES: usize = 7_654_321;
const ENGINE_LOCALS: usize = 50_000;
/// Engine limit (V8's and wasmtime's): the targets of one `br_table`.
pub(crate) const BR_TABLE_TARGETS: u64 = 65_520;

/// Tuning number: the bytes of code a module may hold before the translation is split, the engine's limit
/// less room for the rest of the module.
const MODULE_BYTES: usize = ENGINE_MODULE_BYTES - (64 << 20);
/// Tuning number: the functions a module may hold before the translation is split.
const MODULE_FUNCTIONS: usize = ENGINE_FUNCTIONS - 1000;
/// Tuning number: the blocks a function may have before it is cut in parts. It bounds how deep translating
/// one function nests, and so the translator's own stack.
pub(crate) const FUNCTION_BLOCKS: usize = 2048;

/// A count for each [`Reason`], by its number.
/// By reason number, the translator's and then the guest's own from `GUEST_REASONS`, how many places in
/// the code may take that slow path.
pub type Sites = [u64; SITES];
/// Room for the translator's reasons and up to 16 of a guest's own.
pub const SITES: usize = GUEST_REASONS as usize + 16;

/// The number of a guest's own slow path.
pub(crate) fn guest_reason<G: Guest>(reason: G::Reason) -> u32 {
    const { assert!(G::REASONS.len() <= SITES - GUEST_REASONS as usize) };
    let i = G::REASONS
        .iter()
        .position(|r| r.0 == reason)
        .expect("a listed reason");
    GUEST_REASONS + i as u32
}

pub(crate) fn add_sites(sites: &mut Sites, more: &Sites) {
    for (a, b) in sites.iter_mut().zip(more) {
        *a += b;
    }
}

/// One piece the lookup table holds, with the functions only it calls (its loop pieces).
pub(crate) struct Unit {
    pub(crate) piece: wasm_encoder::Function,
    pub(crate) others: Vec<wasm_encoder::Function>,
    /// The most locals any of them has.
    pub(crate) locals: usize,
    pub(crate) sites: Sites,
}

impl Unit {
    /// The bytes it adds to a module: its bodies, and a few for each function's entries in other sections.
    fn bytes(&self) -> usize {
        std::iter::once(&self.piece)
            .chain(&self.others)
            .map(|f| f.byte_len() + 8)
            .sum()
    }

    fn functions(&self) -> usize {
        1 + self.others.len()
    }

    /// Whether a function of it passes an engine's limit for one function.
    fn too_large(&self) -> bool {
        self.locals > ENGINE_LOCALS
            || std::iter::once(&self.piece)
                .chain(&self.others)
                .any(|f| f.byte_len() > ENGINE_BODY_BYTES)
    }
}

// Function indices of the translator's own hooks; the guest's hooks, the dispatcher and the rest follow
// (see `Layout`).
pub(crate) const F_SYSCALL: u32 = 0;
pub(crate) const F_STOP: u32 = 1;
pub(crate) const F_MEMORY_FAULT: u32 = 2;
pub(crate) const F_LOAD_CODE: u32 = 3;
pub(crate) const F_WARN: u32 = 4;
pub(crate) const F_TRAP: u32 = 5;

/// The function a meaning's hook calls.
pub(crate) fn hook_function<G: Guest>(hook: G::Hook) -> u32 {
    G::F_HOOKS + G::hook_index(hook) as u32
}

// Global indices: the runner's constants.
pub(crate) const G_LOAD_ADDRESS: u32 = 0;
pub(crate) const G_SLOT_BASE: u32 = 1;
pub(crate) const G_TABLE_BASE: u32 = 2;
pub(crate) const G_THREAD_BLOCK: u32 = 3;
pub(crate) const G_PERMISSIONS: u32 = 4;
pub(crate) const G_PERMISSION_PAGES: u32 = 5;
pub(crate) const G_SLOTS: u32 = 6;
pub(crate) const G_SLOTS_COVER: u32 = 7;
/// How many values a code module imports.
pub(crate) const IMPORTED_VALUES: usize = 8;

/// Each of the imported values above is copied into a global of the module's own, at this index plus the
/// import's: wasmtime reads an imported global through a pointer, and a module's own directly.
pub(crate) const G_OWN: u32 = IMPORTED_VALUES as u32;

/// The type of each imported value.
pub(crate) fn imported_value_type(global: u32) -> ValType {
    match global {
        G_LOAD_ADDRESS | G_TABLE_BASE | G_PERMISSION_PAGES | G_SLOTS_COVER => ValType::I64,
        _ => ValType::I32,
    }
}

/// Where a code module's lookups find the array of slots.
pub(crate) fn slots(table_base: Fixed, at: Fixed, cover: Fixed) -> crate::thread::Slots {
    crate::thread::Slots {
        memory: MEMORY_STATE,
        table_base: table_base.value(),
        at: at.value(),
        cover: cover.value(),
    }
}

pub(crate) const MEMORY_STATE: u32 = 1;

/// Where the dispatcher and the pieces of look-ahead none find the thread block.
const BLOCK: Fixed = Fixed::Global(G_THREAD_BLOCK);
const BLOCK_LOAD_ADDRESS: Fixed = Fixed::Global(G_LOAD_ADDRESS);

/// The bytes of the engine's stack a piece is taken to use: a fixed part and a slot for each local.
pub(crate) fn frame_bytes<G: Guest>(locals: &[ValType]) -> i64 {
    let slots: i64 = locals
        .iter()
        .map(|t| if *t == ValType::V128 { 16 } else { 8 })
        .sum();
    64 + 8 * i64::from(G::PARAMETERS) + slots
}

/// Leave to the bottom loop when the depth in i64 local `depth` is past the budget, with `pc` at the
/// entry. Only the passed registers may be in locals.
pub(crate) fn depth_check<G: Guest>(s: &mut InstructionSink<'_>, depth: u32, entry: u64) {
    s.local_get(depth)
        .i64_const(STACK_BUDGET)
        .i64_gt_u()
        .if_(BlockType::Empty);
    save::<G>(s, |s| {
        s.i64_const(entry as i64);
    });
    s.i32_const(UNWIND_DEEP)
        .call(G::F_UNWIND)
        .unreachable()
        .end();
}

/// Translate a blob from the given starts. `code` is where in the blob the code is; starts and every piece
/// are blob offsets. The output never holds an absolute address.
pub fn translate<G: Guest>(
    blob: &[u8],
    code: Range<u64>,
    starts: &[u64],
    look_ahead: LookAhead,
    optimise: Optimise,
    host: G::Host,
    offer: G::Offer,
) -> Result<Translation, Error> {
    let omit = Omit::default();
    translate_omitting::<G>(blob, code, starts, look_ahead, optimise, host, offer, omit)
}

/// [`translate`] without some checks at full look-ahead, only to measure what they cost. Look-ahead none
/// always keeps every check.
#[allow(clippy::too_many_arguments)]
pub fn translate_omitting<G: Guest>(
    blob: &[u8],
    code: Range<u64>,
    starts: &[u64],
    look_ahead: LookAhead,
    optimise: Optimise,
    host: G::Host,
    offer: G::Offer,
    omit: Omit,
) -> Result<Translation, Error> {
    let how = (look_ahead, optimise);
    translate_within::<G>(blob, code, starts, how, (host, offer), omit, MODULE_BYTES)
}

/// [`translate`] split into modules of at most `module_bytes` of code each, only to test calls between
/// modules on small programs.
#[cfg(feature = "measure")]
#[allow(clippy::too_many_arguments)]
pub fn translate_in_modules<G: Guest>(
    blob: &[u8],
    code: Range<u64>,
    starts: &[u64],
    look_ahead: LookAhead,
    optimise: Optimise,
    host: G::Host,
    offer: G::Offer,
    module_bytes: usize,
) -> Result<Translation, Error> {
    let omit = Omit::default();
    let how = (look_ahead, optimise);
    translate_within::<G>(blob, code, starts, how, (host, offer), omit, module_bytes)
}

fn translate_within<G: Guest>(
    blob: &[u8],
    code: Range<u64>,
    starts: &[u64],
    (look_ahead, optimise): (LookAhead, Optimise),
    (host, offer): (G::Host, G::Offer),
    omit: Omit,
    module_bytes: usize,
) -> Result<Translation, Error> {
    if code.end > blob.len() as u64 || code.start > code.end || !code.start.is_multiple_of(G::ALIGN)
    {
        return Err(Error::BadCodeRange);
    }
    let blob_code = Code::<G> {
        blob,
        range: code,
        host,
        offer,
    };
    for &start in starts {
        if !blob_code.inside(start) {
            return Err(Error::BadStart(start));
        }
    }
    let mut counts = [0; Means::ALL.len()];
    let given: BTreeSet<u64> = starts.iter().copied().collect();
    let read: Vec<Range<u64>>;
    let modules = match look_ahead {
        LookAhead::Bounded(0) => return Err(Error::NoBound),
        LookAhead::None | LookAhead::Bounded(1) => {
            let found = none_found(&blob_code, starts, look_ahead == LookAhead::None);
            read = spans(found.iter().map(|(o, d)| (*o, d.length)));
            let entries: Vec<u64> = found.iter().map(|f| f.0).collect();
            for entry in &entries {
                let means = if given.contains(entry) {
                    Means::Given
                } else {
                    Means::Followed
                };
                counts[means as usize] += 1;
            }
            let first = none_emit(&blob_code, &found, 0..found.len());
            let how = (look_ahead, optimise);
            modules::<G>(blob, how, offer, &entries, first, module_bytes, |group| {
                none_emit(&blob_code, &found, group)
            })
        }
        LookAhead::Bounded(_) | LookAhead::Full => {
            let bound = match look_ahead {
                LookAhead::Bounded(n) => Some(n as usize),
                _ => None,
            };
            // A function too large for an engine is cut in two by more starts, until none is.
            let mut starts = starts.to_vec();
            loop {
                let planned = crate::full::plan(&blob_code, &starts, bound, omit, optimise);
                let entries = planned.entries();
                let first = planned.emit(0..entries.len());
                let over: Vec<usize> = (0..first.len()).filter(|&i| first[i].too_large()).collect();
                if over.is_empty() {
                    read = spans(planned.decoded());
                    // Starts added to cut a function too large for an engine count as cuts.
                    for entry in &entries {
                        let means = match planned.means(*entry) {
                            Means::Given if !given.contains(entry) => Means::Cut,
                            means => means,
                        };
                        counts[means as usize] += 1;
                    }
                    let emit = |group| planned.emit(group);
                    let how = (look_ahead, optimise);
                    break modules::<G>(blob, how, offer, &entries, first, module_bytes, emit);
                }
                for i in over {
                    starts.extend(planned.halves(i));
                }
            }
        }
    };
    Ok(Translation {
        modules,
        read,
        found: counts,
    })
}

/// The bytes of instructions given by offset and length, as sorted ranges that neither overlap nor touch.
fn spans(instructions: impl Iterator<Item = (u64, u8)>) -> Vec<Range<u64>> {
    let mut all: Vec<Range<u64>> = instructions
        .map(|(offset, length)| offset..offset + u64::from(length))
        .collect();
    all.sort_unstable_by_key(|r| r.start);
    let mut spans: Vec<Range<u64>> = Vec::new();
    for r in all {
        match spans.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => spans.push(r),
        }
    }
    spans
}

/// The code modules of a translation whose pieces the lookup table holds are at `entries`: `first`, every
/// unit emitted for one module, if that holds at most `module_bytes` of code and passes no engine limit,
/// otherwise as many modules as needed, each emitted for its group of units by `emit`.
fn modules<G: Guest>(
    blob: &[u8],
    how: (LookAhead, Optimise),
    offer: G::Offer,
    entries: &[u64],
    first: Vec<Unit>,
    module_bytes: usize,
    emit: impl Fn(Range<usize>) -> Vec<Unit>,
) -> Vec<CodeModule> {
    let mut groups: Vec<Range<usize>> = Vec::new();
    let (mut bytes, mut functions) = (0, 0);
    for (i, unit) in first.iter().enumerate() {
        match groups.last_mut() {
            Some(group)
                if bytes + unit.bytes() <= module_bytes
                    && functions + unit.functions() <= MODULE_FUNCTIONS =>
            {
                group.end = i + 1;
                bytes += unit.bytes();
                functions += unit.functions();
            }
            _ => {
                groups.push(i..i + 1);
                (bytes, functions) = (unit.bytes(), unit.functions());
            }
        }
    }
    if groups.len() <= 1 {
        return vec![module::<G>(blob, how, offer, entries, first)];
    }
    drop(first);
    groups
        .into_iter()
        .map(|group| module::<G>(blob, how, offer, &entries[group.clone()], emit(group)))
        .collect()
}

/// A code module holding these units, the table's pieces at `entries`.
fn module<G: Guest>(
    blob: &[u8],
    how: (LookAhead, Optimise),
    offer: G::Offer,
    entries: &[u64],
    units: Vec<Unit>,
) -> CodeModule {
    let mut module = Module::new();
    module.section(&types::<G>());
    module.section(&imports::<G>());
    let mut functions = FunctionSection::new();
    let mut code_section = CodeSection::new();
    let mut sites = [0; SITES];
    for unit in &units {
        add_sites(&mut sites, &unit.sites);
    }
    functions.function(T_DISPATCHER);
    code_section.function(&dispatcher::<G>());
    functions.function(T_UNWIND);
    code_section.function(&unwinder());
    functions.function(T_DISPATCHER);
    code_section.function(&exact::<G>());
    functions.function(G::T_SAVE);
    code_section.function(&saver::<G>());
    functions.function(G::T_PERMISSION);
    code_section.function(&permission_checker());
    for (i, (_, function)) in crate::full::flags_word_functions::<G>().iter().enumerate() {
        functions.function(G::T_FLAGS_WORD + i as u32);
        code_section.function(function);
    }
    let others: Vec<wasm_encoder::Function> = units
        .into_iter()
        .map(|unit| {
            functions.function(0);
            code_section.function(&unit.piece);
            unit.others
        })
        .collect::<Vec<_>>()
        .into_iter()
        .flatten()
        .collect();
    for function in &others {
        functions.function(0);
        code_section.function(function);
    }
    module.section(&functions);
    module.section(&own_globals());
    module.section(&elements::<G>(entries.len() as u32));
    module.section(&code_section);
    module.section(&description::<G>(blob, how, offer, entries.iter()));
    CodeModule {
        wasm: module.finish(),
        sites,
        pieces: entries.to_vec(),
    }
}

/// The blob, where its code is, and what the translation is made for.
pub(crate) struct Code<'a, G: Guest> {
    pub blob: &'a [u8],
    pub range: Range<u64>,
    pub host: G::Host,
    pub offer: G::Offer,
}

impl<G: Guest> Code<'_, G> {
    /// Whether an instruction may start at this offset: aligned, with at least an alignment unit of code
    /// left. Targets can wrap below zero, so nothing here may overflow.
    pub fn inside(&self, offset: u64) -> bool {
        offset.is_multiple_of(G::ALIGN)
            && self.range.contains(&offset)
            && self.range.end - offset >= G::ALIGN
    }

    /// The code from an offset inside it to its end.
    pub fn bytes(&self, offset: u64) -> &[u8] {
        &self.blob[offset as usize..self.range.end as usize]
    }

    /// The instruction at an offset inside the code, with its length and effects; `None` when it would run
    /// past the end of the code.
    pub fn decode(&self, offset: u64) -> Option<Decoded<G>> {
        let (instruction, length) = G::decode(self.bytes(offset))?;
        Some(Decoded {
            length,
            instruction,
            effects: wcontainers_guest_cpu::effects::<G>(&instruction, self.host, self.offer),
        })
    }
}

/// One instruction: its length in bytes, and what it is and does.
pub(crate) struct Decoded<G: Guest> {
    pub length: u8,
    pub instruction: G::Instruction,
    pub effects: Effects<G>,
}

impl<G: Guest> Clone for Decoded<G> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<G: Guest> Copy for Decoded<G> {}

/// Look-ahead none: every instruction is its own piece. With `reach`, every instruction direct control flow
/// reaches from the starts, else the starts alone. The instructions, in order.
fn none_found<G: Guest>(code: &Code<'_, G>, starts: &[u64], reach: bool) -> Vec<(u64, Decoded<G>)> {
    let mut found = BTreeMap::new();
    let mut work: Vec<u64> = starts.to_vec();
    while let Some(offset) = work.pop() {
        if !code.inside(offset) || found.contains_key(&offset) {
            continue;
        }
        let Some(decoded) = code.decode(offset) else {
            continue;
        };
        found.insert(offset, decoded);
        if !reach {
            continue;
        }
        let flow = G::flow(&decoded.instruction);
        if flow.falls_through {
            work.push(offset + u64::from(decoded.length));
        }
        if let Some(target) = flow.target {
            work.push(offset.wrapping_add_signed(target));
        }
    }
    found.into_iter().collect()
}

/// The pieces of look-ahead none of the instructions `group` of those found, for a module holding them alone.
fn none_emit<G: Guest>(
    code: &Code<'_, G>,
    found: &[(u64, Decoded<G>)],
    group: Range<usize>,
) -> Vec<Unit> {
    let found = &found[group];
    let index: BTreeMap<u64, u32> = found
        .iter()
        .enumerate()
        .map(|(i, &(offset, _))| (offset, G::F_FIRST_PIECE + i as u32))
        .collect();
    found
        .iter()
        .map(|(offset, decoded)| {
            let mut emitter = NoneEmitter::<G> {
                host: code.host,
                offer: code.offer,
                offset: *offset,
                length: decoded.length,
                detail: G::trap_detail(code.bytes(*offset)),
                pieces: &index,
                body: Vec::new(),
                locals: Vec::new(),
                sites: [0; SITES],
            };
            emitter.entry();
            G::emit(&mut emitter, &decoded.instruction);
            emitter.sink().end();
            let locals = emitter.locals.len();
            let mut function = wasm_encoder::Function::new_with_locals_types(emitter.locals);
            function.raw(emitter.body);
            Unit {
                piece: function,
                others: Vec::new(),
                locals,
                sites: emitter.sites,
            }
        })
        .collect()
}

/// Where a piece reads one of the module's immutable imported values: the global, or a local the piece
/// copied it into at its entry, which the engine can keep in a register.
#[derive(Clone, Copy)]
pub(crate) enum Fixed {
    Global(u32),
    Local(u32),
}

impl Fixed {
    pub(crate) fn push(self, s: &mut InstructionSink<'_>) {
        self.value().push(s);
    }

    fn value(self) -> crate::thread::Value {
        match self {
            Fixed::Global(g) => crate::thread::Value::Global(G_OWN + g),
            Fixed::Local(l) => crate::thread::Value::Local(l),
        }
    }
}

/// Where the permission table is: its place in the state memory and its length in pages.
#[derive(Clone, Copy)]
pub(crate) struct Table {
    pub(crate) at: Fixed,
    pub(crate) pages: Fixed,
}

impl Table {
    pub(crate) const GLOBALS: Table = Table {
        at: Fixed::Global(G_PERMISSIONS),
        pages: Fixed::Global(G_PERMISSION_PAGES),
    };
}

/// Push the permission table byte of the page of the address in i64 local `at`, leaving the page number in
/// i64 local `page`. Pages past the table read the byte after it, which allows nothing.
fn permission_byte(s: &mut InstructionSink<'_>, table: Table, at: u32, page: u32) {
    s.local_get(at).i64_const(12).i64_shr_u().local_tee(page);
    table.pages.push(s);
    s.local_get(page);
    table.pages.push(s);
    s.i64_lt_u().select().i32_wrap_i64();
    table.at.push(s);
    s.i32_add().i32_load8_u(memarg(MEMORY_STATE, 0, 0));
}

/// The permission table bit of the page an access needs.
pub(crate) fn page_bit(access: Access) -> u8 {
    match access {
        Access::Read => PAGE_READ,
        Access::Write => PAGE_WRITE,
    }
}

/// Pop an i32 and raise the thread's stop flag when it is not zero.
pub(crate) fn raise_stop<G: Guest>(s: &mut InstructionSink<'_>, block: Fixed) {
    s.if_(BlockType::Empty);
    block.push(s);
    s.i32_const(1)
        .i32_store(memarg(MEMORY_STATE, G::STOP_FLAG_OFFSET as u64, 2))
        .end();
}

/// Set i32 local `allowed` to whether the i64 `length` bytes from the address in i64 local `at` (locals
/// both) may all be accessed as `this` says, with `page` and `last` i64 locals to work in. No length is
/// allowed that runs past the top of the address space.
pub(crate) fn permission_range(
    s: &mut InstructionSink<'_>,
    table: Table,
    (at, length): (u32, u32),
    this: u8,
    (page, last): (u32, u32),
    allowed: u32,
) {
    s.i32_const(1).local_set(allowed);
    s.local_get(length)
        .i64_eqz()
        .i32_eqz()
        .if_(BlockType::Empty);
    // The last byte, which must not wrap below the first.
    s.local_get(at)
        .local_get(length)
        .i64_add()
        .i64_const(1)
        .i64_sub()
        .local_tee(last)
        .local_get(at)
        .i64_lt_u()
        .if_(BlockType::Empty)
        .i32_const(0)
        .local_set(allowed)
        .else_();
    s.local_get(last).i64_const(12).i64_shr_u().local_set(last);
    s.local_get(at).i64_const(12).i64_shr_u().local_set(page);
    s.block(BlockType::Empty).loop_(BlockType::Empty);
    // Pages past the table read the byte after it, which allows nothing.
    s.local_get(page);
    table.pages.push(s);
    s.local_get(page);
    table.pages.push(s);
    s.i64_lt_u().select().i32_wrap_i64();
    table.at.push(s);
    s.i32_add()
        .i32_load8_u(memarg(MEMORY_STATE, 0, 0))
        .i32_const(this.into())
        .i32_and()
        .i32_eqz()
        .if_(BlockType::Empty)
        .i32_const(0)
        .local_set(allowed)
        .br(2)
        .end();
    s.local_get(page)
        .local_get(last)
        .i64_ge_u()
        .br_if(1)
        .local_get(page)
        .i64_const(1)
        .i64_add()
        .local_set(page)
        .br(0)
        .end()
        .end();
    s.end().end();
}

/// Open an `if` entered unless the page of the address in i64 local `at`, and the page after it, allow an
/// access as `this` (permission table bits for one page) says. This is the fast test: it is also entered
/// for an access that would be allowed but sits on a page before one that does not allow it.
pub(crate) fn permission_fast(
    s: &mut InstructionSink<'_>,
    table: Table,
    at: u32,
    this: u8,
    page: u32,
) {
    // The bits of the next page sit two above those of this one.
    let both = this | (this << 2);
    debug_assert_eq!(PAGE_READ << 2, NEXT_PAGE_READ);
    debug_assert_eq!(PAGE_WRITE << 2, NEXT_PAGE_WRITE);
    permission_byte(s, table, at, page);
    s.i32_const(both.into())
        .i32_and()
        .i32_const(both.into())
        .i32_ne()
        .if_(BlockType::Empty);
}

/// Where the exact permission test is: in place, with the table and an i64 and an i32 local it may use, or in
/// the module's function for it, which is smaller but makes the engine keep what is live across a call.
#[derive(Clone, Copy)]
pub(crate) enum Exact {
    InPlace(Table, (u32, u32)),
    Called,
}

/// Open an `if` entered when `len` bytes (at most a page) at the address in i64 local `at` may not all be
/// accessed as `this` says, with the first address that may not be accessed in i64 local `fault_at`. The
/// caller writes the slow path and ends the `if`.
pub(crate) fn permission_exact<G: Guest>(
    s: &mut InstructionSink<'_>,
    at: u32,
    len: u32,
    this: u8,
    fault_at: u32,
    exact: Exact,
) {
    // The table's bits reach only to the next page.
    assert!(u64::from(len) <= PAGE_SIZE && len > 0);
    let Exact::InPlace(table, (page, allowed)) = exact else {
        s.local_get(at)
            .i32_const(len as i32)
            .i32_const(this.into())
            .call(G::F_PERMISSION)
            .local_set(fault_at)
            .if_(BlockType::Empty);
        return;
    };
    let next = this << 2;
    permission_byte(s, table, at, page);
    // Bits wanted and missing: this page's, and the next page's if the access runs onto it.
    s.local_tee(allowed)
        .i32_const(-1)
        .i32_xor()
        .i32_const((this | next).into())
        .i32_const(this.into())
        .local_get(at)
        .i64_const(PAGE_SIZE as i64 - 1)
        .i64_and()
        .i64_const(len.into())
        .i64_add()
        .i64_const(PAGE_SIZE as i64)
        .i64_gt_u()
        .select()
        .i32_and()
        .if_(BlockType::Empty);
    // The address itself if this page forbids it, otherwise the start of the next page.
    s.local_get(at)
        .local_get(at)
        .i64_const(PAGE_SIZE as i64 - 1)
        .i64_or()
        .i64_const(1)
        .i64_add()
        .local_get(allowed)
        .i32_const(this.into())
        .i32_and()
        .i32_const(this.into())
        .i32_ne()
        .select()
        .local_set(fault_at);
}

/// The function [`permission_exact`] calls. Parameters: the address, the length and the permission bits;
/// results: whether the access may not be made (not zero), then the first address that may not be accessed.
fn permission_checker() -> wasm_encoder::Function {
    const AT: u32 = 0;
    const LEN: u32 = 1;
    const THIS: u32 = 2;
    const PAGE: u32 = 3;
    const ALLOWED: u32 = 4;
    let mut f = wasm_encoder::Function::new([(1, ValType::I64), (1, ValType::I32)]);
    let mut s = f.instructions();
    permission_byte(&mut s, Table::GLOBALS, AT, PAGE);
    // Bits wanted and missing: this page's, and the next page's (two above) if the access runs onto it.
    debug_assert_eq!(PAGE_READ << 2, NEXT_PAGE_READ);
    s.local_tee(ALLOWED)
        .i32_const(-1)
        .i32_xor()
        .local_get(THIS)
        .local_get(THIS)
        .i32_const(2)
        .i32_shl()
        .i32_or()
        .local_get(THIS)
        .local_get(AT)
        .i64_const(PAGE_SIZE as i64 - 1)
        .i64_and()
        .local_get(LEN)
        .i64_extend_i32_u()
        .i64_add()
        .i64_const(PAGE_SIZE as i64)
        .i64_gt_u()
        .select()
        .i32_and();
    // The address itself if this page forbids it, otherwise the start of the next page.
    s.local_get(AT)
        .local_get(AT)
        .i64_const(PAGE_SIZE as i64 - 1)
        .i64_or()
        .i64_const(1)
        .i64_add()
        .local_get(ALLOWED)
        .local_get(THIS)
        .i32_and()
        .local_get(THIS)
        .i32_ne()
        .select()
        .end();
    f
}

/// Keep the run of pages, from the allowed page of the address in i64 local `at` and at most 8 long, that
/// allow an access as `this` says: i64 locals `low` (its first address) and `room` (how far past `low` an
/// access of at most `longest` bytes may start and still end within the run). i64 local `page` is a
/// temporary.
pub(crate) fn keep_pages(
    s: &mut InstructionSink<'_>,
    table: Table,
    (at, page): (u32, u32),
    this: u8,
    longest: u32,
    (low, room): (u32, u32),
) {
    s.local_get(at)
        .i64_const(PAGE_SHIFT)
        .i64_shr_u()
        .local_tee(page)
        .i64_const(PAGE_SHIFT)
        .i64_shl()
        .local_set(low);
    // The table bytes of the page and the seven after it, when they are all in the table.
    s.local_get(page).i64_const(8).i64_add();
    table.pages.push(s);
    s.i64_le_u().if_(BlockType::Result(ValType::I64));
    let every = i64::from(this) * 0x0101_0101_0101_0101;
    s.i64_const(every);
    table.at.push(s);
    s.local_get(page)
        .i32_wrap_i64()
        .i32_add()
        .i64_load(memarg(MEMORY_STATE, 0, 0))
        .i64_const(-1)
        .i64_xor()
        .i64_and()
        // Bytes up to the first page that does not allow it, then the run's length in bytes.
        .i64_ctz()
        .i64_const(3)
        .i64_shr_u()
        .i64_const(PAGE_SHIFT)
        .i64_shl()
        .else_()
        .i64_const(PAGE_SIZE as i64)
        .end()
        .i64_const(i64::from(longest) - 1)
        .i64_sub()
        .local_set(room);
}

// Type indices of the dispatcher and the unwind tag; the rest follow the guest's hooks (see `Layout`).
const T_DISPATCHER: u32 = 6;
const T_UNWIND: u32 = 7;

pub(crate) fn types<G: Guest>() -> TypeSection {
    let mut types = TypeSection::new();
    piece_type::<G>(&mut types); // 0
    types.ty().function([ValType::I64; 7], [ValType::I64]); // 1: syscall
    types.ty().function([], [ValType::I32]); // 2: stop
    types
        .ty()
        .function([ValType::I64, ValType::I32], [ValType::I32]); // 3: memory_fault
    types
        .ty()
        .function([ValType::I64, ValType::I32], [ValType::I32]); // 4: load_code
    types.ty().function([ValType::I32, ValType::I64], []); // 5: warn, trap
    types.ty().function(
        vec![ValType::I64; G::PARAMETERS as usize + 1],
        vec![ValType::I64; G::RESULTS as usize],
    ); // 6: dispatcher
    types.ty().function([ValType::I32], []); // 7: unwind tag
    // From 8: the guest's hooks.
    for hook in G::HOOKS {
        types
            .ty()
            .function(hook.params.iter().copied(), hook.results.iter().copied());
    }
    types
        .ty()
        .function(vec![ValType::I64; G::PASSED as usize + 1], []); // save
    types.ty().function(
        [ValType::I64, ValType::I32, ValType::I32],
        [ValType::I32, ValType::I64],
    ); // permission
    // Then the flags word functions.
    for (params, _) in crate::full::flags_word_functions::<G>() {
        types.ty().function(params, [ValType::I32]);
    }
    types
}

pub(crate) fn imports<G: Guest>() -> ImportSection {
    let mut imports = ImportSection::new();
    for (name, ty) in [
        (HOOK_SYSCALL, 1),
        (HOOK_STOP, 2),
        (HOOK_MEMORY_FAULT, 3),
        (HOOK_LOAD_CODE, 4),
        (HOOK_WARN, 5),
        (HOOK_TRAP, 5),
    ] {
        imports.import(HOOKS, name, EntityType::Function(ty));
    }
    for (i, hook) in G::HOOKS.iter().enumerate() {
        let ty = G::T_HOOKS + i as u32;
        imports.import(HOOKS, hook.name, EntityType::Function(ty));
    }
    imports.import(
        ENV,
        MEMORY,
        EntityType::Memory(MemoryType {
            minimum: 0,
            maximum: None,
            memory64: true,
            shared: false,
            page_size_log2: None,
        }),
    );
    imports.import(ENV, STATE, EntityType::Memory(state_memory()));
    let global = |ty| {
        EntityType::Global(GlobalType {
            val_type: ty,
            mutable: false,
            shared: false,
        })
    };
    for (g, name) in [
        LOAD_ADDRESS,
        SLOT_BASE,
        TABLE_BASE,
        THREAD_BLOCK,
        PERMISSIONS,
        PERMISSION_PAGES,
        SLOTS,
        SLOTS_COVER,
    ]
    .into_iter()
    .enumerate()
    {
        imports.import(ENV, name, global(imported_value_type(g as u32)));
    }
    imports.import(
        THREAD,
        LOOKUP,
        EntityType::Table(TableType {
            element_type: piece_ref(),
            table64: false,
            minimum: 0,
            maximum: None,
            shared: false,
        }),
    );
    imports.import(
        THREAD,
        UNWIND,
        EntityType::Tag(TagType {
            kind: TagKind::Exception,
            func_type_idx: 7,
        }),
    );
    imports
}

/// The module's own copies of the imported values.
fn own_globals() -> GlobalSection {
    let mut globals = GlobalSection::new();
    for g in 0..G_OWN {
        let ty = GlobalType {
            val_type: imported_value_type(g),
            mutable: false,
            shared: false,
        };
        globals.global(ty, &ConstExpr::global_get(g));
    }
    globals
}

/// The thread block slots of the passed registers, in parameter order.
pub(crate) fn passed_slots<G: Guest>() -> impl DoubleEndedIterator<Item = Slot> {
    G::PASSED_IN.iter().map(|&bit| Slot::of_bit::<G>(bit))
}

/// The thread block slots of the registers a piece gives back, in result order.
pub(crate) fn result_slots<G: Guest>() -> impl DoubleEndedIterator<Item = Slot> {
    G::PASSED_OUT.iter().map(|&bit| Slot::of_bit::<G>(bit))
}

/// Push a piece's results: each register passed out, from its parameter when it is passed in, else from the
/// thread block, where every register is written back before a piece returns.
pub(crate) fn push_results<G: Guest>(s: &mut InstructionSink<'_>, block: Fixed) {
    for &bit in G::PASSED_OUT {
        match G::passed(bit) {
            Some(local) => {
                s.local_get(local);
            }
            None => {
                block.push(s);
                Slot::of_bit::<G>(bit).load(s);
            }
        }
    }
}

/// The registers passed into pieces but not out of them, each with its parameter: a callee may change them,
/// so a return leaves them in the thread block and the caller takes them from there.
pub(crate) fn passed_in_only<G: Guest>() -> impl Iterator<Item = (u32, u32)> {
    G::PASSED_IN
        .iter()
        .enumerate()
        .filter(|(_, bit)| !G::PASSED_OUT.contains(bit))
        .map(|(i, &bit)| (i as u32, bit))
}

/// Store the registers passed in only (see [`passed_in_only`]) into the thread block, before a return.
pub(crate) fn save_passed_in_only<G: Guest>(s: &mut InstructionSink<'_>, block: Fixed) {
    for (local, bit) in passed_in_only::<G>() {
        Slot::of_bit::<G>(bit).store_from(s, block, local);
    }
}

/// Store the passed registers of a piece (or the dispatcher) into the thread block.
pub(crate) fn save_passed<G: Guest>(s: &mut InstructionSink<'_>, block: Fixed) {
    for (i, slot) in passed_slots::<G>().enumerate() {
        slot.store_from(s, block, i as u32);
    }
}

/// Store the passed registers, and `pc` at the blob offset `offset` pushes, in the thread block, through the
/// module's function for it: every way to the runner does this.
pub(crate) fn save<G: Guest>(
    s: &mut InstructionSink<'_>,
    offset: impl FnOnce(&mut InstructionSink<'_>),
) {
    for i in 0..G::PASSED {
        s.local_get(i);
    }
    offset(s);
    s.call(G::F_SAVE);
}

/// The function [`save`] calls. Parameters: the passed registers, then pc as a blob offset.
fn saver<G: Guest>() -> wasm_encoder::Function {
    let mut f = wasm_encoder::Function::new([]);
    let mut s = f.instructions();
    save_passed::<G>(&mut s, BLOCK);
    set_pc::<G>(&mut s, BLOCK, |s| {
        BLOCK_LOAD_ADDRESS.push(s);
        s.local_get(G::PASSED).i64_add();
    });
    s.end();
    f
}

/// Set `pc` in the thread block to the guest address `push` pushes.
pub(crate) fn set_pc<G: Guest>(
    s: &mut InstructionSink<'_>,
    block: Fixed,
    push: impl FnOnce(&mut InstructionSink<'_>),
) {
    block.push(s);
    push(s);
    Slot::pc::<G>().store(s);
}

/// The dispatcher: go to the piece for a guest address through the lookup table, asking the runner for
/// code when there is none. Parameters: the piece parameters, then the address.
pub(crate) fn dispatcher<G: Guest>() -> wasm_encoder::Function {
    let target = G::PARAMETERS;
    let offset = G::PARAMETERS + 1;
    let piece = G::PARAMETERS + 2;
    let mut f = wasm_encoder::Function::new([(1, ValType::I64), (1, ValType::Ref(piece_ref()))]);
    let mut s = f.instructions();
    set_pc::<G>(&mut s, BLOCK, |s| {
        s.local_get(target);
    });
    s.loop_(BlockType::Empty);
    let slots = slots(
        Fixed::Global(G_TABLE_BASE),
        Fixed::Global(G_SLOTS),
        Fixed::Global(G_SLOTS_COVER),
    );
    crate::thread::lookup::<G>(&mut s, target, slots, offset, piece);
    s.local_get(piece).ref_is_null().if_(BlockType::Empty);
    save_passed::<G>(&mut s, BLOCK);
    crate::thread::misaligned_code::<G>(&mut s, target, F_TRAP, |s| {
        s.i32_const(UNWIND_LEAVE).call(G::F_UNWIND).unreachable();
    });
    s.local_get(target)
        .i32_const(0)
        .call(F_LOAD_CODE)
        .i32_eqz()
        .if_(BlockType::Empty)
        .i32_const(UNWIND_LEAVE)
        .call(G::F_UNWIND)
        .unreachable()
        .end()
        .br(1)
        .end()
        .end();
    for i in 0..G::PARAMETERS {
        s.local_get(i);
    }
    s.local_get(piece)
        .ref_as_non_null()
        .return_call_ref(0)
        .end();
    f
}

/// Run the instruction at a guest address from its piece of look-ahead none, which has every slow path in
/// place, keeping the WebAssembly stack; it carries on through the table. Parameters: those of the
/// dispatcher.
pub(crate) fn exact<G: Guest>() -> wasm_encoder::Function {
    let target = G::PARAMETERS;
    let mut f = wasm_encoder::Function::new([]);
    let mut s = f.instructions();
    set_pc::<G>(&mut s, BLOCK, |s| {
        s.local_get(target);
    });
    save_passed::<G>(&mut s, BLOCK);
    s.i32_const(Reason::LeftToAddress as i32)
        .local_get(target)
        .call(F_WARN);
    s.local_get(target)
        .i32_const(1)
        .call(F_LOAD_CODE)
        .i32_eqz()
        .if_(BlockType::Empty)
        .i32_const(UNWIND_LEAVE)
        .call(G::F_UNWIND)
        .unreachable()
        .end();
    for i in 0..G::PARAMETERS {
        s.local_get(i);
    }
    s.i32_const(EXACT_SLOT)
        .table_get(0)
        .ref_as_non_null()
        .return_call_ref(0)
        .end();
    f
}

/// Throw the unwind exception with the payload given. Pieces call this instead of throwing themselves: in
/// wasmtime a function that throws saves every callee-saved register on entry, which would tax every call.
pub(crate) fn unwinder() -> wasm_encoder::Function {
    let mut f = wasm_encoder::Function::new([]);
    f.instructions().local_get(0).throw(0).end();
    f
}

/// Call the stop hook and unwind with the payload it returns unless it is 0 (carry on); `payload` is an i32
/// local.
pub(crate) fn stop_hook<G: Guest>(s: &mut InstructionSink<'_>, payload: u32) {
    s.call(F_STOP)
        .local_tee(payload)
        .if_(BlockType::Empty)
        .local_get(payload)
        .call(G::F_UNWIND)
        .unreachable()
        .end();
}

/// Put every piece in the lookup table at instantiation, in order from the slot base.
pub(crate) fn elements<G: Guest>(pieces: u32) -> ElementSection {
    let mut elements = ElementSection::new();
    let refs: Vec<ConstExpr> = (0..pieces)
        .map(|i| ConstExpr::ref_func(G::F_FIRST_PIECE + i))
        .collect();
    elements.active(
        Some(0),
        &ConstExpr::global_get(G_SLOT_BASE),
        Elements::Expressions(piece_ref(), refs.into()),
    );
    elements
}

pub(crate) fn description<'a, G: Guest>(
    blob: &[u8],
    (look_ahead, optimise): (LookAhead, Optimise),
    offer: G::Offer,
    pieces: impl ExactSizeIterator<Item = &'a u64>,
) -> CustomSection<'static> {
    let (kind, bound) = match look_ahead {
        LookAhead::None => (0, 0),
        LookAhead::Full => (1, 0),
        LookAhead::Bounded(n) => (2, n),
    };
    let mut data = vec![4, G::ID as u8];
    u64::from(G::offer_key(offer)).encode(&mut data);
    data.extend([kind, optimise as u8]);
    bound.encode(&mut data);
    data.extend_from_slice(blake3::hash(blob).as_bytes());
    (pieces.len() as u64).encode(&mut data);
    for offset in pieces {
        offset.encode(&mut data);
    }
    CustomSection {
        name: DESCRIPTION_SECTION.into(),
        data: data.into(),
    }
}

/// The emitter for look-ahead none: registers and flags live in globals, except the passed registers,
/// which are parameters; every instruction ends by passing control to the piece for the next address.
struct NoneEmitter<'a, G: Guest> {
    host: G::Host,
    offer: G::Offer,
    offset: u64,
    length: u8,
    /// What a trap reports of the instruction's bytes.
    detail: i64,
    pieces: &'a BTreeMap<u64, u32>,
    body: Vec<u8>,
    locals: Vec<ValType>,
    sites: Sites,
}

impl<G: Guest> NoneEmitter<'_, G> {
    /// The checks every piece starts with: each one passes control on.
    fn entry(&mut self) {
        let offset = self.offset;
        depth_check::<G>(&mut self.sink(), G::L_DEPTH, offset);
        self.sites[Reason::StackNearlyFull as usize] += 1;
        self.stop_check(0);
    }

    /// The offset of the next instruction.
    fn next_offset(&self) -> u64 {
        self.offset + u64::from(self.length)
    }

    /// Call the stop hook if the stop flag is raised, with `pc` at this instruction plus `step`.
    fn stop_check(&mut self, step: u64) {
        self.sites[Reason::StoppedByFlag as usize] += 1;
        let pc = self.offset + step;
        let payload = self.local(ValType::I32);
        let mut s = self.sink();
        BLOCK.push(&mut s);
        s.i32_load(memarg(MEMORY_STATE, G::STOP_FLAG_OFFSET as u64, 2))
            .if_(BlockType::Empty);
        save::<G>(&mut s, |s| {
            s.i64_const(pc as i64);
        });
        stop_hook::<G>(&mut s, payload);
        s.end();
    }

    /// Save registers with `pc` at this instruction plus `step`.
    fn save(&mut self, step: u64) {
        let pc = self.offset + step;
        save::<G>(&mut self.sink(), |s| {
            s.i64_const(pc as i64);
        });
    }

    /// Push the passed registers, then the return address the caller expects and the depth.
    fn arguments(&mut self, ret: impl FnOnce(&mut InstructionSink<'_>), depth: i64) {
        let mut s = self.sink();
        for i in 0..G::PASSED {
            s.local_get(i);
        }
        ret(&mut s);
        s.local_get(G::L_DEPTH);
        if depth != 0 {
            s.i64_const(depth).i64_add();
        }
    }

    /// Transfer to a target, as a tail call or a plain call, with the return address already pushed.
    fn transfer(&mut self, target: Target, tail: bool) {
        let direct = match target {
            Target::Relative(by) => {
                let offset = self.offset.wrapping_add_signed(by);
                match self.pieces.get(&offset) {
                    Some(&function) => Some(function),
                    None => {
                        address(&mut self.sink(), offset);
                        None
                    }
                }
            }
            Target::Address(local) => {
                self.sink().local_get(local);
                None
            }
        };
        let function = direct.unwrap_or(G::F_DISPATCH);
        if tail {
            self.sink().return_call(function);
        } else {
            self.sink().call(function);
        }
    }

    fn leave(&mut self) {
        self.sink()
            .i32_const(UNWIND_LEAVE)
            .call(G::F_UNWIND)
            .unreachable();
    }

    /// Push a register kept in the thread block.
    fn load(&mut self, slot: Slot) {
        let mut s = self.sink();
        BLOCK.push(&mut s);
        slot.load(&mut s);
    }

    /// Pop a value into a register kept in the thread block.
    fn store(&mut self, slot: Slot) {
        let value = Emitter::<G>::local(self, slot.ty());
        let mut s = self.sink();
        s.local_set(value);
        slot.store_from(&mut s, BLOCK, value);
    }
}

/// After a return that does not go where its caller expects, with `pc` set to its target: return to the
/// bottom loop if it is the caller.
pub(crate) fn bottom_return<G: Guest>(s: &mut InstructionSink<'_>, block: Fixed) {
    s.local_get(G::L_RETURN)
        .i64_const(G::UNREACHABLE_RETURN | G::FLAGS_WANTED)
        .i64_eq()
        .if_(BlockType::Empty);
    push_results::<G>(s, block);
    s.return_().end();
}

/// Push the guest address of a blob offset.
pub(crate) fn address(s: &mut InstructionSink<'_>, offset: u64) {
    s.global_get(G_OWN + G_LOAD_ADDRESS)
        .i64_const(offset as i64)
        .i64_add();
}

/// With return addresses on the stack, move the stack pointer up past the one a return has read.
pub(crate) fn pop_return_address<G: Guest>(s: &mut InstructionSink<'_>) {
    if let ReturnAddress::Stack { bytes } = G::RETURN_ADDRESS {
        s.local_get(G::L_SP)
            .i64_const(bytes.into())
            .i64_add()
            .local_set(G::L_SP);
    }
}

/// Push whether the caller asks for the flags, as an i32 not zero when it does. A bit in the low word is
/// read by wrapping; one above it by a compare.
pub(crate) fn push_flags_wanted<G: Guest>(s: &mut InstructionSink<'_>) {
    s.local_get(G::L_RETURN)
        .i64_const(G::FLAGS_WANTED)
        .i64_and();
    if G::FLAGS_WANTED as u64 <= u64::from(u32::MAX) {
        s.i32_wrap_i64();
    } else {
        s.i64_const(0).i64_ne();
    }
}

/// Push the return address the caller expects, without the flags-wanted bit.
pub(crate) fn push_expected_return<G: Guest>(s: &mut InstructionSink<'_>) {
    s.local_get(G::L_RETURN);
    if G::FLAGS_WANTED != 0 {
        s.i64_const(!G::FLAGS_WANTED).i64_and();
    }
}

/// Push a trap's detail.
pub(crate) fn trap_detail(s: &mut InstructionSink<'_>, detail: TrapDetail, instruction: i64) {
    match detail {
        TrapDetail::Instruction => s.i64_const(instruction),
        TrapDetail::Local(local) => s.local_get(local),
        TrapDetail::Const(value) => s.i64_const(value),
    };
}

impl<G: Guest> Emitter<G> for NoneEmitter<'_, G> {
    fn sink(&mut self) -> InstructionSink<'_> {
        InstructionSink::new(&mut self.body)
    }

    fn local(&mut self, ty: ValType) -> u32 {
        self.locals.push(ty);
        G::PARAMETERS + self.locals.len() as u32 - 1
    }

    fn get(&mut self, r: G::Reg) {
        let mut s = self.sink();
        match G::reg_bit(r) {
            None => {
                s.i64_const(0);
            }
            Some(bit) => match G::passed(bit) {
                Some(local) => {
                    s.local_get(local);
                }
                None => {
                    BLOCK.push(&mut s);
                    Slot::of_bit::<G>(bit).load(&mut s);
                }
            },
        }
    }

    fn set(&mut self, r: G::Reg) {
        match G::reg_bit(r) {
            None => {
                self.sink().drop();
            }
            Some(bit) => match G::passed(bit) {
                Some(local) => {
                    self.sink().local_set(local);
                }
                None => self.store(Slot::of_bit::<G>(bit)),
            },
        }
    }

    fn this_address(&mut self, by: i64) {
        let offset = self.offset.wrapping_add_signed(by);
        address(&mut self.sink(), offset);
    }

    fn set_flags(&mut self, flags: G::Flags) {
        store_flags::<G, _>(self, BLOCK, |e, flag| {
            G::flag_value(&mut e.sink(), flags, flag)
        });
    }

    fn flag(&mut self, flag: G::Flag) {
        load_flag::<G>(&mut self.sink(), BLOCK, flag);
    }

    fn get_vector(&mut self, v: G::Vector) {
        self.load(Slot::of_bit::<G>(G::vector_bit(v)));
    }

    fn set_vector(&mut self, v: G::Vector) {
        self.store(Slot::of_bit::<G>(G::vector_bit(v)));
    }

    fn get_control(&mut self, control: G::Control) {
        self.load(Slot::control::<G>(control));
    }

    fn set_control(&mut self, control: G::Control) {
        self.store(Slot::control::<G>(control));
    }

    fn hook(&mut self, hook: G::Hook) {
        // Only the passed registers are not in the thread block already.
        let registers = G::HOOKS[G::hook_index(hook)].registers;
        let passed = || {
            G::PASSED_IN
                .iter()
                .enumerate()
                .filter(|(_, b)| registers.has_bit(**b))
        };
        let mut s = self.sink();
        for (i, &bit) in passed() {
            Slot::of_bit::<G>(bit).store_from(&mut s, BLOCK, i as u32);
        }
        s.call(hook_function::<G>(hook));
        for (i, &bit) in passed() {
            Slot::of_bit::<G>(bit).load_into(&mut s, BLOCK, i as u32);
        }
    }

    fn host(&self) -> G::Host {
        self.host
    }

    fn offer(&self) -> G::Offer {
        self.offer
    }

    fn check(&mut self, address: u32, bytes: u32, access: Access, _which: u8) {
        let this = page_bit(access);
        let (page, fault_at) = (self.local(ValType::I64), self.local(ValType::I64));
        self.sites[Reason::MemoryFault as usize] += 1;
        let pc = self.offset;
        let mut s = self.sink();
        G::data_address(&mut s, address);
        s.local_set(address);
        permission_fast(&mut s, Table::GLOBALS, address, this, page);
        permission_exact::<G>(&mut s, address, bytes, this, fault_at, Exact::Called);
        save::<G>(&mut s, |s| {
            s.i64_const(pc as i64);
        });
        s.local_get(fault_at)
            .i32_const(i32::from(access == Access::Write))
            .call(F_MEMORY_FAULT)
            .call(G::F_UNWIND)
            .unreachable()
            .end()
            .end();
    }

    fn check_range(&mut self, address: u32, length: u32, access: Access) -> u32 {
        let (page, last) = (self.local(ValType::I64), self.local(ValType::I64));
        let allowed = self.local(ValType::I32);
        let mut s = self.sink();
        G::data_address(&mut s, address);
        s.local_set(address);
        permission_range(
            &mut s,
            Table::GLOBALS,
            (address, length),
            page_bit(access),
            (page, last),
            allowed,
        );
        allowed
    }

    fn warn(&mut self, reason: G::Reason) {
        let number = guest_reason::<G>(reason);
        self.sites[number as usize] += 1;
        self.sink().i32_const(number as i32);
        self.this_address(0);
        self.sink().call(F_WARN);
    }

    fn raise_stop(&mut self) {
        raise_stop::<G>(&mut self.sink(), BLOCK);
    }

    fn jump(&mut self, target: Target) {
        self.arguments(
            |s| {
                s.local_get(G::L_RETURN);
            },
            0,
        );
        self.transfer(target, true);
    }

    fn jump_if(&mut self, target: Target) {
        self.sink().if_(BlockType::Empty);
        self.jump(target);
        self.sink().end();
    }

    fn call(&mut self, target: Target) {
        let next = self.next_offset();
        // The flags are always in the thread block here.
        let frame = frame_bytes::<G>(&self.locals);
        self.arguments(
            |s| {
                // No return address has the bit, so adding it sets it.
                address(s, next + G::FLAGS_WANTED as u64);
            },
            frame,
        );
        self.transfer(target, false);
        // A register passed out but not in is also in the thread block, where it lives here.
        let mut s = self.sink();
        for &bit in G::PASSED_OUT.iter().rev() {
            match G::passed(bit) {
                Some(local) => s.local_set(local),
                None => s.drop(),
            };
        }
        for (local, bit) in passed_in_only::<G>() {
            Slot::of_bit::<G>(bit).load_into(&mut s, BLOCK, local);
        }
    }

    fn ret(&mut self, target: u32) {
        let mut s = self.sink();
        pop_return_address::<G>(&mut s);
        save_passed_in_only::<G>(&mut s, BLOCK);
        s.local_get(target);
        push_expected_return::<G>(&mut s);
        s.i64_eq().if_(BlockType::Empty);
        push_results::<G>(&mut s, BLOCK);
        s.return_().end();
        // Not where the caller expects: go to the address from the bottom loop.
        set_pc::<G>(&mut s, BLOCK, |s| {
            s.local_get(target);
        });
        bottom_return::<G>(&mut s, BLOCK);
        save_passed::<G>(&mut s, BLOCK);
        s.i32_const(UNWIND_GO_TO).call(G::F_UNWIND).unreachable();
    }

    fn syscall(&mut self) {
        let step = u64::from(self.length);
        self.save(step);
        let registers = G::SYSCALL;
        self.get(registers.number);
        for r in registers.args {
            self.get(r);
        }
        self.sink().call(F_SYSCALL);
        self.set(registers.result);
        // The kernel raises the flag during the call when the thread must not carry on.
        self.stop_check(step);
    }

    fn trap(&mut self, trap: G::Trap) {
        let (kind, detail, pc) = G::trap_kind(trap);
        if kind == TrapKind::Unsupported {
            self.sites[Reason::UnsupportedInstructionReached as usize] += 1;
        }
        self.save(match pc {
            TrapPc::At => 0,
            TrapPc::After => u64::from(self.length),
        });
        let instruction = self.detail;
        let mut s = self.sink();
        s.i32_const(kind as i32);
        trap_detail(&mut s, detail, instruction);
        s.call(F_TRAP);
        self.leave();
    }

    fn next(&mut self) {
        self.jump(Target::Relative(i64::from(self.length)));
    }
}
