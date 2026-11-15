//! Code modules: the pieces of one translation, and the emitter for look-ahead none.

use crate::contract::*;
use crate::thread::{memarg, piece_ref, piece_type, registers, state_memory};
use std::collections::BTreeMap;
use std::ops::Range;
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, CustomSection, ElementSection, Elements, Encode, EntityType,
    FunctionSection, GlobalType, ImportSection, InstructionSink, MemoryType, Module, TableType,
    TagKind, TagType, TypeSection, ValType,
};
use wcontainers_arm64::{Access, Emitter, Flag, Flags, Reg, Target, Trap, flag_value};

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
    /// The stop check.
    pub stop: bool,
    /// The page permission check.
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

/// How many registers a piece takes as parameters and gives back as results: x0 to x3 and sp.
pub const PASSED: u32 = 5;

// Function indices: the hooks, then the dispatcher, then the pieces.
pub(crate) const F_SYSCALL: u32 = 0;
pub(crate) const F_STOP: u32 = 1;
pub(crate) const F_MEMORY_FAULT: u32 = 2;
pub(crate) const F_LOAD_CODE: u32 = 3;
pub(crate) const F_WARN: u32 = 4;
pub(crate) const F_TRAP: u32 = 5;
pub(crate) const F_DISPATCH: u32 = 6;
pub(crate) const F_FIRST_PIECE: u32 = 7;

// Global indices: the runner's constants, then the thread's registers.
pub(crate) const G_LOAD_ADDRESS: u32 = 0;
pub(crate) const G_SLOT_BASE: u32 = 1;
pub(crate) const G_TABLE_BASE: u32 = 2;
pub(crate) const G_THREAD_BLOCK: u32 = 3;
pub(crate) const G_PERMISSIONS: u32 = 4;
pub(crate) const G_PERMISSION_PAGES: u32 = 5;
pub(crate) const G_FIRST_REGISTER: u32 = 6;

pub(crate) const MEMORY_STATE: u32 = 1;

pub(crate) fn g_x(n: u8) -> u32 {
    G_FIRST_REGISTER + u32::from(n)
}
pub(crate) const G_SP: u32 = G_FIRST_REGISTER + 31;
pub(crate) const G_PC: u32 = G_FIRST_REGISTER + 32;
pub(crate) fn g_flag(flag: Flag) -> u32 {
    G_FIRST_REGISTER
        + 33
        + match flag {
            Flag::N => 0,
            Flag::Z => 1,
            Flag::C => 2,
            Flag::V => 3,
        }
}
pub(crate) const G_TPIDR: u32 = G_FIRST_REGISTER + 37;
pub(crate) fn g_v(n: u8) -> u32 {
    G_FIRST_REGISTER + 40 + u32::from(n)
}

// Parameter locals of every piece.
pub(crate) const L_SP: u32 = 4;
pub(crate) const L_RETURN: u32 = 5;

/// Translate a blob from the given starts. `code` is where in the blob the code is; starts and every piece
/// are blob offsets. The output never holds an absolute address.
pub fn translate(
    blob: &[u8],
    code: Range<u64>,
    starts: &[u64],
    look_ahead: LookAhead,
) -> Result<Translation, Error> {
    translate_omitting(blob, code, starts, look_ahead, Omit::default())
}

/// [`translate`] without some checks at full look-ahead, only to measure what they cost. Look-ahead none
/// always keeps every check.
pub fn translate_omitting(
    blob: &[u8],
    code: Range<u64>,
    starts: &[u64],
    look_ahead: LookAhead,
    omit: Omit,
) -> Result<Translation, Error> {
    if code.end > blob.len() as u64 || code.start > code.end || !code.start.is_multiple_of(4) {
        return Err(Error::BadCodeRange);
    }
    let blob_code = Code { blob, range: code };
    for &start in starts {
        if !blob_code.inside(start) {
            return Err(Error::BadStart(start));
        }
    }
    let pieces = match look_ahead {
        LookAhead::None => none_pieces(&blob_code, starts),
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
    for function in pieces.values() {
        functions.function(0);
        code_section.function(function);
    }
    module.section(&functions);
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

/// Open an `if` entered when `len` bytes (at most a page) at the address in i64 local `at` may not all be
/// accessed as `this` (permission table bits for one page) says, with the first address that may not be
/// accessed in i64 local `fault_at`. The caller writes the slow path and ends the `if`.
pub(crate) fn permission_test(
    s: &mut InstructionSink<'_>,
    at: u32,
    len: u32,
    this: u8,
    locals: (u32, u32, u32),
) {
    let (page, allowed, fault_at) = locals;
    // The bits of the next page sit two above those of this one.
    let next = this << 2;
    debug_assert_eq!(PAGE_READ << 2, NEXT_PAGE_READ);
    debug_assert_eq!(PAGE_WRITE << 2, NEXT_PAGE_WRITE);
    // The page's byte, or nothing allowed above the table.
    s.local_get(at)
        .i64_const(12)
        .i64_shr_u()
        .local_tee(page)
        .global_get(G_PERMISSION_PAGES)
        .i64_lt_u()
        .if_(BlockType::Result(ValType::I32))
        .global_get(G_PERMISSIONS)
        .local_get(page)
        .i32_wrap_i64()
        .i32_add()
        .i32_load8_u(memarg(MEMORY_STATE, 0, 0))
        .else_()
        .i32_const(0)
        .end()
        .local_tee(allowed);
    // Bits wanted and missing: this page's, and the next page's if the access runs onto it.
    s.i32_const(-1)
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

pub(crate) fn types() -> TypeSection {
    let mut types = TypeSection::new();
    piece_type(&mut types); // 0
    types.ty().function([ValType::I64; 7], [ValType::I64]); // 1: syscall
    types.ty().function([], [ValType::I32]); // 2: stop
    types.ty().function([ValType::I64, ValType::I32], []); // 3: memory_fault
    types.ty().function([ValType::I64], [ValType::I32]); // 4: load_code
    types.ty().function([ValType::I32, ValType::I64], []); // 5: warn, trap
    types
        .ty()
        .function([ValType::I64; 7], [ValType::I64; PASSED as usize]); // 6: dispatcher
    types.ty().function([ValType::I32], []); // 7: unwind tag
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
    let global = |ty, mutable| {
        EntityType::Global(GlobalType {
            val_type: ty,
            mutable,
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
        imports.import(ENV, name, global(ty, false));
    }
    for r in &registers() {
        imports.import(THREAD, &r.name, global(r.ty, true));
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

/// Store the passed registers of a piece (or the dispatcher) into their globals.
pub(crate) fn save_passed(s: &mut InstructionSink<'_>) {
    for i in 0..4 {
        s.local_get(i).global_set(g_x(i as u8));
    }
    s.local_get(L_SP).global_set(G_SP);
}

/// The dispatcher: go to the piece for a guest address through the lookup table, asking the runner for
/// code when there is none. Parameters: the piece parameters, then the address.
pub(crate) fn dispatcher() -> wasm_encoder::Function {
    const TARGET: u32 = 6;
    const OFFSET: u32 = 7;
    const PIECE: u32 = 8;
    let mut f = wasm_encoder::Function::new([(1, ValType::I64), (1, ValType::Ref(piece_ref()))]);
    let mut s = f.instructions();
    s.local_get(TARGET).global_set(G_PC);
    s.loop_(BlockType::Empty);
    crate::thread::lookup(&mut s, G_PC, G_TABLE_BASE, OFFSET, PIECE);
    s.local_get(PIECE).ref_is_null().if_(BlockType::Empty);
    save_passed(&mut s);
    crate::thread::misaligned_code(&mut s, TARGET, F_TRAP);
    s.i32_const(UNWIND_LEAVE).throw(0).end();
    s.local_get(TARGET)
        .call(F_LOAD_CODE)
        .i32_eqz()
        .if_(BlockType::Empty)
        .i32_const(UNWIND_LEAVE)
        .throw(0)
        .end()
        .br(1)
        .end()
        .end();
    for i in 0..6 {
        s.local_get(i);
    }
    s.local_get(PIECE)
        .ref_as_non_null()
        .return_call_ref(0)
        .end();
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
        s.global_get(G_THREAD_BLOCK)
            .i32_atomic_load(memarg(
                MEMORY_STATE,
                wcontainers_abi::STOP_FLAG_OFFSET as u64,
                2,
            ))
            .if_(BlockType::Empty);
        save_passed(&mut s);
        address(&mut s, pc);
        s.global_set(G_PC)
            .call(F_STOP)
            .if_(BlockType::Empty)
            .i32_const(UNWIND_LEAVE)
            .throw(0)
            .end()
            .end();
    }

    /// Save registers with `pc` at this instruction plus `step`.
    fn save(&mut self, step: u64) {
        let pc = self.offset + step;
        let mut s = self.sink();
        save_passed(&mut s);
        address(&mut s, pc);
        s.global_set(G_PC);
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
        self.sink().i32_const(UNWIND_LEAVE).throw(0);
    }
}

/// Push the guest address of a blob offset.
pub(crate) fn address(s: &mut InstructionSink<'_>, offset: u64) {
    s.global_get(G_LOAD_ADDRESS)
        .i64_const(offset as i64)
        .i64_add();
}

impl Emitter for NoneEmitter<'_> {
    fn sink(&mut self) -> InstructionSink<'_> {
        InstructionSink::new(&mut self.body)
    }

    fn local(&mut self, ty: ValType) -> u32 {
        self.locals.push(ty);
        6 + self.locals.len() as u32 - 1
    }

    fn get(&mut self, r: Reg) {
        let mut s = self.sink();
        match r {
            Reg::X(n) if n < 4 => s.local_get(n.into()),
            Reg::X(n) => s.global_get(g_x(n)),
            Reg::Sp => s.local_get(L_SP),
            Reg::Zero => s.i64_const(0),
        };
    }

    fn set(&mut self, r: Reg) {
        let mut s = self.sink();
        match r {
            Reg::X(n) if n < 4 => s.local_set(n.into()),
            Reg::X(n) => s.global_set(g_x(n)),
            Reg::Sp => s.local_set(L_SP),
            Reg::Zero => s.drop(),
        };
    }

    fn this_address(&mut self) {
        let offset = self.offset;
        address(&mut self.sink(), offset);
    }

    fn set_flags(&mut self, flags: Flags) {
        for flag in Flag::ALL {
            let mut s = self.sink();
            flag_value(&mut s, flags, flag);
            s.global_set(g_flag(flag));
        }
    }

    fn flag(&mut self, flag: Flag) {
        self.sink().global_get(g_flag(flag));
    }

    fn get_vector(&mut self, n: u8) {
        self.sink().global_get(g_v(n));
    }

    fn set_vector(&mut self, n: u8) {
        self.sink().global_set(g_v(n));
    }

    fn get_thread_pointer(&mut self) {
        self.sink().global_get(G_TPIDR);
    }

    fn set_thread_pointer(&mut self) {
        self.sink().global_set(G_TPIDR);
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
        permission_test(&mut s, address, bytes, this, locals);
        save_passed(&mut s);
        self::address(&mut s, pc);
        s.global_set(G_PC)
            .local_get(locals.2)
            .i32_const(i32::from(access == Access::Write))
            .call(F_MEMORY_FAULT)
            .i32_const(UNWIND_LEAVE)
            .throw(0)
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
        self.arguments(|s| address(s, next));
        self.transfer(target, false);
        let mut s = self.sink();
        s.local_set(L_SP);
        for i in (0..4).rev() {
            s.local_set(i);
        }
    }

    fn ret(&mut self, target: u32) {
        let mut s = self.sink();
        s.local_get(target)
            .local_get(L_RETURN)
            .i64_eq()
            .if_(BlockType::Empty);
        for i in 0..PASSED {
            s.local_get(i);
        }
        s.return_().end();
        // Not where the caller expects: go to the address from the bottom loop.
        save_passed(&mut s);
        s.local_get(target)
            .global_set(G_PC)
            .i32_const(UNWIND_GO_TO)
            .throw(0);
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
