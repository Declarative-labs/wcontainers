//! Code modules: the pieces of one translation, and the emitter for look-ahead none.

use crate::contract::*;
use crate::state::{Slot, load_flag, store_flags};
use crate::thread::{memarg, piece_ref, piece_type, state_memory};
use std::collections::BTreeMap;
use std::ops::Range;
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, CustomSection, ElementSection, Elements, Encode, EntityType,
    FunctionSection, GlobalSection, GlobalType, ImportSection, InstructionSink, MemoryType, Module,
    TableType, TagKind, TagType, TypeSection, ValType,
};
use wcontainers_arm64::{
    Access, Control, Emitter, Flag, Flags, Hook, Host, Reg, Target, Trap, flag_value,
};

/// How much code the translator sees at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LookAhead {
    /// One instruction per piece. The permanent reference.
    None,
    /// The whole blob: one piece per function found in it.
    Full,
}

/// Checks a translation can leave out, only so that what each costs can be measured. Output without any of
/// them is not correct.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Omit {
    /// The stop check at jumps back and at piece entries (not the one after a system call, which is how the
    /// kernel stops a thread).
    pub stop: bool,
    /// The page permission check, and with it the untagging of data addresses, which it does.
    pub permissions: bool,
    /// The check that a return goes where the caller expects.
    pub returns: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A start address is not a multiple of four inside the code range.
    BadStart(u64),
    /// The code range is not inside the blob, or not a multiple of four bytes.
    BadCodeRange,
}

/// One translation: a code module and where its pieces start.
pub struct Translation {
    pub wasm: Vec<u8>,
    /// Blob offsets of the pieces, in order.
    pub pieces: Vec<u64>,
}

/// How many argument registers, from x0 up, a piece takes as parameters and gives back as results.
pub(crate) const ARGUMENTS: u32 = 2;
/// How many registers a piece takes as parameters and gives back as results: the argument registers and
/// sp.
pub const PASSED: u32 = ARGUMENTS + 1;

// Function indices: the hooks, then the dispatcher, then the pieces.
pub(crate) const F_SYSCALL: u32 = 0;
pub(crate) const F_STOP: u32 = 1;
pub(crate) const F_MEMORY_FAULT: u32 = 2;
pub(crate) const F_LOAD_CODE: u32 = 3;
pub(crate) const F_WARN: u32 = 4;
pub(crate) const F_TRAP: u32 = 5;
pub(crate) const F_FLOAT: u32 = 6;
pub(crate) const F_CLOCK: u32 = 7;
pub(crate) const F_CODE_CHANGED: u32 = 8;
pub(crate) const F_DISPATCH: u32 = 9;
pub(crate) const F_UNWIND: u32 = 10;
pub(crate) const F_EXACT: u32 = 11;
pub(crate) const F_FIRST_PIECE: u32 = 12;

/// The function a meaning's hook calls.
pub(crate) fn hook_function(hook: Hook) -> u32 {
    match hook {
        Hook::Float => F_FLOAT,
        Hook::Clock => F_CLOCK,
        Hook::CodeChanged => F_CODE_CHANGED,
    }
}

// Global indices: the runner's constants.
pub(crate) const G_LOAD_ADDRESS: u32 = 0;
pub(crate) const G_SLOT_BASE: u32 = 1;
pub(crate) const G_TABLE_BASE: u32 = 2;
pub(crate) const G_THREAD_BLOCK: u32 = 3;
pub(crate) const G_PERMISSIONS: u32 = 4;
pub(crate) const G_PERMISSION_PAGES: u32 = 5;

/// Each of the six imported values above is copied into a global of the module's own, at this index plus
/// the import's: wasmtime reads an imported global through a pointer, and a module's own directly.
pub(crate) const G_OWN: u32 = 6;

pub(crate) const MEMORY_STATE: u32 = 1;

/// Where the dispatcher and the pieces of look-ahead none find the thread block.
const BLOCK: Fixed = Fixed::Global(G_THREAD_BLOCK);

// Parameter locals of every piece.
pub(crate) const L_SP: u32 = ARGUMENTS;
pub(crate) const L_RETURN: u32 = ARGUMENTS + 1;
/// How many parameters a piece has: the passed registers and the expected return address.
pub(crate) const PARAMETERS: u32 = PASSED + 1;

/// Translate a blob from the given starts. `code` is where in the blob the code is; starts and every piece
/// are blob offsets. The output never holds an absolute address.
pub fn translate(
    blob: &[u8],
    code: Range<u64>,
    starts: &[u64],
    look_ahead: LookAhead,
    host: Host,
) -> Result<Translation, Error> {
    translate_omitting(blob, code, starts, look_ahead, host, Omit::default())
}

/// [`translate`] without some checks at full look-ahead, only to measure what they cost. Look-ahead none
/// always keeps every check.
pub fn translate_omitting(
    blob: &[u8],
    code: Range<u64>,
    starts: &[u64],
    look_ahead: LookAhead,
    host: Host,
    omit: Omit,
) -> Result<Translation, Error> {
    if code.end > blob.len() as u64 || code.start > code.end || !code.start.is_multiple_of(4) {
        return Err(Error::BadCodeRange);
    }
    let blob_code = Code {
        blob,
        range: code,
        host,
    };
    for &start in starts {
        if !blob_code.inside(start) {
            return Err(Error::BadStart(start));
        }
    }
    // The pieces the lookup table holds, and functions only pieces call, whose indices follow theirs.
    let (pieces, others) = match look_ahead {
        LookAhead::None => (none_pieces(&blob_code, starts), Vec::new()),
        LookAhead::Full => crate::full::pieces(&blob_code, starts, omit),
    };
    let index: BTreeMap<u64, u32> = pieces
        .keys()
        .enumerate()
        .map(|(i, &offset)| (offset, F_FIRST_PIECE + i as u32))
        .collect();

    let mut module = Module::new();
    module.section(&types());
    module.section(&imports());
    let mut functions = FunctionSection::new();
    let mut code_section = CodeSection::new();
    functions.function(6);
    code_section.function(&dispatcher());
    functions.function(7);
    code_section.function(&unwinder());
    functions.function(6);
    code_section.function(&exact());
    for function in pieces.values().chain(&others) {
        functions.function(0);
        code_section.function(function);
    }
    module.section(&functions);
    module.section(&own_globals());
    module.section(&elements(&index));
    module.section(&code_section);
    module.section(&description(blob, look_ahead, pieces.keys()));
    Ok(Translation {
        wasm: module.finish(),
        pieces: pieces.into_keys().collect(),
    })
}

/// The blob and where its code is.
pub(crate) struct Code<'a> {
    pub blob: &'a [u8],
    pub range: Range<u64>,
    pub host: Host,
}

impl Code<'_> {
    /// Whether a whole instruction sits at this offset. Targets can wrap below zero, so nothing here may
    /// overflow.
    pub fn inside(&self, offset: u64) -> bool {
        offset.is_multiple_of(4) && self.range.contains(&offset) && self.range.end - offset >= 4
    }

    pub fn word(&self, offset: u64) -> u32 {
        let at = offset as usize;
        u32::from_le_bytes(self.blob[at..at + 4].try_into().unwrap())
    }

    /// Little endian bytes of the blob, if they are all inside it.
    pub fn read(&self, offset: u64, bytes: u8) -> Option<u64> {
        let at = usize::try_from(offset).ok()?;
        let slice = self.blob.get(at..at.checked_add(bytes.into())?)?;
        Some(
            slice
                .iter()
                .rev()
                .fold(0, |value, &b| value << 8 | u64::from(b)),
        )
    }
}

/// Look-ahead none: follow direct control flow from the starts; every instruction reached is its own piece.
fn none_pieces(code: &Code<'_>, starts: &[u64]) -> BTreeMap<u64, wasm_encoder::Function> {
    let mut found = BTreeMap::new();
    let mut work: Vec<u64> = starts.to_vec();
    while let Some(offset) = work.pop() {
        if !code.inside(offset) || found.contains_key(&offset) {
            continue;
        }
        let instruction = wcontainers_arm64::decode(code.word(offset));
        found.insert(offset, instruction);
        let flow = instruction.flow();
        if flow.falls_through {
            work.push(offset + 4);
        }
        if let Some(target) = flow.target {
            work.push(offset.wrapping_add_signed(target));
        }
    }
    let index: BTreeMap<u64, u32> = found
        .keys()
        .enumerate()
        .map(|(i, &offset)| (offset, F_FIRST_PIECE + i as u32))
        .collect();
    found
        .iter()
        .map(|(&offset, instruction)| {
            let mut emitter = NoneEmitter {
                host: code.host,
                offset,
                word: code.word(offset),
                pieces: &index,
                body: Vec::new(),
                locals: Vec::new(),
            };
            emitter.entry();
            wcontainers_arm64::emit(&mut emitter, instruction);
            emitter.sink().end();
            let mut function = wasm_encoder::Function::new_with_locals_types(emitter.locals);
            function.raw(emitter.body);
            (offset, function)
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
        match self {
            Fixed::Global(g) => s.global_get(G_OWN + g),
            Fixed::Local(l) => s.local_get(l),
        };
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
    let both = this | this << 2;
    debug_assert_eq!(PAGE_READ << 2, NEXT_PAGE_READ);
    debug_assert_eq!(PAGE_WRITE << 2, NEXT_PAGE_WRITE);
    permission_byte(s, table, at, page);
    s.i32_const(both.into())
        .i32_and()
        .i32_const(both.into())
        .i32_ne()
        .if_(BlockType::Empty);
}

/// Open an `if` entered when `len` bytes (at most a page) at the address in i64 local `at` may not all be
/// accessed as `this` says, with the first address that may not be accessed in i64 local `fault_at`. The
/// caller writes the slow path and ends the `if`.
pub(crate) fn permission_exact(
    s: &mut InstructionSink<'_>,
    table: Table,
    at: u32,
    len: u32,
    this: u8,
    locals: (u32, u32, u32),
) {
    let (page, allowed, fault_at) = locals;
    let next = this << 2;
    permission_byte(s, table, at, page);
    // Bits wanted and missing: this page's, and the next page's if the access runs onto it.
    s.local_tee(allowed)
        .i32_const(-1)
        .i32_xor()
        .i32_const((this | next).into())
        .i32_const(this.into())
        .local_get(at)
        .i64_const(0xfff)
        .i64_and()
        .i64_const(len.into())
        .i64_add()
        .i64_const(0x1000)
        .i64_gt_u()
        .select()
        .i32_and()
        .if_(BlockType::Empty);
    // The address itself if this page forbids it, otherwise the start of the next page.
    s.local_get(at)
        .local_get(at)
        .i64_const(0xfff)
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

/// The longest single access, in bytes (a pair of 128 bit registers).
const LONGEST_ACCESS: i64 = 32;

/// Keep the run of pages, from the allowed page in i64 local `page` and at most 8 long, that allow an access
/// as `this` says: i64 locals `low` (its first address) and `room` (how far past `low` an access may start
/// and still end within the run).
pub(crate) fn keep_pages(
    s: &mut InstructionSink<'_>,
    table: Table,
    page: u32,
    this: u8,
    low: u32,
    room: u32,
) {
    s.local_get(page).i64_const(12).i64_shl().local_set(low);
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
        .i64_const(12)
        .i64_shl()
        .else_()
        .i64_const(4096)
        .end()
        .i64_const(LONGEST_ACCESS - 1)
        .i64_sub()
        .local_set(room);
}

pub(crate) fn types() -> TypeSection {
    let mut types = TypeSection::new();
    piece_type(&mut types); // 0
    types.ty().function([ValType::I64; 7], [ValType::I64]); // 1: syscall
    types.ty().function([], [ValType::I32]); // 2: stop
    types.ty().function([ValType::I64, ValType::I32], []); // 3: memory_fault
    types.ty().function([ValType::I64], [ValType::I32]); // 4: load_code
    types.ty().function([ValType::I32, ValType::I64], []); // 5: warn, trap
    types.ty().function(
        [ValType::I64; PARAMETERS as usize + 1],
        [ValType::I64; PASSED as usize],
    ); // 6: dispatcher
    types.ty().function([ValType::I32], []); // 7: unwind tag
    types.ty().function(
        [ValType::I32, ValType::I64, ValType::I64, ValType::I64],
        [ValType::I64],
    ); // 8: float
    types.ty().function([], [ValType::I64]); // 9: clock
    types.ty().function([ValType::I64], []); // 10: code_changed
    types
}

pub(crate) fn imports() -> ImportSection {
    let mut imports = ImportSection::new();
    for (name, ty) in [
        (HOOK_SYSCALL, 1),
        (HOOK_STOP, 2),
        (HOOK_MEMORY_FAULT, 3),
        (HOOK_LOAD_CODE, 4),
        (HOOK_WARN, 5),
        (HOOK_TRAP, 5),
        (HOOK_FLOAT, 8),
        (HOOK_CLOCK, 9),
        (HOOK_CODE_CHANGED, 10),
    ] {
        imports.import(HOOKS, name, EntityType::Function(ty));
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
    for (name, ty) in [
        (LOAD_ADDRESS, ValType::I64),
        (SLOT_BASE, ValType::I32),
        (TABLE_BASE, ValType::I64),
        (THREAD_BLOCK, ValType::I32),
        (PERMISSIONS, ValType::I32),
        (PERMISSION_PAGES, ValType::I64),
    ] {
        imports.import(ENV, name, global(ty));
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
    for (g, ty) in [
        (G_LOAD_ADDRESS, ValType::I64),
        (G_SLOT_BASE, ValType::I32),
        (G_TABLE_BASE, ValType::I64),
        (G_THREAD_BLOCK, ValType::I32),
        (G_PERMISSIONS, ValType::I32),
        (G_PERMISSION_PAGES, ValType::I64),
    ] {
        let ty = GlobalType {
            val_type: ty,
            mutable: false,
            shared: false,
        };
        globals.global(ty, &ConstExpr::global_get(g));
    }
    globals
}

/// The thread block slots of the passed registers, in parameter order.
pub(crate) fn passed_slots() -> impl DoubleEndedIterator<Item = Slot> {
    (0..ARGUMENTS as u8)
        .map(Slot::X)
        .chain(std::iter::once(Slot::Sp))
}

/// Store the passed registers of a piece (or the dispatcher) into the thread block.
pub(crate) fn save_passed(s: &mut InstructionSink<'_>, block: Fixed) {
    for (i, slot) in passed_slots().enumerate() {
        slot.store_from(s, block, i as u32);
    }
}

/// Set `pc` in the thread block to the guest address `push` pushes.
pub(crate) fn set_pc(
    s: &mut InstructionSink<'_>,
    block: Fixed,
    push: impl FnOnce(&mut InstructionSink<'_>),
) {
    block.push(s);
    push(s);
    Slot::Pc.store(s);
}

/// The dispatcher: go to the piece for a guest address through the lookup table, asking the runner for
/// code when there is none. Parameters: the piece parameters, then the address.
pub(crate) fn dispatcher() -> wasm_encoder::Function {
    const TARGET: u32 = PARAMETERS;
    const OFFSET: u32 = PARAMETERS + 1;
    const PIECE: u32 = PARAMETERS + 2;
    let mut f = wasm_encoder::Function::new([(1, ValType::I64), (1, ValType::Ref(piece_ref()))]);
    let mut s = f.instructions();
    set_pc(&mut s, BLOCK, |s| {
        s.local_get(TARGET);
    });
    s.loop_(BlockType::Empty);
    crate::thread::lookup(&mut s, TARGET, G_OWN + G_TABLE_BASE, OFFSET, PIECE);
    s.local_get(PIECE).ref_is_null().if_(BlockType::Empty);
    save_passed(&mut s, BLOCK);
    crate::thread::misaligned_code(&mut s, TARGET, F_TRAP);
    s.i32_const(UNWIND_LEAVE).call(F_UNWIND).unreachable().end();
    s.local_get(TARGET)
        .call(F_LOAD_CODE)
        .i32_eqz()
        .if_(BlockType::Empty)
        .i32_const(UNWIND_LEAVE)
        .call(F_UNWIND)
        .unreachable()
        .end()
        .br(1)
        .end()
        .end();
    for i in 0..PARAMETERS {
        s.local_get(i);
    }
    s.local_get(PIECE)
        .ref_as_non_null()
        .return_call_ref(0)
        .end();
    f
}

/// Carry on at a guest address in code translated there at look-ahead none, which has every slow path in
/// place, keeping the WebAssembly stack. Parameters: those of the dispatcher.
pub(crate) fn exact() -> wasm_encoder::Function {
    const TARGET: u32 = PARAMETERS;
    let mut f = wasm_encoder::Function::new([]);
    let mut s = f.instructions();
    set_pc(&mut s, BLOCK, |s| {
        s.local_get(TARGET);
    });
    save_passed(&mut s, BLOCK);
    s.i32_const(Reason::LeftToAddress as i32)
        .local_get(TARGET)
        .call(F_WARN);
    s.local_get(TARGET)
        .call(F_LOAD_CODE)
        .i32_eqz()
        .if_(BlockType::Empty)
        .i32_const(UNWIND_LEAVE)
        .call(F_UNWIND)
        .unreachable()
        .end();
    for i in 0..=PARAMETERS {
        s.local_get(i);
    }
    s.return_call(F_DISPATCH).end();
    f
}

/// Throw the unwind exception with the payload given. Pieces call this instead of throwing themselves: in
/// wasmtime a function that throws saves every callee-saved register on entry, which would tax every call.
pub(crate) fn unwinder() -> wasm_encoder::Function {
    let mut f = wasm_encoder::Function::new([]);
    f.instructions().local_get(0).throw(0).end();
    f
}

/// Put every piece in the lookup table at instantiation, one segment per run of consecutive pieces.
pub(crate) fn elements(index: &BTreeMap<u64, u32>) -> ElementSection {
    let mut elements = ElementSection::new();
    let mut runs: Vec<(u64, Vec<u32>)> = Vec::new();
    for (&offset, &function) in index {
        match runs.last_mut() {
            Some((start, run)) if *start + 4 * run.len() as u64 == offset => run.push(function),
            _ => runs.push((offset, vec![function])),
        }
    }
    for (start, run) in runs {
        let at = ConstExpr::extended([
            wasm_encoder::Instruction::GlobalGet(G_SLOT_BASE),
            wasm_encoder::Instruction::I32Const((start / 4) as i32),
            wasm_encoder::Instruction::I32Add,
        ]);
        let refs: Vec<ConstExpr> = run.into_iter().map(ConstExpr::ref_func).collect();
        elements.active(
            Some(0),
            &at,
            Elements::Expressions(piece_ref(), refs.into()),
        );
    }
    elements
}

pub(crate) fn description<'a>(
    blob: &[u8],
    look_ahead: LookAhead,
    pieces: impl ExactSizeIterator<Item = &'a u64>,
) -> CustomSection<'static> {
    let mut data = vec![1, look_ahead as u8];
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
struct NoneEmitter<'a> {
    host: Host,
    offset: u64,
    word: u32,
    pieces: &'a BTreeMap<u64, u32>,
    body: Vec<u8>,
    locals: Vec<ValType>,
}

impl NoneEmitter<'_> {
    /// The stop check every piece starts with: each one passes control on.
    fn entry(&mut self) {
        self.stop_check(0);
    }

    /// Call the stop hook if the stop flag is raised, with `pc` at this instruction plus `step`.
    fn stop_check(&mut self, step: u64) {
        let pc = self.offset + step;
        let mut s = self.sink();
        BLOCK.push(&mut s);
        s.i32_load(memarg(
            MEMORY_STATE,
            wcontainers_abi::STOP_FLAG_OFFSET as u64,
            2,
        ))
        .if_(BlockType::Empty);
        save_passed(&mut s, BLOCK);
        set_pc(&mut s, BLOCK, |s| address(s, pc));
        s.call(F_STOP)
            .if_(BlockType::Empty)
            .i32_const(UNWIND_LEAVE)
            .call(F_UNWIND)
            .unreachable()
            .end()
            .end();
    }

    /// Save registers with `pc` at this instruction plus `step`.
    fn save(&mut self, step: u64) {
        let pc = self.offset + step;
        let mut s = self.sink();
        save_passed(&mut s, BLOCK);
        set_pc(&mut s, BLOCK, |s| address(s, pc));
    }

    /// Push the passed registers, then the return address the caller expects.
    fn arguments(&mut self, ret: impl FnOnce(&mut InstructionSink<'_>)) {
        let mut s = self.sink();
        for i in 0..PASSED {
            s.local_get(i);
        }
        ret(&mut s);
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
        let function = direct.unwrap_or(F_DISPATCH);
        if tail {
            self.sink().return_call(function);
        } else {
            self.sink().call(function);
        }
    }

    fn leave(&mut self) {
        self.sink()
            .i32_const(UNWIND_LEAVE)
            .call(F_UNWIND)
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
        let value = self.local(slot.ty());
        let mut s = self.sink();
        s.local_set(value);
        slot.store_from(&mut s, BLOCK, value);
    }
}

/// Push the guest address of a blob offset.
pub(crate) fn address(s: &mut InstructionSink<'_>, offset: u64) {
    s.global_get(G_OWN + G_LOAD_ADDRESS)
        .i64_const(offset as i64)
        .i64_add();
}

impl Emitter for NoneEmitter<'_> {
    fn sink(&mut self) -> InstructionSink<'_> {
        InstructionSink::new(&mut self.body)
    }

    fn local(&mut self, ty: ValType) -> u32 {
        self.locals.push(ty);
        PARAMETERS + self.locals.len() as u32 - 1
    }

    fn get(&mut self, r: Reg) {
        let mut s = self.sink();
        match r {
            Reg::X(n) if u32::from(n) < ARGUMENTS => {
                s.local_get(n.into());
            }
            Reg::X(n) => {
                BLOCK.push(&mut s);
                Slot::X(n).load(&mut s);
            }
            Reg::Sp => {
                s.local_get(L_SP);
            }
            Reg::Zero => {
                s.i64_const(0);
            }
        }
    }

    fn set(&mut self, r: Reg) {
        match r {
            Reg::X(n) if u32::from(n) < ARGUMENTS => {
                self.sink().local_set(n.into());
            }
            Reg::X(n) => self.store(Slot::X(n)),
            Reg::Sp => {
                self.sink().local_set(L_SP);
            }
            Reg::Zero => {
                self.sink().drop();
            }
        }
    }

    fn this_address(&mut self) {
        let offset = self.offset;
        address(&mut self.sink(), offset);
    }

    fn set_flags(&mut self, flags: Flags) {
        store_flags(self, BLOCK, |e, flag| {
            flag_value(&mut e.sink(), flags, flag)
        });
    }

    fn flag(&mut self, flag: Flag) {
        load_flag(&mut self.sink(), BLOCK, flag);
    }

    fn get_vector(&mut self, n: u8) {
        self.load(Slot::Vector(n));
    }

    fn set_vector(&mut self, n: u8) {
        self.store(Slot::Vector(n));
    }

    fn get_control(&mut self, control: Control) {
        self.load(Slot::Control(control));
    }

    fn set_control(&mut self, control: Control) {
        self.store(Slot::Control(control));
    }

    fn hook(&mut self, hook: Hook) {
        self.sink().call(hook_function(hook));
    }

    fn host(&self) -> Host {
        self.host
    }

    fn check(&mut self, address: u32, bytes: u32, access: Access) {
        let this = match access {
            Access::Read => PAGE_READ,
            Access::Write => PAGE_WRITE,
        };
        let locals = (
            self.local(ValType::I64),
            self.local(ValType::I32),
            self.local(ValType::I64),
        );
        let pc = self.offset;
        let mut s = self.sink();
        wcontainers_arm64::untag(&mut s, address);
        s.local_set(address);
        permission_fast(&mut s, Table::GLOBALS, address, this, locals.0);
        permission_exact(&mut s, Table::GLOBALS, address, bytes, this, locals);
        save_passed(&mut s, BLOCK);
        set_pc(&mut s, BLOCK, |s| self::address(s, pc));
        s.local_get(locals.2)
            .i32_const(i32::from(access == Access::Write))
            .call(F_MEMORY_FAULT)
            .i32_const(UNWIND_LEAVE)
            .call(F_UNWIND)
            .unreachable()
            .end()
            .end();
    }

    fn jump(&mut self, target: Target) {
        self.arguments(|s| {
            s.local_get(L_RETURN);
        });
        self.transfer(target, true);
    }

    fn jump_if(&mut self, target: Target) {
        self.sink().if_(BlockType::Empty);
        self.jump(target);
        self.sink().end();
    }

    fn call(&mut self, target: Target) {
        let next = self.offset + 4;
        // The flags are always in the thread block here.
        self.arguments(|s| {
            address(s, next);
            s.i64_const(FLAGS_WANTED).i64_or();
        });
        self.transfer(target, false);
        let mut s = self.sink();
        s.local_set(L_SP);
        for i in (0..ARGUMENTS).rev() {
            s.local_set(i);
        }
    }

    fn ret(&mut self, target: u32) {
        let mut s = self.sink();
        s.local_get(target)
            .local_get(L_RETURN)
            .i64_const(!FLAGS_WANTED)
            .i64_and()
            .i64_eq()
            .if_(BlockType::Empty);
        for i in 0..PASSED {
            s.local_get(i);
        }
        s.return_().end();
        // Not where the caller expects: go to the address from the bottom loop.
        save_passed(&mut s, BLOCK);
        set_pc(&mut s, BLOCK, |s| {
            s.local_get(target);
        });
        s.i32_const(UNWIND_GO_TO).call(F_UNWIND).unreachable();
    }

    fn syscall(&mut self) {
        self.save(4);
        self.get(Reg::X(8));
        for i in 0..6 {
            self.get(Reg::X(i));
        }
        self.sink().call(F_SYSCALL).local_set(0);
        // The kernel raises the flag during the call when the thread must not carry on.
        self.stop_check(4);
    }

    fn trap(&mut self, trap: Trap) {
        self.save(0);
        let word = i64::from(self.word);
        let mut s = self.sink();
        match trap {
            Trap::Undefined => s.i32_const(TrapKind::Undefined as i32).i64_const(word),
            Trap::Unsupported => s.i32_const(TrapKind::Unsupported as i32).i64_const(word),
            Trap::Breakpoint(comment) => s
                .i32_const(TrapKind::Breakpoint as i32)
                .i64_const(comment.into()),
            Trap::Alignment(address) => s.i32_const(TrapKind::Alignment as i32).local_get(address),
        };
        s.call(F_TRAP);
        self.leave();
    }

    fn next(&mut self) {
        self.jump(Target::Relative(4));
    }
}
